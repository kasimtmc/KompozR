//! What the system's services are told and tell, in KompozR's terms (C++'s KompozInput statics, Compose's
//! AndroidComposeView's and its platform pieces): the haptic feedback types in KompozServices' order, the back
//! gesture's steps (KompozBack's) as the host's `BackEventCompat`s, the refresh rate the app votes for (its own
//! preference, and the display's fastest while it is busy and a second after: Android 15's frame rate categories,
//! as C++ voted), the system's palettes (Android 12's 65 system colours, the wallpaper's colours), the bold text
//! setting as the fonts' weights (Compose's AndroidFontResolveInterceptor) and the animator duration scale. Pure:
//! tested on the host.

use kompoz_graphics::Color;
use kompoz_graphics::material_color::hct::Hct;
use kompoz_graphics::material_color::palettes::DynamicTonalPalette;
use kompoz_graphics::material_color::wallpaper::GOOGLE_BLUE;
use kompoz_runtime::State;
use kompoz_text::font::{
    FontFamily, FontFamilyResolver, FontLoadError, FontStyle, FontSynthesis, FontWeight, Typeface,
};
use kompoz_text::shape::FontBackend;
use kompoz_ui::back::{BackEventCompat, SwipeEdge};
use kompoz_ui::platform::{FrameRateCategory, HapticFeedbackType};
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;

// ---- Haptics ----

/// A haptic feedback type as KompozServices' `haptic(type)` takes it (its `when`, in the core's order: the
/// system's constant for it, or the nearest the device's Android has, chosen there by name).
pub fn haptic_index(kind: HapticFeedbackType) -> i32 {
    match kind {
        HapticFeedbackType::LongPress => 0,
        HapticFeedbackType::TextHandleMove => 1,
        HapticFeedbackType::Confirm => 2,
        HapticFeedbackType::Reject => 3,
        HapticFeedbackType::ContextClick => 4,
        HapticFeedbackType::GestureEnd => 5,
        HapticFeedbackType::GestureThresholdActivate => 6,
        HapticFeedbackType::KeyboardTap => 7,
        HapticFeedbackType::SegmentTick => 8,
        HapticFeedbackType::SegmentFrequentTick => 9,
        HapticFeedbackType::ToggleOn => 10,
        HapticFeedbackType::ToggleOff => 11,
        HapticFeedbackType::VirtualKey => 12,
    }
}

// ---- Back ----

/// A step of back as KompozBack tells it (its `PHASE_*`).
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum BackPhase {
    /// The gesture began (Android 14's `onBackStarted`).
    Started(BackEventCompat),
    /// It went on (`onBackProgressed`).
    Progressed(BackEventCompat),
    /// It was called off (`onBackCancelled`).
    Cancelled,
    /// Back: the gesture's end, the button, the key (`onBackInvoked`; `onBackPressed` before Android 13).
    Invoked,
}

/// KompozBack's step: its phase (0 started, 1 progressed, 2 cancelled, 3 invoked), the touch (px in the view), how
/// far along (0 to 1), the edge (`BackEvent.EDGE_LEFT` 0, `EDGE_RIGHT` 1, `EDGE_NONE` 2), the frame's time (ms).
pub fn back_phase(phase: i32, x: f32, y: f32, progress: f32, edge: i32, frame_time_millis: i64) -> Option<BackPhase> {
    let finite = |v: f32| if v.is_finite() { v } else { 0.0 };
    let event = BackEventCompat {
        touch_x: finite(x),
        touch_y: finite(y),
        progress: finite(progress).clamp(0.0, 1.0),
        swipe_edge: match edge {
            0 => SwipeEdge::Left,
            1 => SwipeEdge::Right,
            _ => SwipeEdge::None,
        },
        frame_time_millis,
    };
    match phase {
        0 => Some(BackPhase::Started(event)),
        1 => Some(BackPhase::Progressed(event)),
        2 => Some(BackPhase::Cancelled),
        3 => Some(BackPhase::Invoked),
        _ => None,
    }
}

// ---- The refresh rate ----

/// How long the app counts as busy after it last was (a finger lifted, the last frame of a fling): the display
/// stays at its fastest that long, so the next touch isn't met at the slow rate (C++'s second, as Android 15's views
/// keep their high category a while after they last moved).
pub const BUSY_LINGER_NS: i64 = 1_000_000_000;

/// The refresh rate voted for: a rate (Hz; 0 none) and a category.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct FrameRateVote {
    pub rate: f32,
    pub category: FrameRateCategory,
}

/// The app's vote, worked out after each frame (C++'s `composed`): the rate a node asked for
/// (`preferred_frame_rate`), and the category — a node's, raised to High while the app is busy (a finger on the
/// screen, a frame wanted: a scroll, an animation) and a second after; else none (the system keeps an app that
/// doesn't ask at its normal rate). Told only as it changes.
#[derive(Default)]
pub struct FrameRateVoter {
    last_busy: Option<i64>,
    told: Option<FrameRateVote>,
}

