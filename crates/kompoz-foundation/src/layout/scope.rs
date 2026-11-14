//! The modifiers of Compose's `BoxScope`, `RowScope` and `ColumnScope`: what a child tells the Box, Row or Column
//! it is in (its parent data). One trait holds them all, as the content closure has no scope to hold them (see
//! the module's doc).

use super::box_::BoxChildData;
use super::row_column::{AlignmentLineProvider, CrossAxisAlignment, RowColumnParentData};
use kompoz_graphics::Density;
use kompoz_runtime::named;
use kompoz_ui::layout::{AlignmentLine, HorizontalAlignmentLine, ParentDataModifierNode, VerticalAlignmentLine};
use kompoz_ui::{
    Alignment, FirstBaseline, HorizontalAlignment, Modifier, ModifierNode, ModifierNodeElement, Placeable,
    VerticalAlignment, node_capabilities,
};
use std::any::Any;
use std::rc::Rc;

/// The modifiers a child of a `Box`, a `Row` or a `Column` tells its parent with (Compose's `BoxScope`,
/// `RowScope` and `ColumnScope` modifiers).
#[named(trait LayoutScopeModifiers)]
impl Modifier {
    /// Places it by `alignment` in its parent, over what the parent says (Compose's `align`): an [`Alignment`]
    /// in a Box (`BoxScope.align`), a [`VerticalAlignment`] in a Row (`RowScope.align`), a
    /// [`HorizontalAlignment`] in a Column (`ColumnScope.align`).
    fn align(self, alignment: impl ScopeAlignment) -> Modifier {
        alignment.then_aligned(self)
    }

    /// Makes it as big as its Box once the Box's other children made it (Compose's `BoxScope.matchParentSize`):
    /// unlike `fill_max_size`, it doesn't make the Box bigger.
    fn match_parent_size(self) -> Modifier {
        self.then_element(BoxChildDataElement { alignment: Alignment::Center, match_parent_size: true })
    }

    /// Gives it a share of what its Row's or Column's other children leave, by its `weight` against its weighted
    /// siblings' (Compose's `RowScope.weight` and `ColumnScope.weight`); with `fill`, it is made that big, else at
    /// most that big. Panics on a weight that isn't more than zero.
    fn weight(self, weight: f32, #[default(true)] fill: bool) -> Modifier {
        assert!(weight > 0.0, "invalid weight {weight}; must be greater than zero");
        // (an infinite weight is the greatest finite one, as Compose's)
        self.then_element(LayoutWeightElement { weight: weight.min(f32::MAX), fill })
    }

    /// [`weight`](LayoutScopeModifiers::weight) with all its parameters, for plain code.
    fn weight_with(self, weight: f32, fill: bool) -> Modifier {
        assert!(weight > 0.0, "invalid weight {weight}; must be greater than zero");
        self.then_element(LayoutWeightElement { weight: weight.min(f32::MAX), fill })
    }

    /// Lines it up across with its siblings that align by a line too, by `line` (Compose's `alignBy`): a
    /// [`HorizontalAlignmentLine`] in a Row (`RowScope.alignBy(alignmentLine)`), a [`VerticalAlignmentLine`] in a
    /// Column (`ColumnScope.alignBy(alignmentLine)`), or a closure working out the line's position from the
    /// measured child, `|p: &Placeable| …` (`alignBy { }`, in either; a closure isn't equal to another, so its node
    /// is updated whenever it composes again, as Compose's lambda). The parent grows to fit them all lined up.
    fn align_by<M>(self, line: impl AlignBy<M>) -> Modifier {
        line.then_aligned_by(self)
    }

    /// Lines its first baseline up with its siblings' (Compose's `RowScope.alignByBaseline`).
    fn align_by_baseline(self) -> Modifier {
        self.then_element(WithAlignmentLineElement { line: FirstBaseline.into() })
    }
}

/// What [`align`](LayoutScopeModifiers::align) takes: an [`Alignment`] for a Box's child, a
/// [`VerticalAlignment`] for a Row's, a [`HorizontalAlignment`] for a Column's.
pub trait ScopeAlignment {
    /// `modifier` with this alignment.
    fn then_aligned(self, modifier: Modifier) -> Modifier;
}

impl ScopeAlignment for Alignment {
    fn then_aligned(self, modifier: Modifier) -> Modifier {
        modifier.then_element(BoxChildDataElement { alignment: self, match_parent_size: false })
    }
}

impl ScopeAlignment for VerticalAlignment {
    fn then_aligned(self, modifier: Modifier) -> Modifier {
        modifier.then_element(VerticalAlignElement { alignment: self })
    }
}

