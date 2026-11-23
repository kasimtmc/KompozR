//! The window's size classes, as androidx.window's window-core 1.5.0 (`androidx.window.core.layout`):
//! [`WindowSizeClass`] (the breakpoints a window is at least: width 600/840/1200/1600 dp, height 480/900 dp), the
//! recommended sets of them ([`WindowSizeClass::BREAKPOINTS_V1`], [`WindowSizeClass::BREAKPOINTS_V2`]) and the
//! selectors choosing a set's class for a size ([`WindowSizeClassSelectors`]); the deprecated width and height
//! buckets ([`WindowWidthSizeClass`], [`WindowHeightSizeClass`]).

use kompoz_runtime::kt::KtSet;
use std::fmt;
use std::sync::OnceLock;

/// Breakpoints for a viewport (window-core's `WindowSizeClass`): the lower bounds of its width and height buckets,
/// in dp. Lay out by these, not by exact sizes; check them from the largest breakpoint down
/// ([`is_width_at_least_breakpoint`](Self::is_width_at_least_breakpoint) and the others are order dependent, a
/// smaller class matching every larger breakpoint's check too). Equal by its two bounds.
///
/// Kotlin: `WindowSizeClass(minWidthDp: Int, minHeightDp: Int)`
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct WindowSizeClass {
    min_width_dp: i32,
    min_height_dp: i32,
}

/// Kotlin: `WindowSizeClass(minWidthDp: Int, minHeightDp: Int)`
///
/// Panics when either is negative (Kotlin's `require`).
pub fn WindowSizeClass(min_width_dp: i32, min_height_dp: i32) -> WindowSizeClass {
    WindowSizeClass::new(min_width_dp, min_height_dp)
}

impl WindowSizeClass {
    /// A lower bound for a size class with Medium width in dp.
    pub const WIDTH_DP_MEDIUM_LOWER_BOUND: i32 = 600;
    /// A lower bound for a size class with Expanded width in dp.
    pub const WIDTH_DP_EXPANDED_LOWER_BOUND: i32 = 840;
    /// A lower bound for a size class with Large width in dp.
    pub const WIDTH_DP_LARGE_LOWER_BOUND: i32 = 1200;
    /// A lower bound for a size class width Extra Large width in dp.
    pub const WIDTH_DP_EXTRA_LARGE_LOWER_BOUND: i32 = 1600;
    /// A lower bound for a size class with Medium height in dp.
    pub const HEIGHT_DP_MEDIUM_LOWER_BOUND: i32 = 480;
    /// A lower bound for a size class with Expanded height in dp.
    pub const HEIGHT_DP_EXPANDED_LOWER_BOUND: i32 = 900;

    const WIDTH_DP_BREAKPOINTS_V1: [i32; 3] =
        [0, Self::WIDTH_DP_MEDIUM_LOWER_BOUND, Self::WIDTH_DP_EXPANDED_LOWER_BOUND];
    const WIDTH_DP_BREAKPOINTS_V2: [i32; 5] = [
        0,
        Self::WIDTH_DP_MEDIUM_LOWER_BOUND,
        Self::WIDTH_DP_EXPANDED_LOWER_BOUND,
        Self::WIDTH_DP_LARGE_LOWER_BOUND,
        Self::WIDTH_DP_EXTRA_LARGE_LOWER_BOUND,
    ];
    const HEIGHT_DP_BREAKPOINTS_V1: [i32; 3] =
        [0, Self::HEIGHT_DP_MEDIUM_LOWER_BOUND, Self::HEIGHT_DP_EXPANDED_LOWER_BOUND];
    const HEIGHT_DP_BREAKPOINTS_V2: [i32; 3] = Self::HEIGHT_DP_BREAKPOINTS_V1;

