//! Arrangements, as Compose's `Arrangement`: where a `Row` or a `Column` puts its children along its main axis.
//!
//! Compose's `Arrangement.Horizontal`, `Arrangement.Vertical` and `Arrangement.HorizontalOrVertical` are one type
//! here, [`Arrangement<A>`], of an axis `A`: [`HorizontalArrangement`] (a Row's) or [`VerticalArrangement`] (a
//! Column's). What Compose has for both axes is generic over it, so `Arrangement::SpaceBetween` and
//! `Arrangement::spaced_by(8.dp())` go to a Row and to a Column alike, while `Arrangement::Start` is a Row's only
//! and `Arrangement::Top` a Column's only. Compose's `Arrangement.Absolute` is [`AbsoluteArrangement`].

use super::{round, same_dp};
use kompoz_graphics::{Density, Dp, LayoutDirection};
use kompoz_ui::{Alignment, HorizontalAlignment, VerticalAlignment};
use std::fmt;
use std::marker::PhantomData;
use std::rc::Rc;

/// The axis an arrangement arranges along: [`Horizontal`] or [`Vertical`].
pub trait ArrangementAxis: 'static {}

/// The horizontal axis: a Row's arrangements (Compose's `Arrangement.Horizontal`).
pub enum Horizontal {}

/// The vertical axis: a Column's arrangements (Compose's `Arrangement.Vertical`).
pub enum Vertical {}

impl ArrangementAxis for Horizontal {}
impl ArrangementAxis for Vertical {}

/// How a layout puts its children one after the other along an axis `A` (Compose's `Arrangement.Horizontal` and
/// `Arrangement.Vertical`): given the space it has and its children's sizes, their positions, and the space it
/// wants between two children (`spacing`). A horizontal one mirrors right to left (Start is the right), but the
/// absolute ones ([`AbsoluteArrangement`]). Equal when made the same way (a custom one: when it is the same one).
pub struct Arrangement<A: ArrangementAxis> {
    kind: Kind,
    axis: PhantomData<fn() -> A>,
}

/// A Row's arrangement (Compose's `Arrangement.Horizontal`).
pub type HorizontalArrangement = Arrangement<Horizontal>;

/// A Column's arrangement (Compose's `Arrangement.Vertical`).
pub type VerticalArrangement = Arrangement<Vertical>;

/// A custom horizontal arrangement's function: (density, total size, sizes, layout direction, out positions).
type ArrangeFn = Rc<dyn Fn(&Density, i32, &[i32], LayoutDirection, &mut [i32])>;

#[derive(Clone)]
enum Kind {
    Start,
    End,
    Top,
    Bottom,
    Center,
    SpaceEvenly,
    SpaceBetween,
    SpaceAround,
    AbsoluteLeft,
    AbsoluteCenter,
    AbsoluteRight,
    AbsoluteSpaceBetween,
    AbsoluteSpaceEvenly,
    AbsoluteSpaceAround,
    /// Compose's SpacedAligned: `space` between children, the group placed by `alignment` in what is left.
    SpacedAligned {
        space: Dp,
        rtl_mirror: bool,
        alignment: Option<SpacingAlignment>,
    },
    Custom {
        spacing: Dp,
        arrange: ArrangeFn,
    },
}

/// Where a spaced arrangement puts its children's group in the space they leave (Compose's
/// SpacingAlignmentCalculator).
#[derive(Clone, PartialEq, Debug)]
pub enum SpacingAlignment {
    Horizontal(HorizontalAlignment),
    Vertical(VerticalAlignment),
}

impl SpacingAlignment {
    fn align(&self, size: i32, layout_direction: LayoutDirection) -> i32 {
        match self {
            SpacingAlignment::Horizontal(a) => a.align(0, size, layout_direction),
            SpacingAlignment::Vertical(a) => a.align(0, size),
        }
    }
}

/// An alignment a spaced or aligned arrangement takes: a [`HorizontalAlignment`] for a Row's, a
/// [`VerticalAlignment`] for a Column's (Compose's `spacedBy(space, alignment)` and `aligned(alignment)`
/// overloads).
pub trait ArrangementAlignment {
    type Axis: ArrangementAxis;
    fn into_spacing_alignment(self) -> SpacingAlignment;
}

impl ArrangementAlignment for HorizontalAlignment {
    type Axis = Horizontal;
    fn into_spacing_alignment(self) -> SpacingAlignment {
        SpacingAlignment::Horizontal(self)
    }
}