impl ScopeAlignment for HorizontalAlignment {
    fn then_aligned(self, modifier: Modifier) -> Modifier {
        modifier.then_element(HorizontalAlignElement { alignment: self })
    }
}

/// What [`align_by`](LayoutScopeModifiers::align_by) takes: a [`HorizontalAlignmentLine`] (a Row's child), a
/// [`VerticalAlignmentLine`] (a Column's), or a closure `Fn(&Placeable) -> i32` giving the line's position in the
/// child (`M` tells the three apart).
pub trait AlignBy<M> {
    /// `modifier` aligned by this.
    fn then_aligned_by(self, modifier: Modifier) -> Modifier;
}

#[doc(hidden)]
pub enum HorizontalLineMarker {}
#[doc(hidden)]
pub enum VerticalLineMarker {}
#[doc(hidden)]
pub enum BlockMarker {}

impl AlignBy<HorizontalLineMarker> for HorizontalAlignmentLine {
    fn then_aligned_by(self, modifier: Modifier) -> Modifier {
        modifier.then_element(WithAlignmentLineElement { line: self.into() })
    }
}

impl AlignBy<VerticalLineMarker> for VerticalAlignmentLine {
    fn then_aligned_by(self, modifier: Modifier) -> Modifier {
        modifier.then_element(WithAlignmentLineElement { line: self.into() })
    }
}

impl<F: Fn(&Placeable) -> i32 + 'static> AlignBy<BlockMarker> for F {
    fn then_aligned_by(self, modifier: Modifier) -> Modifier {
        modifier.then_element(WithAlignmentLineBlockElement { block: Rc::new(self) })
    }
}

// ---- Box: align, match_parent_size ----

#[derive(PartialEq)]
struct BoxChildDataElement {
    alignment: Alignment,
    match_parent_size: bool,
}

impl ModifierNodeElement for BoxChildDataElement {
    type Node = BoxChildDataNode;
    fn create(&self) -> BoxChildDataNode {
        BoxChildDataNode { alignment: self.alignment.clone(), match_parent_size: self.match_parent_size }
    }
    fn update(&self, node: &mut BoxChildDataNode) {
        node.alignment = self.alignment.clone();
        node.match_parent_size = self.match_parent_size;
    }
    fn name(&self) -> &'static str {
        if self.match_parent_size { "matchParentSize" } else { "align" }
    }
}

/// The node of a Box child's `align` and `match_parent_size` (Compose's BoxChildDataNode): it says itself,
/// whatever the nodes after it said.
struct BoxChildDataNode {
    alignment: Alignment,
    match_parent_size: bool,
}

impl ModifierNode for BoxChildDataNode {
    node_capabilities!(parent_data);
}

impl ParentDataModifierNode for BoxChildDataNode {
    fn modify_parent_data(&mut self, _: &Density, _: Option<Box<dyn Any>>) -> Option<Box<dyn Any>> {
        Some(Box::new(BoxChildData { alignment: self.alignment.clone(), match_parent_size: self.match_parent_size }))
    }
}

// ---- Row and Column: weight ----

#[derive(PartialEq)]
struct LayoutWeightElement {
    weight: f32,
    fill: bool,
}

impl ModifierNodeElement for LayoutWeightElement {
    type Node = LayoutWeightNode;
    fn create(&self) -> LayoutWeightNode {
        LayoutWeightNode { weight: self.weight, fill: self.fill }
    }
    fn update(&self, node: &mut LayoutWeightNode) {
        node.weight = self.weight;
        node.fill = self.fill;
    }
    fn name(&self) -> &'static str {
        "weight"
    }
}

/// The node of `weight` (Compose's LayoutWeightNode).
struct LayoutWeightNode {
    weight: f32,
    fill: bool,
}

impl ModifierNode for LayoutWeightNode {
    node_capabilities!(parent_data);
}

impl ParentDataModifierNode for LayoutWeightNode {
    fn modify_parent_data(&mut self, _: &Density, parent_data: Option<Box<dyn Any>>) -> Option<Box<dyn Any>> {
        let mut data = RowColumnParentData::from(parent_data);
        data.weight = self.weight;
        data.fill = self.fill;
        Some(Box::new(data))
    }
}

// ---- Row: align(VerticalAlignment) ----

#[derive(PartialEq)]
struct VerticalAlignElement {
    alignment: VerticalAlignment,
}

impl ModifierNodeElement for VerticalAlignElement {
    type Node = VerticalAlignNode;
    fn create(&self) -> VerticalAlignNode {
        VerticalAlignNode { vertical: self.alignment.clone() }
    }
    fn update(&self, node: &mut VerticalAlignNode) {
        node.vertical = self.alignment.clone();
    }
    fn name(&self) -> &'static str {
        "align"
    }
}

