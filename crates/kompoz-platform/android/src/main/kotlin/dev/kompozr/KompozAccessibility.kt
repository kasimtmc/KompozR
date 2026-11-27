package dev.kompozr

import android.content.ClipboardManager
import android.graphics.Rect
import android.graphics.RectF
import android.os.Build
import android.os.Bundle
import android.os.SystemClock
import android.view.MotionEvent
import android.view.View
import android.view.accessibility.AccessibilityEvent
import android.view.accessibility.AccessibilityManager
import android.view.accessibility.AccessibilityNodeInfo
import android.view.accessibility.AccessibilityNodeInfo.AccessibilityAction
import android.view.accessibility.AccessibilityNodeProvider
import java.nio.ByteBuffer
import java.nio.ByteOrder
import java.text.BreakIterator

/**
 * Accessibility for a KompozR app (Compose's AndroidComposeViewAccessibilityDelegateCompat, C++ Kompoz's
 * SemanticsProvider): the app's semantics tree as KompozView's virtual views, through an [AccessibilityNodeProvider].
 *
 * The tree comes from the app as one buffer ([Tree]): each node's class (by its role), texts, states, actions,
 * range, collection and place in one, scroll, selection and lines, in reading order. It is asked for when a node's
 * info is wanted and the copy here is stale (the app said its semantics or layout changed), built there and then on
 * this, the UI thread (the app holds its host here: nothing waits for a frame). What changed is asked for at most
 * every 100 ms while accessibility is on (Compose's SendRecurringAccessibilityEventsIntervalMillis) and sent as
 * accessibility events: subtrees, texts, states, panes, errors, selections, scrolls, the focus.
 *
 * Accessibility focus, hover exploration (a screen reader's finger: the virtual view under it, as the app finds
 * it) and moves through a text by characters, words, lines, paragraphs and pages (Compose's AccessibilityIterators)
 * are kept here; the rest of what is done to a node goes to the app. A node that keeps no selection of its own has a
 * cursor kept here (Compose's accessibilityCursorPosition). The texts said for states (on, off, selected, a percent,
 * …) are this library's resources, in the system's language.
 */
internal class KompozAccessibility(private val view: KompozView) : View.AccessibilityDelegate() {
    private val manager = view.context.getSystemService(AccessibilityManager::class.java)
    private val provider = Provider()

    /** The tree as last asked for, and whether the app has changed since. */
    private var tree: Tree? = null
    private var stale = true

    private var accessibilityFocused = INVALID_ID
    private var hovered = INVALID_ID

    /** A check for changes posted, and when the last one ran (uptime, ms). */
    private var checkPosted = false
    private var lastCheck = 0L
    private val checker = Runnable { check() }

    /** The cursor kept for a node that keeps none (UTF-16 offset; -1: none), and its node. */
    private var cursor = -1
    private var cursorNode = INVALID_ID

    /** A move by a granularity, told once its selection is (Compose's PendingTextTraversedEvent). */
    private class Traversal(
        val node: Int,
        val action: Int,
        val granularity: Int,
        val from: Int,
        val to: Int,
        val time: Long,
    )
    private var traversal: Traversal? = null

    /** Each node's custom actions' ids by their labels (kept for the same action across changes, Compose's). */
    private val customIds = HashMap<Int, HashMap<String, Int>>()

    /** Accessibility is on (a service listens). */
    val enabled: Boolean get() = manager.isEnabled

    private val touchExploration: Boolean get() = manager.isEnabled && manager.isTouchExplorationEnabled

    override fun getAccessibilityNodeProvider(host: View): AccessibilityNodeProvider = provider

    /** The app's semantics or layout changed: the copy is stale; the changes told in a while. */
    fun changed() {
        stale = true
        if (enabled) schedule()
    }

    /** The view is gone from its window: nothing posted left behind. */
    fun detached() {
        view.removeCallbacks(checker)
        checkPosted = false
        tree = null
        stale = true
    }

    private fun schedule() {
        if (checkPosted) return
        checkPosted = true
        val wait = (lastCheck + SEND_INTERVAL_MS - SystemClock.uptimeMillis()).coerceAtLeast(0)
        view.postDelayed(checker, wait)
    }

    /** The tree and what changed, from the app; the events sent (none while it is busy: tried again later). */
    private fun check() {
        checkPosted = false
        lastCheck = SystemClock.uptimeMillis()
        if (!enabled) return
        val bytes = view.accessibilityTree(true)
        if (bytes == null) {
            stale = true
            schedule()
            return
        }
        val reader = Reader(bytes)
        tree = Tree.read(reader)
        stale = false
        sendEvents(reader)
    }

    /** The tree now (asked for again if stale; the copy while the app is busy). */
    private fun current(): Tree? {
        if (stale || tree == null) {
            view.accessibilityTree(false)?.let {
                tree = Tree.read(Reader(it))
                stale = false
            }
        }
        return tree
    }

    // ---- Node infos ----

    private inner class Provider : AccessibilityNodeProvider() {
        override fun createAccessibilityNodeInfo(virtualViewId: Int): AccessibilityNodeInfo? = createNodeInfo(virtualViewId)

        override fun performAction(virtualViewId: Int, action: Int, arguments: Bundle?): Boolean =
            perform(virtualViewId, action, arguments)

        override fun addExtraDataToAccessibilityNodeInfo(
            virtualViewId: Int,
            info: AccessibilityNodeInfo,
            extraDataKey: String,
            arguments: Bundle?,
        ) = addExtraData(virtualViewId, info, extraDataKey, arguments)

        /** The nodes at or under `virtualViewId` whose text, description or hint holds `text` (any case). */
        override fun findAccessibilityNodeInfosByText(text: String, virtualViewId: Int): List<AccessibilityNodeInfo>? {
            val t = current() ?: return null
            val wanted = text.lowercase()
            val out = mutableListOf<AccessibilityNodeInfo>()
            fun visit(id: Int) {
                val n = t.nodes[id] ?: return
                val holds = listOf(n.text, n.contentDescription, n.hint).any { it?.lowercase()?.contains(wanted) == true }
                if (holds) createNodeInfo(id)?.let { out += it }
                n.children.forEach(::visit)
            }
            visit(virtualViewId)
            return out
        }

        override fun findFocus(focus: Int): AccessibilityNodeInfo? = when (focus) {
            AccessibilityNodeInfo.FOCUS_ACCESSIBILITY ->
                if (accessibilityFocused == INVALID_ID) null else createNodeInfo(accessibilityFocused)
            // (The field the input method types into: its node, the input connection's.)
            AccessibilityNodeInfo.FOCUS_INPUT -> current()?.inputFocus()?.let { createNodeInfo(it) }
            else -> null
        }
    }

    /** Whether the asking service is an accessibility tool (it may see sensitive nodes). */
    private fun fromTool(): Boolean = Build.VERSION.SDK_INT >= 34 && manager.isRequestFromAccessibilityTool

