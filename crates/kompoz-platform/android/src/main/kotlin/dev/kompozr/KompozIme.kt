package dev.kompozr

import android.graphics.Matrix
import android.graphics.PointF
import android.graphics.RectF
import android.os.Build
import android.os.Bundle
import android.os.CancellationSignal
import android.os.LocaleList
import android.text.Editable
import android.text.InputType
import android.text.Selection
import android.text.TextUtils
import android.view.inputmethod.BaseInputConnection
import android.view.ViewConfiguration
import android.view.inputmethod.CursorAnchorInfo
import android.view.inputmethod.DeleteGesture
import android.view.inputmethod.DeleteRangeGesture
import android.view.inputmethod.EditorBoundsInfo
import android.view.inputmethod.EditorInfo
import android.view.inputmethod.ExtractedText
import android.view.inputmethod.ExtractedTextRequest
import android.view.inputmethod.HandwritingGesture
import android.view.inputmethod.InputConnection
import android.view.inputmethod.InputConnectionWrapper
import android.view.inputmethod.InputMethodManager
import android.view.inputmethod.InsertGesture
import android.view.inputmethod.JoinOrSplitGesture
import android.view.inputmethod.PreviewableHandwritingGesture
import android.view.inputmethod.RemoveSpaceGesture
import android.view.inputmethod.SelectGesture
import android.view.inputmethod.SelectRangeGesture
import java.util.concurrent.Executor
import java.util.function.IntConsumer

/**
 * The input method's side of a KompozR app (C++ Kompoz's KompozInput IME, Compose's TextInputServiceAndroid and
 * CursorAnchorInfoController): the focused text field's session as the app shows it — its text, selection and
 * composition in an [Editable] (the input method's source of truth: what it reads, what it edits, at once), its
 * `EditorInfo` (input type, IME options, private options, hint locales, content types, stylus handwriting: the
 * app's mapping of its ImeOptions) — and what the input method does to it, sent back to the app a batch at a time
 * ([KompozInputConnection]). The app tells it again when the field ends up elsewhere (it changed its text, a key
 * typed, a filter took an edit back): the input method is restarted for a new text, its selection updated
 * otherwise. Where the caret and the field are goes to the input method as its cursor anchor and editor bounds; the
 * extracted text is reported while it monitors it; stylus handwriting is started for the field (Android 14). Once
 * the field is laid out (the app's text layout of it, Compose's `updateTextLayoutResult`), the cursor anchor has its
 * characters' boxes, its visible lines and the caret's glyph, and the editor bounds its decoration box (Compose's
 * CursorAnchorInfoBuilder); Android 14's handwriting gestures and their previews are hit-tested against it by the app
 * (Compose's HandwritingGesture.android.kt) and made through the input connection, as the input method's own edits.
 *
 * Sessions are numbered by the app; edits come back with theirs, so that an old one's are dropped there. A session
 * closed and another opened at once (the focus moving to another field) keep the keyboard up.
 */
internal class KompozIme(private val view: KompozView) {
    private val imm = view.context.getSystemService(InputMethodManager::class.java)

    /** The session shown now (0: none: the view is no text editor). */
    var session = 0
        private set

    val active: Boolean get() = session != 0

    /** The field's text, selection and composition as the input method sees them. */
    val editable: Editable = Editable.Factory.getInstance().newEditable("")

    private var inputType = InputType.TYPE_CLASS_TEXT
    private var imeOptions = 0
    private var privateImeOptions: String? = null
    private var hintLocales: LocaleList? = null
    private var acceptsContent = false
    private var stylusHandwriting = false

    /** The input method's connection now (one per `onCreateInputConnection`). */
    private var connection: KompozInputConnection? = null

    /** A session's end, done after what the app does at once (another session opening keeps the keyboard). */
    private var pendingHide: Runnable? = null

    /** The app's state for the Editable that came during a batch of edits: applied after it. */
    private var pendingUpdate: Pair<ByteArray, IntArray>? = null

    /** What marks a composition in the Editable (BaseInputConnection's own spans, set through one over it). */
    private val spans = object : BaseInputConnection(view, true) {
        override fun getEditable() = this@KompozIme.editable
    }

    /** The caret and the field, px in the view (the field's: none if unknown). */
    private val caret = RectF()
    private var editor: RectF? = null

