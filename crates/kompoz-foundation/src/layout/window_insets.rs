//! Window insets, as Compose's `WindowInsets`: how far the system's UI (bars, the keyboard, a display cutout,
//! gesture edges) reaches into the window on each side, the kinds the platform reports, and their arithmetic.

use super::padding::PaddingValues;
use kompoz_graphics::{Density, Dp, LayoutDirection, UnitExt};
use kompoz_runtime::{MutableState, mutable_state_of, named};
use kompoz_ui::platform::{Insets, LocalDensity, LocalSystemInsets, SystemInsets};
use std::cell::RefCell;
use std::fmt;
use std::ops::Add;
use std::rc::Rc;

/// How far something reaches into the window on each side, in pixels, as worked out for a density and a layout
/// direction (Compose's WindowInsets): fixed sides (in px or dp), one of the kinds the platform reports
/// ([`status_bars`](WindowInsets::status_bars) and the others), one worked out from others (`union`, `exclude`,
/// `add`, `only`), padding values, a [`MutableWindowInsets`], or what a [`CustomWindowInsets`] works out. Equal
/// when made the same way of equal parts (a kind: of equal values; a mutable or custom one: when it is the same
/// one).
#[derive(Clone, PartialEq)]
pub struct WindowInsets {
    repr: Repr,
}

#[derive(Clone)]
enum Repr {
    Fixed { left: i32, top: i32, right: i32, bottom: i32 },
    FixedDp { left: Dp, top: Dp, right: Dp, bottom: Dp },
    // A kind the platform reports (Compose's ValueInsets), by its name.
    Value { name: &'static str, insets: Insets },
    Union(Rc<WindowInsets>, Rc<WindowInsets>),
    Exclude(Rc<WindowInsets>, Rc<WindowInsets>),
    Added(Rc<WindowInsets>, Rc<WindowInsets>),
    Limit(Rc<WindowInsets>, WindowInsetsSides),
    PaddingValues(PaddingValues),
    Mutable(MutableWindowInsets),
    Custom(Rc<dyn CustomWindowInsets>),
}

/// Insets of one's own (Compose's `WindowInsets` interface implemented): its sides in pixels, worked out when
/// asked.
pub trait CustomWindowInsets {
    fn get_left(&self, density: &Density, layout_direction: LayoutDirection) -> i32;
    fn get_top(&self, density: &Density) -> i32;
    fn get_right(&self, density: &Density, layout_direction: LayoutDirection) -> i32;
    fn get_bottom(&self, density: &Density) -> i32;
}

impl WindowInsets {
    const fn of(repr: Repr) -> WindowInsets {
        WindowInsets { repr }
    }

    /// None on any side (Compose's `WindowInsets()`).
    pub const Zero: WindowInsets = WindowInsets::of(Repr::Fixed { left: 0, top: 0, right: 0, bottom: 0 });

    /// Each side its own, in pixels (Compose's `WindowInsets(left, top, right, bottom)` of Ints).
    pub const fn new(left: i32, top: i32, right: i32, bottom: i32) -> WindowInsets {
        WindowInsets::of(Repr::Fixed { left, top, right, bottom })
    }

    /// Each side its own, in dp (Compose's `WindowInsets(left, top, right, bottom)` of Dps); in a kompozable body,
    /// [`WindowInsets(left = …)`](fn@WindowInsets) names them.
    pub const fn of_dp(left: Dp, top: Dp, right: Dp, bottom: Dp) -> WindowInsets {
        WindowInsets::of(Repr::FixedDp { left, top, right, bottom })
    }

    /// Insets `custom` works out.
    pub fn custom(custom: impl CustomWindowInsets + 'static) -> WindowInsets {
        WindowInsets::of(Repr::Custom(Rc::new(custom)))
    }