/// The node of a Row child's `align` (Compose's VerticalAlignNode).
struct VerticalAlignNode {
    vertical: VerticalAlignment,
}

impl ModifierNode for VerticalAlignNode {
    node_capabilities!(parent_data);
}

impl ParentDataModifierNode for VerticalAlignNode {
    fn modify_parent_data(&mut self, _: &Density, parent_data: Option<Box<dyn Any>>) -> Option<Box<dyn Any>> {
        let mut data = RowColumnParentData::from(parent_data);
        data.cross_axis_alignment = Some(CrossAxisAlignment::Vertical(self.vertical.clone()));
        Some(Box::new(data))
    }
}

// ---- Column: align(HorizontalAlignment) ----

#[derive(PartialEq)]
struct HorizontalAlignElement {
    alignment: HorizontalAlignment,
}

impl ModifierNodeElement for HorizontalAlignElement {
    type Node = HorizontalAlignNode;
    fn create(&self) -> HorizontalAlignNode {
        HorizontalAlignNode { horizontal: self.alignment.clone() }
    }
    fn update(&self, node: &mut HorizontalAlignNode) {
        node.horizontal = self.alignment.clone();
    }
    fn name(&self) -> &'static str {
        "align"
    }
}

/// The node of a Column child's `align` (Compose's HorizontalAlignNode).
struct HorizontalAlignNode {
    horizontal: HorizontalAlignment,
}

impl ModifierNode for HorizontalAlignNode {
    node_capabilities!(parent_data);
}

impl ParentDataModifierNode for HorizontalAlignNode {
    fn modify_parent_data(&mut self, _: &Density, parent_data: Option<Box<dyn Any>>) -> Option<Box<dyn Any>> {
        let mut data = RowColumnParentData::from(parent_data);
        data.cross_axis_alignment = Some(CrossAxisAlignment::Horizontal(self.horizontal.clone()));
        Some(Box::new(data))
    }
}

// ---- Row and Column: align_by ----

#[derive(PartialEq)]
struct WithAlignmentLineElement {
    line: AlignmentLine,
}

impl ModifierNodeElement for WithAlignmentLineElement {
    type Node = WithAlignmentLineNode;
    fn create(&self) -> WithAlignmentLineNode {
        WithAlignmentLineNode { line: self.line }
    }
    fn update(&self, node: &mut WithAlignmentLineNode) {
        node.line = self.line;
    }
    fn name(&self) -> &'static str {
        "alignBy"
    }
}

/// The node of `align_by(line)` (Compose's SiblingsAlignedNode.WithAlignmentLineNode).
struct WithAlignmentLineNode {
    line: AlignmentLine,
}

impl ModifierNode for WithAlignmentLineNode {
    node_capabilities!(parent_data);
}

impl ParentDataModifierNode for WithAlignmentLineNode {
    fn modify_parent_data(&mut self, _: &Density, parent_data: Option<Box<dyn Any>>) -> Option<Box<dyn Any>> {
        let mut data = RowColumnParentData::from(parent_data);
        data.cross_axis_alignment = Some(CrossAxisAlignment::AlignmentLine(AlignmentLineProvider::Value(self.line)));
        Some(Box::new(data))
    }
}

struct WithAlignmentLineBlockElement {
    block: Rc<dyn Fn(&Placeable) -> i32>,
}

impl PartialEq for WithAlignmentLineBlockElement {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.block, &other.block)
    }
}

impl ModifierNodeElement for WithAlignmentLineBlockElement {
    type Node = WithAlignmentLineBlockNode;
    fn create(&self) -> WithAlignmentLineBlockNode {
        WithAlignmentLineBlockNode { block: self.block.clone() }
    }
    fn update(&self, node: &mut WithAlignmentLineBlockNode) {
        node.block = self.block.clone();
    }
    fn name(&self) -> &'static str {
        "alignBy"
    }
}

/// The node of `align_by(block)` (Compose's SiblingsAlignedNode.WithAlignmentLineBlockNode).
struct WithAlignmentLineBlockNode {
    block: Rc<dyn Fn(&Placeable) -> i32>,
}

impl ModifierNode for WithAlignmentLineBlockNode {
    node_capabilities!(parent_data);
}

impl ParentDataModifierNode for WithAlignmentLineBlockNode {
    fn modify_parent_data(&mut self, _: &Density, parent_data: Option<Box<dyn Any>>) -> Option<Box<dyn Any>> {
        let mut data = RowColumnParentData::from(parent_data);
        data.cross_axis_alignment =
            Some(CrossAxisAlignment::AlignmentLine(AlignmentLineProvider::Block(self.block.clone())));
        Some(Box::new(data))
    }
}