    @Suppress("DEPRECATION")
    private fun createNodeInfo(id: Int): AccessibilityNodeInfo? {
        val t = current() ?: return null
        val n = t.nodes[id] ?: return null
        val sensitive = n.flags and FLAG_SENSITIVE != 0
        if (sensitive && !fromTool()) return null
        val info = AccessibilityNodeInfo.obtain(view, id)
        if (Build.VERSION.SDK_INT >= 34) info.setAccessibilityDataSensitive(sensitive)
        if (id == HOST_VIEW_ID) {
            (view.parentForAccessibility as? View)?.let { info.setParent(it) }
        } else {
            info.setParent(view, n.parent)
        }
        info.packageName = view.context.packageName
        info.className = n.className
        n.roleDescription?.let { info.extras.putCharSequence(ROLE_DESCRIPTION_KEY, view.context.getString(it)) }
        // Bounds: in the view, to the screen; in the parent's (as the deprecated field still is read).
        val location = IntArray(2)
        view.getLocationOnScreen(location)
        info.setBoundsInScreen(n.bounds.toScreen(location[0], location[1]))
        val parentBounds = t.nodes[n.parent]?.bounds
        val inParent = Rect(n.bounds.rounded())
        if (parentBounds != null && id != HOST_VIEW_ID) {
            inParent.offset(-parentBounds.left.toInt(), -parentBounds.top.toInt())
        }
        info.setBoundsInParent(inParent)
        info.isImportantForAccessibility = n.flags and FLAG_IMPORTANT != 0
        for (child in n.children) {
            val c = t.nodes[child] ?: continue
            if (c.flags and FLAG_SENSITIVE != 0 && !fromTool()) continue
            info.addChild(view, child)
        }
        if (id != HOST_VIEW_ID) info.drawingOrder = n.drawingOrder
        // Accessibility focus: ours.
        if (id == accessibilityFocused) {
            info.isAccessibilityFocused = true
            info.addAction(AccessibilityAction.ACTION_CLEAR_ACCESSIBILITY_FOCUS)
        } else {
            info.isAccessibilityFocused = false
            info.addAction(AccessibilityAction.ACTION_ACCESSIBILITY_FOCUS)
        }
        info.text = n.text
        info.contentDescription = n.contentDescription
        info.hintText = n.hint
        info.isShowingHintText = n.flags and FLAG_SHOWING_HINT != 0
        n.error?.let {
            info.isContentInvalid = true
            info.error = it
        }
        stateDescription(n)?.let {
            if (Build.VERSION.SDK_INT >= 30) info.stateDescription = it
            else info.extras.putCharSequence(STATE_DESCRIPTION_KEY, it)
        }
        info.isCheckable = n.flags and FLAG_CHECKABLE != 0
        info.isChecked = n.flags and FLAG_CHECKED != 0
        info.isSelected = n.flags and FLAG_SELECTED != 0
        info.isHeading = n.flags and FLAG_HEADING != 0
        info.isTextEntryKey = n.flags and FLAG_TEXT_ENTRY_KEY != 0
        info.isPassword = n.flags and FLAG_PASSWORD != 0
        info.isEditable = n.flags and FLAG_EDITABLE != 0
        info.isMultiLine = n.flags and FLAG_MULTI_LINE != 0
        info.maxTextLength = n.maxTextLength
        info.isEnabled = n.flags and FLAG_ENABLED != 0
        info.isFocusable = n.flags and FLAG_FOCUSABLE != 0
        info.isFocused = n.flags and FLAG_FOCUSED != 0
        info.isVisibleToUser = n.flags and FLAG_VISIBLE != 0
        info.isClickable = n.flags and FLAG_CLICKABLE != 0
        info.isLongClickable = n.flags and FLAG_LONG_CLICKABLE != 0
        info.isDismissable = n.flags and FLAG_DISMISSABLE != 0
        info.isScrollable = n.flags and FLAG_SCROLLABLE != 0
        info.isScreenReaderFocusable = n.flags and FLAG_SCREEN_READER_FOCUSABLE != 0
        info.liveRegion = n.liveRegion
        info.paneTitle = n.paneTitle
        addActions(info, n)
        if (n.iterable != null) {
            if (n.contentDescription == null && n.selectionStart >= 0) {
                info.setTextSelection(n.selectionStart, n.selectionEnd)
            } else if (id == cursorNode && cursor >= 0) {
                info.setTextSelection(cursor, cursor)
            }
            info.movementGranularities = n.granularities
        }
        val extra = mutableListOf(EXTRA_DATA_ID_KEY)
        if (n.flags and FLAG_CHARACTER_LOCATIONS != 0) {
            extra += AccessibilityNodeInfo.EXTRA_DATA_TEXT_CHARACTER_LOCATION_KEY
            if (Build.VERSION.SDK_INT >= 34) {
                extra += AccessibilityNodeInfo.EXTRA_DATA_TEXT_CHARACTER_LOCATION_IN_WINDOW_KEY
            }
        }
        if (n.testTag != null) extra += EXTRA_DATA_TEST_TAG_KEY
        info.availableExtraData = extra
        n.range?.let { info.rangeInfo = AccessibilityNodeInfo.RangeInfo.obtain(1, it[0], it[1], it[2]) }
        n.collection?.let {
            info.collectionInfo = AccessibilityNodeInfo.CollectionInfo.obtain(
                it[0], it[1], false, AccessibilityNodeInfo.CollectionInfo.SELECTION_MODE_NONE,
            )
        }
        n.item?.let {
            info.collectionItemInfo =
                AccessibilityNodeInfo.CollectionItemInfo.obtain(it[0], it[1], it[2], it[3], false, n.itemSelected)
        }
        // The node read after this one (reading order: Compose's traversal order).
        t.next(id)?.let { info.setTraversalBefore(view, it) }
        return info
    }

    /** The state said: the node's own, else one of ours by its kind. */
    private fun stateDescription(n: Node): String? {
        n.stateDescription?.let { return it }
        val r = view.context.resources
        return when (n.state) {
            STATE_ON -> r.getString(R.string.kompoz_state_on)
            STATE_OFF -> r.getString(R.string.kompoz_state_off)
            STATE_INDETERMINATE -> r.getString(R.string.kompoz_indeterminate)
            STATE_SELECTED -> r.getString(R.string.kompoz_selected)
            STATE_NOT_SELECTED -> r.getString(R.string.kompoz_not_selected)
            STATE_PERCENT -> r.getString(R.string.kompoz_template_percent, n.statePercent)
            STATE_IN_PROGRESS -> r.getString(R.string.kompoz_in_progress)
            STATE_EMPTY -> r.getString(R.string.kompoz_state_empty)
            else -> null
        }
    }