    /// The space at the left, in pixels (Compose's `getLeft`).
    pub fn get_left(&self, density: &Density, layout_direction: LayoutDirection) -> i32 {
        match &self.repr {
            Repr::Fixed { left, .. } => *left,
            Repr::FixedDp { left, .. } => density.round_to_px(*left),
            Repr::Value { insets, .. } => insets.left,
            Repr::Union(a, b) => a.get_left(density, layout_direction).max(b.get_left(density, layout_direction)),
            Repr::Exclude(a, b) => {
                (a.get_left(density, layout_direction) - b.get_left(density, layout_direction)).max(0)
            }
            Repr::Added(a, b) => a.get_left(density, layout_direction) + b.get_left(density, layout_direction),
            Repr::Limit(insets, sides) => {
                let side = if layout_direction == LayoutDirection::Ltr {
                    WindowInsetsSides::AllowLeftInLtr
                } else {
                    WindowInsetsSides::AllowLeftInRtl
                };
                if sides.has_any(side) { insets.get_left(density, layout_direction) } else { 0 }
            }
            Repr::PaddingValues(p) => density.round_to_px(p.calculate_left_padding(layout_direction)),
            Repr::Mutable(m) => m.insets().get_left(density, layout_direction),
            Repr::Custom(c) => c.get_left(density, layout_direction),
        }
    }

    /// The space at the top, in pixels (Compose's `getTop`).
    pub fn get_top(&self, density: &Density) -> i32 {
        match &self.repr {
            Repr::Fixed { top, .. } => *top,
            Repr::FixedDp { top, .. } => density.round_to_px(*top),
            Repr::Value { insets, .. } => insets.top,
            Repr::Union(a, b) => a.get_top(density).max(b.get_top(density)),
            Repr::Exclude(a, b) => (a.get_top(density) - b.get_top(density)).max(0),
            Repr::Added(a, b) => a.get_top(density) + b.get_top(density),
            Repr::Limit(insets, sides) => {
                if sides.has_any(WindowInsetsSides::Top) {
                    insets.get_top(density)
                } else {
                    0
                }
            }
            Repr::PaddingValues(p) => density.round_to_px(p.calculate_top_padding()),
            Repr::Mutable(m) => m.insets().get_top(density),
            Repr::Custom(c) => c.get_top(density),
        }
    }

    /// The space at the right, in pixels (Compose's `getRight`).
    pub fn get_right(&self, density: &Density, layout_direction: LayoutDirection) -> i32 {
        match &self.repr {
            Repr::Fixed { right, .. } => *right,
            Repr::FixedDp { right, .. } => density.round_to_px(*right),
            Repr::Value { insets, .. } => insets.right,
            Repr::Union(a, b) => a.get_right(density, layout_direction).max(b.get_right(density, layout_direction)),
            Repr::Exclude(a, b) => {
                (a.get_right(density, layout_direction) - b.get_right(density, layout_direction)).max(0)
            }
            Repr::Added(a, b) => a.get_right(density, layout_direction) + b.get_right(density, layout_direction),
            Repr::Limit(insets, sides) => {
                let side = if layout_direction == LayoutDirection::Ltr {
                    WindowInsetsSides::AllowRightInLtr
                } else {
                    WindowInsetsSides::AllowRightInRtl
                };
                if sides.has_any(side) { insets.get_right(density, layout_direction) } else { 0 }
            }
            Repr::PaddingValues(p) => density.round_to_px(p.calculate_right_padding(layout_direction)),
            Repr::Mutable(m) => m.insets().get_right(density, layout_direction),
            Repr::Custom(c) => c.get_right(density, layout_direction),
        }
    }

    /// The space at the bottom, in pixels (Compose's `getBottom`).
    pub fn get_bottom(&self, density: &Density) -> i32 {
        match &self.repr {
            Repr::Fixed { bottom, .. } => *bottom,
            Repr::FixedDp { bottom, .. } => density.round_to_px(*bottom),
            Repr::Value { insets, .. } => insets.bottom,
            Repr::Union(a, b) => a.get_bottom(density).max(b.get_bottom(density)),
            Repr::Exclude(a, b) => (a.get_bottom(density) - b.get_bottom(density)).max(0),
            Repr::Added(a, b) => a.get_bottom(density) + b.get_bottom(density),
            Repr::Limit(insets, sides) => {
                if sides.has_any(WindowInsetsSides::Bottom) {
                    insets.get_bottom(density)
                } else {
                    0
                }
            }
            Repr::PaddingValues(p) => density.round_to_px(p.calculate_bottom_padding()),
            Repr::Mutable(m) => m.insets().get_bottom(density),
            Repr::Custom(c) => c.get_bottom(density),
        }
    }

