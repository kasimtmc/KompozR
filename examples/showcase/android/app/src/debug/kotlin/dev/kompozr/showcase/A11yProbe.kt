package dev.kompozr.showcase

import android.app.Activity
import android.app.Instrumentation
import android.content.Intent
import android.os.Build
import android.os.Bundle
import android.os.SystemClock
import android.view.accessibility.AccessibilityEvent
import android.view.accessibility.AccessibilityNodeInfo

/**
 * The showcase's accessibility as a service sees it, without TalkBack (debug builds; `cargo xtask android a11y`):
 * an instrumentation of the app whose UiAutomation is an accessibility service. It starts the activity, then runs
 * its `steps` argument (`;`-separated) on the app's window and reports what it found in its result:
 *
 * - `dump`: every node (class, text, description, state, hint, flags, range, collection, actions);
 * - (`<text>`: a node's text or description, or the start of a field's label; `~<label>`: a field's only)
 * - `click:<text>`, `longclick:<text>`, `focus:<text>`, `scroll:<text>`, `scrollback:<text>` (an empty text: the
 *   first scrollable), `expand:<text>`, `collapse:<text>`: that action on the first node saying `<text>`;
 * - `settext:<text>=<value>`, `select:<text>=<start>,<end>`, `next:<text>=<granularity>`: a text's actions;
 * - `chars:<text>`: its characters' boxes (the extra data);
 * - `progress:<text>=<value>`: a slider set; `custom:<text>=<label>`: its custom action of that label;
 * - `scrollcapture`: a long screenshot's session run in the app's process (Android 12: the view's target found,
 *   started, two parts asked for — the viewport and the one below it —, their pixels checked, ended);
 * - `has:<text>`: a node saying it is there; `pick:<text>`: a node of any window saying it clicked (an autofill
 *   service's suggestion);
 * - `wait:<ms>`; `events`: the accessibility events since the last `events`, by type (content changes with their
 *   change types).
 *
 * Each line of the result is one finding; a step that fails says "FAIL" (the xtask fails the run on it).
 */
class A11yProbe : Instrumentation() {
    private val events = mutableListOf<String>()

    override fun onCreate(arguments: Bundle?) {
        super.onCreate(arguments)
        steps = arguments?.getString("steps") ?: "dump"
        start()
    }

    private var steps = "dump"
    private var activity: Activity? = null

    override fun onStart() {
        super.onStart()
        val out = StringBuilder()
        try {
            val ui = uiAutomation
            ui.serviceInfo = ui.serviceInfo.apply {
                flags = flags or android.accessibilityservice.AccessibilityServiceInfo.FLAG_RETRIEVE_INTERACTIVE_WINDOWS
            }
            ui.setOnAccessibilityEventListener { e ->
                val line = describe(e)
                synchronized(events) { events += line }
            }
            val intent = Intent(targetContext, MainActivity::class.java).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
            activity = startActivitySync(intent)
            waitForIdleSync()
            // (The app's first frames: its tree comes after the first.)
            root(10_000) ?: throw IllegalStateException("no window of the app's")
            for (step in steps.split(';').map(String::trim).filter(String::isNotEmpty)) {
                out.append("> ").append(step).append('\n')
                run(step, out)
            }
            val result = Bundle()
            result.putString("result", out.toString())
            finish(Activity.RESULT_OK, result)
        } catch (t: Throwable) {
            val result = Bundle()
            result.putString("result", out.toString())
            result.putString("error", "FAIL ${t.javaClass.simpleName}: ${t.message}")
            finish(Activity.RESULT_CANCELED, result)
        }
    }

    /** The app's window's root, once it has the app's nodes. */
    private fun root(timeoutMs: Long): AccessibilityNodeInfo? {
        val until = SystemClock.uptimeMillis() + timeoutMs
        while (SystemClock.uptimeMillis() < until) {
            val r = uiAutomation.rootInActiveWindow
            if (r != null && r.packageName == targetContext.packageName && count(r) > 2) return r
            SystemClock.sleep(200)
        }
        return null
    }