    private fun addActions(info: AccessibilityNodeInfo, n: Node) {
        val a = n.actions
        fun add(bit: Int, action: AccessibilityAction) {
            if (a and bit != 0) info.addAction(action)
        }
        add(ACTION_CLICK, AccessibilityAction.ACTION_CLICK)
        add(ACTION_LONG_CLICK, AccessibilityAction.ACTION_LONG_CLICK)
        add(ACTION_FOCUS, AccessibilityAction.ACTION_FOCUS)
        add(ACTION_CLEAR_FOCUS, AccessibilityAction.ACTION_CLEAR_FOCUS)
        add(ACTION_COPY, AccessibilityAction.ACTION_COPY)
        add(ACTION_SET_TEXT, AccessibilityAction.ACTION_SET_TEXT)
        if (Build.VERSION.SDK_INT >= 30) add(ACTION_IME_ENTER, AccessibilityAction.ACTION_IME_ENTER)
        add(ACTION_CUT, AccessibilityAction.ACTION_CUT)
        // (Paste only while the clipboard has text.)
        if (a and ACTION_PASTE != 0 && clipboardHasText()) info.addAction(AccessibilityAction.ACTION_PASTE)
        add(ACTION_SET_SELECTION, AccessibilityAction.ACTION_SET_SELECTION)
        add(ACTION_NEXT_AT_GRANULARITY, AccessibilityAction.ACTION_NEXT_AT_MOVEMENT_GRANULARITY)
        add(ACTION_PREVIOUS_AT_GRANULARITY, AccessibilityAction.ACTION_PREVIOUS_AT_MOVEMENT_GRANULARITY)
        add(ACTION_SCROLL_FORWARD, AccessibilityAction.ACTION_SCROLL_FORWARD)
        add(ACTION_SCROLL_BACKWARD, AccessibilityAction.ACTION_SCROLL_BACKWARD)
        add(ACTION_SCROLL_UP, AccessibilityAction.ACTION_SCROLL_UP)
        add(ACTION_SCROLL_DOWN, AccessibilityAction.ACTION_SCROLL_DOWN)
        add(ACTION_SCROLL_LEFT, AccessibilityAction.ACTION_SCROLL_LEFT)
        add(ACTION_SCROLL_RIGHT, AccessibilityAction.ACTION_SCROLL_RIGHT)
        add(ACTION_SCROLL_TO_POSITION, AccessibilityAction.ACTION_SCROLL_TO_POSITION)
        add(ACTION_SET_PROGRESS, AccessibilityAction.ACTION_SET_PROGRESS)
        add(ACTION_PAGE_UP, AccessibilityAction.ACTION_PAGE_UP)
        add(ACTION_PAGE_DOWN, AccessibilityAction.ACTION_PAGE_DOWN)
        add(ACTION_PAGE_LEFT, AccessibilityAction.ACTION_PAGE_LEFT)
        add(ACTION_PAGE_RIGHT, AccessibilityAction.ACTION_PAGE_RIGHT)
        add(ACTION_EXPAND, AccessibilityAction.ACTION_EXPAND)
        add(ACTION_COLLAPSE, AccessibilityAction.ACTION_COLLAPSE)
        add(ACTION_DISMISS, AccessibilityAction.ACTION_DISMISS)
        add(ACTION_SHOW_ON_SCREEN, AccessibilityAction.ACTION_SHOW_ON_SCREEN)
        // Custom actions: each label keeps the id it had (Compose's), new ones take the free ones.
        if (n.customActions.isNotEmpty()) {
            val old = customIds[n.id] ?: HashMap()
            val ids = HashMap<String, Int>()
            val free = CUSTOM_ACTION_IDS.filter { it !in old.values }.toMutableList()
            for (label in n.customActions) {
                if (label in ids) continue
                val actionId = old[label] ?: free.removeFirstOrNull() ?: break
                ids[label] = actionId
                info.addAction(AccessibilityAction(actionId, label))
            }
            customIds[n.id] = ids
        } else {
            customIds.remove(n.id)
        }
    }

    private fun clipboardHasText(): Boolean {
        val clipboard = view.context.getSystemService(ClipboardManager::class.java) ?: return false
        return clipboard.hasPrimaryClip() &&
            clipboard.primaryClipDescription?.hasMimeType("text/*") == true
    }

    private fun addExtraData(id: Int, info: AccessibilityNodeInfo, key: String, arguments: Bundle?) {
        val n = current()?.nodes?.get(id) ?: return
        when (key) {
            EXTRA_DATA_ID_KEY -> info.extras.putInt(key, id)
            EXTRA_DATA_TEST_TAG_KEY -> n.testTag?.let { info.extras.putCharSequence(key, it) }
            AccessibilityNodeInfo.EXTRA_DATA_TEXT_CHARACTER_LOCATION_KEY,
            AccessibilityNodeInfo.EXTRA_DATA_TEXT_CHARACTER_LOCATION_IN_WINDOW_KEY -> {
                val start = arguments?.getInt(AccessibilityNodeInfo.EXTRA_DATA_TEXT_CHARACTER_LOCATION_ARG_START_INDEX, -1) ?: -1
                val length = arguments?.getInt(AccessibilityNodeInfo.EXTRA_DATA_TEXT_CHARACTER_LOCATION_ARG_LENGTH, -1) ?: -1
                val textLength = n.iterable?.length ?: Int.MAX_VALUE
                if (length <= 0 || start < 0 || start >= textLength) return
                val boxes = view.characterBounds(id, start, minOf(length, MAX_CHARACTER_LOCATIONS)) ?: return
                // In the screen (or, for the in-window key, in the window).
                val at = IntArray(2)
                if (key == AccessibilityNodeInfo.EXTRA_DATA_TEXT_CHARACTER_LOCATION_KEY) view.getLocationOnScreen(at)
                else view.getLocationInWindow(at)
                val rects = arrayOfNulls<RectF>(boxes.size / 4)
                for (i in rects.indices) {
                    val l = boxes[i * 4]
                    if (l.isNaN()) continue
                    rects[i] = RectF(l + at[0], boxes[i * 4 + 1] + at[1], boxes[i * 4 + 2] + at[0], boxes[i * 4 + 3] + at[1])
                }
                info.extras.putParcelableArray(key, rects)
            }
        }
    }

    // ---- Actions ----