    /**
     * The field's layout, px in the view (none until it is laid out): each UTF-16 unit's box (`l, t, r, b`) and its
     * CursorAnchorInfo flags, the visible lines' boxes, the caret's glyph (`x, top, baseline, bottom`) and its flags,
     * the decoration box.
     */
    private var characterBounds = FloatArray(0)
    private var characterFlags = IntArray(0)
    private var lineBounds = FloatArray(0)
    private var marker = FloatArray(0)
    private var markerFlags = 0
    private var decoration = FloatArray(0)

    /** The input method follows the cursor anchor (`requestCursorUpdates` with `CURSOR_UPDATE_MONITOR`). */
    var cursorMonitor = false

    /** The input method follows the extracted text (its request's token; -1: it doesn't). */
    var extractedTextToken = -1

    /** A field's session shown (or the same one again: its keyboard asked for), the input method restarted for it. */
    fun show(
        session: Int,
        text: ByteArray,
        offsets: IntArray,
        inputType: Int,
        imeOptions: Int,
        privateImeOptions: ByteArray?,
        hintLocales: ByteArray?,
        flags: Int,
    ) {
        pendingHide?.let { view.removeCallbacks(it) }
        pendingHide = null
        pendingUpdate = null
        this.session = session
        this.inputType = inputType
        this.imeOptions = imeOptions
        this.privateImeOptions = privateImeOptions?.let { String(it) }
        this.hintLocales = hintLocales?.let { LocaleList.forLanguageTags(String(it).replace('\n', ',')) }
        acceptsContent = flags and 1 != 0
        stylusHandwriting = flags and 2 != 0
        setEditable(String(text), offsets)
        extractedTextToken = -1
        cursorMonitor = false
        if (!view.hasFocus()) view.requestFocus()
        imm.restartInput(view)
        if (flags and 4 != 0) imm.showSoftInput(view, 0)
    }

    /** The app's state of the field (it differs from the Editable's): the input method told. */
    fun update(session: Int, text: ByteArray, offsets: IntArray) {
        if (session != this.session) return
        if ((connection?.batchDepth ?: 0) > 0) {
            pendingUpdate = text to offsets
            return
        }
        val newText = String(text)
        val textChanged = editable.toString() != newText
        setEditable(newText, offsets)
        if (textChanged) {
            // (Compose's: a text the input method didn't type restarts it, its composing and suggestions anew.)
            imm.restartInput(view)
        } else {
            updateSelection()
            reportExtractedText()
            cursorAnchorInfo()
        }
    }

    /** The session ended: the keyboard hidden, unless another session opens meanwhile. */
    fun hide(session: Int) {
        if (session != this.session) return
        this.session = 0
        pendingUpdate = null
        layout(FloatArray(0), IntArray(0), FloatArray(0), FloatArray(0), 0, FloatArray(0))
        connection?.closeConnection()
        connection = null
        val hide = Runnable {
            pendingHide = null
            if (this.session == 0) {
                imm.hideSoftInputFromWindow(view.windowToken, 0)
                imm.restartInput(view)
            }
        }
        pendingHide = hide
        view.post(hide)
    }

    /** The keyboard hidden (the session stays: a hardware keyboard still types into it). */
    fun hideKeyboard() {
        imm.hideSoftInputFromWindow(view.windowToken, 0)
    }

    /** The input method's handwriting started for the field (a stylus wrote over it). */
    fun startStylusHandwriting() {
        if (active && stylusHandwriting && Build.VERSION.SDK_INT >= 34) imm.startStylusHandwriting(view)
    }

    /** Where the caret and the field are (`[caret l, t, r, b, field l, t, r, b]`, px in the view; NaN: unknown). */
    fun cursor(rects: FloatArray) {
        if (rects.size < 8) return
        caret.set(rects[0], rects[1], rects[2], rects[3])
        editor = if (rects[4].isNaN()) null else RectF(rects[4], rects[5], rects[6], rects[7])
        if (cursorMonitor) cursorAnchorInfo()
    }

    /** The field's layout as the app has it (empty arrays: not laid out for the Editable's text). */
    fun layout(
        characters: FloatArray,
        flags: IntArray,
        lines: FloatArray,
        marker: FloatArray,
        markerFlags: Int,
        editor: FloatArray,
    ) {
        characterBounds = characters
        characterFlags = flags
        lineBounds = lines
        this.marker = marker
        this.markerFlags = markerFlags
        decoration = editor
        if (cursorMonitor) cursorAnchorInfo()
    }