impl ArrangementAlignment for VerticalAlignment {
    type Axis = Vertical;
    fn into_spacing_alignment(self) -> SpacingAlignment {
        SpacingAlignment::Vertical(self)
    }
}

impl<A: ArrangementAxis> Arrangement<A> {
    const fn of(kind: Kind) -> Arrangement<A> {
        Arrangement { kind, axis: PhantomData }
    }

    /// The children in the middle, no space between them (Compose's `Arrangement.Center`).
    pub const Center: Arrangement<A> = Arrangement::of(Kind::Center);

    /// The free space spread evenly before, between and after the children (Compose's `Arrangement.SpaceEvenly`).
    pub const SpaceEvenly: Arrangement<A> = Arrangement::of(Kind::SpaceEvenly);

    /// The free space spread evenly between the children, none before the first or after the last (Compose's
    /// `Arrangement.SpaceBetween`).
    pub const SpaceBetween: Arrangement<A> = Arrangement::of(Kind::SpaceBetween);

    /// The free space spread evenly between the children, half as much before the first and after the last
    /// (Compose's `Arrangement.SpaceAround`).
    pub const SpaceAround: Arrangement<A> = Arrangement::of(Kind::SpaceAround);

    /// `space` between children, packed at the start (mirrored right to left), however much space there is
    /// (Compose's `Arrangement.spacedBy(space)`).
    pub fn spaced_by(space: Dp) -> Arrangement<A> {
        Arrangement::of(Kind::SpacedAligned {
            space,
            rtl_mirror: true,
            alignment: Some(SpacingAlignment::Horizontal(Alignment::Start)),
        })
    }

    /// `space` between children, the group placed by `alignment` in what is left: a [`HorizontalAlignment`] for a
    /// Row, a [`VerticalAlignment`] for a Column (Compose's `Arrangement.spacedBy(space, alignment)`).
    pub fn spaced_by_aligned<L: ArrangementAlignment<Axis = A>>(space: Dp, alignment: L) -> Arrangement<A> {
        let alignment = alignment.into_spacing_alignment();
        // (a horizontal one mirrors right to left, a vertical one has no direction)
        let rtl_mirror = matches!(alignment, SpacingAlignment::Horizontal(_));
        Arrangement::of(Kind::SpacedAligned { space, rtl_mirror, alignment: Some(alignment) })
    }

    /// The children next to each other, placed by `alignment` (Compose's `Arrangement.aligned(alignment)`).
    pub fn aligned<L: ArrangementAlignment<Axis = A>>(alignment: L) -> Arrangement<A> {
        let alignment = alignment.into_spacing_alignment();
        let rtl_mirror = matches!(alignment, SpacingAlignment::Horizontal(_));
        Arrangement::of(Kind::SpacedAligned { space: Dp(0.0), rtl_mirror, alignment: Some(alignment) })
    }

    /// The space it wants between two children (Compose's `spacing`): its layout's intrinsic sizes count it.
    pub fn spacing(&self) -> Dp {
        match &self.kind {
            Kind::SpacedAligned { space, .. } => *space,
            Kind::Custom { spacing, .. } => *spacing,
            _ => Dp(0.0),
        }
    }

    /// Writes each child's position into `out_positions`, from `sizes` and the `total_size` there is.
    fn arrange_in(
        &self,
        density: &Density,
        total_size: i32,
        sizes: &[i32],
        layout_direction: LayoutDirection,
        out_positions: &mut [i32],
    ) {
        let ltr = layout_direction == LayoutDirection::Ltr;
        match &self.kind {
            Kind::Start if ltr => place_left_or_top(sizes, out_positions, false),
            Kind::Start => place_right_or_bottom(total_size, sizes, out_positions, true),
            Kind::End if ltr => place_right_or_bottom(total_size, sizes, out_positions, false),
            Kind::End => place_left_or_top(sizes, out_positions, true),
            Kind::Top => place_left_or_top(sizes, out_positions, false),
            Kind::Bottom => place_right_or_bottom(total_size, sizes, out_positions, false),
            Kind::Center => place_center(total_size, sizes, out_positions, !ltr),
            Kind::SpaceEvenly => place_space_evenly(total_size, sizes, out_positions, !ltr),
            Kind::SpaceBetween => place_space_between(total_size, sizes, out_positions, !ltr),
            Kind::SpaceAround => place_space_around(total_size, sizes, out_positions, !ltr),
            Kind::AbsoluteLeft => place_left_or_top(sizes, out_positions, false),
            Kind::AbsoluteCenter => place_center(total_size, sizes, out_positions, false),
            Kind::AbsoluteRight => place_right_or_bottom(total_size, sizes, out_positions, false),
            Kind::AbsoluteSpaceBetween => place_space_between(total_size, sizes, out_positions, false),
            Kind::AbsoluteSpaceEvenly => place_space_evenly(total_size, sizes, out_positions, false),
            Kind::AbsoluteSpaceAround => place_space_around(total_size, sizes, out_positions, false),
            Kind::SpacedAligned { space, rtl_mirror, alignment } => spaced_aligned(
                density.round_to_px(*space),
                *rtl_mirror,
                alignment.as_ref(),
                total_size,
                sizes,
                layout_direction,
                out_positions,
            ),
            Kind::Custom { arrange, .. } => arrange(density, total_size, sizes, layout_direction, out_positions),
        }
    }
}

