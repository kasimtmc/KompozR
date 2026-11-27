package dev.kompozr.showcase

import android.app.assist.AssistStructure
import android.os.CancellationSignal
import android.service.autofill.AutofillService
import android.service.autofill.Dataset
import android.service.autofill.FillCallback
import android.service.autofill.FillRequest
import android.service.autofill.FillResponse
import android.service.autofill.SaveCallback
import android.service.autofill.SaveRequest
import android.util.Log
import android.view.autofill.AutofillId
import android.view.autofill.AutofillValue
import android.widget.RemoteViews

/**
 * An autofill service for the debug build's accessibility probe (the emulator has none; `cargo xtask android a11y
 * --autofill` makes it the device's): it logs the structure it is given (tag "kompoz-probe": each node's virtual id,
 * type, hints, class, value) and offers one dataset, "KompozR probe", filling an e-mail field with
 * `ada@example.com` and a password field with `parola`; a save request is logged too.
 */
class ProbeAutofillService : AutofillService() {
    override fun onFillRequest(request: FillRequest, signal: CancellationSignal, callback: FillCallback) {
        val structure = request.fillContexts.last().structure
        val fill = mutableListOf<Pair<AutofillId, String>>()
        for (w in 0 until structure.windowNodeCount) visit(structure.getWindowNodeAt(w).rootViewNode, 0, fill)
        if (fill.isEmpty()) {
            callback.onSuccess(null)
            return
        }
        val presentation = RemoteViews(packageName, android.R.layout.simple_list_item_1)
        presentation.setTextViewText(android.R.id.text1, "KompozR probe")
        @Suppress("DEPRECATION")
        val dataset = Dataset.Builder(presentation).apply {
            for ((id, value) in fill) setValue(id, AutofillValue.forText(value))
        }.build()
        callback.onSuccess(FillResponse.Builder().addDataset(dataset).build())
    }

    private fun visit(n: AssistStructure.ViewNode, depth: Int, fill: MutableList<Pair<AutofillId, String>>) {
        val hints = n.autofillHints?.toList() ?: emptyList()
        if (n.autofillType != 0 || hints.isNotEmpty()) {
            Log.i(
                TAG,
                "node ${n.autofillId} type=${n.autofillType} hints=$hints class=${n.className} " +
                    "value=${n.autofillValue?.let { if (it.isText) it.textValue else it }}",
            )
        }
        val id = n.autofillId
        if (id != null) {
            when {
                "emailAddress" in hints -> fill += id to "ada@example.com"
                "password" in hints -> fill += id to "parola"
            }
        }
        for (i in 0 until n.childCount) visit(n.getChildAt(i), depth + 1, fill)
    }

    override fun onSaveRequest(request: SaveRequest, callback: SaveCallback) {
        Log.i(TAG, "save request: ${request.fillContexts.size} contexts")
        callback.onSuccess()
    }

    companion object {
        private const val TAG = "kompoz-probe"
    }
}
