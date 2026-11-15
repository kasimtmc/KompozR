//! Padding from alignment lines, as Compose's `Modifier.paddingFrom` and `Modifier.paddingFromBaseline`: space
//! enough that a line of what a layout wraps is a given distance from its edges.

use super::{coerce_in, same_dp};
use kompoz_graphics::{Dp, TextUnit};
use kompoz_runtime::named;
use kompoz_ui::layout::LayoutModifierNode;
use kompoz_ui::{
    AlignmentLine, Constraints, FirstBaseline, LastBaseline, Measurable, MeasureResult, MeasureScope, Modifier,
    ModifierNode, ModifierNodeElement, node_capabilities,
};

/// The alignment line padding modifiers (Compose's `paddingFrom` and `paddingFromBaseline`, in dp and, the `_sp`
/// ones, Compose's TextUnit overloads in sp, which follow the font scale).
#[named(trait AlignmentLinePaddingModifiers)]
impl Modifier {
    /// Padding enough that `alignment_line` is at least `before` from the top (a horizontal line) or the start
    /// (a vertical one) and at least `after` from the bottom or the end, as far as the incoming max lets it; an
    /// unspecified distance isn't asked for (Compose's `paddingFrom(alignmentLine, before, after)`). Content that
    /// has no such line is taken to have it at its top or start. Panics on a negative distance.
    fn padding_from(
        self,
        alignment_line: impl Into<AlignmentLine>,
        #[default(Dp::Unspecified)] before: Dp,
        #[default(Dp::Unspecified)] after: Dp,
    ) -> Modifier {
        self.then_element(AlignmentLineOffsetElement::new(
            alignment_line.into(),
            Distance::Dp(before),
            Distance::Dp(after),
        ))
    }

    /// [`padding_from`](AlignmentLinePaddingModifiers::padding_from) with all its parameters, for plain code.
    fn padding_from_with(self, alignment_line: impl Into<AlignmentLine>, before: Dp, after: Dp) -> Modifier {
        self.then_element(AlignmentLineOffsetElement::new(
            alignment_line.into(),
            Distance::Dp(before),
            Distance::Dp(after),
        ))
    }

    /// The same, the distances in sp (Compose's `paddingFrom(alignmentLine, before: TextUnit, after: TextUnit)`).
    fn padding_from_sp(
        self,
        alignment_line: impl Into<AlignmentLine>,
        #[default(TextUnit::Unspecified)] before: TextUnit,
        #[default(TextUnit::Unspecified)] after: TextUnit,
    ) -> Modifier {
        self.then_element(AlignmentLineOffsetElement::new(
            alignment_line.into(),
            Distance::Sp(before),
            Distance::Sp(after),
        ))
    }

    /// [`padding_from_sp`](AlignmentLinePaddingModifiers::padding_from_sp) with all its parameters, for plain code.
    fn padding_from_sp_with(
        self,
        alignment_line: impl Into<AlignmentLine>,
        before: TextUnit,
        after: TextUnit,
    ) -> Modifier {
        self.then_element(AlignmentLineOffsetElement::new(
            alignment_line.into(),
            Distance::Sp(before),
            Distance::Sp(after),
        ))
    }

    /// The first baseline `top` from the top and the last one `bottom` from the bottom, each only if specified
    /// (Compose's `paddingFromBaseline(top, bottom)`): text laid out to a typographic grid.
    fn padding_from_baseline(
        self,
        #[default(Dp::Unspecified)] top: Dp,
        #[default(Dp::Unspecified)] bottom: Dp,
    ) -> Modifier {
        padding_from_baseline(self, Distance::Dp(top), Distance::Dp(bottom))
    }

    /// [`padding_from_baseline`](AlignmentLinePaddingModifiers::padding_from_baseline) with all its parameters,
    /// for plain code.
    fn padding_from_baseline_with(self, top: Dp, bottom: Dp) -> Modifier {
        padding_from_baseline(self, Distance::Dp(top), Distance::Dp(bottom))
    }

    /// The same in sp (Compose's `paddingFromBaseline(top: TextUnit, bottom: TextUnit)`).
    fn padding_from_baseline_sp(
        self,
        #[default(TextUnit::Unspecified)] top: TextUnit,
        #[default(TextUnit::Unspecified)] bottom: TextUnit,
    ) -> Modifier {
        padding_from_baseline(self, Distance::Sp(top), Distance::Sp(bottom))
    }

    /// [`padding_from_baseline_sp`](AlignmentLinePaddingModifiers::padding_from_baseline_sp) with all its
    /// parameters, for plain code.
    fn padding_from_baseline_sp_with(self, top: TextUnit, bottom: TextUnit) -> Modifier {
        padding_from_baseline(self, Distance::Sp(top), Distance::Sp(bottom))
    }
}

/// Compose's `paddingFromBaseline`: a `paddingFrom` of each baseline given.
fn padding_from_baseline(modifier: Modifier, top: Distance, bottom: Distance) -> Modifier {
    let mut m = modifier;
    if top.is_specified() {
        m = m.then_element(AlignmentLineOffsetElement::new(FirstBaseline.into(), top, top.unspecified()));
    }
    if bottom.is_specified() {
        m = m.then_element(AlignmentLineOffsetElement::new(LastBaseline.into(), bottom.unspecified(), bottom));
    }
    m
}