impl Arrangement<Horizontal> {
    /// The children at the start (the left, or the right right to left), next to each other (Compose's
    /// `Arrangement.Start`).
    pub const Start: HorizontalArrangement = Arrangement::of(Kind::Start);

    /// The children at the end (Compose's `Arrangement.End`).
    pub const End: HorizontalArrangement = Arrangement::of(Kind::End);

    /// A horizontal arrangement of one's own (Compose's `object : Arrangement.Horizontal`): `arrange(density,
    /// total_size, sizes, layout_direction, out_positions)` writes the positions; `spacing` is what intrinsic
    /// sizes count between two children.
    pub fn horizontal(
        spacing: Dp,
        arrange: impl Fn(&Density, i32, &[i32], LayoutDirection, &mut [i32]) + 'static,
    ) -> HorizontalArrangement {
        Arrangement::of(Kind::Custom { spacing, arrange: Rc::new(arrange) })
    }

    /// Writes into `out_positions` where each child of `sizes` goes in `total_size`, laid out
    /// `layout_direction` (Compose's `Arrangement.Horizontal.arrange`).
    pub fn arrange(
        &self,
        density: &Density,
        total_size: i32,
        sizes: &[i32],
        layout_direction: LayoutDirection,
        out_positions: &mut [i32],
    ) {
        self.arrange_in(density, total_size, sizes, layout_direction, out_positions)
    }
}

impl Arrangement<Vertical> {
    /// The children at the top, one under the other (Compose's `Arrangement.Top`).
    pub const Top: VerticalArrangement = Arrangement::of(Kind::Top);

    /// The children at the bottom (Compose's `Arrangement.Bottom`).
    pub const Bottom: VerticalArrangement = Arrangement::of(Kind::Bottom);

    /// A vertical arrangement of one's own (Compose's `object : Arrangement.Vertical`): `arrange(density,
    /// total_size, sizes, out_positions)` writes the positions; `spacing` is what intrinsic sizes count between
    /// two children.
    pub fn vertical(spacing: Dp, arrange: impl Fn(&Density, i32, &[i32], &mut [i32]) + 'static) -> VerticalArrangement {
        Arrangement::of(Kind::Custom {
            spacing,
            arrange: Rc::new(move |d, total, sizes, _, out| arrange(d, total, sizes, out)),
        })
    }

    /// Writes into `out_positions` where each child of `sizes` goes in `total_size` (Compose's
    /// `Arrangement.Vertical.arrange`).
    pub fn arrange(&self, density: &Density, total_size: i32, sizes: &[i32], out_positions: &mut [i32]) {
        self.arrange_in(density, total_size, sizes, LayoutDirection::Ltr, out_positions)
    }
}

impl<A: ArrangementAxis> Clone for Arrangement<A> {
    fn clone(&self) -> Self {
        Arrangement::of(self.kind.clone())
    }
}

impl<A: ArrangementAxis> PartialEq for Arrangement<A> {
    fn eq(&self, other: &Self) -> bool {
        use Kind::*;
        match (&self.kind, &other.kind) {
            (
                SpacedAligned { space: s1, rtl_mirror: r1, alignment: a1 },
                SpacedAligned { space: s2, rtl_mirror: r2, alignment: a2 },
            ) => same_dp(*s1, *s2) && r1 == r2 && a1 == a2,
            (Custom { spacing: s1, arrange: f1 }, Custom { spacing: s2, arrange: f2 }) => {
                same_dp(*s1, *s2) && Rc::ptr_eq(f1, f2)
            }
            // (the others carry nothing)
            (a, b) => std::mem::discriminant(a) == std::mem::discriminant(b),
        }
    }
}

