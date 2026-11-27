package dev.kompozr

import android.annotation.TargetApi
import android.app.Activity
import android.graphics.Color
import android.graphics.Insets
import android.graphics.PathIterator
import android.os.Build
import android.os.CancellationSignal
import android.view.View
import android.view.WindowInsets
import android.view.WindowInsetsAnimation
import android.view.WindowInsetsAnimationControlListener
import android.view.WindowInsetsAnimationController
import android.view.WindowInsetsController
import android.view.WindowManager
import android.view.animation.LinearInterpolator

/**
 * The window's insets for a KompozR app (C++'s KompozInput insets, Compose's WindowInsetsHolder and its
 * `enableEdgeToEdge()`): the activity's window drawn edge to edge on every Android (enforced from 15; set here on
 * the older ones too, so that all lay out the same: the app pads by the insets), the system bars' icons light or
 * dark as the app (or, until it says, the night mode) wants, and every kind of inset told to the app as it changes —
 * the status, navigation and caption bars, the keyboard, the display cutout (its bounds and shape) and its
 * waterfall, the gesture edges and the tappable elements, each with whether it is shown and what it would cover
 * shown — and as it moves: a keyboard sliding in or a bar sliding away, frame by frame (WindowInsetsAnimation,
 * Android 11). The keyboard can be moved by hand too (`ime_nested_scroll`: WindowInsetsAnimationController).
 *
 * Its methods `setSystemBarsAppearance`, `canControlIme`, `imeControlBy` and `imeControlEnd` are the app's to call
 * (by name, from the native side, on this thread).
 */
internal class KompozInsets(private val activity: Activity, private val view: KompozView) {
    /** The system bars' icons the app asked for (null: as the night mode wants them). */
    private var appearance: Pair<Boolean, Boolean>? = null

    /** The animations under way: each one's types, the insets before it and after it. */
    private val running = ArrayList<Running>()

    private class Running(val animation: Any, val types: Int, val before: WindowInsets?) {
        var after: WindowInsets? = null
    }

    init {
        view.setOnApplyWindowInsetsListener { _, insets ->
            tell(insets)
            insets
        }
        if (Build.VERSION.SDK_INT >= 30) Animations.attach(this)
    }

    // ---- Edge to edge ----

    /**
     * The window drawn behind the system bars and the cutout, as Compose's `enableEdgeToEdge()` makes it: the
     * bars transparent (a three-button navigation bar keeps the system's scrim for its contrast), the content laid
     * out under them all, their icons as the night mode wants (`dark`) unless the app said otherwise.
     */
    @Suppress("DEPRECATION")
    fun enableEdgeToEdge(dark: Boolean) {
        val window = activity.window
        if (Build.VERSION.SDK_INT >= 30) {
            window.setDecorFitsSystemWindows(false)
        } else {
            val decor = window.decorView
            decor.systemUiVisibility = decor.systemUiVisibility or View.SYSTEM_UI_FLAG_LAYOUT_STABLE or
                View.SYSTEM_UI_FLAG_LAYOUT_HIDE_NAVIGATION or View.SYSTEM_UI_FLAG_LAYOUT_FULLSCREEN
        }
        window.statusBarColor = Color.TRANSPARENT
        window.navigationBarColor = Color.TRANSPARENT
        window.isStatusBarContrastEnforced = false
        window.isNavigationBarContrastEnforced = true
        val attributes = window.attributes
        attributes.layoutInDisplayCutoutMode = if (Build.VERSION.SDK_INT >= 30) {
            WindowManager.LayoutParams.LAYOUT_IN_DISPLAY_CUTOUT_MODE_ALWAYS
        } else {
            WindowManager.LayoutParams.LAYOUT_IN_DISPLAY_CUTOUT_MODE_SHORT_EDGES
        }
        window.attributes = attributes
        val (lightStatus, lightNavigation) = appearance ?: (!dark to !dark)
        applyAppearance(lightStatus, lightNavigation)
    }

    /** Dark icons over light system bars (true) or light ones over dark: the app's from now on. */
    @Suppress("unused") // (the app's)
    fun setSystemBarsAppearance(lightStatusBars: Boolean, lightNavigationBars: Boolean) {
        appearance = lightStatusBars to lightNavigationBars
        applyAppearance(lightStatusBars, lightNavigationBars)
    }