    private fun perform(id: Int, action: Int, arguments: Bundle?): Boolean {
        val t = current() ?: return false
        val n = t.nodes[id] ?: return false
        if (n.flags and FLAG_SENSITIVE != 0 && !fromTool()) return false
        val done = when (action) {
            AccessibilityNodeInfo.ACTION_ACCESSIBILITY_FOCUS -> return requestAccessibilityFocus(id)
            AccessibilityNodeInfo.ACTION_CLEAR_ACCESSIBILITY_FOCUS -> return clearAccessibilityFocus(id)
            AccessibilityNodeInfo.ACTION_NEXT_AT_MOVEMENT_GRANULARITY,
            AccessibilityNodeInfo.ACTION_PREVIOUS_AT_MOVEMENT_GRANULARITY -> {
                if (arguments == null) return false
                val granularity = arguments.getInt(AccessibilityNodeInfo.ACTION_ARGUMENT_MOVEMENT_GRANULARITY_INT)
                val extend = arguments.getBoolean(AccessibilityNodeInfo.ACTION_ARGUMENT_EXTEND_SELECTION_BOOLEAN)
                return traverse(n, granularity, action == AccessibilityNodeInfo.ACTION_NEXT_AT_MOVEMENT_GRANULARITY, extend)
            }
            AccessibilityNodeInfo.ACTION_SET_SELECTION -> {
                val start = arguments?.getInt(AccessibilityNodeInfo.ACTION_ARGUMENT_SELECTION_START_INT, -1) ?: -1
                val end = arguments?.getInt(AccessibilityNodeInfo.ACTION_ARGUMENT_SELECTION_END_INT, -1) ?: -1
                val set = setSelection(n, start, end, false)
                if (set) sendEvent(createEvent(id, AccessibilityEvent.TYPE_WINDOW_CONTENT_CHANGED))
                set
            }
            AccessibilityNodeInfo.ACTION_CLICK -> {
                val clicked = request(id, REQUEST_CLICK)
                sendEvent(createEvent(id, AccessibilityEvent.TYPE_VIEW_CLICKED))
                clicked
            }
            AccessibilityNodeInfo.ACTION_LONG_CLICK -> request(id, REQUEST_LONG_CLICK)
            AccessibilityNodeInfo.ACTION_FOCUS -> {
                // (Out of touch mode first: the node might not be focusable in it, Compose's b/387576999.)
                if (view.isInTouchMode) view.requestFocusFromTouch()
                request(id, REQUEST_FOCUS)
            }
            AccessibilityNodeInfo.ACTION_CLEAR_FOCUS -> request(id, REQUEST_CLEAR_FOCUS)
            AccessibilityNodeInfo.ACTION_SCROLL_FORWARD -> request(id, REQUEST_SCROLL_FORWARD)
            AccessibilityNodeInfo.ACTION_SCROLL_BACKWARD -> request(id, REQUEST_SCROLL_BACKWARD)
            android.R.id.accessibilityActionScrollUp -> request(id, REQUEST_SCROLL_UP)
            android.R.id.accessibilityActionScrollDown -> request(id, REQUEST_SCROLL_DOWN)
            android.R.id.accessibilityActionScrollLeft -> request(id, REQUEST_SCROLL_LEFT)
            android.R.id.accessibilityActionScrollRight -> request(id, REQUEST_SCROLL_RIGHT)
            android.R.id.accessibilityActionScrollToPosition -> {
                val row = arguments?.getInt(AccessibilityNodeInfo.ACTION_ARGUMENT_ROW_INT, -1) ?: -1
                val column = arguments?.getInt(AccessibilityNodeInfo.ACTION_ARGUMENT_COLUMN_INT, -1) ?: -1
                if (row < 0 && column < 0) return false
                request(id, REQUEST_SCROLL_TO_POSITION, row.coerceAtLeast(0), column.coerceAtLeast(0))
            }
            android.R.id.accessibilityActionSetProgress -> {
                if (arguments?.containsKey(AccessibilityNodeInfo.ACTION_ARGUMENT_PROGRESS_VALUE) != true) return false
                request(id, REQUEST_SET_PROGRESS, value = arguments.getFloat(AccessibilityNodeInfo.ACTION_ARGUMENT_PROGRESS_VALUE))
            }
            AccessibilityNodeInfo.ACTION_SET_TEXT -> {
                val text = arguments?.getCharSequence(AccessibilityNodeInfo.ACTION_ARGUMENT_SET_TEXT_CHARSEQUENCE)
                request(id, REQUEST_SET_TEXT, text = (text ?: "").toString())
            }
            AccessibilityNodeInfo.ACTION_EXPAND -> request(id, REQUEST_EXPAND)
            AccessibilityNodeInfo.ACTION_COLLAPSE -> request(id, REQUEST_COLLAPSE)
            AccessibilityNodeInfo.ACTION_DISMISS -> request(id, REQUEST_DISMISS)
            AccessibilityNodeInfo.ACTION_COPY -> request(id, REQUEST_COPY)
            AccessibilityNodeInfo.ACTION_CUT -> request(id, REQUEST_CUT)
            AccessibilityNodeInfo.ACTION_PASTE -> request(id, REQUEST_PASTE)
            android.R.id.accessibilityActionImeEnter -> request(id, REQUEST_IME_ENTER)
            android.R.id.accessibilityActionPageUp -> request(id, REQUEST_PAGE_UP)
            android.R.id.accessibilityActionPageDown -> request(id, REQUEST_PAGE_DOWN)
            android.R.id.accessibilityActionPageLeft -> request(id, REQUEST_PAGE_LEFT)
            android.R.id.accessibilityActionPageRight -> request(id, REQUEST_PAGE_RIGHT)
            android.R.id.accessibilityActionShowOnScreen ->
                // (No scrollable above it in the app: the view's own parents asked.)
                request(id, REQUEST_SHOW_ON_SCREEN) || view.requestRectangleOnScreen(n.bounds.rounded())
            else -> {
                val label = customIds[id]?.entries?.firstOrNull { it.value == action }?.key ?: return false
                val index = n.customActions.indexOf(label)
                if (index < 0) return false
                request(id, REQUEST_CUSTOM, index)
            }
        }
        if (done) stale = true
        return done
    }

    private fun request(id: Int, code: Int, a: Int = 0, b: Int = 0, value: Float = 0f, text: String? = null): Boolean =
        view.accessibilityAction(id, code, a, b, value, text?.toByteArray())

    private fun requestAccessibilityFocus(id: Int): Boolean {
        if (!touchExploration || id == accessibilityFocused) return false
        if (accessibilityFocused != INVALID_ID) {
            sendEvent(createEvent(accessibilityFocused, AccessibilityEvent.TYPE_VIEW_ACCESSIBILITY_FOCUS_CLEARED))
        }
        accessibilityFocused = id
        view.invalidate()
        sendEvent(createEvent(id, AccessibilityEvent.TYPE_VIEW_ACCESSIBILITY_FOCUSED))
        return true
    }

    private fun clearAccessibilityFocus(id: Int): Boolean {
        if (id != accessibilityFocused) return false
        accessibilityFocused = INVALID_ID
        view.invalidate()
        sendEvent(createEvent(id, AccessibilityEvent.TYPE_VIEW_ACCESSIBILITY_FOCUS_CLEARED))
        return true
    }

    // ---- Moving through a text (Compose's traverseAtGranularity) ----

    private fun traverse(n: Node, granularity: Int, forward: Boolean, extend: Boolean): Boolean {
        if (n.id != cursorNode) {
            cursor = -1
            cursorNode = n.id
        }
        val text = n.iterable ?: return false
        if (text.isEmpty()) return false
        val iterator = segments(n, granularity) ?: return false
        var current = selectionEnd(n)
        if (current < 0) current = if (forward) 0 else text.length
        val range = (if (forward) iterator.following(current) else iterator.preceding(current)) ?: return false
        val (segmentStart, segmentEnd) = range
        var start: Int
        val end: Int
        if (extend && n.flags and FLAG_EDITABLE != 0 && n.contentDescription == null) {
            start = selectionStart(n)
            if (start < 0) start = if (forward) segmentStart else segmentEnd
            end = if (forward) segmentEnd else segmentStart
        } else {
            start = if (forward) segmentEnd else segmentStart
            end = start
        }
        val action = if (forward) AccessibilityNodeInfo.ACTION_NEXT_AT_MOVEMENT_GRANULARITY
        else AccessibilityNodeInfo.ACTION_PREVIOUS_AT_MOVEMENT_GRANULARITY
        traversal = Traversal(n.id, action, granularity, segmentStart, segmentEnd, SystemClock.uptimeMillis())
        setSelection(n, start, end, true)
        return true
    }

