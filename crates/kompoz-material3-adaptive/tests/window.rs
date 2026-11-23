//! The window's size classes (window-core 1.5.0) and adaptive info (material3-adaptive 1.2.0): the C++ Kompoz's
//! size-class assertions (its `window_size_classes_and_the_navigation_they_suit` is ported whole, navigation and
//! all, in the navigation suite's tests), window-core's samples, Compose's behaviour of the classes, selectors,
//! posture and `currentWindowAdaptiveInfo`.

#![allow(non_snake_case, non_upper_case_globals, deprecated)]

#[path = "../../kompoz-foundation/tests/common/mod.rs"]
mod common;

use common::*;
use kompoz_graphics::*;
use kompoz_material3_adaptive::prelude::*;
use kompoz_runtime::kt::{KtToString, set_of};
use kompoz_runtime::*;
use kompoz_test::KompozTestRule;
use kompoz_ui::media_query::{self, MediaInfo};
use kompoz_ui::platform::LocalDensity;
use std::cell::RefCell;
use std::rc::Rc;

tests!(
    size_classes_are_the_breakpoints_a_window_is_at_least,
    the_recommended_breakpoint_sets_and_their_selectors,
    window_core_samples,
    deprecated_width_and_height_buckets,
    current_window_adaptive_info_follows_the_window,
    current_window_adaptive_info_v2_has_the_large_widths,
    postures_and_their_hinges,
);

const W: i32 = WindowSizeClass::WIDTH_DP_MEDIUM_LOWER_BOUND;
const WE: i32 = WindowSizeClass::WIDTH_DP_EXPANDED_LOWER_BOUND;
const H: i32 = WindowSizeClass::HEIGHT_DP_MEDIUM_LOWER_BOUND;
const HE: i32 = WindowSizeClass::HEIGHT_DP_EXPANDED_LOWER_BOUND;

fn size_classes_are_the_breakpoints_a_window_is_at_least() {
    // The C++'s checks of WindowSizeClass::compute (window-core's deprecated `compute`, the V1 breakpoints).
    let phone = WindowSizeClass::compute(400.0, 800.0);
    assert_eq!((phone.min_width_dp(), phone.min_height_dp()), (0, H));
    assert!(phone.is_height_at_least_breakpoint(H) && !phone.is_width_at_least_breakpoint(W));
    assert_eq!(WindowSizeClass::compute(700.0, 1000.0), WindowSizeClass(W, HE));
    assert_eq!(WindowSizeClass::compute(600.0, 480.0), WindowSizeClass(W, H));
    assert_eq!(WindowSizeClass::compute(599.5, 479.0), WindowSizeClass(0, 0));
    let big = WindowSizeClass::compute(840.0, 900.0);
    assert_eq!((big.min_width_dp(), big.min_height_dp()), (WE, HE));
    // Order dependent: a class matches each smaller breakpoint too.
    assert!(
        big.is_at_least_breakpoint(W, H) && big.is_at_least_breakpoint(WE, HE) && !big.is_at_least_breakpoint(1200, 0)
    );
    // The constants, toString, equality, the float constructor truncating.
    assert_eq!(
        (
            WindowSizeClass::WIDTH_DP_LARGE_LOWER_BOUND,
            WindowSizeClass::WIDTH_DP_EXTRA_LARGE_LOWER_BOUND,
            WindowSizeClass::HEIGHT_DP_EXPANDED_LOWER_BOUND
        ),
        (1200, 1600, 900)
    );
    assert_eq!(WindowSizeClass(600, 480).to_string(), "WindowSizeClass(minWidthDp=600, minHeightDp=480)");
    assert_eq!(WindowSizeClass::of_float(600.9, 480.99), WindowSizeClass(600, 480));
    assert!(std::panic::catch_unwind(|| WindowSizeClass(-1, 0)).is_err());
    assert!(std::panic::catch_unwind(|| WindowSizeClass::compute(-1.0, 0.0)).is_ok()); // compute clamps to 0
}