    @Suppress("DEPRECATION")
    private fun applyAppearance(lightStatus: Boolean, lightNavigation: Boolean) {
        val window = activity.window
        if (Build.VERSION.SDK_INT >= 30) {
            val controller = window.insetsController ?: return
            val status = WindowInsetsController.APPEARANCE_LIGHT_STATUS_BARS
            val navigation = WindowInsetsController.APPEARANCE_LIGHT_NAVIGATION_BARS
            controller.setSystemBarsAppearance(
                (if (lightStatus) status else 0) or (if (lightNavigation) navigation else 0),
                status or navigation,
            )
        } else {
            val decor = window.decorView
            var flags = decor.systemUiVisibility and
                (View.SYSTEM_UI_FLAG_LIGHT_STATUS_BAR or View.SYSTEM_UI_FLAG_LIGHT_NAVIGATION_BAR).inv()
            if (lightStatus) flags = flags or View.SYSTEM_UI_FLAG_LIGHT_STATUS_BAR
            if (lightNavigation) flags = flags or View.SYSTEM_UI_FLAG_LIGHT_NAVIGATION_BAR
            decor.systemUiVisibility = flags
        }
    }

    // ---- The insets told ----

    /** The last step of the animations under way (the insets of the kinds that move, where they are now). */
    private var progress: WindowInsets? = null

    /**
     * The view's insets as they are now, told to the app: the kinds that move where their animation has them (the
     * window says where they end as soon as they start: as Compose's, they stay where they were until they move).
     */
    private fun tell(insets: WindowInsets) {
        val ints = ArrayList<Int>(160)
        val floats = ArrayList<Float>()
        if (Build.VERSION.SDK_INT >= 30) Api30.kinds(insets, running, progress, ints) else kinds29(insets, ints)
        // The keyboard sliding: from where to where.
        val ime = if (Build.VERSION.SDK_INT >= 30) running.firstOrNull { it.types and Api30.ime != 0 } else null
        ints.add(if (ime != null) 1 else 0)
        addInsets(ints, ime?.before?.let { Api30.insets(it, Api30.ime) })
        addInsets(ints, ime?.after?.let { Api30.insets(it, Api30.ime) })
        // The cutouts' bounds.
        val cutout = insets.displayCutout
        val rects = cutout?.boundingRects ?: emptyList()
        ints.add(rects.size)
        for (r in rects) {
            ints.add(r.left); ints.add(r.top); ints.add(r.right); ints.add(r.bottom)
        }
        // The animations under way, each kind of each.
        if (Build.VERSION.SDK_INT >= 30) Api30.animations(running, ints, floats) else ints.add(0)
        // The cutouts' shape (Android 14 iterates a Path).
        if (Build.VERSION.SDK_INT >= 34 && cutout != null) Api34.cutoutShape(cutout, floats)
        nativeInsets(view, ints.toIntArray(), floats.toFloatArray())
    }

    /** Android 10's insets: the stable ones are the bars', what the window's go beyond them the keyboard's. */
    @Suppress("DEPRECATION")
    private fun kinds29(insets: WindowInsets, ints: ArrayList<Int>) {
        val imeBottom = (insets.systemWindowInsetBottom - insets.stableInsetBottom).coerceAtLeast(0)
        val cutout = insets.displayCutout
        val kinds = arrayOf(
            Insets.of(0, insets.stableInsetTop, 0, 0), // status bars
            Insets.of(insets.stableInsetLeft, 0, insets.stableInsetRight, insets.stableInsetBottom), // navigation
            Insets.NONE, // caption bar
            Insets.of(0, 0, 0, imeBottom), // the keyboard
            cutout?.let { Insets.of(it.safeInsetLeft, it.safeInsetTop, it.safeInsetRight, it.safeInsetBottom) }
                ?: Insets.NONE,
            insets.systemGestureInsets,
            insets.mandatorySystemGestureInsets,
            insets.tappableElementInsets,
            Insets.NONE, // waterfall (Android 11)
        )
        for ((k, i) in kinds.withIndex()) {
            addInsets(ints, i)
            ints.add(if (k == IME) (if (imeBottom > 0) 1 else 0) else 1)
            ints.add(0)
            addInsets(ints, null)
        }
    }

    /** Android 11's insets per type, and what moves them. */
    @TargetApi(30)
    private object Api30 {
        val ime get() = WindowInsets.Type.ime()

        /** The types in the app's order (InsetsKind's); the waterfall is the cutout's. */
        private val types
            get() = intArrayOf(
                WindowInsets.Type.statusBars(),
                WindowInsets.Type.navigationBars(),
                WindowInsets.Type.captionBar(),
                WindowInsets.Type.ime(),
                WindowInsets.Type.displayCutout(),
                WindowInsets.Type.systemGestures(),
                WindowInsets.Type.mandatorySystemGestures(),
                WindowInsets.Type.tappableElement(),
            )