    private fun count(n: AccessibilityNodeInfo): Int {
        var c = 1
        for (i in 0 until n.childCount) n.getChild(i)?.let { c += count(it) }
        return c
    }

    /** The first node (depth first) that says `text` (its text, description or hint), or the first scrollable. */
    private fun find(text: String, scrollable: Boolean = false): AccessibilityNodeInfo {
        val r = root(5_000) ?: throw IllegalStateException("no window")
        // (Its text or description that, or a field's label beginning with it; `~label`: a field's only.)
        fun says(n: AccessibilityNodeInfo) = if (text.startsWith("~")) {
            n.isEditable && n.hintText?.toString()?.startsWith(text.substring(1)) == true
        } else {
            listOf(n.text, n.contentDescription).any { it?.toString() == text } ||
                n.hintText?.toString()?.startsWith(text) == true
        }
        fun visit(n: AccessibilityNodeInfo): AccessibilityNodeInfo? {
            if (if (text.isEmpty()) scrollable && n.isScrollable else says(n)) return n
            for (i in 0 until n.childCount) n.getChild(i)?.let { c -> visit(c)?.let { return it } }
            return null
        }
        return visit(r) ?: throw IllegalStateException("no node says \"$text\"")
    }

    private fun run(step: String, out: StringBuilder) {
        val (name, rest) = step.split(':', limit = 2).let { it[0] to it.getOrElse(1) { "" } }
        val (target, value) = rest.split('=', limit = 2).let { it[0] to it.getOrElse(1) { "" } }
        fun act(action: Int, args: Bundle? = null, scrollable: Boolean = false) {
            val n = find(target, scrollable)
            val done = n.performAction(action, args)
            out.append(if (done) "done " else "FAIL not done ").append(line(n)).append('\n')
            waitForIdleSync()
        }
        when (name) {
            "dump" -> dump(root(5_000) ?: throw IllegalStateException("no window"), 0, out)
            "click" -> act(AccessibilityNodeInfo.ACTION_CLICK)
            "longclick" -> act(AccessibilityNodeInfo.ACTION_LONG_CLICK)
            "focus" -> act(AccessibilityNodeInfo.ACTION_FOCUS)
            "scroll" -> act(AccessibilityNodeInfo.ACTION_SCROLL_FORWARD, scrollable = true)
            "scrollback" -> act(AccessibilityNodeInfo.ACTION_SCROLL_BACKWARD, scrollable = true)
            "expand" -> act(AccessibilityNodeInfo.ACTION_EXPAND)
            "collapse" -> act(AccessibilityNodeInfo.ACTION_COLLAPSE)
            "settext" -> act(
                AccessibilityNodeInfo.ACTION_SET_TEXT,
                Bundle().apply { putCharSequence(AccessibilityNodeInfo.ACTION_ARGUMENT_SET_TEXT_CHARSEQUENCE, value) },
            )
            "select" -> {
                val (start, end) = value.split(',').map(String::toInt)
                act(
                    AccessibilityNodeInfo.ACTION_SET_SELECTION,
                    Bundle().apply {
                        putInt(AccessibilityNodeInfo.ACTION_ARGUMENT_SELECTION_START_INT, start)
                        putInt(AccessibilityNodeInfo.ACTION_ARGUMENT_SELECTION_END_INT, end)
                    },
                )
            }
            "next" -> act(
                AccessibilityNodeInfo.ACTION_NEXT_AT_MOVEMENT_GRANULARITY,
                Bundle().apply { putInt(AccessibilityNodeInfo.ACTION_ARGUMENT_MOVEMENT_GRANULARITY_INT, value.toInt()) },
            )
            "progress" -> act(
                android.R.id.accessibilityActionSetProgress,
                Bundle().apply { putFloat(AccessibilityNodeInfo.ACTION_ARGUMENT_PROGRESS_VALUE, value.toFloat()) },
            )
            "custom" -> {
                val n = find(target)
                val action = n.actionList.firstOrNull { it.label?.toString() == value }
                    ?: throw IllegalStateException("no custom action \"$value\"")
                act(action.id)
            }
            "chars" -> {
                val n = find(target)
                val length = n.text?.length ?: 0
                val args = Bundle().apply {
                    putInt(AccessibilityNodeInfo.EXTRA_DATA_TEXT_CHARACTER_LOCATION_ARG_START_INDEX, 0)
                    putInt(AccessibilityNodeInfo.EXTRA_DATA_TEXT_CHARACTER_LOCATION_ARG_LENGTH, length)
                }
                val key = AccessibilityNodeInfo.EXTRA_DATA_TEXT_CHARACTER_LOCATION_KEY
                val refreshed = n.refreshWithExtraData(key, args)
                @Suppress("DEPRECATION")
                val boxes = n.extras.getParcelableArray(key)
                val shown = boxes?.count { it != null } ?: 0
                out.append(if (refreshed && shown > 0) "" else "FAIL ").append("$shown of $length characters' boxes")
                out.append(" (extra data: ${n.availableExtraData})\n")
            }
            "wait" -> SystemClock.sleep(target.toLong())
            // The system's clipboard as another app sets and reads it (KompozClipboard's side is the app's).
            "setclip" -> {
                val clipboard = targetContext.getSystemService(android.content.ClipboardManager::class.java)
                runOnMainSync { clipboard.setPrimaryClip(android.content.ClipData.newPlainText("probe", rest)) }
                out.append("clipboard set: ").append(rest).append('\n')
            }
            "getclip" -> {
                val clipboard = targetContext.getSystemService(android.content.ClipboardManager::class.java)
                var text: CharSequence? = null
                runOnMainSync { text = clipboard.primaryClip?.getItemAt(0)?.coerceToText(targetContext) }
                out.append("clipboard holds: ").append(text).append('\n')
            }
            // KompozContent (the library's, internal: by reflection) as the image loader's threads call it.
            "http" -> {
                val content = Class.forName("dev.kompozr.KompozContent")
                val body = content.getMethod("httpGet", ByteArray::class.java).invoke(null, rest.toByteArray()) as ByteArray?
                val decode = content.getMethod(
                    "decodeImage", ByteArray::class.java, Int::class.java, Int::class.java, Boolean::class.java,
                )
                val image = body?.let { decode.invoke(null, it, 0, 0, false) as IntArray? }
                out.append(if (body == null) "FAIL " else "").append("http ${body?.size ?: 0} bytes")
                out.append(image?.let { ", decoded ${it[0]}x${it[1]}" } ?: "").append('\n')
            }
            "scrollcapture" -> scrollCapture(out)
            "has" -> {
                val n = find(target)
                out.append("found ").append(line(n)).append('\n')
            }
            "pick" -> {
                // (In any window: an autofill service's suggestions are a window of the system's.)
                val until = SystemClock.uptimeMillis() + 10_000
                var picked: AccessibilityNodeInfo? = null
                while (picked == null && SystemClock.uptimeMillis() < until) {
                    for (w in uiAutomation.windows) {
                        val r = w.root ?: continue
                        picked = r.findAccessibilityNodeInfosByText(target).firstOrNull()
                        if (picked != null) break
                    }
                    if (picked == null) SystemClock.sleep(300)
                }
                val p = picked ?: throw IllegalStateException("no window says \"$target\"")
                var clickable: AccessibilityNodeInfo? = p
                while (clickable != null && !clickable.isClickable) clickable = clickable.parent
                val done = (clickable ?: p).performAction(AccessibilityNodeInfo.ACTION_CLICK)
                out.append(if (done) "picked " else "FAIL not picked ").append(target).append('\n')
                waitForIdleSync()
            }
            "events" -> {
                val seen = synchronized(events) { events.toList().also { events.clear() } }
                seen.groupingBy { it }.eachCount().forEach { (e, n) -> out.append("$n× $e\n") }
                if (seen.isEmpty()) out.append("(no events)\n")
            }
            else -> throw IllegalArgumentException("unknown step $name")
        }
    }

