package dev.kompozr

import android.os.Bundle
import android.view.KeyEvent
import android.view.inputmethod.BaseInputConnection
import android.view.inputmethod.CompletionInfo
import android.view.inputmethod.CorrectionInfo
import android.view.inputmethod.ExtractedText
import android.view.inputmethod.ExtractedTextRequest
import android.view.inputmethod.InputConnection
import android.view.inputmethod.InputContentInfo
import android.view.inputmethod.TextAttribute
import java.io.ByteArrayOutputStream

/**
 * The input method's connection to the focused field (C++ Kompoz's KompozInput.Connection, Compose's
 * RecordingInputConnection): Android's own [BaseInputConnection] over [KompozIme]'s Editable (a full editor: the
 * text around the caret, the selected and extracted text, the caps mode answered from it), each edit both applied
 * to the Editable and recorded; a batch's edits (`beginBatchEdit` … `endBatchEdit`, or one edit alone) sent to the
 * app together with what they left, so the field takes them as the input method's commands (its filters run). Keys
 * the input method sends go to the view as hardware keys do (`sendKeyEvent`: the field's key handling); its action
 * key, the content it commits and the context menu's actions go to the app.
 */
internal class KompozInputConnection(private val view: KompozView, private val ime: KompozIme) :
    BaseInputConnection(view, true) {

    /** How deep in batches the input method is (the edits go to the app as the outermost ends). */
    var batchDepth = 0
        private set

    private val ops = ArrayList<Int>()
    private val texts = ByteArrayOutputStream()
    private var closed = false

    override fun getEditable() = ime.editable

    override fun beginBatchEdit(): Boolean {
        if (closed) return false
        batchDepth++
        return true
    }

    override fun endBatchEdit(): Boolean {
        if (batchDepth == 0) return false
        batchDepth--
        if (batchDepth == 0) flush()
        return batchDepth > 0
    }

    override fun closeConnection() {
        if (closed) return
        if (batchDepth > 0) {
            batchDepth = 0
            flush()
        }
        closed = true
        super.closeConnection()
        ime.closed(this)
    }

    /** `apply` as one edit: recorded (`op`, its two arguments, its text), applied to the Editable, sent unless a
     * batch goes on. */
    private inline fun edit(op: Int, a: Int, b: Int, text: CharSequence?, apply: () -> Boolean): Boolean {
        if (closed) return false
        batchDepth++
        val bytes = text?.toString()?.toByteArray() ?: ByteArray(0)
        ops.add(op)
        ops.add(a)
        ops.add(b)
        ops.add(bytes.size)
        texts.write(bytes)
        val done = apply()
        endBatchEdit()
        return done
    }

    /** The batch's edits to the app (through KompozIme: with what they left). */
    private fun flush() {
        if (ops.isEmpty()) return
        val recorded = ops.toIntArray()
        val bytes = texts.toByteArray()
        ops.clear()
        texts.reset()
        ime.edited(this, recorded, bytes)
    }

    override fun commitText(text: CharSequence?, newCursorPosition: Int): Boolean =
        edit(OP_COMMIT_TEXT, newCursorPosition, 0, text ?: "") { super.commitText(text ?: "", newCursorPosition) }

    override fun setComposingText(text: CharSequence?, newCursorPosition: Int): Boolean =
        edit(OP_SET_COMPOSING_TEXT, newCursorPosition, 0, text ?: "") {
            super.setComposingText(text ?: "", newCursorPosition)
        }

    override fun setComposingRegion(start: Int, end: Int): Boolean =
        edit(OP_SET_COMPOSING_REGION, start, end, null) { super.setComposingRegion(start, end) }

    override fun finishComposingText(): Boolean =
        edit(OP_FINISH_COMPOSING_TEXT, 0, 0, null) { super.finishComposingText() }

    override fun deleteSurroundingText(beforeLength: Int, afterLength: Int): Boolean =
        edit(OP_DELETE_SURROUNDING_TEXT, beforeLength, afterLength, null) {
            super.deleteSurroundingText(beforeLength, afterLength)
        }

    override fun deleteSurroundingTextInCodePoints(beforeLength: Int, afterLength: Int): Boolean =
        edit(OP_DELETE_SURROUNDING_TEXT_IN_CODE_POINTS, beforeLength, afterLength, null) {
            super.deleteSurroundingTextInCodePoints(beforeLength, afterLength)
        }

    override fun setSelection(start: Int, end: Int): Boolean =
        edit(OP_SET_SELECTION, start, end, null) { super.setSelection(start, end) }

    /** A suggestion put in place of a range (Android 14; Gboard's): InputConnection's own steps, as the field takes
     * them. */
    override fun replaceText(
        start: Int,
        end: Int,
        text: CharSequence,
        newCursorPosition: Int,
        textAttribute: TextAttribute?,
    ): Boolean {
        if (closed) return false
        beginBatchEdit()
        finishComposingText()
        setSelection(start, end)
        commitText(text, newCursorPosition)
        endBatchEdit()
        return true
    }

    override fun commitCompletion(text: CompletionInfo?): Boolean =
        text?.text?.let { commitText(it, 1) } ?: false

    /** (The field shows no correction highlight: nothing to do, as Compose's.) */
    override fun commitCorrection(correctionInfo: CorrectionInfo?): Boolean = !closed

    override fun performEditorAction(editorAction: Int): Boolean {
        if (closed) return false
        ime.editorAction(editorAction)
        return true
    }

    /** The input method's context menu: select all as a selection, cut, copy and paste as their keys (Compose's). */
    override fun performContextMenuAction(id: Int): Boolean {
        if (closed) return false
        when (id) {
            android.R.id.selectAll -> return setSelection(0, ime.editable.length)
            android.R.id.cut -> sendKey(KeyEvent.KEYCODE_CUT)
            android.R.id.copy -> sendKey(KeyEvent.KEYCODE_COPY)
            android.R.id.paste -> sendKey(KeyEvent.KEYCODE_PASTE)
            else -> return false
        }
        return true
    }

    private fun sendKey(code: Int) {
        sendKeyEvent(KeyEvent(KeyEvent.ACTION_DOWN, code))
        sendKeyEvent(KeyEvent(KeyEvent.ACTION_UP, code))
    }

    override fun getExtractedText(request: ExtractedTextRequest?, flags: Int): ExtractedText? {
        if (closed) return null
        if (flags and InputConnection.GET_EXTRACTED_TEXT_MONITOR != 0) ime.extractedTextToken = request?.token ?: 0
        return ime.extractedText(request)
    }

    override fun requestCursorUpdates(cursorUpdateMode: Int): Boolean {
        if (closed) return false
        ime.cursorMonitor = cursorUpdateMode and InputConnection.CURSOR_UPDATE_MONITOR != 0
        if (cursorUpdateMode and InputConnection.CURSOR_UPDATE_IMMEDIATE != 0) ime.cursorAnchorInfo()
        return true
    }

    /** An image, a GIF, a sticker the keyboard sends: its URI's read permission asked for, then given to the field
     * (Compose's `onCommitContent`); whether it took it. */
    override fun commitContent(inputContentInfo: InputContentInfo, flags: Int, opts: Bundle?): Boolean {
        if (closed) return false
        if (flags and InputConnection.INPUT_CONTENT_GRANT_READ_URI_PERMISSION != 0) {
            try {
                inputContentInfo.requestPermission()
            } catch (_: Exception) {
                return false
            }
        }
        val description = inputContentInfo.description
        val types = (0 until description.mimeTypeCount).joinToString("\n") { description.getMimeType(it) }
        return ime.content(types, inputContentInfo.contentUri.toString(), inputContentInfo.linkUri?.toString())
    }

    /** (A private command the field doesn't know: not taken.) */
    override fun performPrivateCommand(action: String?, data: Bundle?): Boolean = false

    companion object {
        // The edits as the app reads them (kompoz-platform's android::ime::op).
        const val OP_COMMIT_TEXT = 1
        const val OP_SET_COMPOSING_TEXT = 2
        const val OP_SET_COMPOSING_REGION = 3
        const val OP_FINISH_COMPOSING_TEXT = 4
        const val OP_DELETE_SURROUNDING_TEXT = 5
        const val OP_DELETE_SURROUNDING_TEXT_IN_CODE_POINTS = 6
        const val OP_SET_SELECTION = 7
    }
}