impl<A: ArrangementAxis> fmt::Debug for Arrangement<A> {
    /// Compose's toString.
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let name = match &self.kind {
            Kind::Start => "Arrangement#Start",
            Kind::End => "Arrangement#End",
            Kind::Top => "Arrangement#Top",
            Kind::Bottom => "Arrangement#Bottom",
            Kind::Center => "Arrangement#Center",
            Kind::SpaceEvenly => "Arrangement#SpaceEvenly",
            Kind::SpaceBetween => "Arrangement#SpaceBetween",
            Kind::SpaceAround => "Arrangement#SpaceAround",
            Kind::AbsoluteLeft => "AbsoluteArrangement#Left",
            Kind::AbsoluteCenter => "AbsoluteArrangement#Center",
            Kind::AbsoluteRight => "AbsoluteArrangement#Right",
            Kind::AbsoluteSpaceBetween => "AbsoluteArrangement#SpaceBetween",
            Kind::AbsoluteSpaceEvenly => "AbsoluteArrangement#SpaceEvenly",
            Kind::AbsoluteSpaceAround => "AbsoluteArrangement#SpaceAround",
            Kind::SpacedAligned { space, rtl_mirror, alignment } => {
                let prefix = if *rtl_mirror { "" } else { "Absolute" };
                return write!(f, "{prefix}Arrangement#spacedAligned({space:?}, {alignment:?})");
            }
            Kind::Custom { spacing, .. } => return write!(f, "Arrangement(custom, spacing = {spacing:?})"),
        };
        f.write_str(name)
    }
}

/// Arrangements that don't mirror right to left (Compose's `Arrangement.Absolute`).
pub struct AbsoluteArrangement;

impl AbsoluteArrangement {
    /// The children at the left, whatever the layout direction (Compose's `Arrangement.Absolute.Left`).
    pub const Left: HorizontalArrangement = Arrangement::of(Kind::AbsoluteLeft);

    /// The children in the middle (Compose's `Arrangement.Absolute.Center`).
    pub const Center: HorizontalArrangement = Arrangement::of(Kind::AbsoluteCenter);

    /// The children at the right (Compose's `Arrangement.Absolute.Right`).
    pub const Right: HorizontalArrangement = Arrangement::of(Kind::AbsoluteRight);

    /// The free space between the children, from the left (Compose's `Arrangement.Absolute.SpaceBetween`).
    pub const SpaceBetween: HorizontalArrangement = Arrangement::of(Kind::AbsoluteSpaceBetween);

    /// The free space before, between and after the children, from the left (Compose's
    /// `Arrangement.Absolute.SpaceEvenly`).
    pub const SpaceEvenly: HorizontalArrangement = Arrangement::of(Kind::AbsoluteSpaceEvenly);

    /// The free space around the children, from the left (Compose's `Arrangement.Absolute.SpaceAround`).
    pub const SpaceAround: HorizontalArrangement = Arrangement::of(Kind::AbsoluteSpaceAround);

    /// `space` between children from the left (or the top), not aligned (Compose's
    /// `Arrangement.Absolute.spacedBy(space)`).
    pub fn spaced_by<A: ArrangementAxis>(space: Dp) -> Arrangement<A> {
        Arrangement::of(Kind::SpacedAligned { space, rtl_mirror: false, alignment: None })
    }

    /// `space` between children from the left (or the top), the group placed by `alignment` (Compose's
    /// `Arrangement.Absolute.spacedBy(space, alignment)`).
    pub fn spaced_by_aligned<L: ArrangementAlignment>(space: Dp, alignment: L) -> Arrangement<L::Axis> {
        Arrangement::of(Kind::SpacedAligned {
            space,
            rtl_mirror: false,
            alignment: Some(alignment.into_spacing_alignment()),
        })
    }