    /** A scroll capture session driven by hand, as the system's screenshot would. */
    private fun scrollCapture(out: StringBuilder) {
        if (Build.VERSION.SDK_INT < 31) {
            out.append("(no scroll capture before Android 12)\n")
            return
        }
        val decor = activity?.window?.decorView as? android.view.ViewGroup ?: throw IllegalStateException("no activity")
        val targets = mutableListOf<android.view.ScrollCaptureTarget>()
        runOnMainSync {
            decor.dispatchScrollCaptureSearch(
                android.graphics.Rect(0, 0, decor.width, decor.height),
                android.graphics.Point(0, 0),
            ) { targets += it }
        }
        val target = targets.firstOrNull() ?: throw IllegalStateException("FAIL no scroll capture target")
        val bounds = target.scrollBounds ?: target.localVisibleRect
        out.append("target ${target.containingView.javaClass.simpleName} bounds=$bounds\n")
        val reader = android.media.ImageReader.newInstance(
            bounds.width(), bounds.height(), android.graphics.PixelFormat.RGBA_8888, 2,
            android.hardware.HardwareBuffer.USAGE_GPU_COLOR_OUTPUT or android.hardware.HardwareBuffer.USAGE_CPU_READ_OFTEN,
        )
        val session = android.view.ScrollCaptureSession(reader.surface, android.graphics.Rect(bounds), android.graphics.Point(0, 0))
        val callback = target.callback
        fun await(block: (Runnable) -> Unit) {
            val latch = java.util.concurrent.CountDownLatch(1)
            runOnMainSync { block(Runnable { latch.countDown() }) }
            if (!latch.await(10, java.util.concurrent.TimeUnit.SECONDS)) throw IllegalStateException("FAIL timed out")
        }
        await { done -> callback.onScrollCaptureStart(session, android.os.CancellationSignal(), done) }
        val h = bounds.height()
        for (area in listOf(android.graphics.Rect(0, 0, bounds.width(), h), android.graphics.Rect(0, h, bounds.width(), 2 * h))) {
            var got: android.graphics.Rect? = null
            await { done ->
                callback.onScrollCaptureImageRequest(session, android.os.CancellationSignal(), area) {
                    got = it
                    done.run()
                }
            }
            SystemClock.sleep(300)
            val image = reader.acquireLatestImage()
            var lit = 0
            if (image != null) {
                val plane = image.planes[0]
                val buffer = plane.buffer
                var i = 0
                while (i + 3 < buffer.limit()) {
                    if (buffer.get(i + 3).toInt() != 0) lit++
                    i += 4 * 97 // (a sample of the pixels)
                }
                image.close()
            }
            val r = got
            val ok = r != null && !r.isEmpty && lit > 0
            out.append(if (ok) "" else "FAIL ").append("asked $area: captured $r, $lit sampled pixels drawn\n")
        }
        await { done -> callback.onScrollCaptureEnd(done) }
        out.append("ended\n")
        reader.close()
    }