/// A distance from a line: dp, or sp (Compose's two elements, AlignmentLineOffsetDpElement and
/// AlignmentLineOffsetTextUnitElement).
#[derive(Clone, Copy, Debug)]
enum Distance {
    Dp(Dp),
    Sp(TextUnit),
}

impl Distance {
    fn is_specified(self) -> bool {
        match self {
            Distance::Dp(d) => d.is_specified(),
            Distance::Sp(t) => t.is_specified(),
        }
    }

    /// An unspecified distance of the same kind.
    fn unspecified(self) -> Distance {
        match self {
            Distance::Dp(_) => Distance::Dp(Dp::Unspecified),
            Distance::Sp(_) => Distance::Sp(TextUnit::Unspecified),
        }
    }

    /// In dp, unspecified if it isn't given.
    fn to_dp(self, scope: &MeasureScope) -> Dp {
        match self {
            Distance::Dp(d) => d,
            Distance::Sp(t) if t.is_specified() => scope.text_to_dp(t),
            Distance::Sp(_) => Dp::Unspecified,
        }
    }
}

impl PartialEq for Distance {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Distance::Dp(a), Distance::Dp(b)) => same_dp(*a, *b),
            (Distance::Sp(a), Distance::Sp(b)) => a == b || (a.is_unspecified() && b.is_unspecified()),
            _ => false,
        }
    }
}

#[derive(PartialEq)]
struct AlignmentLineOffsetElement {
    alignment_line: AlignmentLine,
    before: Distance,
    after: Distance,
}

impl AlignmentLineOffsetElement {
    /// Panics on a negative distance in dp (as Compose's dp element; its sp one checks nothing).
    #[track_caller]
    fn new(alignment_line: AlignmentLine, before: Distance, after: Distance) -> AlignmentLineOffsetElement {
        let ok = |d: Distance| match d {
            Distance::Dp(d) => d.0 >= 0.0 || d.is_unspecified(),
            Distance::Sp(_) => true,
        };
        assert!(ok(before) && ok(after), "Padding from alignment line must be a non-negative number");
        AlignmentLineOffsetElement { alignment_line, before, after }
    }
}

impl ModifierNodeElement for AlignmentLineOffsetElement {
    type Node = AlignmentLineOffsetNode;
    fn create(&self) -> AlignmentLineOffsetNode {
        AlignmentLineOffsetNode { alignment_line: self.alignment_line, before: self.before, after: self.after }
    }
    fn update(&self, node: &mut AlignmentLineOffsetNode) {
        node.alignment_line = self.alignment_line;
        node.before = self.before;
        node.after = self.after;
    }
    fn name(&self) -> &'static str {
        "paddingFrom"
    }
}

/// The node of `padding_from` (Compose's AlignmentLineOffsetDpNode and AlignmentLineOffsetTextUnitNode): what it
/// wraps measured without the incoming min across the line, then padded before and after it so the line is
/// where it is asked, within the incoming max.
struct AlignmentLineOffsetNode {
    alignment_line: AlignmentLine,
    before: Distance,
    after: Distance,
}

impl ModifierNode for AlignmentLineOffsetNode {
    node_capabilities!(layout);
}

impl LayoutModifierNode for AlignmentLineOffsetNode {
    fn measure(
        &mut self,
        scope: &MeasureScope,
        measurable: &dyn Measurable,
        constraints: Constraints,
    ) -> MeasureResult {
        let (before, after) = (self.before.to_dp(scope), self.after.to_dp(scope));
        alignment_line_offset_measure(scope, self.alignment_line, before, after, measurable, constraints)
    }
}

/// Compose's `alignmentLineOffsetMeasure`.
fn alignment_line_offset_measure(
    scope: &MeasureScope,
    alignment_line: AlignmentLine,
    before: Dp,
    after: Dp,
    measurable: &dyn Measurable,
    constraints: Constraints,
) -> MeasureResult {
    let horizontal = alignment_line.is_horizontal();
    // Loose constraints across the line.
    let placeable = measurable.measure(if horizontal {
        Constraints::new(constraints.min_width, constraints.max_width, 0, constraints.max_height)
    } else {
        Constraints::new(0, constraints.max_width, constraints.min_height, constraints.max_height)
    });
    let line = placeable.get(alignment_line);
    let line_position = if line != AlignmentLine::Unspecified { line } else { 0 };
    let axis = if horizontal { placeable.height() } else { placeable.width() };
    let axis_max = if horizontal { constraints.max_height } else { constraints.max_width };
    // The padding that puts the line where it is asked before and after it.
    let before_px = if before.is_specified() { scope.round_to_px(before) } else { 0 };
    let after_px = if after.is_specified() { scope.round_to_px(after) } else { 0 };
    let padding_before = coerce_in(before_px - line_position, 0, axis_max - axis);
    let padding_after = coerce_in(after_px - axis + line_position, 0, axis_max - axis - padding_before);
    let width = if horizontal {
        placeable.width()
    } else {
        (padding_before + placeable.width() + padding_after).max(constraints.min_width)
    };
    let height = if horizontal {
        (padding_before + placeable.height() + padding_after).max(constraints.min_height)
    } else {
        placeable.height()
    };
    let before_given = before.is_specified();
    scope.layout(width, height, move |p| {
        let x = if horizontal {
            0
        } else if before_given {
            padding_before
        } else {
            width - padding_after - placeable.width()
        };
        let y = if !horizontal {
            0
        } else if before_given {
            padding_before
        } else {
            height - padding_after - placeable.height()
        };
        p.place_relative(&placeable, x, y);
    })
}
