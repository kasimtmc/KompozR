package dev.kompozr

import android.graphics.Rect
import android.os.Build
import android.text.InputType
import android.util.SparseArray
import android.view.View
import android.view.ViewStructure
import android.view.autofill.AutofillManager
import android.view.autofill.AutofillValue

/**
 * Autofill for a KompozR app (Compose's AndroidAutofillManager and `ViewStructure.populate`): the view's virtual
 * structure as the app gives it — its semantics nodes related to autofill (a content type, a content data type,
 * fillable data), each with its autofill id, type, hints, value, texts, states and bounds — what a service fills in
 * given back to the app's nodes, and the AutofillManager told what the app does: a field entered and left, its value
 * changed, shown or gone, the form committed or cancelled, a field asking to be filled. The app's notifications come
 * after its frame; they are told after the call that brought them (the manager may ask the view back).
 */
internal class KompozAutofill(private val view: KompozView) {
    private val manager: AutofillManager? = view.context.getSystemService(AutofillManager::class.java)

    /** The structure (`onProvideAutofillVirtualStructure`'s): `root` the view's own, the app's nodes below it. */
    fun provideStructure(root: ViewStructure) {
        val bytes = view.autofillStructure() ?: return
        val viewId = view.autofillId ?: return
        val r = KompozAccessibility.Reader(bytes)
        val count = r.int()
        val structures = arrayOfNulls<ViewStructure>(count)
        val at = IntArray(2)
        view.getLocationInWindow(at)
        for (i in 0 until count) {
            val id = r.int()
            val parent = r.int()
            val type = r.int()
            val hints = Array(r.int()) { r.str() ?: "" }
            val value = readValue(r)
            val description = r.str()
            val focused = r.int()
            val selected = r.int()
            val clickable = r.bool()
            val longClickable = r.bool()
            val focusable = r.bool()
            val enabled = r.bool()
            val checkable = r.bool()
            val checked = r.bool()
            val sensitive = r.bool()
            val visible = r.bool()
            val left = r.float()
            val top = r.float()
            val right = r.float()
            val bottom = r.float()
            val text = r.str()
            val className = r.str()
            val maxTextLength = r.int()
            val password = r.bool()
            val s = if (parent < 0) root else {
                val p = structures[parent] ?: continue
                p.newChild(p.addChildCount(1))
            }
            structures[i] = s
            if (parent >= 0) s.setAutofillId(viewId, id)
            s.setId(id, view.context.packageName, null, null)
            if (type != AUTOFILL_TYPE_NONE) s.setAutofillType(type)
            if (hints.isNotEmpty()) s.setAutofillHints(hints)
            value?.let { s.setAutofillValue(it) }
            description?.let { s.setContentDescription(it) }
            if (focused >= 0) s.setFocused(focused != 0)
            if (selected >= 0) s.setSelected(selected != 0)
            if (clickable) s.setClickable(true)
            if (longClickable) s.setLongClickable(true)
            if (focusable) s.setFocusable(true)
            if (!enabled) s.setEnabled(false)
            if (checkable) {
                s.setCheckable(true)
                s.setChecked(checked)
            }
            s.setDataIsSensitive(sensitive)
            s.setVisibility(if (visible) View.VISIBLE else View.INVISIBLE)
            s.setDimens(left.toInt(), top.toInt(), 0, 0, (right - left).toInt(), (bottom - top).toInt())
            text?.let { s.setText(it) }
            className?.let { s.setClassName(it) }
            if (maxTextLength >= 0 && Build.VERSION.SDK_INT >= 28) s.setMaxTextLength(maxTextLength)
            if (password) s.setInputType(InputType.TYPE_CLASS_TEXT or InputType.TYPE_TEXT_VARIATION_PASSWORD)
        }
    }

    /** What a service filled in (the view's `autofill(values)`), given to the app's nodes. */
    fun fill(values: SparseArray<AutofillValue>) {
        for (i in 0 until values.size()) {
            val id = values.keyAt(i)
            val value = values.valueAt(i) ?: continue
            when {
                value.isText -> view.autofillValue(id, AUTOFILL_TYPE_TEXT, 0, value.textValue.toString().toByteArray())
                value.isToggle -> view.autofillValue(id, AUTOFILL_TYPE_TOGGLE, if (value.toggleValue) 1 else 0, null)
                value.isList -> view.autofillValue(id, AUTOFILL_TYPE_LIST, value.listValue.toLong(), null)
                value.isDate -> view.autofillValue(id, AUTOFILL_TYPE_DATE, value.dateValue, null)
            }
        }
    }

    /** The app's notifications (KompozR's `encode_notices`), told once the call that brought them is done. */
    fun notify(bytes: ByteArray) {
        view.post { tell(bytes) }
    }

    private fun tell(bytes: ByteArray) {
        val m = manager ?: return
        val r = KompozAccessibility.Reader(bytes)
        repeat(r.int()) {
            val kind = r.int()
            val id = r.int()
            when (kind) {
                NOTICE_ENTERED -> m.notifyViewEntered(view, id, screenRect(r))
                NOTICE_REQUEST -> m.requestAutofill(view, id, screenRect(r))
                NOTICE_EXITED -> m.notifyViewExited(view, id)
                NOTICE_VALUE -> readValue(r)?.let { m.notifyValueChanged(view, id, it) }
                NOTICE_VISIBILITY -> m.notifyViewVisibilityChanged(view, id, r.bool())
                NOTICE_COMMIT -> m.commit()
                NOTICE_CANCEL -> m.cancel()
            }
        }
    }

    /** A rectangle in the view (four floats) in the screen. */
    private fun screenRect(r: KompozAccessibility.Reader): Rect {
        val left = r.float()
        val top = r.float()
        val right = r.float()
        val bottom = r.float()
        val at = IntArray(2)
        view.getLocationOnScreen(at)
        return Rect(left.toInt() + at[0], top.toInt() + at[1], right.toInt() + at[0], bottom.toInt() + at[1])
    }

    /** A value as the app writes it: its type, a number, a text (none: no value). */
    private fun readValue(r: KompozAccessibility.Reader): AutofillValue? {
        val type = r.int()
        val number = r.long()
        val text = r.str()
        return when (type) {
            AUTOFILL_TYPE_TEXT -> AutofillValue.forText(text ?: "")
            AUTOFILL_TYPE_TOGGLE -> AutofillValue.forToggle(number != 0L)
            AUTOFILL_TYPE_LIST -> AutofillValue.forList(number.toInt())
            AUTOFILL_TYPE_DATE -> AutofillValue.forDate(number)
            else -> null
        }
    }

    companion object {
        private const val AUTOFILL_TYPE_NONE = View.AUTOFILL_TYPE_NONE
        private const val AUTOFILL_TYPE_TEXT = View.AUTOFILL_TYPE_TEXT
        private const val AUTOFILL_TYPE_TOGGLE = View.AUTOFILL_TYPE_TOGGLE
        private const val AUTOFILL_TYPE_LIST = View.AUTOFILL_TYPE_LIST
        private const val AUTOFILL_TYPE_DATE = View.AUTOFILL_TYPE_DATE

        // KompozR's android::autofill `notice`.
        private const val NOTICE_ENTERED = 0
        private const val NOTICE_EXITED = 1
        private const val NOTICE_VALUE = 2
        private const val NOTICE_VISIBILITY = 3
        private const val NOTICE_COMMIT = 4
        private const val NOTICE_CANCEL = 5
        private const val NOTICE_REQUEST = 6
    }
}
