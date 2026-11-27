package dev.kompozr

import android.app.Activity
import android.content.ClipData
import android.content.ContentResolver
import android.graphics.Bitmap
import android.graphics.Canvas
import android.graphics.Point
import android.view.DragAndDropPermissions
import android.view.DragEvent
import android.view.View
import java.nio.ByteBuffer

/**
 * Drag and drop for a KompozR app (C++'s KompozView drags, Compose's AndroidDragAndDropManager): a drag from
 * another app (or the app's own, handed to the system as it left the window) heard over the view — it entering,
 * moving, leaving, dropped — with its ClipDescription's types; a drop's ClipData kept for the app to read as a
 * target asks for a type, with the permissions its content URIs need (`requestDragAndDropPermissions`) until the
 * drop is done. And the app's own drags started as the system's ([start]: `View.startDragAndDrop`, global, other
 * apps given read access to its URIs), shown by a shadow of the app's drag decoration where the app drew one.
 *
 * Its methods `start`, `read` and `dropDone` are the app's to call (by name, from the native side, on this thread).
 */
internal class KompozDragAndDrop(private val activity: Activity, private val view: KompozView) : View.OnDragListener {
    /** The drop the app is reading, and the permissions its URIs were given. */
    private var dropped: ClipData? = null
    private var permissions: DragAndDropPermissions? = null

    init {
        view.setOnDragListener(this)
    }

    override fun onDrag(v: View, event: DragEvent): Boolean {
        val own = event.localState === LOCAL_STATE
        val description = event.clipDescription?.let { KompozClipboard.describe(it) }
        if (event.action == DragEvent.ACTION_DROP) {
            done()
            dropped = event.clipData
            val clip = event.clipData
            val hasContent = clip != null &&
                (0 until clip.itemCount).any { clip.getItemAt(it).uri?.scheme == ContentResolver.SCHEME_CONTENT }
            if (hasContent && !own) permissions = activity.requestDragAndDropPermissions(event)
        }
        val result = event.action == DragEvent.ACTION_DRAG_ENDED && event.result
        val taken = nativeDrag(view, event.action, event.x, event.y, description, own, result)
        if (event.action == DragEvent.ACTION_DROP) done()
        return event.action != DragEvent.ACTION_DROP || taken
    }

    /** The drop's content as `mimeType` (null: not that type, or no drop being read). */
    @Suppress("unused") // (the app's)
    fun read(mimeType: ByteArray): ByteArray? =
        dropped?.let { KompozClipboard.read(activity, it, mimeType.toString(Charsets.UTF_8)) }

    /** The drop read: its ClipData and its URIs' permissions let go. */
    private fun done() {
        dropped = null
        permissions?.release()
        permissions = null
    }

    /**
     * The app's own drag started as the system's: its entry (kompoz-platform's android::clip), its shadow (RGBA,
     * `width` by `height`; none: the system's empty one) with the touch at (`touchX`, `touchY`) in it. Whether it
     * started (the system refuses one without a finger or a button down).
     */
    @Suppress("unused") // (the app's)
    fun start(entry: ByteArray, rgba: ByteArray?, width: Int, height: Int, touchX: Int, touchY: Int): Boolean {
        val clip = KompozClipboard.clipData(entry) ?: return false
        val bitmap = if (rgba != null && width > 0 && height > 0 && rgba.size >= width * height * 4) {
            Bitmap.createBitmap(width, height, Bitmap.Config.ARGB_8888).apply {
                copyPixelsFromBuffer(ByteBuffer.wrap(rgba))
            }
        } else {
            null
        }
        val shadow = object : View.DragShadowBuilder() {
            override fun onProvideShadowMetrics(size: Point, touch: Point) {
                size.set(maxOf(1, bitmap?.width ?: 1), maxOf(1, bitmap?.height ?: 1))
                touch.set(touchX.coerceIn(0, size.x - 1), touchY.coerceIn(0, size.y - 1))
            }

            override fun onDrawShadow(canvas: Canvas) {
                bitmap?.let { canvas.drawBitmap(it, 0f, 0f, null) }
            }
        }
        val flags = View.DRAG_FLAG_GLOBAL or View.DRAG_FLAG_GLOBAL_URI_READ
        return view.startDragAndDrop(clip, shadow, LOCAL_STATE, flags)
    }

    private external fun nativeDrag(
        view: KompozView,
        action: Int,
        x: Float,
        y: Float,
        description: ByteArray?,
        own: Boolean,
        result: Boolean,
    ): Boolean

    companion object {
        /** What marks the app's own drags (their local state). */
        private val LOCAL_STATE = Any()
    }
}
