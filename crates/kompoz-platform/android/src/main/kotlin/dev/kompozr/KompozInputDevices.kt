package dev.kompozr

import android.content.pm.PackageManager
import android.content.res.Configuration
import android.hardware.input.InputManager
import android.os.Build
import android.os.Handler
import android.os.Looper
import android.view.InputDevice
import android.view.ViewConfiguration

/**
 * The input devices and the system's gesture thresholds as the app reads them (C++ Kompoz's
 * KompozInput.mediaInfo and viewConfiguration; kompoz-platform's android::devices): the most precise pointer there
 * is, a physical keyboard, a microphone, a camera, how far the screen is; the touch slop, the long press and double
 * tap timeouts, the fling velocities, the handwriting slop, the least touch target. Told as the view is attached,
 * as input devices come, go or change, and as the configuration changes.
 */
internal class KompozInputDevices(private val view: KompozView) : InputManager.InputDeviceListener {
    private val input = view.context.getSystemService(InputManager::class.java)
    private var listening = false

    fun start() {
        if (!listening) {
            input.registerInputDeviceListener(this, Handler(Looper.getMainLooper()))
            listening = true
        }
        tell()
    }

    fun stop() {
        if (listening) {
            input.unregisterInputDeviceListener(this)
            listening = false
        }
    }

    override fun onInputDeviceAdded(deviceId: Int) = tell()
    override fun onInputDeviceRemoved(deviceId: Int) = tell()
    override fun onInputDeviceChanged(deviceId: Int) = tell()

    /** The devices and thresholds to the app. */
    fun tell() = view.devices(mediaInfo(), viewConfiguration())

    /** `[pointer (0 none, 1 blunt, 2 coarse, 3 fine), keyboard, microphone, camera, distance (0 near, 1 medium,
     * 2 far)]`. */
    private fun mediaInfo(): IntArray {
        var pointer = 0
        var keyboard = false
        for (id in InputDevice.getDeviceIds()) {
            val device = InputDevice.getDevice(id) ?: continue
            if (device.isVirtual) continue
            fun has(source: Int) = device.sources and source == source
            pointer = maxOf(
                pointer,
                when {
                    has(InputDevice.SOURCE_MOUSE) || has(InputDevice.SOURCE_STYLUS) ||
                        has(InputDevice.SOURCE_BLUETOOTH_STYLUS) || has(InputDevice.SOURCE_TOUCHPAD) -> 3
                    has(InputDevice.SOURCE_TOUCHSCREEN) -> 2
                    has(InputDevice.SOURCE_DPAD) || has(InputDevice.SOURCE_GAMEPAD) ||
                        has(InputDevice.SOURCE_JOYSTICK) -> 1
                    else -> 0
                },
            )
            if (device.keyboardType == InputDevice.KEYBOARD_TYPE_ALPHABETIC) keyboard = true
        }
        val configuration = view.resources.configuration
        // (A keyboard slid away doesn't type.)
        if (configuration.hardKeyboardHidden == Configuration.HARDKEYBOARDHIDDEN_YES) keyboard = false
        val packages = view.context.packageManager
        val uiMode = configuration.uiMode and Configuration.UI_MODE_TYPE_MASK
        val distance = when {
            uiMode == Configuration.UI_MODE_TYPE_TELEVISION ||
                packages.hasSystemFeature(PackageManager.FEATURE_LEANBACK) -> 2
            uiMode == Configuration.UI_MODE_TYPE_CAR ||
                packages.hasSystemFeature(PackageManager.FEATURE_AUTOMOTIVE) -> 1
            else -> 0
        }
        return intArrayOf(
            pointer,
            if (keyboard) 1 else 0,
            if (packages.hasSystemFeature(PackageManager.FEATURE_MICROPHONE)) 1 else 0,
            if (packages.hasSystemFeature(PackageManager.FEATURE_CAMERA_ANY)) 1 else 0,
            distance,
        )
    }

    /** `[touchSlop px, longPressTimeout ms, doubleTapTimeout ms, doubleTapMinTime ms, minFling px/s, maxFling px/s,
     * handwritingSlop px, minimumTouchTarget px]` (Compose's AndroidViewConfiguration). */
    private fun viewConfiguration(): FloatArray {
        val c = ViewConfiguration.get(view.context)
        val density = view.resources.displayMetrics.density
        return floatArrayOf(
            c.scaledTouchSlop.toFloat(),
            ViewConfiguration.getLongPressTimeout().toFloat(),
            ViewConfiguration.getDoubleTapTimeout().toFloat(),
            DOUBLE_TAP_MIN_TIME_MS,
            c.scaledMinimumFlingVelocity.toFloat(),
            c.scaledMaximumFlingVelocity.toFloat(),
            if (Build.VERSION.SDK_INT >= 34) c.scaledHandwritingSlop.toFloat() else 0f,
            MINIMUM_TOUCH_TARGET_DP * density,
        )
    }

    companion object {
        /** ViewConfiguration's hidden DOUBLE_TAP_MIN_TIME (Compose's AndroidViewConfiguration uses it too). */
        private const val DOUBLE_TAP_MIN_TIME_MS = 40f

        /** Compose's minimum touch target, 48 dp. */
        private const val MINIMUM_TOUCH_TARGET_DP = 48f
    }
}