    /// The larger of these and `insets` on each side (Compose's `union`).
    pub fn union(&self, insets: &WindowInsets) -> WindowInsets {
        WindowInsets::of(Repr::Union(Rc::new(self.clone()), Rc::new(insets.clone())))
    }

    /// What is left of these past `insets` on each side, at least zero (Compose's `exclude`): what an inner
    /// layout still has to keep clear of when an outer one took `insets`.
    pub fn exclude(&self, insets: &WindowInsets) -> WindowInsets {
        WindowInsets::of(Repr::Exclude(Rc::new(self.clone()), Rc::new(insets.clone())))
    }

    /// These and `insets` added side by side (Compose's `add`; also `+`).
    pub fn add(&self, insets: &WindowInsets) -> WindowInsets {
        WindowInsets::of(Repr::Added(Rc::new(self.clone()), Rc::new(insets.clone())))
    }

    /// These on `sides` only, zero on the others (Compose's `only`).
    pub fn only(&self, sides: WindowInsetsSides) -> WindowInsets {
        WindowInsets::of(Repr::Limit(Rc::new(self.clone()), sides))
    }

    /// As padding values in the density composed in (Compose's `asPaddingValues()`, read in a kompozable body;
    /// elsewhere the default density).
    pub fn as_padding_values(&self) -> PaddingValues {
        PaddingValues::of_insets(self.clone(), LocalDensity.current())
    }

    /// As padding values in `density` (Compose's `asPaddingValues(density)`).
    pub fn as_padding_values_with(&self, density: Density) -> PaddingValues {
        PaddingValues::of_insets(self.clone(), density)
    }

    /// Padding values as insets (Compose's `PaddingValues.asInsets`).
    pub(crate) fn of_padding_values(padding_values: PaddingValues) -> WindowInsets {
        WindowInsets::of(Repr::PaddingValues(padding_values))
    }