    private fun selectionStart(n: Node): Int =
        if (n.contentDescription == null && n.selectionStart >= 0) n.selectionStart else cursor

    private fun selectionEnd(n: Node): Int =
        if (n.contentDescription == null && n.selectionEnd >= 0) n.selectionEnd else cursor

    /**
     * A selection set (Compose's setAccessibilitySelection): the node's own where it keeps one (its selection's
     * change, told with the tree's next changes, tells the move), else our cursor and the events at once.
     */
    private fun setSelection(n: Node, start: Int, end: Int, traversalMode: Boolean): Boolean {
        if (view.accessibilityAction(n.id, REQUEST_SET_SELECTION, start, end, if (traversalMode) 1f else 0f, null)) {
            stale = true
            return true
        }
        if (start == end && end == cursor && n.id == cursorNode) return false
        val text = n.iterable ?: return false
        cursorNode = n.id
        cursor = if (start >= 0 && start == end && end <= text.length) start else -1
        val event = createEvent(n.id, AccessibilityEvent.TYPE_VIEW_TEXT_SELECTION_CHANGED)
        if (text.isNotEmpty()) {
            event.fromIndex = cursor
            event.toIndex = cursor
            event.itemCount = text.length
        }
        event.text.add(text)
        sendEvent(event)
        sendTraversal(n.id)
        return true
    }

    /** A move by a granularity told (within a second of it: Compose's TextTraversedEventTimeoutMillis). */
    private fun sendTraversal(id: Int) {
        val t = traversal ?: return
        if (t.node != id) return
        traversal = null
        if (SystemClock.uptimeMillis() - t.time > 1000) return
        val event = createEvent(id, AccessibilityEvent.TYPE_VIEW_TEXT_TRAVERSED_AT_MOVEMENT_GRANULARITY)
        event.fromIndex = t.from
        event.toIndex = t.to
        event.action = t.action
        event.movementGranularity = t.granularity
        current()?.nodes?.get(id)?.iterable?.let { event.text.add(it) }
        sendEvent(event)
    }

    /** The segments of a node's text at a granularity (Compose's AccessibilityIterators). */
    private fun segments(n: Node, granularity: Int): Segments? {
        val text = n.iterable ?: return null
        val locale = view.context.resources.configuration.locales[0]
        return when (granularity) {
            AccessibilityNodeInfo.MOVEMENT_GRANULARITY_CHARACTER ->
                CharacterSegments(text, BreakIterator.getCharacterInstance(locale))
            AccessibilityNodeInfo.MOVEMENT_GRANULARITY_WORD -> WordSegments(text, BreakIterator.getWordInstance(locale))
            AccessibilityNodeInfo.MOVEMENT_GRANULARITY_PARAGRAPH -> ParagraphSegments(text)
            AccessibilityNodeInfo.MOVEMENT_GRANULARITY_LINE ->
                if (n.lineStarts.isEmpty()) null else LineSegments(text, n.lineStarts)
            AccessibilityNodeInfo.MOVEMENT_GRANULARITY_PAGE ->
                if (n.lineStarts.isEmpty()) null else PageSegments(text, n.lineStarts, n.lineTops, n.bounds.height())
            else -> null
        }
    }

    // ---- Hover exploration ----

    /** A screen reader's finger over the view (Compose's dispatchHoverEvent): the virtual view under it. */
    fun hover(event: MotionEvent): Boolean {
        if (!touchExploration) return false
        return when (event.action) {
            MotionEvent.ACTION_HOVER_MOVE, MotionEvent.ACTION_HOVER_ENTER -> {
                current() // (the app's tree up to date for the finger)
                val id = view.accessibilityHit(event.x, event.y)
                updateHovered(id)
                id != INVALID_ID
            }
            MotionEvent.ACTION_HOVER_EXIT -> {
                if (hovered == INVALID_ID) return false
                updateHovered(INVALID_ID)
                true
            }
            else -> false
        }
    }

    private fun updateHovered(id: Int) {
        if (hovered == id) return
        val previous = hovered
        hovered = id
        // (Enter before the exit, as the framework sends them.)
        if (id != INVALID_ID) sendEvent(createEvent(id, AccessibilityEvent.TYPE_VIEW_HOVER_ENTER))
        if (previous != INVALID_ID) sendEvent(createEvent(previous, AccessibilityEvent.TYPE_VIEW_HOVER_EXIT))
    }

    // ---- Events ----

    @Suppress("DEPRECATION")
    private fun createEvent(id: Int, type: Int): AccessibilityEvent {
        val event = AccessibilityEvent.obtain(type)
        event.isEnabled = true
        event.className = "android.view.View"
        event.packageName = view.context.packageName
        event.setSource(view, id)
        tree?.nodes?.get(id)?.let {
            event.isPassword = it.flags and FLAG_PASSWORD != 0
            if (Build.VERSION.SDK_INT >= 34) event.setAccessibilityDataSensitive(it.flags and FLAG_SENSITIVE != 0)
        }
        return event
    }

    private fun sendEvent(event: AccessibilityEvent) {
        if (!enabled) return
        view.parent?.requestSendAccessibilityEvent(view, event)
    }

    /** The content change bits this system knows of (the others: none, UNDEFINED). */
    private fun known(changes: Int): Int {
        var allowed = AccessibilityEvent.CONTENT_CHANGE_TYPE_SUBTREE or AccessibilityEvent.CONTENT_CHANGE_TYPE_TEXT or
            AccessibilityEvent.CONTENT_CHANGE_TYPE_CONTENT_DESCRIPTION or
            AccessibilityEvent.CONTENT_CHANGE_TYPE_PANE_TITLE or AccessibilityEvent.CONTENT_CHANGE_TYPE_PANE_APPEARED or
            AccessibilityEvent.CONTENT_CHANGE_TYPE_PANE_DISAPPEARED
        if (Build.VERSION.SDK_INT >= 30) allowed = allowed or AccessibilityEvent.CONTENT_CHANGE_TYPE_STATE_DESCRIPTION
        if (Build.VERSION.SDK_INT >= 35) allowed = allowed or CHANGE_ERROR or CHANGE_CONTENT_INVALID or CHANGE_ENABLED
        if (Build.VERSION.SDK_INT >= 36) allowed = allowed or CHANGE_CHECKED
        return changes and allowed
    }