    /// One of these lower bounds. Panics when either is negative (Kotlin's `require`).
    pub fn new(min_width_dp: i32, min_height_dp: i32) -> WindowSizeClass {
        assert!(min_width_dp >= 0, "Expected minWidthDp to be at least 0, minWidthDp: {min_width_dp}.");
        assert!(min_height_dp >= 0, "Expected minHeightDp to be at least 0, minHeightDp: {min_height_dp}.");
        WindowSizeClass { min_width_dp, min_height_dp }
    }

    /// A convenience constructor that truncates to ints (Kotlin's secondary constructor).
    ///
    /// Kotlin: `WindowSizeClass(widthDp: Float, heightDp: Float)`
    pub fn of_float(width_dp: f32, height_dp: f32) -> WindowSizeClass {
        WindowSizeClass::new(width_dp as i32, height_dp as i32)
    }

    /// The lower bound for the width of the size class in dp.
    pub fn min_width_dp(&self) -> i32 {
        self.min_width_dp
    }

    /// The lower bound for the height of the size class in dp.
    pub fn min_height_dp(&self) -> i32 {
        self.min_height_dp
    }

    /// The [`WindowWidthSizeClass`] that corresponds to the width of the window.
    #[deprecated(note = "Use either isWidthAtLeastBreakpoint or isAtLeastBreakpoint to check matching bounds.")]
    #[allow(deprecated)]
    pub fn window_width_size_class(&self) -> WindowWidthSizeClass {
        WindowWidthSizeClass::compute(self.min_width_dp as f32)
    }

    /// The [`WindowHeightSizeClass`] that corresponds to the height of the window.
    #[deprecated(note = "Use either isHeightAtLeastBreakpoint or isAtLeastBreakpoint to check matching bounds.")]
    #[allow(deprecated)]
    pub fn window_height_size_class(&self) -> WindowHeightSizeClass {
        WindowHeightSizeClass::compute(self.min_height_dp as f32)
    }

    /// Whether [`min_width_dp`](Self::min_width_dp) is at least `width_dp_breakpoint`. Order dependent: select from
    /// the largest breakpoint down.
    pub fn is_width_at_least_breakpoint(&self, width_dp_breakpoint: i32) -> bool {
        self.min_width_dp >= width_dp_breakpoint
    }

    /// Whether [`min_height_dp`](Self::min_height_dp) is at least `height_dp_breakpoint`. Order dependent: select
    /// from the largest breakpoint down.
    pub fn is_height_at_least_breakpoint(&self, height_dp_breakpoint: i32) -> bool {
        self.min_height_dp >= height_dp_breakpoint
    }

    /// Whether both bounds are at least the breakpoints. Order dependent: select from the largest breakpoints down.
    pub fn is_at_least_breakpoint(&self, width_dp_breakpoint: i32, height_dp_breakpoint: i32) -> bool {
        self.is_width_at_least_breakpoint(width_dp_breakpoint)
            && self.is_height_at_least_breakpoint(height_dp_breakpoint)
    }

    fn create_breakpoint_set(width_breakpoints: &[i32], height_breakpoints: &[i32]) -> KtSet<WindowSizeClass> {
        let mut set = KtSet::new();
        for &width in width_breakpoints {
            for &height in height_breakpoints {
                set.add(WindowSizeClass::new(width, height));
            }
        }
        set
    }