fn the_recommended_breakpoint_sets_and_their_selectors() {
    let v1 = WindowSizeClass::BREAKPOINTS_V1();
    assert_eq!(v1.size(), 9);
    assert!(v1.contains(WindowSizeClass(WE, HE)) && !v1.contains(WindowSizeClass(1200, 0)));
    let v2 = WindowSizeClass::BREAKPOINTS_V2();
    assert_eq!(v2.size(), 15);
    assert!(v2.contains(WindowSizeClass(1600, 900)));
    // Width first, the largest height breaking ties.
    assert_eq!(v1.compute_window_size_class(700, 1000), WindowSizeClass(W, HE));
    assert_eq!(v1.compute_window_size_class(2000, 300), WindowSizeClass(WE, 0));
    assert_eq!(v2.compute_window_size_class(2000, 300), WindowSizeClass(1600, 0));
    assert_eq!(v2.compute_window_size_class_of_float(1299.9, 899.9), WindowSizeClass(1200, H));
    // window-core's documented examples.
    let set = set_of([WindowSizeClass(300, 300), WindowSizeClass(300, 600)]);
    assert_eq!(set.compute_window_size_class_of_float(300.5, 800.5), WindowSizeClass(300, 600));
    assert_eq!(set.compute_window_size_class(300, 400), WindowSizeClass(300, 300));
    assert_eq!(set.compute_window_size_class(200, 400), WindowSizeClass(0, 0)); // nothing matches
    let wide = set_of([WindowSizeClass(300, 300), WindowSizeClass(600, 300)]);
    assert_eq!(wide.compute_window_size_class_prefer_height(800, 300), WindowSizeClass(600, 300));
    assert_eq!(wide.compute_window_size_class_prefer_height(400, 300), WindowSizeClass(300, 300));
}

/// window-core's WindowSizeClassSamples.kt: `calculateWindowSizeClass`, `processWindowSizeClassWidthOnly`.
fn window_core_samples() {
    fn calculate_window_size_class(width_dp: i32, height_dp: i32) -> WindowSizeClass {
        WindowSizeClass::BREAKPOINTS_V1().compute_window_size_class(width_dp, height_dp)
    }
    fn process_window_size_class_width_only(size_class: WindowSizeClass) -> &'static str {
        if size_class.is_width_at_least_breakpoint(WindowSizeClass::WIDTH_DP_EXPANDED_LOWER_BOUND) {
            "EXPANDED"
        } else if size_class.is_width_at_least_breakpoint(WindowSizeClass::WIDTH_DP_MEDIUM_LOWER_BOUND) {
            "MEDIUM"
        } else {
            "COMPACT"
        }
    }
    assert_eq!(process_window_size_class_width_only(calculate_window_size_class(400, 800)), "COMPACT");
    assert_eq!(process_window_size_class_width_only(calculate_window_size_class(700, 800)), "MEDIUM");
    assert_eq!(process_window_size_class_width_only(calculate_window_size_class(1300, 800)), "EXPANDED");
}

fn deprecated_width_and_height_buckets() {
    let c = WindowSizeClass(W, HE);
    assert_eq!(c.window_width_size_class(), WindowWidthSizeClass::MEDIUM);
    assert_eq!(c.window_height_size_class(), WindowHeightSizeClass::EXPANDED);
    assert_eq!(WindowSizeClass(0, 0).window_width_size_class(), WindowWidthSizeClass::COMPACT);
    assert_eq!(WindowSizeClass(1600, H).window_width_size_class(), WindowWidthSizeClass::EXPANDED);
    assert_eq!(WindowWidthSizeClass::COMPACT.to_string(), "WindowWidthSizeClass: COMPACT");
    assert_eq!(WindowHeightSizeClass::MEDIUM.to_string(), "WindowHeightSizeClass: MEDIUM");
    assert_ne!(WindowWidthSizeClass::MEDIUM, WindowWidthSizeClass::EXPANDED);
}

