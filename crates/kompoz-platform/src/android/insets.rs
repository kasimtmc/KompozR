//! The window's insets as KompozInsets tells them (C++'s `sendInsets`, Compose's `WindowInsetsHolder` updated from
//! `WindowInsetsCompat` and its `WindowInsetsAnimationCompat` callback): what each kind of system UI covers (the
//! status and navigation bars, the caption bar, the keyboard, the display cutout and its waterfall, the gesture
//! edges, the tappable elements), whether it is shown and what it would cover shown, the cutouts' bounds and shape,
//! and what is moving (the keyboard sliding in, a bar sliding away) frame by frame — into the core's
//! [`SystemInsets`]. Pure: tested on the host.
//!
//! The arrays' layout (KompozInsets.kt's `values`, in this order):
//! - ints: per kind, in [`InsetsKind::All`]'s order, [`KIND_INTS`] each: left, top, right, bottom, shown (0/1), and
//!   whether what it would cover shown is told (0/1) and that (left, top, right, bottom); then the keyboard sliding
//!   (0/1), where it slides from and to (4 + 4); then the cutouts' bounds (a count, then 4 each); then the animations
//!   under way (a count, then per animation: the kind's index, from (4), to (4), its duration in ms);
//! - floats: per animation its fraction and alpha; then the cutouts' shape as segments of [`SEGMENT_FLOATS`] each
//!   (a verb, then the points it goes through after where it starts — a move's one, a line's one, a quad's and a
//!   conic's two, a cubic's three —, and a conic's weight last), none where the system doesn't tell it.

use kompoz_graphics::{IntRect, Path};
use kompoz_ui::platform::{Insets, InsetsAnimation, InsetsKind, SystemInsets};

/// The ints told of each kind.
pub const KIND_INTS: usize = 10;
/// The floats of a cutout shape's segment: its verb, up to three points, a conic's weight.
pub const SEGMENT_FLOATS: usize = 8;

/// A cutout shape segment's verb (KompozInsets' `VERB_*`, Android's `PathIterator.VERB_*` save done).
mod verb {
    pub const MOVE: i32 = 0;
    pub const LINE: i32 = 1;
    pub const QUAD: i32 = 2;
    pub const CONIC: i32 = 3;
    pub const CUBIC: i32 = 4;
    pub const CLOSE: i32 = 5;
}

/// Reads ints in order (what is missing: 0).
struct Ints<'a> {
    values: &'a [i32],
    at: usize,
}

impl Ints<'_> {
    fn next(&mut self) -> i32 {
        let v = self.values.get(self.at).copied().unwrap_or(0);
        self.at += 1;
        v
    }

    fn insets(&mut self) -> Insets {
        Insets::new(self.next(), self.next(), self.next(), self.next())
    }

    /// A count, no more than what is left could hold at `each` ints apiece.
    fn count(&mut self, each: usize) -> usize {
        let n = self.next().max(0) as usize;
        n.min(self.values.len().saturating_sub(self.at) / each.max(1))
    }
}

/// One kind as told.
#[derive(Clone, Copy, Default)]
struct Kind {
    insets: Insets,
    visible: bool,
    ignoring_visibility: Option<Insets>,
}

