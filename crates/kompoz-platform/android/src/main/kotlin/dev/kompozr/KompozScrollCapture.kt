package dev.kompozr

import android.annotation.TargetApi
import android.graphics.Bitmap
import android.graphics.Color
import android.graphics.Point
import android.graphics.PorterDuff
import android.graphics.Rect
import android.os.CancellationSignal
import android.view.ScrollCaptureCallback
import android.view.ScrollCaptureSession
import android.view.ScrollCaptureTarget
import java.nio.ByteBuffer
import java.util.function.Consumer

/**
 * Scroll capture for a KompozR app (Android 12's long screenshots; Compose's ScrollCapture and
 * ComposeScrollCaptureCallback): the scrollable the app finds (the deepest that scrolls vertically, of those the
 * tallest) is the target, scrolled by the app to center each part asked for (without animation) and its pixels — the
 * app's last frame, read back a frame after the scroll — drawn into the capture's surface; scrolled back as the
 * session ends. The view's surface is drawn with Vulkan, so the view itself can't be drawn into a canvas as Compose
 * draws its own.
 */
@TargetApi(31)
internal class KompozScrollCapture(private val view: KompozView) {
    private var token = 0
    private val images = HashMap<Int, Pair<ScrollCaptureSession, Consumer<Rect>>>()
    private val ends = HashMap<Int, Runnable>()

    /** The view's `onScrollCaptureSearch`: the app's scrollable, if it has one. */
    fun search(windowOffset: Point, targets: Consumer<ScrollCaptureTarget>) {
        val v = view.scrollCaptureSearch() ?: return
        val viewport = Rect(v[0], v[1], v[2], v[3])
        val target = ScrollCaptureTarget(
            view,
            Rect(viewport),
            Point(windowOffset.x + viewport.left, windowOffset.y + viewport.top),
            Callback(viewport),
        )
        target.scrollBounds = Rect(viewport)
        targets.accept(target)
    }

    private inner class Callback(private val viewport: Rect) : ScrollCaptureCallback {
        override fun onScrollCaptureSearch(signal: CancellationSignal, onReady: Consumer<Rect>) =
            onReady.accept(Rect(viewport))

        override fun onScrollCaptureStart(session: ScrollCaptureSession, signal: CancellationSignal, onReady: Runnable) {
            view.scrollCaptureStart()
            onReady.run()
        }

        override fun onScrollCaptureImageRequest(
            session: ScrollCaptureSession,
            signal: CancellationSignal,
            captureArea: Rect,
            onComplete: Consumer<Rect>,
        ) {
            val t = ++token
            images[t] = session to onComplete
            signal.setOnCancelListener { images.remove(t) }
            view.scrollCaptureRequest(t, captureArea.left, captureArea.top, captureArea.right, captureArea.bottom)
        }

        override fun onScrollCaptureEnd(onReady: Runnable) {
            val t = ++token
            ends[t] = onReady
            view.scrollCaptureEnd(t)
        }
    }

    /** A part's pixels from the app (none: nothing to draw) and where it is in the capture. */
    fun image(token: Int, rect: IntArray, pixels: ByteArray?, width: Int, height: Int) {
        val (session, onComplete) = images.remove(token) ?: return
        if (pixels != null && width > 0 && height > 0) {
            val bitmap = Bitmap.createBitmap(width, height, Bitmap.Config.ARGB_8888)
            bitmap.copyPixelsFromBuffer(ByteBuffer.wrap(pixels))
            val canvas = session.surface.lockHardwareCanvas()
            try {
                canvas.drawColor(Color.TRANSPARENT, PorterDuff.Mode.CLEAR)
                canvas.drawBitmap(bitmap, 0f, 0f, null)
            } finally {
                session.surface.unlockCanvasAndPost(canvas)
            }
            bitmap.recycle()
        }
        onComplete.accept(Rect(rect[0], rect[1], rect[2], rect[3]))
    }

    /** The session's end done (the scrollable back where it was). */
    fun ended(token: Int) {
        ends.remove(token)?.run()
    }
}