impl FrameRateVoter {
    /// The vote after a frame at `frame_time` (ns), if it changed: the app's own `rate` and `category`, whether it
    /// is `busy`.
    pub fn vote(
        &mut self,
        frame_time: i64,
        busy: bool,
        rate: f32,
        category: FrameRateCategory,
    ) -> Option<FrameRateVote> {
        if busy {
            self.last_busy = Some(frame_time);
        }
        let lingering = self.last_busy.is_some_and(|at| frame_time - at < BUSY_LINGER_NS);
        let category = if lingering && category != FrameRateCategory::NoPreference {
            category.max(FrameRateCategory::High)
        } else {
            category
        };
        let vote = FrameRateVote { rate: if rate.is_finite() { rate.max(0.0) } else { 0.0 }, category };
        (self.told != Some(vote)).then(|| {
            self.told = Some(vote);
            vote
        })
    }

    /// Told again from scratch (a new window: what it was told went with the old one).
    pub fn reset(&mut self) {
        self.told = None;
    }
}

/// A category as KompozServices' `setFrameRate` takes it (0 default, 1 no preference, 2 normal, 3 high).
pub fn category_index(category: FrameRateCategory) -> i32 {
    match category {
        FrameRateCategory::Default => 0,
        FrameRateCategory::NoPreference => 1,
        FrameRateCategory::Normal => 2,
        FrameRateCategory::High => 3,
    }
}

/// What the surface's own vote is (`ANativeWindow_setFrameRate`, Hz; 0: none): the app's rate, else its
/// category's — normal 60, high the display's fastest (`fastest`, Hz).
pub fn surface_rate(vote: FrameRateVote, fastest: f32) -> f32 {
    if vote.rate > 0.0 {
        return vote.rate;
    }
    match vote.category {
        FrameRateCategory::Normal => 60.0,
        FrameRateCategory::High => fastest.max(60.0),
        FrameRateCategory::Default | FrameRateCategory::NoPreference => 0.0,
    }
}

// ---- The system's palettes ----

/// Android 12's system colours (KompozServices' `dynamicColors`: accent1, accent2, accent3, neutral1, neutral2, each
/// in tones 0 (white) … 1000 (black), ARGB) as the host's dynamic palette; none if they aren't all there.
pub fn dynamic_palette(colors: &[i32]) -> Option<DynamicTonalPalette> {
    if colors.len() != 65 {
        return None;
    }
    let key = |k: usize| std::array::from_fn(|i| Color::from_argb(colors[k * 13 + i] as u32));
    Some(DynamicTonalPalette { accent1: key(0), accent2: key(1), accent3: key(2), neutral1: key(3), neutral2: key(4) })
}

/// The palettes made from the wallpaper's colours (`WallpaperColors`' primary, secondary, tertiary; 0: none) as
/// C++ made them: from the first colourful enough to seed one (chroma 5 or more), else Google blue's. None without
/// colours.
pub fn wallpaper_palette(colors: &[i32]) -> Option<DynamicTonalPalette> {
    let given: Vec<u32> = colors.iter().map(|&c| c as u32).filter(|&c| c != 0).collect();
    if given.is_empty() {
        return None;
    }
    let seed = given.iter().copied().find(|&c| Hct::from_int(c).chroma() >= 5.0).unwrap_or(GOOGLE_BLUE);
    Some(DynamicTonalPalette::from_seed(Color::from_argb(seed)))
}

// ---- Bold text ----

/// A weight with the system's adjustment (Android 12's bold text: `Configuration.fontWeightAdjustment`, 300 when
/// on), as Compose's AndroidFontResolveInterceptor adjusts it: kept between 1 and 1000.
pub fn adjusted_weight(weight: FontWeight, adjustment: i32) -> FontWeight {
    if adjustment == 0 {
        return weight;
    }
    FontWeight { weight: weight.weight.saturating_add(adjustment).clamp(1, 1000) }
}

/// The host's font resolver with the system's weight adjustment (Compose's FontFamilyResolver with the platform's
/// resolve interceptor): every face resolved `adjustment` heavier.
pub struct AdjustedResolver {
    pub inner: Rc<dyn FontFamilyResolver>,
    pub adjustment: i32,
}

impl FontFamilyResolver for AdjustedResolver {
    fn preload(&self, family: &FontFamily) -> Pin<Box<dyn Future<Output = Result<(), FontLoadError>>>> {
        self.inner.preload(family)
    }

    fn resolve(
        &self,
        family: Option<&FontFamily>,
        weight: FontWeight,
        style: FontStyle,
        synthesis: FontSynthesis,
    ) -> State<Typeface> {
        self.inner.resolve(family, adjusted_weight(weight, self.adjustment), style, synthesis)
    }

    fn backend(&self) -> Rc<dyn FontBackend> {
        self.inner.backend()
    }
}

// ---- Motion ----