/// The insets KompozInsets told (`ints`, `floats` as the module says).
pub fn system_insets(ints: &[i32], floats: &[f32]) -> SystemInsets {
    let mut r = Ints { values: ints, at: 0 };
    let mut kinds = [Kind::default(); 9];
    for kind in &mut kinds {
        let insets = r.insets();
        let visible = r.next() != 0;
        let told = r.next() != 0;
        let ignoring = r.insets();
        *kind = Kind { insets, visible, ignoring_visibility: told.then_some(ignoring) };
    }
    let k = |kind: InsetsKind| kinds[InsetsKind::All.iter().position(|&x| x == kind).unwrap_or(0)];
    let ime_animating = r.next() != 0;
    let ime_animation_source = r.insets();
    let ime_animation_target = r.insets();
    let cutouts = r.count(4);
    let cutout_rects = (0..cutouts)
        .map(|_| {
            let (left, top, right, bottom) = (r.next(), r.next(), r.next(), r.next());
            IntRect::new(left, top, right, bottom)
        })
        .collect();
    let count = r.count(10);
    let mut animations = Vec::with_capacity(count);
    for i in 0..count {
        let kind = InsetsKind::All.get(r.next().max(0) as usize).copied();
        let source = r.insets();
        let target = r.insets();
        let duration_millis = r.next().max(0) as i64;
        let float = |j: usize| floats.get(i * 2 + j).copied().filter(|v| v.is_finite());
        if let Some(kind) = kind {
            animations.push(InsetsAnimation {
                kind,
                source,
                target,
                fraction: float(0).unwrap_or(0.0).clamp(0.0, 1.0),
                alpha: float(1).unwrap_or(1.0).clamp(0.0, 1.0),
                duration_millis,
            });
        }
    }
    let cutout_path = cutout_path(floats.get(count * 2..).unwrap_or(&[]));
    SystemInsets {
        status_bars: k(InsetsKind::StatusBars).insets,
        navigation_bars: k(InsetsKind::NavigationBars).insets,
        ime: k(InsetsKind::Ime).insets,
        display_cutout: k(InsetsKind::DisplayCutout).insets,
        system_gestures: k(InsetsKind::SystemGestures).insets,
        caption_bar: k(InsetsKind::CaptionBar).insets,
        mandatory_system_gestures: k(InsetsKind::MandatorySystemGestures).insets,
        tappable_element: k(InsetsKind::TappableElement).insets,
        waterfall: k(InsetsKind::Waterfall).insets,
        status_bars_visible: k(InsetsKind::StatusBars).visible,
        navigation_bars_visible: k(InsetsKind::NavigationBars).visible,
        caption_bar_visible: k(InsetsKind::CaptionBar).visible,
        display_cutout_visible: k(InsetsKind::DisplayCutout).visible,
        system_gestures_visible: k(InsetsKind::SystemGestures).visible,
        mandatory_system_gestures_visible: k(InsetsKind::MandatorySystemGestures).visible,
        tappable_element_visible: k(InsetsKind::TappableElement).visible,
        status_bars_ignoring_visibility: k(InsetsKind::StatusBars).ignoring_visibility,
        navigation_bars_ignoring_visibility: k(InsetsKind::NavigationBars).ignoring_visibility,
        caption_bar_ignoring_visibility: k(InsetsKind::CaptionBar).ignoring_visibility,
        display_cutout_ignoring_visibility: k(InsetsKind::DisplayCutout).ignoring_visibility,
        system_gestures_ignoring_visibility: k(InsetsKind::SystemGestures).ignoring_visibility,
        mandatory_system_gestures_ignoring_visibility: k(InsetsKind::MandatorySystemGestures).ignoring_visibility,
        tappable_element_ignoring_visibility: k(InsetsKind::TappableElement).ignoring_visibility,
        cutout_path,
        cutout_rects,
        ime_animating,
        ime_visible: k(InsetsKind::Ime).visible,
        ime_animation_source,
        ime_animation_target,
        animations,
    }
}