    /** The Editable as the app has the field: its text, its selection and composition (UTF-16 offsets). */
    private fun setEditable(text: String, offsets: IntArray) {
        if (editable.toString() != text) editable.replace(0, editable.length, text)
        val length = editable.length
        fun at(i: Int) = offsets.getOrElse(i) { -1 }.coerceIn(-1, length)
        BaseInputConnection.removeComposingSpans(editable)
        Selection.setSelection(editable, at(0).coerceAtLeast(0), at(1).coerceAtLeast(0))
        val (start, end) = at(2) to at(3)
        if (start >= 0 && end >= 0 && start != end) spans.setComposingRegion(minOf(start, end), maxOf(start, end))
    }

    /** The `EditorInfo` of the session's field, and a connection to it (none without a session). */
    fun createInputConnection(outAttrs: EditorInfo): InputConnection? {
        if (!active) return null
        outAttrs.inputType = inputType
        outAttrs.imeOptions = imeOptions
        outAttrs.privateImeOptions = privateImeOptions
        outAttrs.hintLocales = hintLocales
        outAttrs.initialSelStart = Selection.getSelectionStart(editable)
        outAttrs.initialSelEnd = Selection.getSelectionEnd(editable)
        // (As TextView's: the capitals the keyboard starts with where the caret is.)
        outAttrs.initialCapsMode = TextUtils.getCapsMode(editable, outAttrs.initialSelStart.coerceAtLeast(0), inputType)
        if (Build.VERSION.SDK_INT >= 30) outAttrs.setInitialSurroundingText(editable)
        if (acceptsContent) outAttrs.contentMimeTypes = arrayOf("*/*")
        if (stylusHandwriting) {
            if (Build.VERSION.SDK_INT >= 35) {
                outAttrs.isStylusHandwritingEnabled = true
            } else {
                // (Android 14's keyboards read it from the extras: EditorInfoCompat's key.)
                outAttrs.extras = (outAttrs.extras ?: Bundle()).apply { putBoolean(HANDWRITING_EXTRA, true) }
            }
        }
        val c = KompozInputConnection(view, this)
        connection = c
        // (Android 14's handwriting gestures, over the connection: made through it.)
        if (Build.VERSION.SDK_INT >= 34) return Gestures(c)
        return c
    }

    /** The connection with Android 14's handwriting gestures and their previews (Compose's). */
    @android.annotation.TargetApi(34)
    private inner class Gestures(private val c: KompozInputConnection) : InputConnectionWrapper(c, false) {
        override fun performHandwritingGesture(gesture: HandwritingGesture, executor: Executor?, consumer: IntConsumer?) {
            val result = handwritingGesture(c, gesture)
            if (consumer == null) return
            if (executor != null) executor.execute { consumer.accept(result) } else consumer.accept(result)
        }

        override fun previewHandwritingGesture(
            gesture: PreviewableHandwritingGesture,
            cancellationSignal: CancellationSignal?,
        ): Boolean {
            val (kind, values) = encode(gesture) ?: return false
            if (cancellationSignal?.isCanceled == true) return false
            val previewed = view.handwritingGesture(this@KompozIme, session, kind, values, 0f, true).isNotEmpty()
            cancellationSignal?.setOnCancelListener {
                view.handwritingGesture(this@KompozIme, session, CANCEL_PREVIEW, FloatArray(0), 0f, true)
            }
            return previewed
        }
    }

    /** A gesture as the app reads it: its kind, its areas or points in the view, its granularity; none: unknown. */
    private fun encode(gesture: HandwritingGesture): Pair<Int, FloatArray>? {
        if (Build.VERSION.SDK_INT < 34) return null
        val location = IntArray(2)
        view.getLocationOnScreen(location)
        val (x, y) = location[0].toFloat() to location[1].toFloat()
        fun rect(r: RectF) = floatArrayOf(r.left - x, r.top - y, r.right - x, r.bottom - y)
        fun point(p: PointF) = floatArrayOf(p.x - x, p.y - y)
        fun granularity(g: Int) = floatArrayOf(if (g == HandwritingGesture.GRANULARITY_WORD) 1f else 2f)
        return when (gesture) {
            is SelectGesture -> SELECT to rect(gesture.selectionArea) + granularity(gesture.granularity)
            is DeleteGesture -> DELETE to rect(gesture.deletionArea) + granularity(gesture.granularity)
            is SelectRangeGesture -> SELECT_RANGE to
                rect(gesture.selectionStartArea) + rect(gesture.selectionEndArea) + granularity(gesture.granularity)
            is DeleteRangeGesture -> DELETE_RANGE to
                rect(gesture.deletionStartArea) + rect(gesture.deletionEndArea) + granularity(gesture.granularity)
            is JoinOrSplitGesture -> JOIN_OR_SPLIT to point(gesture.joinOrSplitPoint)
            is InsertGesture -> INSERT to point(gesture.insertionPoint)
            is RemoveSpaceGesture -> REMOVE_SPACE to point(gesture.startPoint) + point(gesture.endPoint)
            else -> null
        }
    }

