package dev.kompozr

import android.os.Build
import android.os.SystemClock
import android.util.LongSparseArray
import android.view.View
import android.view.ViewStructure
import android.view.contentcapture.ContentCaptureSession
import android.view.translation.TranslationRequestValue
import android.view.translation.ViewTranslationCallback
import android.view.translation.ViewTranslationRequest
import android.view.translation.ViewTranslationResponse
import android.annotation.TargetApi
import java.util.function.Consumer

/**
 * Content capture and translation for a KompozR app (Compose's AndroidContentCaptureManager): while the view's
 * window is shown and the system captures content (its ContentCaptureSession), the app's shown semantics nodes
 * (the unmerged tree's) are told as virtual views — all of them as the window shows, then at most every 100 ms what
 * appeared (a structure each: its class, texts, description, test tag, text size, bounds), what disappeared and what
 * texts changed, all of them disappearing as it hides. The system's translation (Android 12) asks the nodes' texts,
 * gives their translations to the app (its semantics' text substitutions) and has them shown, hidden or cleared.
 */
internal class KompozContentCapture(private val view: KompozView) {
    private var session: ContentCaptureSession? = null

    /** The app told the session started (it may not have been able to yet: its core starts with the surface). */
    private var started = false
    private var checkPosted = false
    private var lastCheck = 0L
    private val checker = Runnable { check() }

    /** The window shown: a session's start, every node told (as soon as the app can). */
    fun start() {
        if (session != null) return
        session = view.contentCaptureSession ?: return
        started = false
        check()
    }

    /** The window hidden or gone: every node disappearing, the session let go. */
    fun stop() {
        if (session == null) return
        view.removeCallbacks(checker)
        checkPosted = false
        if (started) apply(view.contentCapture(CAPTURE_STOP))
        started = false
        session = null
    }

    /** The app's semantics or layout changed: what changed told in a while. */
    fun changed() {
        if (session == null || checkPosted) return
        checkPosted = true
        view.postDelayed(checker, (lastCheck + SEND_INTERVAL_MS - SystemClock.uptimeMillis()).coerceAtLeast(0))
    }

    private fun check() {
        checkPosted = false
        lastCheck = SystemClock.uptimeMillis()
        if (session == null) return
        val bytes = view.contentCapture(if (started) CAPTURE_CHANGES else CAPTURE_START)
        if (bytes == null) {
            changed() // (the app busy, or not started: again in a while)
            return
        }
        started = true
        apply(bytes)
    }

    /** The app's events (KompozR's capture `encode_events`) told to the session, then flushed (Compose's). */
    private fun apply(bytes: ByteArray?) {
        val s = session ?: return
        val viewId = view.autofillId ?: return
        bytes ?: return
        val r = KompozAccessibility.Reader(bytes)
        val count = r.int()
        if (count == 0) return
        repeat(count) {
            when (r.int()) {
                CAPTURE_APPEARED -> {
                    val id = r.long()
                    val parent = r.long()
                    val index = r.int()
                    val left = r.float()
                    val top = r.float()
                    val width = r.float()
                    val height = r.float()
                    val className = r.str()
                    val text = r.str()
                    val editable = r.str()
                    val description = r.str()
                    val testTag = r.str()
                    val textSize = r.float()
                    val parentId = if (parent < 0) viewId else s.newAutofillId(viewId, parent)
                    val structure: ViewStructure = s.newVirtualViewStructure(parentId, id)
                    structure.extras?.let {
                        it.putLong(KEY_TIMESTAMP, System.currentTimeMillis())
                        it.putInt(KEY_INDEX, index)
                    }
                    testTag?.let { structure.setId(id.toInt(), null, null, it) }
                    className?.let { structure.setClassName(it) }
                    text?.let { structure.setText(it) }
                    editable?.let { structure.setText(it) }
                    description?.let { structure.setContentDescription(it) }
                    if (textSize > 0f) structure.setTextStyle(textSize, 0, 0, 0)
                    structure.setDimens(left.toInt(), top.toInt(), 0, 0, width.toInt(), height.toInt())
                    s.notifyViewAppeared(structure)
                }
                CAPTURE_DISAPPEARED -> s.notifyViewDisappeared(s.newAutofillId(viewId, r.long()))
                CAPTURE_TEXT_CHANGED -> {
                    val id = r.long()
                    s.notifyViewTextChanged(s.newAutofillId(viewId, id), r.str())
                }
            }
        }
        // (A batch's end: Compose's flush, a disappearance of no node.)
        s.notifyViewsDisappeared(viewId, longArrayOf(Long.MIN_VALUE))
    }

    // ---- Translation (Android 12) ----

    /** The nodes' texts asked for (the view's `onCreateVirtualViewTranslationRequests`). */
    @TargetApi(31)
    fun translationRequests(virtualIds: LongArray, requests: Consumer<ViewTranslationRequest>) {
        val viewId = view.autofillId ?: return
        for (id in virtualIds) {
            val text = view.translationText(id) ?: continue
            val request = ViewTranslationRequest.Builder(viewId, id)
                .setValue(ViewTranslationRequest.ID_TEXT, TranslationRequestValue.forText(text))
                .build()
            requests.accept(request)
        }
    }

    /** Their translations, given to the app's nodes (on this thread). */
    @TargetApi(31)
    fun translationResponses(responses: LongSparseArray<ViewTranslationResponse>) {
        if (Build.VERSION.SDK_INT < 31) return
        val apply = Runnable {
            for (i in 0 until responses.size()) {
                val id = responses.keyAt(i)
                val text = responses.valueAt(i)?.getValue(ViewTranslationRequest.ID_TEXT)?.text ?: continue
                view.translation(id, text.toString())
            }
        }
        if (android.os.Looper.getMainLooper().isCurrentThread) apply.run() else view.post(apply)
    }

    /** What shows the translations (Compose's ViewTranslationCallback). */
    @TargetApi(31)
    fun translationCallback(): ViewTranslationCallback = object : ViewTranslationCallback {
        override fun onShowTranslation(view: View): Boolean {
            this@KompozContentCapture.view.showTranslation(TRANSLATION_SHOW)
            return true
        }

        override fun onHideTranslation(view: View): Boolean {
            this@KompozContentCapture.view.showTranslation(TRANSLATION_HIDE)
            return true
        }

        override fun onClearTranslation(view: View): Boolean {
            this@KompozContentCapture.view.showTranslation(TRANSLATION_CLEAR)
            return true
        }
    }

    companion object {
        private const val SEND_INTERVAL_MS = 100L
        private const val KEY_TIMESTAMP = "android.view.contentcapture.EventTimestamp"
        private const val KEY_INDEX = "android.view.ViewStructure.extra.EXTRA_VIEW_NODE_INDEX"

        // KompozR's delegate `CAPTURE_*` asks and capture `kind`s.
        const val CAPTURE_CHANGES = 0
        const val CAPTURE_START = 1
        const val CAPTURE_STOP = 2
        private const val CAPTURE_APPEARED = 0
        private const val CAPTURE_DISAPPEARED = 1
        private const val CAPTURE_TEXT_CHANGED = 2

        private const val TRANSLATION_HIDE = 0
        private const val TRANSLATION_SHOW = 1
        private const val TRANSLATION_CLEAR = 2
    }
}