    /// The recommended breakpoints for window size classes (Compose's `BREAKPOINTS_V1`: width 0, 600, 840 × height
    /// 0, 480, 900).
    pub fn BREAKPOINTS_V1() -> &'static KtSet<WindowSizeClass> {
        static SET: OnceLock<KtSet<WindowSizeClass>> = OnceLock::new();
        SET.get_or_init(|| Self::create_breakpoint_set(&Self::WIDTH_DP_BREAKPOINTS_V1, &Self::HEIGHT_DP_BREAKPOINTS_V1))
    }

    /// The recommended breakpoints including the Large (1200 dp) and Extra Large (1600 dp) widths (Compose's
    /// `BREAKPOINTS_V2`).
    pub fn BREAKPOINTS_V2() -> &'static KtSet<WindowSizeClass> {
        static SET: OnceLock<KtSet<WindowSizeClass>> = OnceLock::new();
        SET.get_or_init(|| Self::create_breakpoint_set(&Self::WIDTH_DP_BREAKPOINTS_V2, &Self::HEIGHT_DP_BREAKPOINTS_V2))
    }

    /// The recommended class for a window of these dp: each bound the largest of [`BREAKPOINTS_V1`](Self::BREAKPOINTS_V1)'s
    /// it reaches. Panics when either is negative (Kotlin's `IllegalArgumentException`).
    #[deprecated(note = "Use computeWindowSizeClass instead.")]
    pub fn compute(dp_width: f32, dp_height: f32) -> WindowSizeClass {
        let width_dp = if dp_width >= Self::WIDTH_DP_EXPANDED_LOWER_BOUND as f32 {
            Self::WIDTH_DP_EXPANDED_LOWER_BOUND
        } else if dp_width >= Self::WIDTH_DP_MEDIUM_LOWER_BOUND as f32 {
            Self::WIDTH_DP_MEDIUM_LOWER_BOUND
        } else {
            0
        };
        let height_dp = if dp_height >= Self::HEIGHT_DP_EXPANDED_LOWER_BOUND as f32 {
            Self::HEIGHT_DP_EXPANDED_LOWER_BOUND
        } else if dp_height >= Self::HEIGHT_DP_MEDIUM_LOWER_BOUND as f32 {
            Self::HEIGHT_DP_MEDIUM_LOWER_BOUND
        } else {
            0
        };
        WindowSizeClass::new(width_dp, height_dp)
    }
}

/// Kotlin's `toString`: `WindowSizeClass(minWidthDp=600, minHeightDp=480)`.
impl fmt::Display for WindowSizeClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "WindowSizeClass(minWidthDp={}, minHeightDp={})", self.min_width_dp, self.min_height_dp)
    }
}

/// Choosing the class of a set for a window's size (window-core's `WindowSizeClassSelectors`, Kotlin's extension
/// functions of `Set<WindowSizeClass>`).
pub trait WindowSizeClassSelectors {
    /// The largest class within (`width_dp`, `height_dp`), preferring width and breaking ties by the largest
    /// height; `WindowSizeClass(0, 0)` if none matches.
    fn compute_window_size_class(&self, width_dp: i32, height_dp: i32) -> WindowSizeClass;

    /// The same of floats, truncated to ints.
    ///
    /// Kotlin: `computeWindowSizeClass(widthDp: Float, heightDp: Float)`
    fn compute_window_size_class_of_float(&self, width_dp: f32, height_dp: f32) -> WindowSizeClass {
        self.compute_window_size_class(width_dp as i32, height_dp as i32)
    }

    /// The largest class within (`width_dp`, `height_dp`), preferring height and breaking ties by the largest
    /// width; `WindowSizeClass(0, 0)` if none matches.
    fn compute_window_size_class_prefer_height(&self, width_dp: i32, height_dp: i32) -> WindowSizeClass;
}

impl WindowSizeClassSelectors for KtSet<WindowSizeClass> {
    fn compute_window_size_class(&self, width_dp: i32, height_dp: i32) -> WindowSizeClass {
        let mut max_width = 0;
        for bucket in self.iter() {
            if bucket.min_width_dp <= width_dp && bucket.min_width_dp > max_width {
                max_width = bucket.min_width_dp;
            }
        }
        let mut found = WindowSizeClass::new(0, 0);
        for bucket in self.iter() {
            if bucket.min_width_dp == max_width
                && bucket.min_height_dp <= height_dp
                && found.min_height_dp <= bucket.min_height_dp
            {
                found = *bucket;
            }
        }
        found
    }