/// The animator duration scale (`Settings.Global.ANIMATOR_DURATION_SCALE`) as the motion duration scale: what the
/// system's setting says (0: animations off), 1 where it says nothing usable.
pub fn animator_scale(scale: f32) -> f32 {
    if scale.is_finite() && scale >= 0.0 { scale } else { 1.0 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn haptics_keep_their_order() {
        assert_eq!(haptic_index(HapticFeedbackType::LongPress), 0);
        assert_eq!(haptic_index(HapticFeedbackType::GestureThresholdActivate), 6);
        assert_eq!(haptic_index(HapticFeedbackType::VirtualKey), 12);
    }

    #[test]
    fn back_steps_are_read() {
        let Some(BackPhase::Progressed(e)) = back_phase(1, 12.0, 300.0, 0.4, 1, 77) else { panic!() };
        assert_eq!(
            (e.touch_x, e.touch_y, e.progress, e.swipe_edge, e.frame_time_millis),
            (12.0, 300.0, 0.4, SwipeEdge::Right, 77)
        );
        let Some(BackPhase::Started(e)) = back_phase(0, f32::NAN, 0.0, 3.0, 2, 0) else { panic!() };
        assert_eq!((e.touch_x, e.progress, e.swipe_edge), (0.0, 1.0, SwipeEdge::None));
        assert_eq!(back_phase(2, 0.0, 0.0, 0.0, 0, 0), Some(BackPhase::Cancelled));
        assert_eq!(back_phase(3, 0.0, 0.0, 0.0, 0, 0), Some(BackPhase::Invoked));
        assert_eq!(back_phase(9, 0.0, 0.0, 0.0, 0, 0), None);
    }

    #[test]
    fn busy_frames_vote_high_for_a_second() {
        let mut v = FrameRateVoter::default();
        let s = 1_000_000_000;
        let idle = FrameRateVote { rate: 0.0, category: FrameRateCategory::Default };
        let high = FrameRateVote { rate: 0.0, category: FrameRateCategory::High };
        assert_eq!(v.vote(0, false, 0.0, FrameRateCategory::Default), Some(idle));
        assert_eq!(v.vote(s / 60, false, 0.0, FrameRateCategory::Default), None); // (unchanged: not told)
        assert_eq!(v.vote(s, true, 0.0, FrameRateCategory::Default), Some(high));
        assert_eq!(v.vote(s + s / 2, false, 0.0, FrameRateCategory::Default), None);
        assert_eq!(v.vote(2 * s + 1, false, 0.0, FrameRateCategory::Default), Some(idle));
        // A node asking for no preference keeps it, busy or not; one asking for a rate is told it.
        let none = FrameRateVote { rate: 0.0, category: FrameRateCategory::NoPreference };
        assert_eq!(v.vote(3 * s, true, 0.0, FrameRateCategory::NoPreference), Some(none));
        let rate = v.vote(3 * s, true, 30.0, FrameRateCategory::NoPreference).expect("changed");
        assert_eq!(surface_rate(rate, 120.0), 30.0);
        assert_eq!(surface_rate(high, 120.0), 120.0);
        assert_eq!(surface_rate(FrameRateVote { rate: 0.0, category: FrameRateCategory::Normal }, 120.0), 60.0);
        assert_eq!(surface_rate(idle, 120.0), 0.0);
        v.reset();
        assert!(v.vote(3 * s, true, 30.0, FrameRateCategory::NoPreference).is_some());
        assert_eq!(category_index(FrameRateCategory::High), 3);
    }

    #[test]
    fn palettes_come_from_the_systems_colours() {
        let colors: Vec<i32> = (0..65).map(|i| 0xff00_0000u32 as i32 | i).collect();
        let p = dynamic_palette(&colors).expect("65 colours");
        assert_eq!(p.accent1[0], Color::from_argb(0xff00_0000));
        assert_eq!(p.accent2[0], Color::from_argb(0xff00_000d));
        assert_eq!(p.neutral2[12], Color::from_argb(0xff00_0040));
        assert!(dynamic_palette(&colors[..64]).is_none());
        // A grey wallpaper seeds Google blue's; a colourful one its own.
        assert_eq!(
            wallpaper_palette(&[0xff80_8080u32 as i32, 0, 0]),
            Some(DynamicTonalPalette::from_seed(Color::from_argb(GOOGLE_BLUE)))
        );
        let red = 0xffd0_2020u32 as i32;
        assert_eq!(
            wallpaper_palette(&[0xff80_8080u32 as i32, red, 0]),
            Some(DynamicTonalPalette::from_seed(Color::from_argb(red as u32)))
        );
        assert_eq!(wallpaper_palette(&[0, 0, 0]), None);
    }

    #[test]
    fn bold_text_adds_to_the_weight() {
        assert_eq!(adjusted_weight(FontWeight::W400, 300), FontWeight::W700);
        assert_eq!(adjusted_weight(FontWeight::W900, 300), FontWeight { weight: 1000 });
        assert_eq!(adjusted_weight(FontWeight::W100, -500), FontWeight { weight: 1 });
        assert_eq!(adjusted_weight(FontWeight::W500, 0), FontWeight::W500);
    }

    #[test]
    fn the_animator_scale_is_kept_sane() {
        assert_eq!(animator_scale(0.0), 0.0);
        assert_eq!(animator_scale(5.0), 5.0);
        assert_eq!(animator_scale(-1.0), 1.0);
        assert_eq!(animator_scale(f32::NAN), 1.0);
    }
}