        fun insets(insets: WindowInsets, type: Int): Insets = insets.getInsets(type)

        fun kinds(insets: WindowInsets, running: List<Running>, progress: WindowInsets?, ints: ArrayList<Int>) {
            for ((k, type) in types.withIndex()) {
                val moving = running.lastOrNull { it.types and type != 0 }
                val now = if (moving != null && insets !== progress) progress ?: moving.before ?: insets else insets
                addInsets(ints, now.getInsets(type))
                // (Whether it is shown: where it is going while it moves, as Compose's `isVisible`.)
                val shown = moving?.after?.isVisible(type) ?: insets.isVisible(type)
                ints.add(if (shown) 1 else 0)
                if (k == IME) {
                    ints.add(0)
                    addInsets(ints, null)
                } else {
                    ints.add(1)
                    addInsets(ints, insets.getInsetsIgnoringVisibility(type))
                }
            }
            val waterfall = insets.displayCutout?.waterfallInsets ?: Insets.NONE
            addInsets(ints, waterfall)
            ints.add(if (waterfall != Insets.NONE) 1 else 0)
            ints.add(0)
            addInsets(ints, null)
        }

        fun animations(running: List<Running>, ints: ArrayList<Int>, floats: ArrayList<Float>) {
            val entries = ArrayList<IntArray>()
            for (r in running) {
                val animation = r.animation as WindowInsetsAnimation
                for ((k, type) in types.withIndex()) {
                    if (r.types and type == 0) continue
                    val from = r.before?.getInsets(type) ?: Insets.NONE
                    val to = r.after?.getInsets(type) ?: from
                    entries += intArrayOf(
                        k, from.left, from.top, from.right, from.bottom, to.left, to.top, to.right, to.bottom,
                        animation.durationMillis.toInt(),
                    )
                    floats.add(animation.interpolatedFraction)
                    floats.add(animation.alpha)
                }
            }
            ints.add(entries.size)
            for (e in entries) for (v in e) ints.add(v)
        }
    }

    /** The cutouts' shape as segments (the app's: a verb, the points after the start, a conic's weight). */
    @TargetApi(34)
    private object Api34 {
        fun cutoutShape(cutout: android.view.DisplayCutout, floats: ArrayList<Float>) {
            val path = cutout.cutoutPath ?: return
            val iterator = path.pathIterator
            val points = FloatArray(8)
            while (iterator.hasNext()) {
                val verb = iterator.next(points, 0)
                val segment = FloatArray(8)
                when (verb) {
                    PathIterator.VERB_MOVE -> { segment[0] = 0f; segment[1] = points[0]; segment[2] = points[1] }
                    PathIterator.VERB_LINE -> { segment[0] = 1f; segment[1] = points[2]; segment[2] = points[3] }
                    PathIterator.VERB_QUAD -> { segment[0] = 2f; points.copyInto(segment, 1, 2, 6) }
                    PathIterator.VERB_CONIC -> { segment[0] = 3f; points.copyInto(segment, 1, 2, 6); segment[7] = points[6] }
                    PathIterator.VERB_CUBIC -> { segment[0] = 4f; points.copyInto(segment, 1, 2, 8) }
                    PathIterator.VERB_CLOSE -> segment[0] = 5f
                    else -> break
                }
                for (v in segment) floats.add(v)
            }
        }
    }

    // ---- What moves the insets (Android 11) ----

    @TargetApi(30)
    private object Animations {
        fun attach(insets: KompozInsets) {
            insets.view.setWindowInsetsAnimationCallback(object :
                WindowInsetsAnimation.Callback(DISPATCH_MODE_CONTINUE_ON_SUBTREE) {
                // Before the window takes its new insets: what they are, the start of each kind that moves.
                override fun onPrepare(animation: WindowInsetsAnimation) {
                    insets.running += Running(animation, animation.typeMask, insets.view.rootWindowInsets)
                }

                // The new insets are the window's now: where each kind ends.
                override fun onStart(
                    animation: WindowInsetsAnimation,
                    bounds: WindowInsetsAnimation.Bounds,
                ): WindowInsetsAnimation.Bounds {
                    insets.running.firstOrNull { it.animation === animation }?.after = insets.view.rootWindowInsets
                    insets.view.rootWindowInsets?.let { insets.tell(it) }
                    return bounds
                }

                override fun onProgress(
                    now: WindowInsets,
                    runningAnimations: MutableList<WindowInsetsAnimation>,
                ): WindowInsets {
                    insets.progress = now
                    insets.tell(now)
                    return now
                }

                override fun onEnd(animation: WindowInsetsAnimation) {
                    insets.running.removeAll { it.animation === animation }
                    if (insets.running.isEmpty()) insets.progress = null
                    insets.view.rootWindowInsets?.let { insets.tell(it) }
                }
            })
        }
    }

