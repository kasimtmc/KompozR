package dev.kompozr

import android.content.Context
import android.content.res.Configuration
import android.os.Build
import android.view.InputDevice
import android.view.KeyCharacterMap
import android.view.KeyEvent
import android.view.MotionEvent
import android.view.PointerIcon
import android.view.Surface
import android.view.SurfaceHolder
import android.view.SurfaceView
import android.view.inputmethod.EditorInfo
import android.view.inputmethod.InputConnection

/**
 * The view a KompozR app draws into (C++ Kompoz's KompozView, Compose's AndroidComposeView): a SurfaceView whose
 * surface the app's GPU device presents to with Vulkan, told to the app as it comes, changes size and goes (the app
 * lets go of a surface before [surfaceDestroyed] returns); its input passed on — every pointer of a MotionEvent
 * with its tool, pressure and historical samples, a touchpad's gestures, the keys with a dead key's accent composed
 * —; the input method's editor while a text field has a session ([KompozIme]: this view is its text editor, as
 * Compose's is), the pointer's icons, the floating text toolbar ([KompozTextToolbar]), the input devices and the
 * system's gesture thresholds ([KompozInputDevices]). The display's frames come to the app from the NDK's
 * choreographer itself.
 *
 * Its methods named `ime*`, `set*PointerIconType` and `*TextToolbar` are the app's to call (by name, from the
 * native side, on this thread).
 */
open class KompozView(context: Context) : SurfaceView(context), SurfaceHolder.Callback {
    private val ime = KompozIme(this)
    private val toolbar = KompozTextToolbar(this)
    private val devices = KompozInputDevices(this)

    /** The icon a hovering stylus shows, if the app set one (`PointerIcon.TYPE_*`; 0: the mouse's). */
    private var stylusPointerIconType = 0

    /** A dead key's accent, for the next key's character. */
    private var deadChar = 0

    init {
        holder.addCallback(this)
        isFocusable = true
        isFocusableInTouchMode = true
        // The whole surface is one view: Android's highlight of the focused view would cover all of it when a
        // field gives the focus back (C++'s disableFocusHighlight). KompozR draws its own focus.
        defaultFocusHighlightEnabled = false
        // A stylus starts handwriting over a text field, not over the view: the fields ask for it themselves
        // (Compose's AndroidComposeView turns the view's own off).
        if (Build.VERSION.SDK_INT >= 34) isAutoHandwritingEnabled = false
    }

    override fun onAttachedToWindow() {
        super.onAttachedToWindow()
        devices.start()
    }

    override fun onDetachedFromWindow() {
        devices.stop()
        toolbar.hide()
        super.onDetachedFromWindow()
    }

    override fun onConfigurationChanged(newConfig: Configuration) {
        super.onConfigurationChanged(newConfig)
        devices.tell() // (a keyboard slid out, the density changed: the thresholds in px)
    }

    override fun surfaceCreated(holder: SurfaceHolder) {}

    override fun surfaceChanged(holder: SurfaceHolder, format: Int, width: Int, height: Int) =
        nativeSurface(holder.surface, width, height)

    override fun surfaceDestroyed(holder: SurfaceHolder) = nativeSurface(null, 0, 0)

    // ---- Pointers ----

    override fun onTouchEvent(event: MotionEvent): Boolean = motion(event)

    /** A mouse's (or a touchpad's, a stylus's) hovering, buttons and wheel. */
    override fun onGenericMotionEvent(event: MotionEvent): Boolean =
        if (event.isFromSource(InputDevice.SOURCE_CLASS_POINTER)) motion(event) else super.onGenericMotionEvent(event)

