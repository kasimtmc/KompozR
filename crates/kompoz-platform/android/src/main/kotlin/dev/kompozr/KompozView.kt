package dev.kompozr

import android.content.Context
import android.os.Build
import android.view.InputDevice
import android.view.KeyCharacterMap
import android.view.KeyEvent
import android.view.MotionEvent
import android.view.Surface
import android.view.SurfaceHolder
import android.view.SurfaceView

/**
 * The view a KompozR app draws into (C++ Kompoz's KompozView): a SurfaceView whose surface the app's GPU device
 * presents to with Vulkan, told to the app as it comes, changes size and goes (the app lets go of a surface before
 * [surfaceDestroyed] returns); its touches, mouse and keys passed on. The display's frames come to the app from the
 * NDK's choreographer itself.
 */
open class KompozView(context: Context) : SurfaceView(context), SurfaceHolder.Callback {
    init {
        holder.addCallback(this)
        isFocusable = true
        isFocusableInTouchMode = true
    }

    override fun surfaceCreated(holder: SurfaceHolder) {}

    override fun surfaceChanged(holder: SurfaceHolder, format: Int, width: Int, height: Int) =
        nativeSurface(holder.surface, width, height)

    override fun surfaceDestroyed(holder: SurfaceHolder) = nativeSurface(null, 0, 0)

    override fun onTouchEvent(event: MotionEvent): Boolean = motion(event)

    /** A mouse's (or a touchpad's) hovering, buttons and wheel. */
    override fun onGenericMotionEvent(event: MotionEvent): Boolean =
        if (event.isFromSource(InputDevice.SOURCE_CLASS_POINTER)) motion(event) else super.onGenericMotionEvent(event)

    /** A MotionEvent as the app reads it: every pointer's id and position, the one the action is about. */
    private fun motion(event: MotionEvent): Boolean {
        val count = event.pointerCount
        val ids = IntArray(count) { event.getPointerId(it) }
        val xy = FloatArray(count * 2)
        for (i in 0 until count) {
            xy[i * 2] = event.getX(i)
            xy[i * 2 + 1] = event.getY(i)
        }
        val index = event.actionIndex
        val time = if (Build.VERSION.SDK_INT >= 34) event.eventTimeNanos else event.eventTime * 1_000_000
        nativeMotion(
            event.actionMasked,
            index,
            ids,
            xy,
            time,
            event.getToolType(index),
            event.actionButton,
            event.getAxisValue(MotionEvent.AXIS_HSCROLL),
            event.getAxisValue(MotionEvent.AXIS_VSCROLL),
            event.metaState,
        )
        return true
    }

    override fun onKeyDown(keyCode: Int, event: KeyEvent): Boolean =
        (!systemKey(keyCode) && key(event, true)) || super.onKeyDown(keyCode, event)

    override fun onKeyUp(keyCode: Int, event: KeyEvent): Boolean =
        (!systemKey(keyCode) && key(event, false)) || super.onKeyUp(keyCode, event)

    /** A key and what it types (a dead key's accent: nothing yet). */
    private fun key(event: KeyEvent, down: Boolean): Boolean {
        val unicode = event.getUnicodeChar(event.metaState)
        val typed = if (unicode > 0 && unicode and KeyCharacterMap.COMBINING_ACCENT == 0) {
            String(Character.toChars(unicode)).toByteArray()
        } else {
            null
        }
        return nativeKey(event.keyCode, down, event.metaState, typed, event.eventTime * 1_000_000)
    }

    /** The keys the system keeps: back (the activity's, then the system's), the volume, power, home. */
    private fun systemKey(keyCode: Int): Boolean = when (keyCode) {
        KeyEvent.KEYCODE_BACK, KeyEvent.KEYCODE_VOLUME_UP, KeyEvent.KEYCODE_VOLUME_DOWN, KeyEvent.KEYCODE_VOLUME_MUTE,
        KeyEvent.KEYCODE_POWER, KeyEvent.KEYCODE_HOME -> true
        else -> false
    }

    private external fun nativeSurface(surface: Surface?, width: Int, height: Int)

    private external fun nativeMotion(
        action: Int,
        index: Int,
        ids: IntArray,
        xy: FloatArray,
        timeNanos: Long,
        toolType: Int,
        actionButton: Int,
        scrollX: Float,
        scrollY: Float,
        meta: Int,
    )

    private external fun nativeKey(keyCode: Int, down: Boolean, meta: Int, typed: ByteArray?, timeNanos: Long): Boolean
}
