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
 * whose `android_app!` registers the natives below as it loads), shows a [KompozView] drawn edge to edge
 * ([KompozInsets]), and tells the app its lifecycle, its configuration (kept across every configuration change: the
 * manifest's `configChanges`, the density's too), the display's refresh rate and the time zone as they change, the
 * links it is opened with (its launch intent's and each new one's: deep links), back ([KompozBack]), memory running
 * short, and its saved state across a process death. The system's other services are [KompozServices]',
 * [KompozClipboard]'s and [KompozDragAndDrop]'s. The app itself lives in native code, on this, the UI thread.
 *
 * An app's activity is this class, or one of its own extending it (to add what it needs). The library's name is the
 * manifest's `dev.kompozr.library` meta-data on the activity ([library]), "main" without it.
 */
open class KompozActivity : Activity() {
    /** The app's view: its surface drawn into, its input. */
    protected lateinit var view: KompozView
        private set

    private lateinit var insets: KompozInsets
    private lateinit var back: KompozBack
    private lateinit var services: KompozServices

    private var displayListener: DisplayManager.DisplayListener? = null
    private var timeZoneReceiver: BroadcastReceiver? = null

    /** The night mode the window's bars were last set up for. */
    private var dark = false

    /** Back is leaving the app ([leaveBack]): the system's own. */
    private var leaving = false

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
        KompozContent.context = applicationContext
        view = KompozView(this)
        setContentView(view)
        view.requestFocus()
        insets = KompozInsets(this, view)
        dark = night(resources.configuration)
        insets.enableEdgeToEdge(dark)
        back = KompozBack(this, view)
        services = KompozServices(this, view)
        nativeCreate(
            view,
            assets,
            filesDir.path.toByteArray(),
            getExternalFilesDir(null)?.path?.toByteArray(),
            cacheDir.path.toByteArray(),
            savedInstanceState?.getByteArray(STATE_KEY),
        )
        nativeServices(insets, back, services, KompozClipboard(this), KompozDragAndDrop(this, view))
        services.start()
        configurationChanged()
        // (The launch intent's link once: an activity made again after a process death was there already, as
        // Navigation's saved `deepLinkHandled` says.)
        if (savedInstanceState == null) deepLink(intent)
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
        // The bars' icons follow the night mode (until the app sets them).
        if (night(newConfig) != dark) {
            dark = night(newConfig)
            insets.enableEdgeToEdge(dark)
        }
        configurationChanged()
    }

    private fun night(configuration: Configuration) =
        configuration.uiMode and Configuration.UI_MODE_NIGHT_MASK == Configuration.UI_MODE_NIGHT_YES

    /** A link the app is opened with again (its launch mode keeps one activity): the app goes there. */
    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        setIntent(intent)
        deepLink(intent)
    }

    /** An intent's link (a VIEW intent's data: `kompoz://…`, an http link the app's filters take) for the app. */
    private fun deepLink(intent: Intent?) {
        if (intent?.action != Intent.ACTION_VIEW) return
        val link = intent.dataString ?: return
        nativeDeepLink(link.toByteArray())
    }

    /**
     * The back key (Android 12 and older, or an app that didn't opt in to Android 13's callbacks): the app's, which
     * leaves the app ([leaveBack]) if nothing in it takes back.
     */
    @Deprecated("Android 13's OnBackInvokedCallback is KompozBack's; this is the older Androids' back key")
    @Suppress("DEPRECATION")
    override fun onBackPressed() {
        if (leaving) super.onBackPressed() else back.pressed()
    }

    /** Back nothing in the app took: the system's own (the activity finished, or its task moved back). */
    @Suppress("DEPRECATION")
    internal fun leaveBack() {
        leaving = true
        try {
            onBackPressed()
        } finally {
            leaving = false
        }
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

    /**
     * Memory short (the system running low while the app runs — Android 13 and older say so —, or the app in the
     * background where the system may end it): the GPU's caches let go. The app's UI hidden is no shortage.
     */
    @Suppress("DEPRECATION")
    override fun onTrimMemory(level: Int) {
        super.onTrimMemory(level)
        val short = level == ComponentCallbacks2.TRIM_MEMORY_RUNNING_LOW ||
            level == ComponentCallbacks2.TRIM_MEMORY_RUNNING_CRITICAL ||
            level >= ComponentCallbacks2.TRIM_MEMORY_BACKGROUND
        if (short) nativeLifecycle(LIFECYCLE_LOW_MEMORY)
    }

    @Deprecated("onTrimMemory's levels say more; the system still calls this when memory is out")
    @Suppress("DEPRECATION")
    override fun onLowMemory() {
        super.onLowMemory()
        nativeLifecycle(LIFECYCLE_LOW_MEMORY)
    }

    /** The app's state (Compose's saved instance state), for the system to give back after a process death. */
    override fun onSaveInstanceState(outState: Bundle) {
        super.onSaveInstanceState(outState)
        nativeSaveState()?.let { outState.putByteArray(STATE_KEY, it) }
    }

    override fun onDestroy() {
        services.stop()
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

    private external fun nativeServices(
        insets: KompozInsets,
        back: KompozBack,
        services: KompozServices,
        clipboard: KompozClipboard,
        dragAndDrop: KompozDragAndDrop,
    )

    private external fun nativeDeepLink(link: ByteArray)

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