    /**
     * A MotionEvent as the app reads it: every pointer's id, tool, position and pressure, the one the action is
     * about, the buttons, the wheel, a touchpad gesture's axes (its swipe's distance and its pinch's scale since the
     * last event: the historical samples' added in), and the positions batched since the last event (oldest first).
     */
    private fun motion(event: MotionEvent): Boolean {
        val count = event.pointerCount
        val ids = IntArray(count) { event.getPointerId(it) }
        val tools = IntArray(count) { event.getToolType(it) }
        val pressure = FloatArray(count) { event.getPressure(it) }
        val xy = FloatArray(count * 2)
        for (i in 0 until count) {
            xy[i * 2] = event.getX(i)
            xy[i * 2 + 1] = event.getY(i)
        }
        val history = event.historySize
        val historyTimes = LongArray(history) {
            if (Build.VERSION.SDK_INT >= 34) event.getHistoricalEventTimeNanos(it)
            else event.getHistoricalEventTime(it) * 1_000_000
        }
        val historyXY = FloatArray(history * count * 2)
        for (h in 0 until history) {
            for (i in 0 until count) {
                historyXY[(h * count + i) * 2] = event.getHistoricalX(i, h)
                historyXY[(h * count + i) * 2 + 1] = event.getHistoricalY(i, h)
            }
        }
        val classification = event.classification
        var swipeX = 0f
        var swipeY = 0f
        var pinch = 1f
        if (Build.VERSION.SDK_INT >= 34) {
            if (classification == MotionEvent.CLASSIFICATION_TWO_FINGER_SWIPE) {
                for (h in 0 until history) {
                    swipeX += event.getHistoricalAxisValue(MotionEvent.AXIS_GESTURE_SCROLL_X_DISTANCE, 0, h)
                    swipeY += event.getHistoricalAxisValue(MotionEvent.AXIS_GESTURE_SCROLL_Y_DISTANCE, 0, h)
                }
                swipeX += event.getAxisValue(MotionEvent.AXIS_GESTURE_SCROLL_X_DISTANCE, 0)
                swipeY += event.getAxisValue(MotionEvent.AXIS_GESTURE_SCROLL_Y_DISTANCE, 0)
            } else if (classification == MotionEvent.CLASSIFICATION_PINCH) {
                // (A missing scale reads 0: no change.)
                for (h in 0 until history) {
                    pinch *= event.getHistoricalAxisValue(MotionEvent.AXIS_GESTURE_PINCH_SCALE_FACTOR, 0, h)
                        .takeIf { it > 0f } ?: 1f
                }
                pinch *= event.getAxisValue(MotionEvent.AXIS_GESTURE_PINCH_SCALE_FACTOR, 0).takeIf { it > 0f } ?: 1f
            }
        }
        val state = intArrayOf(
            event.actionMasked,
            event.actionIndex,
            event.actionButton,
            event.buttonState,
            event.metaState,
            classification,
        )
        val axes = floatArrayOf(
            event.getAxisValue(MotionEvent.AXIS_HSCROLL),
            event.getAxisValue(MotionEvent.AXIS_VSCROLL),
            swipeX,
            swipeY,
            pinch,
            resources.displayMetrics.density,
        )
        val time = if (Build.VERSION.SDK_INT >= 34) event.eventTimeNanos else event.eventTime * 1_000_000
        nativeMotion(state, ids, tools, xy, pressure, axes, historyTimes, historyXY, time)
        return true
    }

    /** The mouse's icon over what it is over, as the app sets it (`PointerIcon.TYPE_*`). */
    @Suppress("unused") // (the app's)
    private fun setPointerIconType(type: Int) {
        pointerIcon = PointerIcon.getSystemIcon(context, type)
    }

    /** A hovering stylus's own icon (0: none, the mouse's). */
    @Suppress("unused") // (the app's)
    private fun setStylusPointerIconType(type: Int) {
        stylusPointerIconType = type
    }

    override fun onResolvePointerIcon(event: MotionEvent, pointerIndex: Int): PointerIcon? {
        val tool = event.getToolType(pointerIndex)
        val stylus = tool == MotionEvent.TOOL_TYPE_STYLUS || tool == MotionEvent.TOOL_TYPE_ERASER
        if (stylus && stylusPointerIconType != 0) return PointerIcon.getSystemIcon(context, stylusPointerIconType)
        return super.onResolvePointerIcon(event, pointerIndex)
    }

    // ---- Keys ----

    override fun onKeyDown(keyCode: Int, event: KeyEvent): Boolean =
        (!systemKey(keyCode) && key(event, true)) || super.onKeyDown(keyCode, event)

    override fun onKeyUp(keyCode: Int, event: KeyEvent): Boolean =
        (!systemKey(keyCode) && key(event, false)) || super.onKeyUp(keyCode, event)

