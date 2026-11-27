package dev.kompozr

import android.graphics.Matrix
import android.graphics.RectF
import android.os.Build
import android.os.Bundle
import android.os.LocaleList
import android.text.Editable
import android.text.InputType
import android.text.Selection
import android.text.TextUtils
import android.view.inputmethod.BaseInputConnection
import android.view.inputmethod.CursorAnchorInfo
import android.view.inputmethod.EditorBoundsInfo
import android.view.inputmethod.EditorInfo
import android.view.inputmethod.ExtractedText
import android.view.inputmethod.ExtractedTextRequest
import android.view.inputmethod.InputConnection
import android.view.inputmethod.InputMethodManager

/**
 * The input method's side of a KompozR app (C++ Kompoz's KompozInput IME, Compose's TextInputServiceAndroid and
 * CursorAnchorInfoController): the focused text field's session as the app shows it — its text, selection and
 * composition in an [Editable] (the input method's source of truth: what it reads, what it edits, at once), its
 * `EditorInfo` (input type, IME options, private options, hint locales, content types, stylus handwriting: the
 * app's mapping of its ImeOptions) — and what the input method does to it, sent back to the app a batch at a time
 * ([KompozInputConnection]). The app tells it again when the field ends up elsewhere (it changed its text, a key
 * typed, a filter took an edit back): the input method is restarted for a new text, its selection updated
 * otherwise. Where the caret and the field are goes to the input method as its cursor anchor and editor bounds; the
 * extracted text is reported while it monitors it; stylus handwriting is started for the field (Android 14).
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
        return c
    }

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
        if (!caret.isEmpty || caret.height() > 0f) {
            b.setInsertionMarkerLocation(
                caret.left,
                caret.top,
                caret.bottom,
                caret.bottom,
                CursorAnchorInfo.FLAG_HAS_VISIBLE_REGION,
            )
        }
        val field = editor
        if (field != null && Build.VERSION.SDK_INT >= 33) {
            b.setEditorBoundsInfo(EditorBoundsInfo.Builder().setEditorBounds(field).setHandwritingBounds(field).build())
        }
        imm.updateCursorAnchorInfo(view, b.build())
    }

    companion object {
        /** EditorInfoCompat's extra for stylus handwriting before Android 15. */
        private const val HANDWRITING_EXTRA = "androidx.core.view.inputmethod.EditorInfoCompat.STYLUS_HANDWRITING_ENABLED"
    }
}