fn current_window_adaptive_info_follows_the_window() {
    let seen: Rc<RefCell<Vec<(WindowAdaptiveInfo, WindowAdaptiveInfo, DpSize, IntSize)>>> = Rc::default();
    let s = seen.clone();
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let info = current_window_adaptive_info();
            let large = current_window_adaptive_info(support_large_and_x_large_width = true);
            s.borrow_mut().push((info, large, current_window_dp_size(), current_window_size()));
        }),
        IntSize::new(700, 1000),
    );
    let last = || seen.borrow().last().cloned().unwrap();
    let (info, large, dp, px) = last();
    assert_eq!(info.window_size_class, WindowSizeClass(W, HE));
    assert_eq!(large.window_size_class, WindowSizeClass(W, HE));
    assert_eq!((dp, px), (DpSize::new(700.dp(), 1000.dp()), IntSize::new(700, 1000)));
    // The desktop's posture: the default one (Compose for desktop's `calculatePosture`; the C++ read its host's
    // media posture — Compose's posture comes from Android's folding features, stage 15's platform).
    assert_eq!(info.window_posture, Posture());
    rule.host().set_media_info(MediaInfo { window_posture: media_query::Posture::Tabletop, ..MediaInfo::default() });
    rule.wait_for_idle();
    assert_eq!(last().0.window_posture, Posture());
    // Wider: the large classes only when asked for.
    rule.host().set_viewport_size(IntSize::new(1700, 800));
    rule.wait_for_idle();
    let (info, large, _, _) = last();
    assert_eq!((info.window_size_class, large.window_size_class), (WindowSizeClass(WE, H), WindowSizeClass(1600, H)));
    assert_eq!(
        info.to_string(),
        "WindowAdaptiveInfo(windowSizeClass=WindowSizeClass(minWidthDp=840, minHeightDp=480), \
         windowPosture=Posture(isTabletop=false, hinges=[]))"
    );
    drop(rule);
    // In dp: a density of 2 halves the pixels.
    let s = seen.clone();
    let _rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let s = s.clone();
            KompozitionLocalProvider(&[LocalDensity.provides(Density((2.0, 1.0)))], move || {
                s.borrow_mut().push((
                    current_window_adaptive_info(),
                    current_window_adaptive_info(),
                    current_window_dp_size(),
                    current_window_size(),
                ));
            });
        }),
        IntSize::new(1300, 1000),
    );
    let (info, _, dp, _) = last();
    assert_eq!((info.window_size_class, dp), (WindowSizeClass(W, H), DpSize::new(650.dp(), 500.dp())));
}

/// (WP8) adaptive 1.3's `currentWindowAdaptiveInfoV2` (material3 1.5's navigation suite samples call it):
/// `currentWindowAdaptiveInfo(supportLargeAndXLargeWidth = true)`, the 1200 and 1600 dp widths among its classes.
fn current_window_adaptive_info_v2_has_the_large_widths() {
    let seen: Rc<RefCell<Vec<(WindowAdaptiveInfo, WindowAdaptiveInfo)>>> = Rc::default();
    let s = seen.clone();
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let v2 = current_window_adaptive_info_v2();
            let large = current_window_adaptive_info(support_large_and_x_large_width = true);
            s.borrow_mut().push((v2, large));
        }),
        IntSize::new(1300, 1000),
    );
    let last = || seen.borrow().last().cloned().unwrap();
    assert_eq!(last().0.window_size_class, WindowSizeClass(WindowSizeClass::WIDTH_DP_LARGE_LOWER_BOUND, HE));
    for (width, class) in [(500, 0), (700, W), (900, WE), (1300, 1200), (1700, 1600)] {
        rule.host().set_viewport_size(IntSize::new(width, 600));
        rule.wait_for_idle();
        let (v2, large) = last();
        assert_eq!(v2, large);
        assert_eq!(v2.window_size_class, WindowSizeClass(class, H));
        assert_eq!(v2.window_posture, Posture());
    }
}

fn postures_and_their_hinges() {
    let vertical = HingeInfo(Rect::new(390.0, 0.0, 410.0, 800.0), false, true, true, false);
    let horizontal = HingeInfo(Rect::new(0.0, 390.0, 800.0, 410.0), true, false, false, true);
    let posture = named_args!(Posture(is_tabletop = true, hinge_list = vec![vertical, horizontal]));
    assert_eq!(posture.separating_vertical_hinge_bounds(), vec![vertical.bounds]);
    assert!(posture.occluding_vertical_hinge_bounds().is_empty());
    assert_eq!(posture.all_vertical_hinge_bounds(), vec![vertical.bounds]);
    assert!(posture.separating_horizontal_hinge_bounds().is_empty());
    assert_eq!(posture.occluding_horizontal_hinge_bounds(), vec![horizontal.bounds]);
    assert_eq!(posture.all_horizontal_hinge_bounds(), vec![horizontal.bounds]);
    assert_ne!(posture, named_args!(Posture(is_tabletop = true)));
    assert_eq!(Posture(), Posture::default());
    assert_eq!(
        named_args!(Posture(hinge_list = vec![horizontal])).kt_to_string(),
        "Posture(isTabletop=false, hinges=[HingeInfo(bounds=Rect.fromLTRB(0.0, 390.0, 800.0, 410.0), isFlat=true, \
         isVertical=false, isSeparating=false, isOccluding=true)])"
    );
}