    /// The children next to each other from the left, the group placed by `alignment` (Compose's
    /// `Arrangement.Absolute.aligned(alignment)`).
    pub fn aligned(alignment: HorizontalAlignment) -> HorizontalArrangement {
        Arrangement::of(Kind::SpacedAligned {
            space: Dp(0.0),
            rtl_mirror: false,
            alignment: Some(SpacingAlignment::Horizontal(alignment)),
        })
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The math (Compose's Arrangement.place*)

/// Compose's `forEachIndexed(reversed)`: `action` for each index, in order or last to first.
fn for_each_index(n: usize, reversed: bool, mut action: impl FnMut(usize)) {
    if reversed { (0..n).rev().for_each(&mut action) } else { (0..n).for_each(&mut action) }
}

/// Compose's SpacedAligned.arrange.
fn spaced_aligned(
    space_px: i32,
    rtl_mirror: bool,
    alignment: Option<&SpacingAlignment>,
    total_size: i32,
    sizes: &[i32],
    layout_direction: LayoutDirection,
    out_positions: &mut [i32],
) {
    if sizes.is_empty() {
        return;
    }
    let is_rtl = rtl_mirror && layout_direction == LayoutDirection::Rtl;
    let mut last_space = 0;
    let mut free_space = total_size;
    if is_rtl {
        for (index, &size) in sizes.iter().enumerate() {
            out_positions[index] = 0.max(free_space - size);
            last_space = space_px.min(out_positions[index]);
            free_space = out_positions[index] - last_space;
        }
        free_space += last_space;
    } else {
        let mut occupied = 0;
        for (index, &size) in sizes.iter().enumerate() {
            out_positions[index] = occupied.min(total_size - size);
            last_space = space_px.min(total_size - out_positions[index] - size);
            occupied = out_positions[index] + size + last_space;
        }
        occupied -= last_space;
        free_space = total_size - occupied;
    }
    if let Some(alignment) = alignment
        && free_space > 0
    {
        let group_position = alignment.align(free_space, layout_direction);
        // Packed at the start (0) left to right; at the end (the free space) right to left.
        let offset = if is_rtl { group_position - free_space } else { group_position };
        if offset != 0 {
            for p in out_positions.iter_mut() {
                *p += offset;
            }
        }
    }
}

fn place_right_or_bottom(total_size: i32, sizes: &[i32], out_positions: &mut [i32], reverse_input: bool) {
    let consumed: i32 = sizes.iter().sum();
    let mut current = total_size - consumed;
    for_each_index(sizes.len(), reverse_input, |index| {
        out_positions[index] = current;
        current += sizes[index];
    });
}

fn place_left_or_top(sizes: &[i32], out_positions: &mut [i32], reverse_input: bool) {
    let mut current = 0;
    for_each_index(sizes.len(), reverse_input, |index| {
        out_positions[index] = current;
        current += sizes[index];
    });
}

fn place_center(total_size: i32, sizes: &[i32], out_positions: &mut [i32], reverse_input: bool) {
    let consumed: i32 = sizes.iter().sum();
    let mut current = (total_size - consumed) as f32 / 2.0;
    for_each_index(sizes.len(), reverse_input, |index| {
        out_positions[index] = round(current);
        current += sizes[index] as f32;
    });
}

fn place_space_evenly(total_size: i32, sizes: &[i32], out_positions: &mut [i32], reverse_input: bool) {
    let consumed: i32 = sizes.iter().sum();
    let gap = (total_size - consumed) as f32 / (sizes.len() + 1) as f32;
    let mut current = gap;
    for_each_index(sizes.len(), reverse_input, |index| {
        out_positions[index] = round(current);
        current += sizes[index] as f32 + gap;
    });
}

fn place_space_between(total_size: i32, sizes: &[i32], out_positions: &mut [i32], reverse_input: bool) {
    if sizes.is_empty() {
        return;
    }
    let consumed: i32 = sizes.iter().sum();
    let gaps = (sizes.len() - 1).max(1);
    let gap = (total_size - consumed) as f32 / gaps as f32;
    // One child right to left: after the one gap, at the right.
    let mut current = if reverse_input && sizes.len() == 1 { gap } else { 0.0 };
    for_each_index(sizes.len(), reverse_input, |index| {
        out_positions[index] = round(current);
        current += sizes[index] as f32 + gap;
    });
}

fn place_space_around(total_size: i32, sizes: &[i32], out_positions: &mut [i32], reverse_input: bool) {
    let consumed: i32 = sizes.iter().sum();
    let gap = if sizes.is_empty() { 0.0 } else { (total_size - consumed) as f32 / sizes.len() as f32 };
    let mut current = gap / 2.0;
    for_each_index(sizes.len(), reverse_input, |index| {
        out_positions[index] = round(current);
        current += sizes[index] as f32 + gap;
    });
}