    private fun dump(n: AccessibilityNodeInfo, depth: Int, out: StringBuilder) {
        out.append("  ".repeat(depth)).append(line(n)).append('\n')
        for (i in 0 until n.childCount) n.getChild(i)?.let { dump(it, depth + 1, out) }
    }

    /** A node on one line. */
    private fun line(n: AccessibilityNodeInfo): String {
        val parts = mutableListOf<String>(n.className?.toString()?.substringAfterLast('.') ?: "?")
        n.text?.let { parts += "text=\"$it\"" }
        n.contentDescription?.let { parts += "desc=\"$it\"" }
        n.hintText?.let { parts += "hint=\"$it\"" }
        if (Build.VERSION.SDK_INT >= 30) n.stateDescription?.let { parts += "state=\"$it\"" }
        n.extras?.getCharSequence("AccessibilityNodeInfo.roleDescription")?.let { parts += "role=\"$it\"" }
        val flags = listOfNotNull(
            "checkable".takeIf { n.isCheckable },
            @Suppress("DEPRECATION") "checked".takeIf { n.isChecked },
            "selected".takeIf { n.isSelected },
            "clickable".takeIf { n.isClickable },
            "focusable".takeIf { n.isFocusable },
            "focused".takeIf { n.isFocused },
            "editable".takeIf { n.isEditable },
            "scrollable".takeIf { n.isScrollable },
            "heading".takeIf { n.isHeading },
            "disabled".takeIf { !n.isEnabled },
            "password".takeIf { n.isPassword },
            "invisible".takeIf { !n.isVisibleToUser },
        )
        if (flags.isNotEmpty()) parts += flags.joinToString(",")
        n.rangeInfo?.let { parts += "range=${it.min}..${it.max}@${it.current}" }
        n.collectionInfo?.let { parts += "collection=${it.rowCount}x${it.columnCount}" }
        n.collectionItemInfo?.let { parts += "item=${it.rowIndex},${it.columnIndex}" }
        if (n.paneTitle != null) parts += "pane=\"${n.paneTitle}\""
        if (n.liveRegion != 0) parts += "live=${n.liveRegion}"
        if (n.textSelectionStart >= 0) parts += "sel=${n.textSelectionStart},${n.textSelectionEnd}"
        val actions = n.actionList.mapNotNull { a -> a.label?.toString() ?: actionName(a.id) }
        if (actions.isNotEmpty()) parts += "actions=" + actions.joinToString(",")
        val r = android.graphics.Rect()
        n.getBoundsInScreen(r)
        parts += "[${r.left},${r.top}][${r.right},${r.bottom}]"
        return parts.joinToString(" ")
    }