    /** The events the app gave after its tree (KompozR's `encode_events`). */
    private fun sendEvents(r: Reader) {
        val count = r.int()
        repeat(count) {
            val kind = r.int()
            val id = r.int()
            val changes = r.int()
            val a = r.int()
            val b = r.int()
            val c = r.int()
            val d = r.int()
            val text = r.str()
            val before = r.str()
            val description = r.str()
            when (kind) {
                EVENT_CONTENT_CHANGED -> {
                    val e = createEvent(id, AccessibilityEvent.TYPE_WINDOW_CONTENT_CHANGED)
                    e.contentChangeTypes = known(changes)
                    sendEvent(e)
                }
                EVENT_TEXT_CHANGED -> {
                    val e = createEvent(id, AccessibilityEvent.TYPE_VIEW_TEXT_CHANGED)
                    e.className = "android.widget.EditText"
                    e.fromIndex = a
                    e.removedCount = b
                    e.addedCount = c
                    e.isPassword = d != 0
                    e.beforeText = before
                    e.text.add(text)
                    sendEvent(e)
                }
                EVENT_SELECTION_CHANGED -> {
                    val e = createEvent(id, AccessibilityEvent.TYPE_VIEW_TEXT_SELECTION_CHANGED)
                    e.fromIndex = a
                    e.toIndex = b
                    e.itemCount = c
                    e.text.add(text)
                    sendEvent(e)
                    sendTraversal(id)
                }
                EVENT_FOCUSED -> sendEvent(createEvent(id, AccessibilityEvent.TYPE_VIEW_FOCUSED))
                EVENT_SELECTED -> {
                    val e = createEvent(id, AccessibilityEvent.TYPE_VIEW_SELECTED)
                    description?.let { e.contentDescription = it }
                    text?.let { e.text.add(it) }
                    sendEvent(e)
                }
                EVENT_SCROLLED -> {
                    val e = createEvent(id, AccessibilityEvent.TYPE_VIEW_SCROLLED)
                    if (a >= 0) {
                        e.scrollX = a
                        e.maxScrollX = b
                    }
                    if (c >= 0) {
                        e.scrollY = c
                        e.maxScrollY = d
                    }
                    sendEvent(e)
                }
                EVENT_PANE_CHANGED -> {
                    val e = createEvent(id, AccessibilityEvent.TYPE_WINDOW_STATE_CHANGED)
                    e.contentChangeTypes = known(changes)
                    text?.let { e.text.add(it) }
                    sendEvent(e)
                }
            }
        }
    }

    // ---- The tree ----

    /** A node as the app gave it (KompozR's `encode_tree`, in its order). */
    class Node(r: Reader) {
        val id = r.int()
        val parent = r.int()
        val flags = r.int()
        val actions = r.int()
        val granularities = r.int()
        val liveRegion = r.int()
        val roleDescription: Int? = when (r.int()) {
            ROLE_TAB -> R.string.kompoz_tab
            else -> null
        }
        val state = r.int()
        val statePercent = r.int()
        val selectionStart = r.int()
        val selectionEnd = r.int()
        val maxTextLength = r.int()
        val range: FloatArray? = if (r.int() != 0) floatArrayOf(r.float(), r.float(), r.float()) else null
        val collection: IntArray? = run {
            val has = r.bool()
            val v = intArrayOf(r.int(), r.int())
            if (has) v else null
        }
        val item: IntArray?
        val itemSelected: Boolean
        init {
            val has = r.bool()
            val v = intArrayOf(r.int(), r.int(), r.int(), r.int())
            itemSelected = r.bool()
            item = if (has) v else null
        }
        val scrollX: IntArray? = run {
            val has = r.bool()
            val v = intArrayOf(r.int(), r.int())
            if (has) v else null
        }
        val scrollY: IntArray? = run {
            val has = r.bool()
            val v = intArrayOf(r.int(), r.int())
            if (has) v else null
        }
        val bounds = RectF(r.float(), r.float(), r.float(), r.float())
        val className: String = r.str() ?: "android.view.View"
        val text = r.str()
        val contentDescription = r.str()
        val hint = r.str()
        val stateDescription = r.str()
        val error = r.str()
        val paneTitle = r.str()
        val testTag = r.str()
        val iterable = r.str()
        val customActions: List<String> = List(r.int()) { r.str() ?: "" }
        val lineStarts: IntArray
        val lineTops: FloatArray
        init {
            val lines = r.int()
            lineStarts = IntArray(lines)
            lineTops = FloatArray(lines)
            for (i in 0 until lines) {
                lineStarts[i] = r.int()
                lineTops[i] = r.float()
            }
        }
        val children = IntArray(r.int()) { r.int() }

        /** Its place among its parent's children (set as the tree is read). */
        var drawingOrder = 0
    }

    /** The tree: its nodes by id, in reading order. */
    class Tree(val order: IntArray, val nodes: Map<Int, Node>) {
        private val nextOf = HashMap<Int, Int>().also {
            for (i in 0 until order.size - 1) it[order[i]] = order[i + 1]
        }

        /** The node read after `id`. */
        fun next(id: Int): Int? = nextOf[id]

        /** The node with the keyboard focus. */
        fun inputFocus(): Int? = order.lastOrNull { (nodes[it]?.flags ?: 0) and FLAG_FOCUSED != 0 }

        companion object {
            fun read(r: Reader): Tree {
                val count = r.int()
                val order = IntArray(count)
                val nodes = LinkedHashMap<Int, Node>(count)
                for (i in 0 until count) {
                    val n = Node(r)
                    order[i] = n.id
                    nodes[n.id] = n
                }
                // (Each child's place among its parent's: its drawing order, children being in it.)
                for (n in nodes.values) {
                    n.children.forEachIndexed { i, c -> nodes[c]?.drawingOrder = i }
                }
                return Tree(order, nodes)
            }
        }
    }

    // ---- Text segments (Compose's AccessibilityIterators) ----

    abstract class Segments(protected val text: String) {
        abstract fun following(current: Int): Pair<Int, Int>?
        abstract fun preceding(current: Int): Pair<Int, Int>?
        protected fun range(start: Int, end: Int): Pair<Int, Int>? =
            if (start < 0 || end < 0 || start == end) null else start to end
    }

    class CharacterSegments(text: String, private val impl: BreakIterator) : Segments(text) {
        init {
            impl.setText(text)
        }

        override fun following(current: Int): Pair<Int, Int>? {
            if (text.isEmpty() || current >= text.length) return null
            var start = current.coerceAtLeast(0)
            while (!impl.isBoundary(start)) {
                start = impl.following(start)
                if (start == BreakIterator.DONE) return null
            }
            val end = impl.following(start)
            if (end == BreakIterator.DONE) return null
            return range(start, end)
        }

        override fun preceding(current: Int): Pair<Int, Int>? {
            if (text.isEmpty() || current <= 0) return null
            var end = current.coerceAtMost(text.length)
            while (!impl.isBoundary(end)) {
                end = impl.preceding(end)
                if (end == BreakIterator.DONE) return null
            }
            val start = impl.preceding(end)
            if (start == BreakIterator.DONE) return null
            return range(start, end)
        }
    }

    class WordSegments(text: String, private val impl: BreakIterator) : Segments(text) {
        init {
            impl.setText(text)
        }

