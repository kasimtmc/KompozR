package dev.kompozr

import android.graphics.Rect
import android.view.ActionMode
import android.view.Menu
import android.view.MenuItem
import android.view.View
import kotlin.math.ceil
import kotlin.math.floor

/**
 * The floating text toolbar (Compose's AndroidTextToolbar and its FloatingTextActionModeCallback): Android's
 * floating action mode over a text field's or a selection's rect, with the items the app offers — copy, paste,
 * cut, select all, autofill, in Compose's order —; an item clicked goes to the app (its action) and the toolbar
 * closes; shown again over another rect, it moves.
 */
internal class KompozTextToolbar(private val view: KompozView) {
    private var mode: ActionMode? = null
    private var items = 0
    private val rect = Rect()

    private val callback = object : ActionMode.Callback2() {
        override fun onCreateActionMode(mode: ActionMode, menu: Menu): Boolean {
            for ((bit, title) in ITEMS) {
                if (items and bit != 0) {
                    menu.add(0, bit, ITEMS.indexOfFirst { it.first == bit }, title)
                        .setShowAsAction(MenuItem.SHOW_AS_ACTION_ALWAYS)
                }
            }
            return true
        }

        override fun onPrepareActionMode(mode: ActionMode, menu: Menu): Boolean = false

        override fun onActionItemClicked(mode: ActionMode, item: MenuItem): Boolean {
            view.toolbarAction(item.itemId)
            mode.finish()
            return true
        }

        override fun onDestroyActionMode(mode: ActionMode) {
            if (this@KompozTextToolbar.mode === mode) this@KompozTextToolbar.mode = null
        }

        override fun onGetContentRect(mode: ActionMode, view: View, outRect: Rect) = outRect.set(rect)
    }

    /** Shown over `values` (`[left, top, right, bottom]`, px in the view) with the items of `items`' bits. */
    fun show(values: FloatArray, items: Int) {
        if (values.size < 4) return
        rect.set(floor(values[0]).toInt(), floor(values[1]).toInt(), ceil(values[2]).toInt(), ceil(values[3]).toInt())
        val current = mode
        if (current != null && this.items == items) {
            current.invalidateContentRect()
            return
        }
        this.items = items
        current?.finish()
        mode = view.startActionMode(callback, ActionMode.TYPE_FLOATING)
    }

    fun hide() {
        mode?.finish()
        mode = null
    }

    companion object {
        // The items (kompoz-platform's android::text_input::toolbar_item), each its bit and its title.
        private val ITEMS = listOf(
            1 to android.R.string.copy,
            2 to android.R.string.paste,
            4 to android.R.string.cut,
            8 to android.R.string.selectAll,
            16 to android.R.string.autofill,
        )
    }
}