    private fun actionName(id: Int): String? = when (id) {
        AccessibilityNodeInfo.ACTION_CLICK -> "click"
        AccessibilityNodeInfo.ACTION_LONG_CLICK -> "longclick"
        AccessibilityNodeInfo.ACTION_FOCUS -> "focus"
        AccessibilityNodeInfo.ACTION_CLEAR_FOCUS -> "clearfocus"
        AccessibilityNodeInfo.ACTION_SCROLL_FORWARD -> "forward"
        AccessibilityNodeInfo.ACTION_SCROLL_BACKWARD -> "backward"
        AccessibilityNodeInfo.ACTION_SET_TEXT -> "settext"
        AccessibilityNodeInfo.ACTION_SET_SELECTION -> "select"
        AccessibilityNodeInfo.ACTION_EXPAND -> "expand"
        AccessibilityNodeInfo.ACTION_COLLAPSE -> "collapse"
        AccessibilityNodeInfo.ACTION_DISMISS -> "dismiss"
        AccessibilityNodeInfo.ACTION_COPY -> "copy"
        AccessibilityNodeInfo.ACTION_PASTE -> "paste"
        AccessibilityNodeInfo.ACTION_CUT -> "cut"
        android.R.id.accessibilityActionScrollUp -> "up"
        android.R.id.accessibilityActionScrollDown -> "down"
        android.R.id.accessibilityActionScrollLeft -> "left"
        android.R.id.accessibilityActionScrollRight -> "right"
        android.R.id.accessibilityActionSetProgress -> "progress"
        android.R.id.accessibilityActionScrollToPosition -> "toposition"
        android.R.id.accessibilityActionShowOnScreen -> "show"
        android.R.id.accessibilityActionImeEnter -> "ime"
        else -> null // (accessibility focus, granularity moves: every node's)
    }

    /** An event as the report counts it: its type, its change types. */
    private fun describe(e: AccessibilityEvent): String {
        val type = AccessibilityEvent.eventTypeToString(e.eventType)
        return if (e.eventType == AccessibilityEvent.TYPE_WINDOW_CONTENT_CHANGED ||
            e.eventType == AccessibilityEvent.TYPE_WINDOW_STATE_CHANGED
        ) {
            "$type changes=${e.contentChangeTypes}"
        } else {
            type
        }
    }
}