/// The cutouts' shape from its segments (none: not told, or nothing in it).
fn cutout_path(segments: &[f32]) -> Option<Path> {
    let mut path = Path::new();
    for s in segments.as_chunks::<SEGMENT_FLOATS>().0 {
        if s.iter().any(|v| !v.is_finite()) {
            continue;
        }
        match s[0] as i32 {
            verb::MOVE => path.move_to(s[1], s[2]),
            verb::LINE => path.line_to(s[1], s[2]),
            verb::QUAD => path.quadratic_to(s[1], s[2], s[3], s[4]),
            verb::CONIC => path.conic_to(s[1], s[2], s[3], s[4], s[7]),
            verb::CUBIC => path.cubic_to(s[1], s[2], s[3], s[4], s[5], s[6]),
            verb::CLOSE => path.close(),
            _ => {}
        }
    }
    (!path.is_empty()).then_some(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A kind's ints.
    fn kind(insets: [i32; 4], visible: bool, ignoring: Option<[i32; 4]>) -> Vec<i32> {
        let mut v = insets.to_vec();
        v.push(visible as i32);
        v.push(ignoring.is_some() as i32);
        v.extend(ignoring.unwrap_or([0; 4]));
        v
    }

    #[test]
    fn each_kind_is_read_in_its_place() {
        let mut ints = Vec::new();
        ints.extend(kind([0, 80, 0, 0], true, None)); // status bars
        ints.extend(kind([0, 0, 0, 0], false, Some([0, 0, 0, 120]))); // navigation bars, hidden
        ints.extend(kind([0, 0, 0, 0], true, None)); // caption bar
        ints.extend(kind([0, 0, 0, 700], true, None)); // the keyboard
        ints.extend(kind([0, 90, 0, 0], true, Some([0, 90, 0, 0]))); // cutout
        ints.extend(kind([40, 0, 40, 60], true, None)); // gestures
        ints.extend(kind([0, 0, 0, 60], true, None)); // mandatory gestures
        ints.extend(kind([0, 80, 0, 0], true, None)); // tappable
        ints.extend(kind([3, 0, 3, 0], true, None)); // waterfall
        ints.extend([1, 0, 0, 0, 0, 0, 0, 0, 700]); // sliding up
        ints.extend([1, 500, 0, 580, 90]); // one cutout
        ints.extend([1, 3, 0, 0, 0, 0, 0, 0, 0, 700, 250]); // the keyboard's animation
        let floats = [0.5, 1.0];
        let s = system_insets(&ints, &floats);
        assert_eq!(s.status_bars, Insets::new(0, 80, 0, 0));
        assert!(!s.navigation_bars_visible && s.status_bars_visible);
        assert_eq!(s.navigation_bars_ignoring_visibility, Some(Insets::new(0, 0, 0, 120)));
        assert_eq!(s.ignoring_visibility(InsetsKind::NavigationBars), Some(Insets::new(0, 0, 0, 120)));
        assert_eq!(s.ignoring_visibility(InsetsKind::StatusBars), Some(Insets::new(0, 80, 0, 0)));
        assert_eq!(s.ime, Insets::new(0, 0, 0, 700));
        assert!(s.ime_visible && s.ime_animating);
        assert_eq!(s.ime_animation_target, Insets::new(0, 0, 0, 700));
        assert_eq!(s.display_cutout, Insets::new(0, 90, 0, 0));
        assert_eq!(s.system_gestures, Insets::new(40, 0, 40, 60));
        assert_eq!(s.mandatory_system_gestures, Insets::new(0, 0, 0, 60));
        assert_eq!(s.tappable_element, Insets::new(0, 80, 0, 0));
        assert_eq!(s.waterfall, Insets::new(3, 0, 3, 0));
        assert_eq!(s.cutout_rects, vec![IntRect::new(500, 0, 580, 90)]);
        assert!(s.cutout_path.is_none());
        let a = s.animation(InsetsKind::Ime).expect("the keyboard's animation");
        assert_eq!((a.target, a.fraction, a.alpha, a.duration_millis), (Insets::new(0, 0, 0, 700), 0.5, 1.0, 250));
    }

    #[test]
    fn what_is_missing_is_nothing() {
        let s = system_insets(&[], &[]);
        assert_eq!(s.status_bars, Insets::Zero);
        assert!(!s.status_bars_visible && !s.ime_animating && s.animations.is_empty() && s.cutout_rects.is_empty());
        // (A count larger than what follows reads only what is there.)
        let mut ints = vec![0; 9 * KIND_INTS + 9];
        ints.extend([1000, 1, 2, 3]);
        assert!(system_insets(&ints, &[]).cutout_rects.is_empty());
    }

    #[test]
    fn the_cutouts_shape_is_read() {
        let mut ints = vec![0; 9 * KIND_INTS + 9];
        ints.extend([0, 0]); // no rects, no animations
        let mut floats = Vec::new();
        floats.extend([verb::MOVE as f32, 10.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]);
        floats.extend([verb::LINE as f32, 20.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]);
        floats.extend([verb::CONIC as f32, 20.0, 10.0, 10.0, 10.0, 0.0, 0.0, 0.7]);
        floats.extend([verb::CLOSE as f32, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]);
        let path = system_insets(&ints, &floats).cutout_path.expect("a shape");
        let b = path.get_bounds();
        assert_eq!((b.left, b.top, b.right, b.bottom), (10.0, 0.0, 20.0, 10.0));
    }
}
