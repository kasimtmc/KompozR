//! Padding, as Compose's `Modifier.padding` family and `PaddingValues`: space around what a layout wraps.

use super::same_dp;
use kompoz_graphics::{Dp, LayoutDirection, UnitExt};
use kompoz_runtime::named;
use kompoz_ui::layout::LayoutModifierNode;
use kompoz_ui::{
    Constraints, Measurable, MeasureResult, MeasureScope, Modifier, ModifierNode, ModifierNodeElement,
    node_capabilities,
};
use std::fmt;
use std::ops::{Add, Sub};
use std::rc::Rc;

/// The padding modifiers (Compose's `Modifier.padding` overloads and `Modifier.absolutePadding`). Start and end
/// mirror right to left; left and right don't.
#[named(trait PaddingModifiers)]
impl Modifier {
    /// `all` around (Compose's `padding(all)`).
    fn padding(self, all: Dp) -> Modifier {
        self.then_element(PaddingElement::new(all, all, all, all, true, "padding"))
    }

    /// `horizontal` at the start and the end, `vertical` at the top and the bottom (Compose's
    /// `padding(horizontal, vertical)`).
    fn padding_hv(self, #[default(0.dp())] horizontal: Dp, #[default(0.dp())] vertical: Dp) -> Modifier {
        self.then_element(PaddingElement::new(horizontal, vertical, horizontal, vertical, true, "padding"))
    }

    /// [`padding_hv`](PaddingModifiers::padding_hv) with all its parameters, for plain code.
    fn padding_hv_with(self, horizontal: Dp, vertical: Dp) -> Modifier {
        self.then_element(PaddingElement::new(horizontal, vertical, horizontal, vertical, true, "padding"))
    }

    /// Each side its own (Compose's `padding(start, top, end, bottom)`).
    fn padding_sides(
        self,
        #[default(0.dp())] start: Dp,
        #[default(0.dp())] top: Dp,
        #[default(0.dp())] end: Dp,
        #[default(0.dp())] bottom: Dp,
    ) -> Modifier {
        self.then_element(PaddingElement::new(start, top, end, bottom, true, "padding"))
    }

    /// [`padding_sides`](PaddingModifiers::padding_sides) with all its parameters, for plain code.
    fn padding_sides_with(self, start: Dp, top: Dp, end: Dp, bottom: Dp) -> Modifier {
        self.then_element(PaddingElement::new(start, top, end, bottom, true, "padding"))
    }

    /// What `padding_values` says (Compose's `padding(paddingValues)`).
    fn padding_values(self, padding_values: PaddingValues) -> Modifier {
        self.then_element(PaddingValuesElement { padding_values })
    }

    /// Each side its own, left and right whatever the layout direction (Compose's `absolutePadding`).
    fn absolute_padding(
        self,
        #[default(0.dp())] left: Dp,
        #[default(0.dp())] top: Dp,
        #[default(0.dp())] right: Dp,
        #[default(0.dp())] bottom: Dp,
    ) -> Modifier {
        self.then_element(PaddingElement::new(left, top, right, bottom, false, "absolutePadding"))
    }

    /// [`absolute_padding`](PaddingModifiers::absolute_padding) with all its parameters, for plain code.
    fn absolute_padding_with(self, left: Dp, top: Dp, right: Dp, bottom: Dp) -> Modifier {
        self.then_element(PaddingElement::new(left, top, right, bottom, false, "absolutePadding"))
    }
}

/// Compose's `Constraints.offset(horizontal, vertical)`: grown (shrunk, for negative values) by that many pixels,
/// the mins at least 0, an unbounded max left so.
pub(crate) fn offset(constraints: Constraints, horizontal: i32, vertical: i32) -> Constraints {
    let add_max = |max: i32, value: i32| if max == Constraints::Infinity { max } else { (max + value).max(0) };
    Constraints::new(
        (constraints.min_width + horizontal).max(0),
        add_max(constraints.max_width, horizontal),
        (constraints.min_height + vertical).max(0),
        add_max(constraints.max_height, vertical),
    )
}

// ---------------------------------------------------------------------------------------------------------------
// PaddingValues

/// Padding on each side (Compose's PaddingValues): its start and end mirrored right to left, or left and right
/// that aren't (absolute), or what a [`CustomPaddingValues`] works out. Added and subtracted side by side (`+`,
/// `-`, a difference at least zero). Equal when made the same way of equal lengths (a custom one: when it is the
/// same one).
#[derive(Clone, PartialEq)]
pub struct PaddingValues {
    repr: Repr,
}