    /// A kind the platform reports, named.
    pub(crate) fn value(name: &'static str, insets: Insets) -> WindowInsets {
        WindowInsets::of(Repr::Value { name, insets })
    }
}

/// Compose's `WindowInsets(left = 0.dp, top = 0.dp, right = 0.dp, bottom = 0.dp)`, with named arguments in a
/// kompozable body (`WindowInsets(top = 24.dp())`); plain code calls [`WindowInsets::of_dp`] (or
/// [`WindowInsets::new`] for pixels).
#[named]
pub fn WindowInsets(
    #[default(0.dp())] left: Dp,
    #[default(0.dp())] top: Dp,
    #[default(0.dp())] right: Dp,
    #[default(0.dp())] bottom: Dp,
) -> WindowInsets {
    WindowInsets::of_dp(left, top, right, bottom)
}

/// Side by side (Compose's `add`).
impl Add for WindowInsets {
    type Output = WindowInsets;
    fn add(self, other: WindowInsets) -> WindowInsets {
        WindowInsets::add(&self, &other)
    }
}

impl Default for WindowInsets {
    fn default() -> Self {
        WindowInsets::Zero
    }
}

impl PartialEq for Repr {
    fn eq(&self, other: &Self) -> bool {
        let same = |a: [Dp; 4], b: [Dp; 4]| a.iter().zip(&b).all(|(x, y)| super::same_dp(*x, *y));
        match (self, other) {
            (Repr::Fixed { left, top, right, bottom }, Repr::Fixed { left: l, top: t, right: r, bottom: b }) => {
                (left, top, right, bottom) == (l, t, r, b)
            }
            (Repr::FixedDp { left, top, right, bottom }, Repr::FixedDp { left: l, top: t, right: r, bottom: b }) => {
                same([*left, *top, *right, *bottom], [*l, *t, *r, *b])
            }
            // (Compose's ValueInsets: equal values, whatever the name)
            (Repr::Value { insets: a, .. }, Repr::Value { insets: b, .. }) => a == b,
            (Repr::Union(a, b), Repr::Union(c, d))
            | (Repr::Exclude(a, b), Repr::Exclude(c, d))
            | (Repr::Added(a, b), Repr::Added(c, d)) => a == c && b == d,
            (Repr::Limit(a, s), Repr::Limit(b, t)) => a == b && s == t,
            (Repr::PaddingValues(a), Repr::PaddingValues(b)) => a == b,
            (Repr::Mutable(a), Repr::Mutable(b)) => a == b,
            (Repr::Custom(a), Repr::Custom(b)) => Rc::ptr_eq(a, b),
            _ => false,
        }
    }
}

impl fmt::Debug for WindowInsets {
    /// Compose's toString.
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match &self.repr {
            Repr::Fixed { left, top, right, bottom } => {
                write!(f, "Insets(left={left}, top={top}, right={right}, bottom={bottom})")
            }
            Repr::FixedDp { left, top, right, bottom } => {
                write!(f, "Insets(left={left:?}, top={top:?}, right={right:?}, bottom={bottom:?})")
            }
            Repr::Value { name, insets } => write!(
                f,
                "{name}(left={}, top={}, right={}, bottom={})",
                insets.left, insets.top, insets.right, insets.bottom
            ),
            Repr::Union(a, b) => write!(f, "({a:?} ∪ {b:?})"),
            Repr::Exclude(a, b) => write!(f, "({a:?} - {b:?})"),
            Repr::Added(a, b) => write!(f, "({a:?} + {b:?})"),
            Repr::Limit(insets, sides) => write!(f, "({insets:?} only {sides:?})"),
            Repr::PaddingValues(p) => {
                let ltr = LayoutDirection::Ltr;
                write!(
                    f,
                    "PaddingValues({:?}, {:?}, {:?}, {:?})",
                    p.calculate_left_padding(ltr),
                    p.calculate_top_padding(),
                    p.calculate_right_padding(ltr),
                    p.calculate_bottom_padding()
                )
            }
            Repr::Mutable(m) => write!(f, "MutableWindowInsets({:?})", m.insets()),
            Repr::Custom(_) => write!(f, "WindowInsets(custom)"),
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The kinds

/// The larger of each side.
fn side_max(a: Insets, b: Insets) -> Insets {
    Insets::new(a.left.max(b.left), a.top.max(b.top), a.right.max(b.right), a.bottom.max(b.bottom))
}

/// The kinds of insets the platform reports (Compose's `WindowInsets.Companion` properties, Android's
/// WindowInsetsCompat types), read from `LocalSystemInsets` (what `KompozHost::set_system_insets` sets): read
/// them in a kompozable body, which composes again when the platform reports others (elsewhere they are what
/// nothing provides: none).
impl WindowInsets {
    fn system() -> SystemInsets {
        LocalSystemInsets.current()
    }

    /// The status bars (Compose's `statusBars`).
    pub fn status_bars() -> WindowInsets {
        WindowInsets::value("statusBars", Self::system().status_bars)
    }

    /// The navigation bars, which interactive UI keeps clear of (Compose's `navigationBars`).
    pub fn navigation_bars() -> WindowInsets {
        WindowInsets::value("navigationBars", Self::system().navigation_bars)
    }

    /// A freeform window's title bar (Compose's `captionBar`).
    pub fn caption_bar() -> WindowInsets {
        WindowInsets::value("captionBar", Self::system().caption_bar)
    }

    /// The status bars, the navigation bars and the caption bar (Compose's `systemBars`).
    pub fn system_bars() -> WindowInsets {
        let s = Self::system();
        WindowInsets::value("systemBars", side_max(side_max(s.status_bars, s.navigation_bars), s.caption_bar))
    }

    /// The software keyboard, where it is as it slides in or out (Compose's `ime`).
    pub fn ime() -> WindowInsets {
        WindowInsets::value("ime", Self::system().ime)
    }

    /// Where the keyboard was when it started sliding (Compose's `imeAnimationSource`): where it is when it
    /// isn't moving.
    pub fn ime_animation_source() -> WindowInsets {
        let s = Self::system();
        WindowInsets::value("imeAnimationSource", if s.ime_animating { s.ime_animation_source } else { s.ime })
    }

    /// Where the keyboard is sliding to (Compose's `imeAnimationTarget`): where it is when it isn't moving, so a
    /// layout can go straight to where it ends up.
    pub fn ime_animation_target() -> WindowInsets {
        let s = Self::system();
        WindowInsets::value("imeAnimationTarget", if s.ime_animating { s.ime_animation_target } else { s.ime })
    }

    /// Whether the keyboard is up or on its way up, not on its way down (Compose's `isImeVisible`).
    pub fn is_ime_visible() -> bool {
        Self::system().ime_visible
    }

    /// Where a display cutout (a camera's) is, which important content keeps clear of (Compose's
    /// `displayCutout`).
    pub fn display_cutout() -> WindowInsets {
        WindowInsets::value("displayCutout", Self::system().display_cutout)
    }

    /// The edges where swipes go to the system: back, home (Compose's `systemGestures`).
    pub fn system_gestures() -> WindowInsets {
        WindowInsets::value("systemGestures", Self::system().system_gestures)
    }

    /// The edges where the system's gestures always win, whatever the app excludes (Compose's
    /// `mandatorySystemGestures`).
    pub fn mandatory_system_gestures() -> WindowInsets {
        WindowInsets::value("mandatorySystemGestures", Self::system().mandatory_system_gestures)
    }

    /// What a tap reaches the system through: a taskbar, a three-button navigation bar (Compose's
    /// `tappableElement`).
    pub fn tappable_element() -> WindowInsets {
        WindowInsets::value("tappableElement", Self::system().tappable_element)
    }

    /// The curved edges of a waterfall display (Compose's `waterfall`).
    pub fn waterfall() -> WindowInsets {
        WindowInsets::value("waterfall", Self::system().waterfall)
    }

    /// What content may be drawn over by: the system bars, the keyboard and the display cutout (Compose's
    /// `safeDrawing`).
    pub fn safe_drawing() -> WindowInsets {
        Self::system_bars().union(&Self::ime()).union(&Self::display_cutout())
    }

    /// Where gestures may be taken for the system's: the tappable elements, the mandatory and other system
    /// gestures, the waterfall (Compose's `safeGestures`).
    pub fn safe_gestures() -> WindowInsets {
        Self::tappable_element()
            .union(&Self::mandatory_system_gestures())
            .union(&Self::system_gestures())
            .union(&Self::waterfall())
    }

    /// Everything drawn over or gestured on: `safe_drawing` and `safe_gestures` (Compose's `safeContent`), what
    /// touch targets keep clear of.
    pub fn safe_content() -> WindowInsets {
        Self::safe_drawing().union(&Self::safe_gestures())
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Sides

/// A set of sides of insets (Compose's WindowInsetsSides): `Start` and `End` follow the layout direction, `Left`
/// and `Right` don't; combined with `+`.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct WindowInsetsSides(u8);

impl WindowInsetsSides {
    //     _---- allowLeft  in ltr
    //    /
    //    | _--- allowRight in ltr
    //    |/
    //    || _-- allowLeft  in rtl
    //    ||/
    //    ||| _- allowRight in rtl
    //    |||/
    //    VVVV
    //    Left   = 1010
    //    Right  = 0101
    //    Start  = 1001
    //    End    = 0110
    const AllowLeftInLtr: WindowInsetsSides = WindowInsetsSides(1 << 3);
    const AllowRightInLtr: WindowInsetsSides = WindowInsetsSides(1 << 2);
    const AllowLeftInRtl: WindowInsetsSides = WindowInsetsSides(1 << 1);
    const AllowRightInRtl: WindowInsetsSides = WindowInsetsSides(1 << 0);

    /// The start: the left left to right, the right right to left.
    pub const Start: WindowInsetsSides = WindowInsetsSides(Self::AllowLeftInLtr.0 | Self::AllowRightInRtl.0);
    /// The end: the right left to right, the left right to left.
    pub const End: WindowInsetsSides = WindowInsetsSides(Self::AllowRightInLtr.0 | Self::AllowLeftInRtl.0);
    pub const Top: WindowInsetsSides = WindowInsetsSides(1 << 4);
    pub const Bottom: WindowInsetsSides = WindowInsetsSides(1 << 5);
    /// The left, whatever the layout direction.
    pub const Left: WindowInsetsSides = WindowInsetsSides(Self::AllowLeftInLtr.0 | Self::AllowLeftInRtl.0);
    /// The right, whatever the layout direction.
    pub const Right: WindowInsetsSides = WindowInsetsSides(Self::AllowRightInLtr.0 | Self::AllowRightInRtl.0);
    /// The left and the right (or the start and the end).
    pub const Horizontal: WindowInsetsSides = WindowInsetsSides(Self::Left.0 | Self::Right.0);
    /// The top and the bottom.
    pub const Vertical: WindowInsetsSides = WindowInsetsSides(Self::Top.0 | Self::Bottom.0);

    /// Whether it has any of `sides`.
    fn has_any(self, sides: WindowInsetsSides) -> bool {
        self.0 & sides.0 != 0
    }
}

/// These sides and `sides` (Compose's `plus`).
impl Add for WindowInsetsSides {
    type Output = WindowInsetsSides;
    // (a union of bits: adding sides is or-ing them)
    #[allow(clippy::suspicious_arithmetic_impl)]
    fn add(self, sides: WindowInsetsSides) -> WindowInsetsSides {
        WindowInsetsSides(self.0 | sides.0)
    }
}

impl fmt::Debug for WindowInsetsSides {
    /// Compose's toString: `WindowInsetsSides(Start+Top)`.
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut names = Vec::new();
        for (sides, name) in [
            (Self::Start, "Start"),
            (Self::Left, "Left"),
            (Self::Top, "Top"),
            (Self::End, "End"),
            (Self::Right, "Right"),
            (Self::Bottom, "Bottom"),
        ] {
            if self.0 & sides.0 == sides.0 {
                names.push(name);
            }
        }
        write!(f, "WindowInsetsSides({})", names.join("+"))
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Mutable insets

/// Insets the app sets as it likes (Compose's MutableWindowInsets): what measures with them is measured again
/// when they are set. Equal only to itself (its clones).
#[derive(Clone)]
pub struct MutableWindowInsets {
    /// Read to subscribe, counted up on each change (insets aren't snapshot values).
    version: MutableState<u64>,
    insets: Rc<RefCell<WindowInsets>>,
}

impl MutableWindowInsets {
    /// Holding `initial_insets` (Compose's default: none).
    pub fn new(initial_insets: WindowInsets) -> MutableWindowInsets {
        MutableWindowInsets { version: mutable_state_of(0), insets: Rc::new(RefCell::new(initial_insets)) }
    }

    /// The insets it holds now (reading them subscribes the reader).
    pub fn insets(&self) -> WindowInsets {
        self.version.get();
        self.insets.borrow().clone()
    }

    /// Holds `insets` from now on: its readers are told.
    pub fn set_insets(&self, insets: WindowInsets) {
        if *self.insets.borrow() == insets {
            return;
        }
        *self.insets.borrow_mut() = insets;
        self.version.set(self.version.peek() + 1);
    }

    /// As window insets, to pad or consume by.
    pub fn as_window_insets(&self) -> WindowInsets {
        WindowInsets::of(Repr::Mutable(self.clone()))
    }
}

impl Default for MutableWindowInsets {
    fn default() -> Self {
        MutableWindowInsets::new(WindowInsets::Zero)
    }
}

impl PartialEq for MutableWindowInsets {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.insets, &other.insets)
    }
}

impl From<MutableWindowInsets> for WindowInsets {
    fn from(m: MutableWindowInsets) -> WindowInsets {
        m.as_window_insets()
    }
}
