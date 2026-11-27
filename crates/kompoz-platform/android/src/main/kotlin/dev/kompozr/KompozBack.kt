package dev.kompozr

import android.annotation.TargetApi
import android.os.Build
import android.os.SystemClock
import android.window.BackEvent
import android.window.OnBackAnimationCallback
import android.window.OnBackInvokedCallback
import android.window.OnBackInvokedDispatcher

/**
 * Back for a KompozR app (C++'s KompozInput back, androidx's OnBackPressedDispatcher and activity-compose's
 * PredictiveBackHandler): below Android 13 the back key comes as the activity's `onBackPressed`; from 13 the
 * app's callback is registered only while something in it takes back (a popup, a dialog, a BackHandler: the app
 * says, [setEnabled]), so that otherwise the system goes back itself, leaving the app with its own animation; from
 * 14 the gesture's steps come too (started, progressed with the touch and how far along, cancelled), as Compose's
 * PredictiveBackHandler hears them. Back nothing in the app took leaves the app as the system would ([leave]).
 *
 * Its methods `setEnabled` and `leave` are the app's to call (by name, from the native side, on this thread).
 */
internal class KompozBack(private val activity: KompozActivity, private val view: KompozView) {
    /** The callback registered (Android 13+), while the app takes back. */
    private var callback: Any? = null

    /** The app takes back now (or no longer): its callback registered (or not) with the window's dispatcher. */
    @Suppress("unused") // (the app's)
    fun setEnabled(enabled: Boolean) {
        if (Build.VERSION.SDK_INT < 33 || enabled == (callback != null)) return
        if (enabled) {
            callback = Api33.register(this)
        } else {
            Api33.unregister(activity, callback)
            callback = null
        }
    }

    /** The back key (the activity's `onBackPressed`: Android 12 and older, or an app that didn't opt in to 13's). */
    fun pressed() = send(PHASE_INVOKED, 0f, 0f, 0f, EDGE_NONE)

    /** Back nothing in the app took: the system's own (the app's callback let go first: it would come back here). */
    @Suppress("unused") // (the app's)
    fun leave() {
        setEnabled(false)
        activity.leaveBack()
    }

    private fun send(phase: Int, x: Float, y: Float, progress: Float, edge: Int, frameTimeMillis: Long = SystemClock.uptimeMillis()) =
        nativeBack(view, phase, x, y, progress, edge, frameTimeMillis)

    @TargetApi(33)
    private object Api33 {
        fun register(back: KompozBack): Any {
            val callback: OnBackInvokedCallback =
                if (Build.VERSION.SDK_INT >= 34) Api34.gesture(back) else OnBackInvokedCallback { back.pressed() }
            back.activity.onBackInvokedDispatcher.registerOnBackInvokedCallback(
                OnBackInvokedDispatcher.PRIORITY_DEFAULT,
                callback,
            )
            return callback
        }

        fun unregister(activity: KompozActivity, callback: Any?) {
            (callback as? OnBackInvokedCallback)?.let { activity.onBackInvokedDispatcher.unregisterOnBackInvokedCallback(it) }
        }
    }

    /** The gesture's steps (Android 14), the touch in the view's pixels. */
    @TargetApi(34)
    private object Api34 {
        fun gesture(back: KompozBack): OnBackInvokedCallback = object : OnBackAnimationCallback {
            override fun onBackStarted(event: BackEvent) = step(back, PHASE_STARTED, event)
            override fun onBackProgressed(event: BackEvent) = step(back, PHASE_PROGRESSED, event)
            override fun onBackCancelled() = back.send(PHASE_CANCELLED, 0f, 0f, 0f, EDGE_NONE)
            override fun onBackInvoked() = back.pressed()
        }

        private fun step(back: KompozBack, phase: Int, event: BackEvent) {
            val at = IntArray(2)
            back.view.getLocationOnScreen(at)
            val time = if (Build.VERSION.SDK_INT >= 36) event.frameTimeMillis else SystemClock.uptimeMillis()
            back.send(phase, event.touchX - at[0], event.touchY - at[1], event.progress, event.swipeEdge, time)
        }
    }

    private external fun nativeBack(
        view: KompozView,
        phase: Int,
        x: Float,
        y: Float,
        progress: Float,
        edge: Int,
        frameTimeMillis: Long,
    )

    companion object {
        // The steps as the app reads them (kompoz-platform's android::system back_phase).
        private const val PHASE_STARTED = 0
        private const val PHASE_PROGRESSED = 1
        private const val PHASE_CANCELLED = 2
        private const val PHASE_INVOKED = 3

        /** `BackEvent.EDGE_NONE` (Android 16's; the app's 2). */
        private const val EDGE_NONE = 2
    }
}