    /**
     * A handwriting gesture hit-tested by the app against the field's layout and made through the connection, a
     * batch of edits (selecting, deleting, inserting the gesture's text or a space, taking spaces out; its fallback
     * text committed where it does nothing): its `HANDWRITING_GESTURE_RESULT_*`.
     */
    private fun handwritingGesture(c: InputConnection, gesture: HandwritingGesture): Int {
        if (Build.VERSION.SDK_INT < 34) return InputConnection.HANDWRITING_GESTURE_RESULT_UNSUPPORTED
        if (!active) return InputConnection.HANDWRITING_GESTURE_RESULT_FAILED
        val (kind, values) = encode(gesture) ?: return InputConnection.HANDWRITING_GESTURE_RESULT_UNSUPPORTED
        val margin = ViewConfiguration.get(view.context).scaledHandwritingGestureLineMargin.toFloat()
        val outcome = view.handwritingGesture(this, session, kind, values, margin, false)
        if (outcome.size < 3) return InputConnection.HANDWRITING_GESTURE_RESULT_FAILED
        val (what, a, b) = Triple(outcome[0], outcome[1], outcome[2])
        val length = editable.length
        if (a !in 0..length || b !in a..length) return InputConnection.HANDWRITING_GESTURE_RESULT_FAILED
        c.beginBatchEdit()
        c.finishComposingText()
        val result = when (what) {
            1 -> c.setSelection(a, b).let { InputConnection.HANDWRITING_GESTURE_RESULT_SUCCESS }
            2 -> {
                c.setSelection(b, b)
                c.deleteSurroundingText(b - a, 0)
                InputConnection.HANDWRITING_GESTURE_RESULT_SUCCESS
            }
            3, 4 -> {
                val text = if (what == 3) (gesture as InsertGesture).textToInsert else " "
                c.setSelection(a, a)
                c.commitText(text, 1)
                InputConnection.HANDWRITING_GESTURE_RESULT_SUCCESS
            }
            5 -> {
                val kept = editable.subSequence(a, b).filterNot { it.isWhitespace() }
                c.setSelection(a, b)
                c.commitText(kept, 1)
                InputConnection.HANDWRITING_GESTURE_RESULT_SUCCESS
            }
            else -> {
                val fallback = gesture.fallbackText
                if (fallback != null) {
                    c.commitText(fallback, 1)
                    InputConnection.HANDWRITING_GESTURE_RESULT_FALLBACK
                } else {
                    InputConnection.HANDWRITING_GESTURE_RESULT_FAILED
                }
            }
        }
        c.endBatchEdit()
        return result
    }

    /** The app's hit test of a handwriting gesture (`text_input`'s `handwriting_gesture`). */
    external fun nativeHandwritingGesture(
        view: KompozView,
        session: Int,
        kind: Int,
        values: FloatArray,
        margin: Float,
        preview: Boolean,
    ): IntArray

    /** A connection closed (the input method let it go). */
    fun closed(c: KompozInputConnection) {
        if (connection === c) connection = null
    }

    /** A batch of the connection's edits done: sent to the app with what they left, the input method told. */
    fun edited(c: KompozInputConnection, ops: IntArray, texts: ByteArray) {
        if (connection !== c || !active) return
        view.imeEdit(session, ops, texts, editable.toString().toByteArray(), offsets())
        updateSelection()
        reportExtractedText()
        cursorAnchorInfo()
        pendingUpdate?.let { (text, offsets) ->
            pendingUpdate = null
            update(session, text, offsets)
        }
    }

    /** The input method's action key. */
    fun editorAction(action: Int) {
        if (active) view.imeAction(session, action)
    }

    /** Content the keyboard commits (its URI, MIME types and link): whether the field took it. */
    fun content(mimeTypes: String, uri: String, linkUri: String?): Boolean =
        active && acceptsContent && view.imeContent(session, mimeTypes.toByteArray(), uri.toByteArray(), linkUri?.toByteArray())

