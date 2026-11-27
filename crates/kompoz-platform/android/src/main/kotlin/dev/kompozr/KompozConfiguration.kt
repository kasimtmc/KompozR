package dev.kompozr

import android.app.Activity
import android.content.res.Configuration
import android.os.Build
import android.view.Display
import java.util.TimeZone

/**
 * What the app reads of the display and the system (kompoz-platform's android::configuration): the density, the
 * text size, the refresh rate, the night mode, the touchscreen and the keyboard, the user's languages, the time zone.
 */
internal object KompozConfiguration {
    /** `[density, fontScale, refreshRate, dark, touchscreen, keyboard]`, the app's order. */
    fun values(activity: Activity): FloatArray {
        val configuration = activity.resources.configuration
        val density = activity.resources.displayMetrics.density
        val night = configuration.uiMode and Configuration.UI_MODE_NIGHT_MASK == Configuration.UI_MODE_NIGHT_YES
        val touchscreen = configuration.touchscreen != Configuration.TOUCHSCREEN_NOTOUCH
        val keyboard = configuration.keyboard == Configuration.KEYBOARD_QWERTY &&
            configuration.hardKeyboardHidden != Configuration.HARDKEYBOARDHIDDEN_YES
        return floatArrayOf(
            density,
            configuration.fontScale,
            display(activity)?.refreshRate ?: 60f,
            if (night) 1f else 0f,
            if (touchscreen) 1f else 0f,
            if (keyboard) 1f else 0f,
        )
    }

    /** The display the activity is on. */
    private fun display(activity: Activity): Display? =
        if (Build.VERSION.SDK_INT >= 30) activity.display
        else @Suppress("DEPRECATION") activity.windowManager.defaultDisplay

    /** The user's languages as BCP 47 tags, one a line, the first the most. */
    fun languages(configuration: Configuration): ByteArray =
        configuration.locales.toLanguageTags().replace(',', '\n').toByteArray()

    /** The system's time zone's id ("Europe/Istanbul"). */
    fun timeZone(): ByteArray = TimeZone.getDefault().id.toByteArray()
}