        override fun following(current: Int): Pair<Int, Int>? {
            if (text.isEmpty() || current >= text.length) return null
            var start = current.coerceAtLeast(0)
            while (!isLetterOrDigit(start) && !isStartBoundary(start)) {
                start = impl.following(start)
                if (start == BreakIterator.DONE) return null
            }
            val end = impl.following(start)
            if (end == BreakIterator.DONE || !isEndBoundary(end)) return null
            return range(start, end)
        }

        override fun preceding(current: Int): Pair<Int, Int>? {
            if (text.isEmpty() || current <= 0) return null
            var end = current.coerceAtMost(text.length)
            while (end > 0 && !isLetterOrDigit(end - 1) && !isEndBoundary(end)) {
                end = impl.preceding(end)
                if (end == BreakIterator.DONE) return null
            }
            val start = impl.preceding(end)
            if (start == BreakIterator.DONE || !isStartBoundary(start)) return null
            return range(start, end)
        }

        private fun isStartBoundary(i: Int) = isLetterOrDigit(i) && (i == 0 || !isLetterOrDigit(i - 1))
        private fun isEndBoundary(i: Int) = (i > 0 && isLetterOrDigit(i - 1)) && (i == text.length || !isLetterOrDigit(i))
        private fun isLetterOrDigit(i: Int) = i >= 0 && i < text.length && Character.isLetterOrDigit(text.codePointAt(i))
    }

    class ParagraphSegments(text: String) : Segments(text) {
        override fun following(current: Int): Pair<Int, Int>? {
            if (text.isEmpty() || current >= text.length) return null
            var start = current.coerceAtLeast(0)
            while (start < text.length && text[start] == '\n' && !isStartBoundary(start)) start++
            if (start >= text.length) return null
            var end = start + 1
            while (end < text.length && !isEndBoundary(end)) end++
            return range(start, end)
        }

        override fun preceding(current: Int): Pair<Int, Int>? {
            if (text.isEmpty() || current <= 0) return null
            var end = current.coerceAtMost(text.length)
            while (end > 0 && text[end - 1] == '\n' && !isEndBoundary(end)) end--
            if (end <= 0) return null
            var start = end - 1
            while (start > 0 && !isStartBoundary(start)) start--
            return range(start, end)
        }

        private fun isStartBoundary(i: Int) = text[i] != '\n' && (i == 0 || text[i - 1] == '\n')
        private fun isEndBoundary(i: Int) = i > 0 && text[i - 1] != '\n' && (i == text.length || text[i] == '\n')
    }

    /** Lines by where they start (each one to the next's start; the last to the end). */
    open class LineSegments(text: String, protected val starts: IntArray) : Segments(text) {
        protected fun lineOf(offset: Int): Int {
            var line = 0
            while (line + 1 < starts.size && starts[line + 1] <= offset) line++
            return line
        }

        protected fun lineEnd(line: Int): Int = if (line + 1 < starts.size) starts[line + 1] else text.length

        override fun following(current: Int): Pair<Int, Int>? {
            if (text.isEmpty() || current >= text.length) return null
            val next = if (current < 0) 0 else {
                val line = lineOf(current)
                if (starts[line] == current) line else line + 1
            }
            if (next >= starts.size) return null
            return range(starts[next], lineEnd(next))
        }

        override fun preceding(current: Int): Pair<Int, Int>? {
            if (text.isEmpty() || current <= 0) return null
            val previous = if (current > text.length) lineOf(text.length) else {
                val line = lineOf(current)
                if (lineEnd(line) == current) line else line - 1
            }
            if (previous < 0) return null
            return range(starts[previous], lineEnd(previous))
        }
    }

    /** Pages: as many lines as the node's height holds, from the line the cursor is on. */
    class PageSegments(text: String, starts: IntArray, private val tops: FloatArray, private val height: Float) :
        LineSegments(text, starts) {
        private fun lineAt(y: Float): Int {
            var line = 0
            while (line + 1 < tops.size && tops[line + 1] <= y) line++
            return line
        }

        override fun following(current: Int): Pair<Int, Int>? {
            if (text.isEmpty() || current >= text.length || height <= 0f) return null
            val start = current.coerceAtLeast(0)
            val nextPageY = tops[lineOf(start)] + height
            val endLine = if (nextPageY < tops.last()) lineAt(nextPageY) - 1 else starts.size - 1
            return range(start, lineEnd(endLine.coerceAtLeast(lineOf(start))))
        }

        override fun preceding(current: Int): Pair<Int, Int>? {
            if (text.isEmpty() || current <= 0 || height <= 0f) return null
            val end = current.coerceAtMost(text.length)
            val previousPageY = tops[lineOf(end)] - height
            val startLine = if (previousPageY > 0f) lineAt(previousPageY) + 1 else 0
            return range(starts[startLine.coerceAtMost(lineOf(end))], end)
        }
    }

    /** The app's little-endian buffers, read in order (KompozR's `Writer`). */
    class Reader(bytes: ByteArray) {
        private val buffer: ByteBuffer = ByteBuffer.wrap(bytes).order(ByteOrder.LITTLE_ENDIAN)

        fun int(): Int = buffer.getInt()
        fun long(): Long = buffer.getLong()
        fun float(): Float = buffer.getFloat()
        fun bool(): Boolean = buffer.getInt() != 0

        /** A string (null: none). */
        fun str(): String? {
            val length = buffer.getInt()
            if (length < 0) return null
            val s = String(buffer.array(), buffer.position(), length, Charsets.UTF_8)
            buffer.position(buffer.position() + length)
            return s
        }
    }