    // ---- The keyboard moved by hand (Android 11) ----
    // Asked for on the first move; moves before the control comes are kept, as is a let go.

    private var control: Any? = null // WindowInsetsAnimationController, once it is ready
    private var controlAsked = false
    private var controlPending = 0f
    private var endPending: Float? = null

    /** Whether the keyboard can be moved by hand here. */
    @Suppress("unused") // (the app's)
    fun canControlIme(): Boolean = Build.VERSION.SDK_INT >= 30

    /** The keyboard moved by `px` up (down if negative). */
    @Suppress("unused") // (the app's)
    fun imeControlBy(px: Float) {
        if (Build.VERSION.SDK_INT < 30) return
        if (control != null) Control.move(this, px) else {
            controlPending += px
            Control.ask(this)
        }
    }

    /** Let go with `velocity` (px/s, up positive): finished shown or hidden. */
    @Suppress("unused") // (the app's)
    fun imeControlEnd(velocity: Float) {
        if (Build.VERSION.SDK_INT < 30) return
        if (control != null) Control.finish(this, velocity)
        else if (controlAsked) endPending = velocity
        else Control.showOrHide(this, velocity)
    }

    @TargetApi(30)
    private object Control {
        fun ask(k: KompozInsets) {
            if (k.controlAsked) return
            val controller = k.view.windowInsetsController ?: return
            k.controlAsked = true
            controller.controlWindowInsetsAnimation(
                WindowInsets.Type.ime(), -1, LinearInterpolator(), CancellationSignal(),
                object : WindowInsetsAnimationControlListener {
                    override fun onReady(c: WindowInsetsAnimationController, types: Int) {
                        k.control = c
                        move(k, k.controlPending)
                        k.controlPending = 0f
                        k.endPending?.let { finish(k, it) }
                        k.endPending = null
                    }

                    override fun onFinished(c: WindowInsetsAnimationController) = reset(k)

                    override fun onCancelled(c: WindowInsetsAnimationController?) = reset(k)
                },
            )
        }

        private fun reset(k: KompozInsets) {
            k.control = null
            k.controlAsked = false
            k.controlPending = 0f
            k.endPending = null
        }

        fun move(k: KompozInsets, px: Float) {
            val c = k.control as? WindowInsetsAnimationController ?: return
            if (c.isFinished || c.isCancelled) return
            val hidden = c.hiddenStateInsets.bottom
            val shown = c.shownStateInsets.bottom
            val next = (c.currentInsets.bottom + Math.round(px)).coerceIn(hidden, maxOf(hidden, shown))
            val fraction = if (shown > hidden) (next - hidden) / (shown - hidden).toFloat() else 1f
            c.setInsetsAndAlpha(Insets.of(0, 0, 0, next), 1f, fraction)
        }

        /** Up or down by the fling (600 dp/s, as C++'s), else whichever it is nearer. */
        fun finish(k: KompozInsets, velocity: Float) {
            val c = k.control as? WindowInsetsAnimationController ?: return
            if (c.isFinished || c.isCancelled) return
            val hidden = c.hiddenStateInsets.bottom
            val shown = c.shownStateInsets.bottom
            val now = c.currentInsets.bottom
            c.finish(shows(k, velocity, now - hidden > (shown - hidden) / 2))
        }

        /** The control never came (the keyboard isn't there to move): shown or hidden by the fling. */
        fun showOrHide(k: KompozInsets, velocity: Float) {
            val controller = k.view.windowInsetsController ?: return
            val visible = k.view.rootWindowInsets?.isVisible(WindowInsets.Type.ime()) ?: false
            if (shows(k, velocity, visible)) controller.show(WindowInsets.Type.ime())
            else controller.hide(WindowInsets.Type.ime())
        }

        private fun shows(k: KompozInsets, velocity: Float, nearer: Boolean): Boolean {
            val fast = 600 * k.view.resources.displayMetrics.density
            return velocity > fast || (velocity >= -fast && nearer)
        }
    }

    private external fun nativeInsets(view: KompozView, ints: IntArray, floats: FloatArray)

    companion object {
        /** The keyboard's place among the kinds. */
        private const val IME = 3

        private fun addInsets(ints: ArrayList<Int>, i: Insets?) {
            ints.add(i?.left ?: 0); ints.add(i?.top ?: 0); ints.add(i?.right ?: 0); ints.add(i?.bottom ?: 0)
        }
    }
}
