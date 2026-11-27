package dev.kompozr

import android.annotation.TargetApi
import android.app.WallpaperManager
import android.content.ActivityNotFoundException
import android.content.Intent
import android.database.ContentObserver
import android.graphics.Rect
import android.net.Uri
import android.os.Build
import android.os.Handler
import android.os.Looper
import android.provider.Settings
import android.util.Log
import android.view.HapticFeedbackConstants
import android.view.View
import android.view.WindowManager

/**
 * The system's services for a KompozR app (C++'s KompozActivity and KompozInput services, Compose's
 * AndroidComposeView and its platform pieces): haptic feedback, opening a URI, keeping the screen on, keeping
 * sensitive content out of screen shares (Android 15), the refresh rate the app votes for (Android 15's frame rate
 * categories on the view; the surface's own vote is the native side's), where the system's edge swipes are kept off,
 * the system's palettes (Android 12's colours, the wallpaper's) and the animator duration scale, both followed as
 * they change.
 *
 * Its methods `haptic`, `openUri`, `setKeepScreenOn`, `setContentSensitive`, `setFrameRate`, `setGestureExclusion`,
 * `dynamicColors`, `wallpaperColors` and `animatorScale` are the app's to call (by name, from the native side, on
 * this thread).
 */
internal class KompozServices(private val activity: KompozActivity, private val view: KompozView) {
    private val handler = Handler(Looper.getMainLooper())
    private var animatorScaleObserver: ContentObserver? = null
    private var colorsListener: Any? = null

    /** Follows the system's settings the app reads (as the activity is created). */
    fun start() {
        animatorScaleObserver = object : ContentObserver(handler) {
            override fun onChange(selfChange: Boolean) = nativeAnimatorScale(view, animatorScale())
        }.also {
            activity.contentResolver.registerContentObserver(
                Settings.Global.getUriFor(Settings.Global.ANIMATOR_DURATION_SCALE), false, it,
            )
        }
        colorsListener = WallpaperManager.OnColorsChangedListener { _, _ -> nativeWallpaperChanged(view) }.also {
            WallpaperManager.getInstance(activity).addOnColorsChangedListener(it, handler)
        }
    }

    /** Stops following them (as the activity is destroyed). */
    fun stop() {
        animatorScaleObserver?.let { activity.contentResolver.unregisterContentObserver(it) }
        animatorScaleObserver = null
        (colorsListener as? WallpaperManager.OnColorsChangedListener)?.let {
            WallpaperManager.getInstance(activity).removeOnColorsChangedListener(it)
        }
        colorsListener = null
    }

    // ---- Called by the app ----

    /**
     * Haptic feedback of the app's type (HapticFeedbackType, in its order), as Compose performs it: the system's
     * constant for it, or the nearest one this Android has (androidx's HapticFeedbackConstantsCompat). The user's
     * touch feedback setting decides whether it is felt.
     */
    @Suppress("unused") // (the app's)
    fun haptic(type: Int) {
        view.performHapticFeedback(hapticConstant(type))
    }