#[derive(Clone)]
enum Repr {
    Relative { start: Dp, top: Dp, end: Dp, bottom: Dp },
    Absolute { left: Dp, top: Dp, right: Dp, bottom: Dp },
    Plus(Rc<PaddingValues>, Rc<PaddingValues>),
    Minus(Rc<PaddingValues>, Rc<PaddingValues>),
    Custom(Rc<dyn CustomPaddingValues>),
}

/// Padding values of one's own (Compose's `PaddingValues` interface implemented): its sides, worked out when
/// asked.
pub trait CustomPaddingValues {
    fn calculate_left_padding(&self, layout_direction: LayoutDirection) -> Dp;
    fn calculate_top_padding(&self) -> Dp;
    fn calculate_right_padding(&self, layout_direction: LayoutDirection) -> Dp;
    fn calculate_bottom_padding(&self) -> Dp;
}

#[track_caller]
fn check_non_negative(sides: [Dp; 4]) {
    assert!(sides.iter().all(|d| d.0 >= 0.0), "Padding must be non-negative (got {sides:?})");
}

impl PaddingValues {
    const fn of(repr: Repr) -> PaddingValues {
        PaddingValues { repr }
    }

    /// No padding (Compose's `PaddingValues.Zero`).
    pub const Zero: PaddingValues =
        PaddingValues::of(Repr::Absolute { left: Dp(0.0), top: Dp(0.0), right: Dp(0.0), bottom: Dp(0.0) });

    /// `all` on each side (Compose's `PaddingValues(all)`).
    #[track_caller]
    pub fn all(all: Dp) -> PaddingValues {
        PaddingValues::new(all, all, all, all)
    }

    /// `horizontal` at the start and the end, `vertical` at the top and the bottom (Compose's
    /// `PaddingValues(horizontal, vertical)`).
    #[track_caller]
    pub fn hv(horizontal: Dp, vertical: Dp) -> PaddingValues {
        PaddingValues::new(horizontal, vertical, horizontal, vertical)
    }

    /// Each side its own, start and end mirrored right to left (Compose's `PaddingValues(start, top, end,
    /// bottom)`). Panics on a negative side.
    #[track_caller]
    pub fn new(start: Dp, top: Dp, end: Dp, bottom: Dp) -> PaddingValues {
        check_non_negative([start, top, end, bottom]);
        PaddingValues::of(Repr::Relative { start, top, end, bottom })
    }

    /// Each side its own, left and right whatever the layout direction (Compose's `PaddingValues.Absolute`).
    /// Panics on a negative side.
    #[track_caller]
    pub fn absolute(left: Dp, top: Dp, right: Dp, bottom: Dp) -> PaddingValues {
        check_non_negative([left, top, right, bottom]);
        PaddingValues::of(Repr::Absolute { left, top, right, bottom })
    }

    /// Padding values `custom` works out.
    pub fn custom(custom: impl CustomPaddingValues + 'static) -> PaddingValues {
        PaddingValues::of(Repr::Custom(Rc::new(custom)))
    }

    /// The padding at the left, laid out `layout_direction` (Compose's `calculateLeftPadding`).
    pub fn calculate_left_padding(&self, layout_direction: LayoutDirection) -> Dp {
        match &self.repr {
            Repr::Relative { start, end, .. } => {
                if layout_direction == LayoutDirection::Ltr {
                    *start
                } else {
                    *end
                }
            }
            Repr::Absolute { left, .. } => *left,
            Repr::Plus(a, b) => a.calculate_left_padding(layout_direction) + b.calculate_left_padding(layout_direction),
            Repr::Minus(a, b) => (a.calculate_left_padding(layout_direction)
                - b.calculate_left_padding(layout_direction))
            .coerce_at_least(Dp(0.0)),
            Repr::Custom(c) => c.calculate_left_padding(layout_direction),
        }
    }

    /// The padding at the top (Compose's `calculateTopPadding`).
    pub fn calculate_top_padding(&self) -> Dp {
        match &self.repr {
            Repr::Relative { top, .. } | Repr::Absolute { top, .. } => *top,
            Repr::Plus(a, b) => a.calculate_top_padding() + b.calculate_top_padding(),
            Repr::Minus(a, b) => (a.calculate_top_padding() - b.calculate_top_padding()).coerce_at_least(Dp(0.0)),
            Repr::Custom(c) => c.calculate_top_padding(),
        }
    }

    /// The padding at the right, laid out `layout_direction` (Compose's `calculateRightPadding`).
    pub fn calculate_right_padding(&self, layout_direction: LayoutDirection) -> Dp {
        match &self.repr {
            Repr::Relative { start, end, .. } => {
                if layout_direction == LayoutDirection::Ltr {
                    *end
                } else {
                    *start
                }
            }
            Repr::Absolute { right, .. } => *right,
            Repr::Plus(a, b) => {
                a.calculate_right_padding(layout_direction) + b.calculate_right_padding(layout_direction)
            }
            Repr::Minus(a, b) => (a.calculate_right_padding(layout_direction)
                - b.calculate_right_padding(layout_direction))
            .coerce_at_least(Dp(0.0)),
            Repr::Custom(c) => c.calculate_right_padding(layout_direction),
        }
    }