    /**
     * A key and what it types: a dead key types nothing and its accent goes onto the next key's character
     * (`KeyEvent.getDeadChar`; a character it doesn't go on: the accent typed, then the character, as Android's
     * key listeners do).
     */
    private fun key(event: KeyEvent, down: Boolean): Boolean {
        val unicode = event.getUnicodeChar(event.metaState)
        var typed: String? = null
        if (unicode and KeyCharacterMap.COMBINING_ACCENT != 0) {
            if (down) deadChar = unicode and KeyCharacterMap.COMBINING_ACCENT_MASK
        } else if (unicode > 0) {
            val char = String(Character.toChars(unicode))
            typed = char
            if (down && deadChar != 0) {
                val composed = KeyEvent.getDeadChar(deadChar, unicode)
                typed = if (composed != 0) String(Character.toChars(composed)) else String(Character.toChars(deadChar)) + char
                deadChar = 0
            }
        }
        val time = event.eventTime * 1_000_000
        return nativeKey(event.keyCode, down, event.metaState, typed?.toByteArray(), time)
    }

    /** The keys the system keeps: back (the activity's, then the system's), the volume, power, home. */
    private fun systemKey(keyCode: Int): Boolean = when (keyCode) {
        KeyEvent.KEYCODE_BACK, KeyEvent.KEYCODE_VOLUME_UP, KeyEvent.KEYCODE_VOLUME_DOWN, KeyEvent.KEYCODE_VOLUME_MUTE,
        KeyEvent.KEYCODE_POWER, KeyEvent.KEYCODE_HOME -> true
        else -> false
    }

    // ---- The input method ----

    /** A text editor while a field has a session (the input method shows its keyboard for it). */
    override fun onCheckIsTextEditor(): Boolean = ime.active

    override fun onCreateInputConnection(outAttrs: EditorInfo): InputConnection? = ime.createInputConnection(outAttrs)

    @Suppress("unused") // (the app's)
    private fun imeShow(
        session: Int,
        text: ByteArray,
        offsets: IntArray,
        inputType: Int,
        imeOptions: Int,
        privateImeOptions: ByteArray?,
        hintLocales: ByteArray?,
        flags: Int,
    ) = ime.show(session, text, offsets, inputType, imeOptions, privateImeOptions, hintLocales, flags)

    @Suppress("unused") // (the app's)
    private fun imeUpdate(session: Int, text: ByteArray, offsets: IntArray) = ime.update(session, text, offsets)

    @Suppress("unused") // (the app's)
    private fun imeHide(session: Int) = ime.hide(session)

    @Suppress("unused") // (the app's)
    private fun imeHideKeyboard() = ime.hideKeyboard()

    @Suppress("unused") // (the app's)
    private fun imeCursor(rects: FloatArray) = ime.cursor(rects)

    @Suppress("unused") // (the app's)
    private fun imeStartHandwriting() = ime.startStylusHandwriting()

    @Suppress("unused") // (the app's)
    private fun showTextToolbar(rect: FloatArray, items: Int) = toolbar.show(rect, items)

    @Suppress("unused") // (the app's)
    private fun hideTextToolbar() = toolbar.hide()

    // What KompozIme, KompozTextToolbar and KompozInputDevices tell the app.
    internal fun imeEdit(session: Int, ops: IntArray, texts: ByteArray, text: ByteArray, offsets: IntArray) =
        nativeImeEdit(session, ops, texts, text, offsets)

    internal fun imeAction(session: Int, editorAction: Int) = nativeImeAction(session, editorAction)

    internal fun imeContent(session: Int, mimeTypes: ByteArray, uri: ByteArray, linkUri: ByteArray?): Boolean =
        nativeImeContent(session, mimeTypes, uri, linkUri)

    internal fun toolbarAction(item: Int) = nativeToolbarAction(item)

    internal fun devices(mediaInfo: IntArray, viewConfiguration: FloatArray) =
        nativeDevices(mediaInfo, viewConfiguration)

    private external fun nativeSurface(surface: Surface?, width: Int, height: Int)

    private external fun nativeMotion(
        state: IntArray,
        ids: IntArray,
        tools: IntArray,
        xy: FloatArray,
        pressure: FloatArray,
        axes: FloatArray,
        historyTimes: LongArray,
        historyXY: FloatArray,
        timeNanos: Long,
    )

    private external fun nativeKey(keyCode: Int, down: Boolean, meta: Int, typed: ByteArray?, timeNanos: Long): Boolean

    private external fun nativeDevices(mediaInfo: IntArray, viewConfiguration: FloatArray)

    private external fun nativeImeEdit(session: Int, ops: IntArray, texts: ByteArray, text: ByteArray, offsets: IntArray)

    private external fun nativeImeAction(session: Int, editorAction: Int)

    private external fun nativeImeContent(session: Int, mimeTypes: ByteArray, uri: ByteArray, linkUri: ByteArray?): Boolean

    private external fun nativeToolbarAction(item: Int)
}