    /** The browser, or the app for the URI's scheme; false if nothing takes it. */
    @Suppress("unused") // (the app's)
    fun openUri(uri: ByteArray): Boolean {
        val text = uri.toString(Charsets.UTF_8)
        return try {
            activity.startActivity(Intent(Intent.ACTION_VIEW, Uri.parse(text)).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK))
            true
        } catch (e: ActivityNotFoundException) {
            Log.w("kompoz", "nothing opens $text")
            false
        }
    }

    /** The screen kept on while the app has a `keep_screen_on` laid out. */
    @Suppress("unused") // (the app's)
    fun setKeepScreenOn(on: Boolean) {
        val flag = WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON
        if (on) activity.window.addFlags(flag) else activity.window.clearFlags(flag)
    }

    /** The window kept out of screen shares and recordings while something sensitive is laid out (Android 15). */
    @Suppress("unused") // (the app's)
    fun setContentSensitive(sensitive: Boolean) {
        if (Build.VERSION.SDK_INT >= 35) {
            view.contentSensitivity =
                if (sensitive) View.CONTENT_SENSITIVITY_SENSITIVE else View.CONTENT_SENSITIVITY_AUTO
        }
    }

    /**
     * The refresh rate the app votes for on the view (Android 15's `setRequestedFrameRate`, as Compose votes): a
     * rate (Hz; 0 none), else a category (0 default, 1 no preference, 2 normal, 3 high).
     */
    @Suppress("unused") // (the app's)
    fun setFrameRate(rate: Float, category: Int) {
        if (Build.VERSION.SDK_INT >= 35) Api35.vote(view, rate, category)
    }

    /** Where the system's edge swipes are kept off (left, top, right, bottom in the view, px; 4 each). */
    @Suppress("unused") // (the app's)
    fun setGestureExclusion(rects: IntArray) {
        view.systemGestureExclusionRects =
            (0 until rects.size / 4).map { Rect(rects[it * 4], rects[it * 4 + 1], rects[it * 4 + 2], rects[it * 4 + 3]) }
    }

    /**
     * Android 12's system colours (Material You): accent1, accent2, accent3, neutral1 and neutral2, each in tones
     * 0 (white) … 1000 (black), as ARGB; null before Android 12.
     */
    @Suppress("unused") // (the app's)
    fun dynamicColors(): IntArray? = if (Build.VERSION.SDK_INT >= 31) Api31.colors(activity) else null

    /** The wallpaper's colours as the system picked them (primary, secondary, tertiary; 0: none); null: none. */
    @Suppress("unused") // (the app's)
    fun wallpaperColors(): IntArray? = try {
        WallpaperManager.getInstance(activity).getWallpaperColors(WallpaperManager.FLAG_SYSTEM)?.let {
            intArrayOf(it.primaryColor.toArgb(), it.secondaryColor?.toArgb() ?: 0, it.tertiaryColor?.toArgb() ?: 0)
        }
    } catch (e: RuntimeException) {
        null
    }

    /** The animator duration scale (Settings' "Animator duration scale"; 0: animations off). */
    @Suppress("unused") // (the app's)
    fun animatorScale(): Float =
        Settings.Global.getFloat(activity.contentResolver, Settings.Global.ANIMATOR_DURATION_SCALE, 1f)

    @TargetApi(35)
    private object Api35 {
        fun vote(view: View, rate: Float, category: Int) {
            view.requestedFrameRate = when {
                rate > 0f -> rate
                category == 1 -> View.REQUESTED_FRAME_RATE_CATEGORY_NO_PREFERENCE
                category == 2 -> View.REQUESTED_FRAME_RATE_CATEGORY_NORMAL
                category == 3 -> View.REQUESTED_FRAME_RATE_CATEGORY_HIGH
                else -> View.REQUESTED_FRAME_RATE_CATEGORY_DEFAULT
            }
        }
    }

    @TargetApi(31)
    private object Api31 {
        private val ids = intArrayOf(
            android.R.color.system_accent1_0, android.R.color.system_accent1_10, android.R.color.system_accent1_50,
            android.R.color.system_accent1_100, android.R.color.system_accent1_200, android.R.color.system_accent1_300,
            android.R.color.system_accent1_400, android.R.color.system_accent1_500, android.R.color.system_accent1_600,
            android.R.color.system_accent1_700, android.R.color.system_accent1_800, android.R.color.system_accent1_900,
            android.R.color.system_accent1_1000,
            android.R.color.system_accent2_0, android.R.color.system_accent2_10, android.R.color.system_accent2_50,
            android.R.color.system_accent2_100, android.R.color.system_accent2_200, android.R.color.system_accent2_300,
            android.R.color.system_accent2_400, android.R.color.system_accent2_500, android.R.color.system_accent2_600,
            android.R.color.system_accent2_700, android.R.color.system_accent2_800, android.R.color.system_accent2_900,
            android.R.color.system_accent2_1000,
            android.R.color.system_accent3_0, android.R.color.system_accent3_10, android.R.color.system_accent3_50,
            android.R.color.system_accent3_100, android.R.color.system_accent3_200, android.R.color.system_accent3_300,
            android.R.color.system_accent3_400, android.R.color.system_accent3_500, android.R.color.system_accent3_600,
            android.R.color.system_accent3_700, android.R.color.system_accent3_800, android.R.color.system_accent3_900,
            android.R.color.system_accent3_1000,
            android.R.color.system_neutral1_0, android.R.color.system_neutral1_10, android.R.color.system_neutral1_50,
            android.R.color.system_neutral1_100, android.R.color.system_neutral1_200, android.R.color.system_neutral1_300,
            android.R.color.system_neutral1_400, android.R.color.system_neutral1_500, android.R.color.system_neutral1_600,
            android.R.color.system_neutral1_700, android.R.color.system_neutral1_800, android.R.color.system_neutral1_900,
            android.R.color.system_neutral1_1000,
            android.R.color.system_neutral2_0, android.R.color.system_neutral2_10, android.R.color.system_neutral2_50,
            android.R.color.system_neutral2_100, android.R.color.system_neutral2_200, android.R.color.system_neutral2_300,
            android.R.color.system_neutral2_400, android.R.color.system_neutral2_500, android.R.color.system_neutral2_600,
            android.R.color.system_neutral2_700, android.R.color.system_neutral2_800, android.R.color.system_neutral2_900,
            android.R.color.system_neutral2_1000,
        )

        fun colors(activity: KompozActivity): IntArray = IntArray(ids.size) { activity.getColor(ids[it]) }
    }

    private external fun nativeAnimatorScale(view: KompozView, scale: Float)

    private external fun nativeWallpaperChanged(view: KompozView)

    companion object {
        /** The system's constant for the app's haptic type, the nearest this Android has (minSdk 29). */
        private fun hapticConstant(type: Int): Int {
            val sdk = Build.VERSION.SDK_INT
            return when (type) {
                0 -> HapticFeedbackConstants.LONG_PRESS
                1 -> HapticFeedbackConstants.TEXT_HANDLE_MOVE
                2 -> if (sdk >= 30) HapticFeedbackConstants.CONFIRM else HapticFeedbackConstants.LONG_PRESS
                3 -> if (sdk >= 30) HapticFeedbackConstants.REJECT else HapticFeedbackConstants.LONG_PRESS
                4 -> HapticFeedbackConstants.CONTEXT_CLICK
                5 -> if (sdk >= 30) HapticFeedbackConstants.GESTURE_END else HapticFeedbackConstants.CLOCK_TICK
                6 -> if (sdk >= 34) HapticFeedbackConstants.GESTURE_THRESHOLD_ACTIVATE else HapticFeedbackConstants.CONTEXT_CLICK
                7 -> HapticFeedbackConstants.KEYBOARD_TAP
                8 -> if (sdk >= 34) HapticFeedbackConstants.SEGMENT_TICK else HapticFeedbackConstants.CLOCK_TICK
                9 -> if (sdk >= 34) HapticFeedbackConstants.SEGMENT_FREQUENT_TICK else HapticFeedbackConstants.CLOCK_TICK
                10 -> if (sdk >= 34) HapticFeedbackConstants.TOGGLE_ON else HapticFeedbackConstants.CONTEXT_CLICK
                11 -> if (sdk >= 34) HapticFeedbackConstants.TOGGLE_OFF else HapticFeedbackConstants.CONTEXT_CLICK
                12 -> HapticFeedbackConstants.VIRTUAL_KEY
                else -> HapticFeedbackConstants.LONG_PRESS
            }
        }
    }
}