    /// The padding at the bottom (Compose's `calculateBottomPadding`).
    pub fn calculate_bottom_padding(&self) -> Dp {
        match &self.repr {
            Repr::Relative { bottom, .. } | Repr::Absolute { bottom, .. } => *bottom,
            Repr::Plus(a, b) => a.calculate_bottom_padding() + b.calculate_bottom_padding(),
            Repr::Minus(a, b) => (a.calculate_bottom_padding() - b.calculate_bottom_padding()).coerce_at_least(Dp(0.0)),
            Repr::Custom(c) => c.calculate_bottom_padding(),
        }
    }

    /// The padding at the start: the left left to right, the right right to left (Compose's
    /// `calculateStartPadding`).
    pub fn calculate_start_padding(&self, layout_direction: LayoutDirection) -> Dp {
        if layout_direction == LayoutDirection::Ltr {
            self.calculate_left_padding(layout_direction)
        } else {
            self.calculate_right_padding(layout_direction)
        }
    }

    /// The padding at the end (Compose's `calculateEndPadding`).
    pub fn calculate_end_padding(&self, layout_direction: LayoutDirection) -> Dp {
        if layout_direction == LayoutDirection::Ltr {
            self.calculate_right_padding(layout_direction)
        } else {
            self.calculate_left_padding(layout_direction)
        }
    }
}

/// Compose's `PaddingValues(start = 0.dp, top = 0.dp, end = 0.dp, bottom = 0.dp)`, with named arguments in a
/// kompozable body (`PaddingValues(top = 8.dp())`); plain code calls [`PaddingValues::new`] and the others.
#[named]
pub fn PaddingValues(
    #[default(0.dp())] start: Dp,
    #[default(0.dp())] top: Dp,
    #[default(0.dp())] end: Dp,
    #[default(0.dp())] bottom: Dp,
) -> PaddingValues {
    PaddingValues::new(start, top, end, bottom)
}

impl PartialEq for Repr {
    fn eq(&self, other: &Self) -> bool {
        let same = |a: [Dp; 4], b: [Dp; 4]| a.iter().zip(&b).all(|(x, y)| same_dp(*x, *y));
        match (self, other) {
            (Repr::Relative { start, top, end, bottom }, Repr::Relative { start: s, top: t, end: e, bottom: b }) => {
                same([*start, *top, *end, *bottom], [*s, *t, *e, *b])
            }
            (Repr::Absolute { left, top, right, bottom }, Repr::Absolute { left: l, top: t, right: r, bottom: b }) => {
                same([*left, *top, *right, *bottom], [*l, *t, *r, *b])
            }
            (Repr::Plus(a, b), Repr::Plus(c, d)) | (Repr::Minus(a, b), Repr::Minus(c, d)) => a == c && b == d,
            (Repr::Custom(a), Repr::Custom(b)) => Rc::ptr_eq(a, b),
            _ => false,
        }
    }
}

impl fmt::Debug for PaddingValues {
    /// Compose's toString.
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match &self.repr {
            Repr::Relative { start, top, end, bottom } => {
                write!(f, "PaddingValues(start={start:?}, top={top:?}, end={end:?}, bottom={bottom:?})")
            }
            Repr::Absolute { left, top, right, bottom } => {
                write!(f, "PaddingValues.Absolute(left={left:?}, top={top:?}, right={right:?}, bottom={bottom:?})")
            }
            Repr::Plus(a, b) => write!(f, "({a:?} + {b:?})"),
            Repr::Minus(a, b) => write!(f, "({a:?} - {b:?})"),
            Repr::Custom(_) => write!(f, "PaddingValues(custom)"),
        }
    }
}

/// Side by side (Compose's `PaddingValues.plus`).
impl Add for PaddingValues {
    type Output = PaddingValues;
    fn add(self, other: PaddingValues) -> PaddingValues {
        PaddingValues::of(Repr::Plus(Rc::new(self), Rc::new(other)))
    }
}