    /** The selection and composition, UTF-16 offsets (-1, -1: no composition). */
    private fun offsets(): IntArray = intArrayOf(
        Selection.getSelectionStart(editable),
        Selection.getSelectionEnd(editable),
        BaseInputConnection.getComposingSpanStart(editable),
        BaseInputConnection.getComposingSpanEnd(editable),
    )

    private fun updateSelection() {
        val o = offsets()
        imm.updateSelection(view, o[0], o[1], o[2], o[3])
    }

    /** The extracted text (the whole of it: the fields are small), as the input method asks for it. */
    fun extractedText(request: ExtractedTextRequest?): ExtractedText = ExtractedText().apply {
        text = if (request != null && request.flags and InputConnection.GET_TEXT_WITH_STYLES != 0) {
            Editable.Factory.getInstance().newEditable(editable)
        } else {
            editable.toString()
        }
        startOffset = 0
        partialStartOffset = -1
        partialEndOffset = -1
        selectionStart = Selection.getSelectionStart(editable)
        selectionEnd = Selection.getSelectionEnd(editable)
        flags = if (inputType and InputType.TYPE_TEXT_FLAG_MULTI_LINE == 0) ExtractedText.FLAG_SINGLE_LINE else 0
    }

    /** The extracted text to an input method following it. */
    private fun reportExtractedText() {
        if (extractedTextToken >= 0) imm.updateExtractedText(view, extractedTextToken, extractedText(null))
    }

    /**
     * The cursor anchor (Compose's CursorAnchorInfoBuilder): the view's place on the screen, the selection, the
     * composing text, the caret as the insertion marker, the field as the editor's and handwriting's bounds.
     */
    fun cursorAnchorInfo() {
        if (!active) return
        val location = IntArray(2)
        view.getLocationOnScreen(location)
        val matrix = Matrix().apply { setTranslate(location[0].toFloat(), location[1].toFloat()) }
        val b = CursorAnchorInfo.Builder().setMatrix(matrix)
        val o = offsets()
        b.setSelectionRange(o[0], o[1])
        if (o[2] >= 0 && o[3] > o[2]) b.setComposingText(o[2], editable.subSequence(o[2], o[3]))
        if (marker.size >= 4) {
            // (Laid out: the caret's glyph, the composing text's characters, the lines that show.)
            b.setInsertionMarkerLocation(marker[0], marker[1], marker[2], marker[3], markerFlags)
            if (o[2] >= 0 && o[3] > o[2]) {
                for (i in o[2] until minOf(o[3], characterFlags.size)) {
                    val r = 4 * i
                    b.addCharacterBounds(
                        i,
                        characterBounds[r],
                        characterBounds[r + 1],
                        characterBounds[r + 2],
                        characterBounds[r + 3],
                        characterFlags[i],
                    )
                }
            }
            if (Build.VERSION.SDK_INT >= 34) {
                for (r in 0 until lineBounds.size / 4) {
                    b.addVisibleLineBounds(lineBounds[4 * r], lineBounds[4 * r + 1], lineBounds[4 * r + 2], lineBounds[4 * r + 3])
                }
            }
        } else if (!caret.isEmpty || caret.height() > 0f) {
            b.setInsertionMarkerLocation(
                caret.left,
                caret.top,
                caret.bottom,
                caret.bottom,
                CursorAnchorInfo.FLAG_HAS_VISIBLE_REGION,
            )
        }
        val field = if (decoration.size >= 4) RectF(decoration[0], decoration[1], decoration[2], decoration[3]) else editor
        if (field != null && Build.VERSION.SDK_INT >= 33) {
            b.setEditorBoundsInfo(EditorBoundsInfo.Builder().setEditorBounds(field).setHandwritingBounds(field).build())
        }
        imm.updateCursorAnchorInfo(view, b.build())
    }

    companion object {
        /** The handwriting gestures' kinds as the app reads them (`handwriting::kind`). */
        private const val SELECT = 1
        private const val DELETE = 2
        private const val SELECT_RANGE = 3
        private const val DELETE_RANGE = 4
        private const val JOIN_OR_SPLIT = 5
        private const val INSERT = 6
        private const val REMOVE_SPACE = 7
        private const val CANCEL_PREVIEW = 8

        /** EditorInfoCompat's extra for stylus handwriting before Android 15. */
        private const val HANDWRITING_EXTRA = "androidx.core.view.inputmethod.EditorInfoCompat.STYLUS_HANDWRITING_ENABLED"
    }
}
