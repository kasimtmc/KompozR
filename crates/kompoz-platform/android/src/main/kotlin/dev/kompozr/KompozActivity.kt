package dev.kompozr

import android.app.Activity
import android.content.BroadcastReceiver
import android.content.ComponentCallbacks2
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.content.pm.PackageManager
import android.content.res.AssetManager
import android.content.res.Configuration
import android.hardware.display.DisplayManager
import android.os.Build
import android.os.Bundle
import android.os.Handler
import android.os.Looper

/**
 * The activity a KompozR app runs in (C++ Kompoz's KompozActivity): it loads the app's library (the Rust app,
 * whose `android_app!` registers the natives below as it loads), shows a [KompozView], and tells the app its
 * lifecycle, its configuration (kept across every configuration change: the manifest's `configChanges`, the
 * density's too), the display's refresh rate and the time zone as they change, and its saved state across a process
 * death. The app itself lives in native code, on this, the UI thread.
 *
 * An app's activity is this class, or one of its own extending it (to add what it needs). The library's name is the
 * manifest's `dev.kompozr.library` meta-data on the activity ([library]), "main" without it.
 */
open class KompozActivity : Activity() {
    /** The app's view: its surface drawn into, its input. */
    protected lateinit var view: KompozView
        private set

    private var displayListener: DisplayManager.DisplayListener? = null
    private var timeZoneReceiver: BroadcastReceiver? = null

    /** The library to load (without `lib` and `.so`): the activity's `dev.kompozr.library` meta-data, else "main". */
    protected open fun library(): String {
        val info = if (Build.VERSION.SDK_INT >= 33) {
            packageManager.getActivityInfo(
                componentName,
                PackageManager.ComponentInfoFlags.of(PackageManager.GET_META_DATA.toLong()),
            )
        } else {
            @Suppress("DEPRECATION")
            packageManager.getActivityInfo(componentName, PackageManager.GET_META_DATA)
        }
        return info.metaData?.getString(LIBRARY_META_DATA) ?: "main"
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        System.loadLibrary(library())
        view = KompozView(this)
        setContentView(view)
        view.requestFocus()
        nativeCreate(
            view,
            assets,
            filesDir.path.toByteArray(),
            getExternalFilesDir(null)?.path?.toByteArray(),
            cacheDir.path.toByteArray(),
            savedInstanceState?.getByteArray(STATE_KEY),
        )
        configurationChanged()
        // The refresh rate changes without a configuration change (a display's mode, a phone's adaptive rate).
        val displays = getSystemService(DisplayManager::class.java)
        displayListener = object : DisplayManager.DisplayListener {
            override fun onDisplayAdded(displayId: Int) {}
            override fun onDisplayRemoved(displayId: Int) {}
            override fun onDisplayChanged(displayId: Int) = configurationChanged()
        }.also { displays.registerDisplayListener(it, Handler(Looper.getMainLooper())) }
        // The time zone changes without one too.
        timeZoneReceiver = object : BroadcastReceiver() {
            override fun onReceive(context: Context, intent: Intent) = configurationChanged()
        }.also {
            val filter = IntentFilter(Intent.ACTION_TIMEZONE_CHANGED)
            if (Build.VERSION.SDK_INT >= 33) registerReceiver(it, filter, RECEIVER_NOT_EXPORTED)
            else registerReceiver(it, filter)
        }
    }

    /** The configuration as the app reads it, told again. */
    private fun configurationChanged() {
        nativeConfiguration(
            KompozConfiguration.values(this),
            KompozConfiguration.languages(resources.configuration),
            KompozConfiguration.timeZone(),
        )
    }

    override fun onConfigurationChanged(newConfig: Configuration) {
        super.onConfigurationChanged(newConfig)
        configurationChanged()
    }

    override fun onStart() {
        super.onStart()
        nativeLifecycle(LIFECYCLE_WILL_ENTER_FOREGROUND)
    }

    override fun onResume() {
        super.onResume()
        nativeLifecycle(LIFECYCLE_DID_ENTER_FOREGROUND)
    }

    override fun onPause() {
        nativeLifecycle(LIFECYCLE_WILL_ENTER_BACKGROUND)
        super.onPause()
    }

    override fun onStop() {
        nativeLifecycle(LIFECYCLE_DID_ENTER_BACKGROUND)
        super.onStop()
    }

    override fun onWindowFocusChanged(hasFocus: Boolean) {
        super.onWindowFocusChanged(hasFocus)
        nativeLifecycle(if (hasFocus) LIFECYCLE_FOCUS_GAINED else LIFECYCLE_FOCUS_LOST)
    }

    /** Memory short while the app is in the background: the GPU's caches let go. */
    override fun onTrimMemory(level: Int) {
        super.onTrimMemory(level)
        if (level >= ComponentCallbacks2.TRIM_MEMORY_BACKGROUND) nativeLifecycle(LIFECYCLE_LOW_MEMORY)
    }

    /** The app's state (Compose's saved instance state), for the system to give back after a process death. */
    override fun onSaveInstanceState(outState: Bundle) {
        super.onSaveInstanceState(outState)
        nativeSaveState()?.let { outState.putByteArray(STATE_KEY, it) }
    }

    override fun onDestroy() {
        displayListener?.let { getSystemService(DisplayManager::class.java).unregisterDisplayListener(it) }
        timeZoneReceiver?.let { unregisterReceiver(it) }
        nativeDestroy()
        super.onDestroy()
    }

    private external fun nativeCreate(
        view: KompozView,
        assets: AssetManager,
        filesDir: ByteArray,
        externalFilesDir: ByteArray?,
        cacheDir: ByteArray,
        savedState: ByteArray?,
    )

    private external fun nativeConfiguration(values: FloatArray, languages: ByteArray, timeZone: ByteArray)

    private external fun nativeLifecycle(event: Int)

    private external fun nativeSaveState(): ByteArray?

    private external fun nativeDestroy()

    companion object {
        /** The activity's meta-data naming the app's library. */
        const val LIBRARY_META_DATA = "dev.kompozr.library"

        /** Where the app's state is in the saved instance state. */
        private const val STATE_KEY = "dev.kompozr.state"

        // The lifecycle as the app hears it (kompoz-platform's android::app LIFECYCLE, in this order).
        private const val LIFECYCLE_WILL_ENTER_BACKGROUND = 0
        private const val LIFECYCLE_DID_ENTER_BACKGROUND = 1
        private const val LIFECYCLE_WILL_ENTER_FOREGROUND = 2
        private const val LIFECYCLE_DID_ENTER_FOREGROUND = 3
        private const val LIFECYCLE_LOW_MEMORY = 4
        private const val LIFECYCLE_FOCUS_GAINED = 5
        private const val LIFECYCLE_FOCUS_LOST = 6
    }
}