    fn compute_window_size_class_prefer_height(&self, width_dp: i32, height_dp: i32) -> WindowSizeClass {
        let mut max_height = 0;
        for bucket in self.iter() {
            if bucket.min_height_dp <= height_dp && bucket.min_height_dp > max_height {
                max_height = bucket.min_height_dp;
            }
        }
        let mut found = WindowSizeClass::new(0, 0);
        for bucket in self.iter() {
            if bucket.min_height_dp == max_height
                && bucket.min_width_dp <= width_dp
                && found.min_width_dp <= bucket.min_width_dp
            {
                found = *bucket;
            }
        }
        found
    }
}

/// A width bucket of the window (window-core's deprecated `WindowWidthSizeClass`): compact (< 600 dp), medium
/// (< 840 dp), expanded. Kotlin's class of a private raw value, its companion's values the associated consts.
#[deprecated(note = "WindowWidthSizeClass will not be developed further, use WindowSizeClass instead.")]
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct WindowWidthSizeClass(i32);

#[allow(deprecated)]
impl WindowWidthSizeClass {
    #[deprecated(note = "WindowWidthSizeClass not be developed further.")]
    pub const COMPACT: WindowWidthSizeClass = WindowWidthSizeClass(0);
    #[deprecated(note = "WindowWidthSizeClass not be developed further.")]
    pub const MEDIUM: WindowWidthSizeClass = WindowWidthSizeClass(1);
    #[deprecated(note = "WindowWidthSizeClass not be developed further.")]
    pub const EXPANDED: WindowWidthSizeClass = WindowWidthSizeClass(2);

    pub(crate) fn compute(dp_width: f32) -> WindowWidthSizeClass {
        assert!(dp_width >= 0.0, "Width must be positive, received {dp_width}");
        if dp_width < 600.0 {
            Self::COMPACT
        } else if dp_width < 840.0 {
            Self::MEDIUM
        } else {
            Self::EXPANDED
        }
    }
}

/// Kotlin's `toString`: `WindowWidthSizeClass: COMPACT`.
#[allow(deprecated)]
impl fmt::Display for WindowWidthSizeClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self.0 {
            0 => "COMPACT",
            1 => "MEDIUM",
            2 => "EXPANDED",
            _ => "UNKNOWN",
        };
        write!(f, "WindowWidthSizeClass: {name}")
    }
}

/// A height bucket of the window (window-core's deprecated `WindowHeightSizeClass`): compact (< 480 dp), medium
/// (< 900 dp), expanded.
#[deprecated(note = "WindowHeightSizeClass will not be developed further, use WindowSizeClass instead.")]
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct WindowHeightSizeClass(i32);

#[allow(deprecated)]
impl WindowHeightSizeClass {
    #[deprecated(note = "WindowHeightSizeClass not be developed further.")]
    pub const COMPACT: WindowHeightSizeClass = WindowHeightSizeClass(0);
    #[deprecated(note = "WindowHeightSizeClass not be developed further.")]
    pub const MEDIUM: WindowHeightSizeClass = WindowHeightSizeClass(1);
    #[deprecated(note = "WindowHeightSizeClass not be developed further.")]
    pub const EXPANDED: WindowHeightSizeClass = WindowHeightSizeClass(2);

    pub(crate) fn compute(dp_height: f32) -> WindowHeightSizeClass {
        assert!(dp_height >= 0.0, "Height must be positive, received {dp_height}");
        if dp_height < 480.0 {
            Self::COMPACT
        } else if dp_height < 900.0 {
            Self::MEDIUM
        } else {
            Self::EXPANDED
        }
    }
}

/// Kotlin's `toString`: `WindowHeightSizeClass: MEDIUM`.
#[allow(deprecated)]
impl fmt::Display for WindowHeightSizeClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self.0 {
            0 => "COMPACT",
            1 => "MEDIUM",
            2 => "EXPANDED",
            _ => "UNKNOWN",
        };
        write!(f, "WindowHeightSizeClass: {name}")
    }
}