    companion object {
        const val HOST_VIEW_ID = AccessibilityNodeProvider.HOST_VIEW_ID
        const val INVALID_ID = Int.MIN_VALUE

        /** How often the changes are told at most (ms). */
        private const val SEND_INTERVAL_MS = 100L
        private const val MAX_CHARACTER_LOCATIONS = AccessibilityNodeInfo.EXTRA_DATA_TEXT_CHARACTER_LOCATION_ARG_MAX_LENGTH

        private const val ROLE_DESCRIPTION_KEY = "AccessibilityNodeInfo.roleDescription"
        private const val STATE_DESCRIPTION_KEY = "androidx.view.accessibility.AccessibilityNodeInfoCompat.STATE_DESCRIPTION_KEY"
        private const val EXTRA_DATA_ID_KEY = "androidx.compose.ui.semantics.id"
        private const val EXTRA_DATA_TEST_TAG_KEY = "androidx.compose.ui.semantics.testTag"

        // (AccessibilityEvent's, by value: newer than this library's minimum.)
        private const val CHANGE_CONTENT_INVALID = 1024
        private const val CHANGE_ERROR = 2048
        private const val CHANGE_ENABLED = 4096
        private const val CHANGE_CHECKED = 8192

        /** Our ids for custom actions (32, as Compose's). */
        private val CUSTOM_ACTION_IDS = intArrayOf(
            R.id.kompoz_custom_action_0, R.id.kompoz_custom_action_1, R.id.kompoz_custom_action_2,
            R.id.kompoz_custom_action_3, R.id.kompoz_custom_action_4, R.id.kompoz_custom_action_5,
            R.id.kompoz_custom_action_6, R.id.kompoz_custom_action_7, R.id.kompoz_custom_action_8,
            R.id.kompoz_custom_action_9, R.id.kompoz_custom_action_10, R.id.kompoz_custom_action_11,
            R.id.kompoz_custom_action_12, R.id.kompoz_custom_action_13, R.id.kompoz_custom_action_14,
            R.id.kompoz_custom_action_15, R.id.kompoz_custom_action_16, R.id.kompoz_custom_action_17,
            R.id.kompoz_custom_action_18, R.id.kompoz_custom_action_19, R.id.kompoz_custom_action_20,
            R.id.kompoz_custom_action_21, R.id.kompoz_custom_action_22, R.id.kompoz_custom_action_23,
            R.id.kompoz_custom_action_24, R.id.kompoz_custom_action_25, R.id.kompoz_custom_action_26,
            R.id.kompoz_custom_action_27, R.id.kompoz_custom_action_28, R.id.kompoz_custom_action_29,
            R.id.kompoz_custom_action_30, R.id.kompoz_custom_action_31,
        )

        // KompozR's android::accessibility `flag`, `action`, `state`, `role_description`, `event_kind`, `request`.
        const val FLAG_CHECKABLE = 1
        const val FLAG_CHECKED = 1 shl 1
        const val FLAG_ENABLED = 1 shl 2
        const val FLAG_FOCUSABLE = 1 shl 3
        const val FLAG_FOCUSED = 1 shl 4
        const val FLAG_CLICKABLE = 1 shl 5
        const val FLAG_LONG_CLICKABLE = 1 shl 6
        const val FLAG_EDITABLE = 1 shl 7
        const val FLAG_PASSWORD = 1 shl 8
        const val FLAG_SCROLLABLE = 1 shl 9
        const val FLAG_HEADING = 1 shl 10
        const val FLAG_SELECTED = 1 shl 11
        const val FLAG_VISIBLE = 1 shl 12
        const val FLAG_SCREEN_READER_FOCUSABLE = 1 shl 13
        const val FLAG_CONTENT_INVALID = 1 shl 14
        const val FLAG_TEXT_ENTRY_KEY = 1 shl 15
        const val FLAG_IMPORTANT = 1 shl 16
        const val FLAG_SENSITIVE = 1 shl 17
        const val FLAG_DISMISSABLE = 1 shl 18
        const val FLAG_MULTI_LINE = 1 shl 19
        const val FLAG_CHARACTER_LOCATIONS = 1 shl 20
        const val FLAG_SHOWING_HINT = 1 shl 21

        const val ACTION_CLICK = 1
        const val ACTION_LONG_CLICK = 1 shl 1
        const val ACTION_FOCUS = 1 shl 2
        const val ACTION_CLEAR_FOCUS = 1 shl 3
        const val ACTION_SCROLL_FORWARD = 1 shl 4
        const val ACTION_SCROLL_BACKWARD = 1 shl 5
        const val ACTION_SCROLL_UP = 1 shl 6
        const val ACTION_SCROLL_DOWN = 1 shl 7
        const val ACTION_SCROLL_LEFT = 1 shl 8
        const val ACTION_SCROLL_RIGHT = 1 shl 9
        const val ACTION_SCROLL_TO_POSITION = 1 shl 10
        const val ACTION_SET_PROGRESS = 1 shl 11
        const val ACTION_SET_TEXT = 1 shl 12
        const val ACTION_SET_SELECTION = 1 shl 13
        const val ACTION_NEXT_AT_GRANULARITY = 1 shl 14
        const val ACTION_PREVIOUS_AT_GRANULARITY = 1 shl 15
        const val ACTION_EXPAND = 1 shl 16
        const val ACTION_COLLAPSE = 1 shl 17
        const val ACTION_DISMISS = 1 shl 18
        const val ACTION_COPY = 1 shl 19
        const val ACTION_CUT = 1 shl 20
        const val ACTION_PASTE = 1 shl 21
        const val ACTION_IME_ENTER = 1 shl 22
        const val ACTION_PAGE_UP = 1 shl 23
        const val ACTION_PAGE_DOWN = 1 shl 24
        const val ACTION_PAGE_LEFT = 1 shl 25
        const val ACTION_PAGE_RIGHT = 1 shl 26
        const val ACTION_SHOW_ON_SCREEN = 1 shl 27

        const val STATE_ON = 1
        const val STATE_OFF = 2
        const val STATE_INDETERMINATE = 3
        const val STATE_SELECTED = 4
        const val STATE_NOT_SELECTED = 5
        const val STATE_PERCENT = 6
        const val STATE_IN_PROGRESS = 7
        const val STATE_EMPTY = 8

        const val ROLE_TAB = 1

        const val EVENT_CONTENT_CHANGED = 0
        const val EVENT_TEXT_CHANGED = 1
        const val EVENT_SELECTION_CHANGED = 2
        const val EVENT_FOCUSED = 3
        const val EVENT_SELECTED = 4
        const val EVENT_SCROLLED = 5
        const val EVENT_PANE_CHANGED = 6

        const val REQUEST_CLICK = 0
        const val REQUEST_LONG_CLICK = 1
        const val REQUEST_FOCUS = 2
        const val REQUEST_CLEAR_FOCUS = 3
        const val REQUEST_SCROLL_FORWARD = 4
        const val REQUEST_SCROLL_BACKWARD = 5
        const val REQUEST_SCROLL_UP = 6
        const val REQUEST_SCROLL_DOWN = 7
        const val REQUEST_SCROLL_LEFT = 8
        const val REQUEST_SCROLL_RIGHT = 9
        const val REQUEST_SCROLL_TO_POSITION = 10
        const val REQUEST_SET_PROGRESS = 11
        const val REQUEST_SET_TEXT = 12
        const val REQUEST_SET_SELECTION = 13
        const val REQUEST_EXPAND = 14
        const val REQUEST_COLLAPSE = 15
        const val REQUEST_DISMISS = 16
        const val REQUEST_COPY = 17
        const val REQUEST_CUT = 18
        const val REQUEST_PASTE = 19
        const val REQUEST_IME_ENTER = 20
        const val REQUEST_PAGE_UP = 21
        const val REQUEST_PAGE_DOWN = 22
        const val REQUEST_PAGE_LEFT = 23
        const val REQUEST_PAGE_RIGHT = 24
        const val REQUEST_SHOW_ON_SCREEN = 25
        const val REQUEST_CUSTOM = 26
    }
}

/** A rectangle in the view, rounded out. */
internal fun RectF.rounded(): Rect =
    Rect(Math.floor(left.toDouble()).toInt(), Math.floor(top.toDouble()).toInt(), Math.ceil(right.toDouble()).toInt(), Math.ceil(bottom.toDouble()).toInt())

/** A rectangle in the view as in the screen, the view's place there given. */
internal fun RectF.toScreen(x: Int, y: Int): Rect = rounded().also { it.offset(x, y) }