/// Side by side, each at least zero (Compose's `PaddingValues.minus`).
impl Sub for PaddingValues {
    type Output = PaddingValues;
    fn sub(self, other: PaddingValues) -> PaddingValues {
        PaddingValues::of(Repr::Minus(Rc::new(self), Rc::new(other)))
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Nodes

struct PaddingElement {
    start: Dp,
    top: Dp,
    end: Dp,
    bottom: Dp,
    rtl_aware: bool,
    name: &'static str,
}

impl PaddingElement {
    /// Panics on a negative side (an unspecified one goes).
    #[track_caller]
    fn new(start: Dp, top: Dp, end: Dp, bottom: Dp, rtl_aware: bool, name: &'static str) -> PaddingElement {
        let sides = [start, top, end, bottom];
        assert!(sides.iter().all(|d| d.0 >= 0.0 || d.is_unspecified()), "Padding must be non-negative (got {sides:?})");
        PaddingElement { start, top, end, bottom, rtl_aware, name }
    }
}

impl PartialEq for PaddingElement {
    fn eq(&self, other: &Self) -> bool {
        same_dp(self.start, other.start)
            && same_dp(self.top, other.top)
            && same_dp(self.end, other.end)
            && same_dp(self.bottom, other.bottom)
            && self.rtl_aware == other.rtl_aware
    }
}

impl ModifierNodeElement for PaddingElement {
    type Node = PaddingNode;
    fn create(&self) -> PaddingNode {
        PaddingNode { start: self.start, top: self.top, end: self.end, bottom: self.bottom, rtl_aware: self.rtl_aware }
    }
    fn update(&self, node: &mut PaddingNode) {
        node.start = self.start;
        node.top = self.top;
        node.end = self.end;
        node.bottom = self.bottom;
        node.rtl_aware = self.rtl_aware;
    }
    fn name(&self) -> &'static str {
        self.name
    }
}

/// The node of `padding` and `absolute_padding` (Compose's PaddingNode): what it wraps measured in what is left
/// inside the padding, placed past it (from the start, or the left when not `rtl_aware`). Its intrinsic sizes are
/// measured (Compose's default ones).
struct PaddingNode {
    start: Dp,
    top: Dp,
    end: Dp,
    bottom: Dp,
    rtl_aware: bool,
}

impl ModifierNode for PaddingNode {
    node_capabilities!(layout);
}

impl LayoutModifierNode for PaddingNode {
    fn measure(
        &mut self,
        scope: &MeasureScope,
        measurable: &dyn Measurable,
        constraints: Constraints,
    ) -> MeasureResult {
        let (start, top) = (scope.round_to_px(self.start), scope.round_to_px(self.top));
        let horizontal = start + scope.round_to_px(self.end);
        let vertical = top + scope.round_to_px(self.bottom);
        let placeable = measurable.measure(offset(constraints, -horizontal, -vertical));
        let width = constraints.constrain_width(placeable.width() + horizontal);
        let height = constraints.constrain_height(placeable.height() + vertical);
        let rtl_aware = self.rtl_aware;
        scope.layout(width, height, move |p| {
            if rtl_aware {
                p.place_relative(&placeable, start, top);
            } else {
                p.place(&placeable, start, top);
            }
        })
    }
}

#[derive(PartialEq)]
struct PaddingValuesElement {
    padding_values: PaddingValues,
}

impl ModifierNodeElement for PaddingValuesElement {
    type Node = PaddingValuesNode;
    fn create(&self) -> PaddingValuesNode {
        PaddingValuesNode { padding_values: self.padding_values.clone() }
    }
    fn update(&self, node: &mut PaddingValuesNode) {
        node.padding_values = self.padding_values.clone();
    }
    fn name(&self) -> &'static str {
        "padding"
    }
}

/// The node of `padding_values` (Compose's PaddingValuesModifier): as [`PaddingNode`], its sides asked of its
/// padding values in the layout's direction.
struct PaddingValuesNode {
    padding_values: PaddingValues,
}

impl ModifierNode for PaddingValuesNode {
    node_capabilities!(layout);
}

impl LayoutModifierNode for PaddingValuesNode {
    fn measure(
        &mut self,
        scope: &MeasureScope,
        measurable: &dyn Measurable,
        constraints: Constraints,
    ) -> MeasureResult {
        let direction = scope.layout_direction();
        let left = self.padding_values.calculate_left_padding(direction);
        let top = self.padding_values.calculate_top_padding();
        let right = self.padding_values.calculate_right_padding(direction);
        let bottom = self.padding_values.calculate_bottom_padding();
        check_non_negative([left, top, right, bottom]);
        let rounded_left = scope.round_to_px(left);
        let horizontal = rounded_left + scope.round_to_px(right);
        let rounded_top = scope.round_to_px(top);
        let vertical = rounded_top + scope.round_to_px(bottom);
        let placeable = measurable.measure(offset(constraints, -horizontal, -vertical));
        let width = constraints.constrain_width(placeable.width() + horizontal);
        let height = constraints.constrain_height(placeable.height() + vertical);
        scope.layout(width, height, move |p| p.place(&placeable, rounded_left, rounded_top))
    }
}
