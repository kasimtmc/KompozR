//! Padding by window insets, as Compose's `Modifier.windowInsetsPadding` family: padding by what of the insets
//! the layouts around haven't taken yet, consuming insets for what is inside, and hearing what was consumed.
//!
//! What is consumed passes down the tree as Compose's does: each insets node finds the nearest one above it (a
//! traversal by the nodes' shared key), takes what that one consumed, works out what it consumes itself, and
//! hands that down to the nearest ones below whenever it changes.

use super::padding::{PaddingValues, offset};
use super::window_insets::WindowInsets;
use kompoz_graphics::{IntOffset, IntSize, LayoutDirection, Offset};
use kompoz_runtime::named;
use kompoz_ui::layout::LayoutModifierNode;
use kompoz_ui::modifier::{TraversableNode, TraversalDecision};
use kompoz_ui::platform::{Insets, LocalSystemInsets, LocalWindowInfo, SystemInsetsHolder};
use kompoz_ui::{
    Constraints, Measurable, MeasureResult, MeasureScope, Modifier, ModifierNode, ModifierNodeElement, NodeCx,
    NodeHandle, node_capabilities,
};
use std::any::Any;
use std::cell::Cell;
use std::rc::Rc;

/// The window insets padding modifiers (Compose's `windowInsetsPadding`, `consumeWindowInsets`,
/// `onConsumedWindowInsetsChanged`, `recalculateWindowInsets` and the paddings by each kind). The paddings by a
/// kind find the host's insets where they are attached (Compose's SystemInsetsPaddingModifierNode), so they work
/// made anywhere; they measure again, composing nothing, when the platform reports other insets.
#[named(trait WindowInsetsPaddingModifiers)]
impl Modifier {
    /// Padding by `insets`, less what the layouts around consumed already; all of `insets` is consumed for what
    /// is inside (Compose's `windowInsetsPadding`). The window's left and right stay the screen's whatever the
    /// layout direction.
    fn window_insets_padding(self, insets: WindowInsets) -> Modifier {
        self.then_element(InsetsPaddingElement { insets: InsetsSource::Given(insets), name: "windowInsetsPadding" })
    }

    /// `insets` taken as consumed for what is inside, without padding for them (Compose's
    /// `consumeWindowInsets(insets)`): the union of them and what the layouts around consumed.
    fn consume_window_insets(self, insets: WindowInsets) -> Modifier {
        self.then_element(UnionInsetsConsumingElement { insets })
    }

    /// Padding the app applied itself taken as consumed for what is inside (Compose's
    /// `consumeWindowInsets(paddingValues)`): added to what the layouts around consumed.
    /// Kotlin: `consumeWindowInsets(paddingValues: PaddingValues)`
    fn consume_window_insets_padding_values(self, padding_values: PaddingValues) -> Modifier {
        self.then_element(PaddingValuesConsumingElement { padding_values })
    }

    /// Calls `block` with what the layouts around (and the nodes before it) consumed, when it is attached and
    /// whenever that changes (Compose's `onConsumedWindowInsetsChanged`); a layout pass that leaves it as it was
    /// doesn't call it.
    fn on_consumed_window_insets_changed(self, block: impl Fn(&WindowInsets) + 'static) -> Modifier {
        self.then_element(ConsumedInsetsElement { block: Rc::new(block) })
    }

    /// What is inside finds the insets it consumes from where it is in the window: what reaches past its own
    /// bounds is taken as consumed (Compose's `recalculateWindowInsets`). It works when its constraints are
    /// fixed (it fills them); otherwise what is inside sees what the layouts around consumed.
    fn recalculate_window_insets(self) -> Modifier {
        self.then_element(RecalculateWindowInsetsElement)
    }

    /// Padding by the system bars, the keyboard and the display cutout (Compose's `safeDrawingPadding`).
    fn safe_drawing_padding(self) -> Modifier {
        self.then_element(InsetsPaddingElement::system(WindowInsets::safe_drawing_of, "safeDrawingPadding"))
    }

    /// Padding by the gesture edges, the tappable elements and the waterfall (Compose's `safeGesturesPadding`).
    fn safe_gestures_padding(self) -> Modifier {
        self.then_element(InsetsPaddingElement::system(WindowInsets::safe_gestures_of, "safeGesturesPadding"))
    }

    /// Padding by everything drawn over or gestured on (Compose's `safeContentPadding`).
    fn safe_content_padding(self) -> Modifier {
        self.then_element(InsetsPaddingElement::system(WindowInsets::safe_content_of, "safeContentPadding"))
    }

    /// Padding by the system bars (Compose's `systemBarsPadding`).
    fn system_bars_padding(self) -> Modifier {
        self.then_element(InsetsPaddingElement::system(WindowInsets::system_bars_of, "systemBarsPadding"))
    }

    /// Padding by the display cutout (Compose's `displayCutoutPadding`).
    fn display_cutout_padding(self) -> Modifier {
        self.then_element(InsetsPaddingElement::system(WindowInsets::display_cutout_of, "displayCutoutPadding"))
    }

    /// Padding by the status bars (Compose's `statusBarsPadding`).
    fn status_bars_padding(self) -> Modifier {
        self.then_element(InsetsPaddingElement::system(WindowInsets::status_bars_of, "statusBarsPadding"))
    }

    /// Padding by the keyboard, as it slides (Compose's `imePadding`).
    fn ime_padding(self) -> Modifier {
        self.then_element(InsetsPaddingElement::system(WindowInsets::ime_of, "imePadding"))
    }

    /// Padding by the navigation bars (Compose's `navigationBarsPadding`).
    fn navigation_bars_padding(self) -> Modifier {
        self.then_element(InsetsPaddingElement::system(WindowInsets::navigation_bars_of, "navigationBarsPadding"))
    }

    /// Padding by the caption bar (Compose's `captionBarPadding`).
    fn caption_bar_padding(self) -> Modifier {
        self.then_element(InsetsPaddingElement::system(WindowInsets::caption_bar_of, "captionBarPadding"))
    }

    /// Padding by the waterfall's curved edges (Compose's `waterfallPadding`).
    fn waterfall_padding(self) -> Modifier {
        self.then_element(InsetsPaddingElement::system(WindowInsets::waterfall_of, "waterfallPadding"))
    }

    /// Padding by the system gesture edges (Compose's `systemGesturesPadding`).
    fn system_gestures_padding(self) -> Modifier {
        self.then_element(InsetsPaddingElement::system(WindowInsets::system_gestures_of, "systemGesturesPadding"))
    }

    /// Padding by the mandatory system gesture edges (Compose's `mandatorySystemGesturesPadding`).
    fn mandatory_system_gestures_padding(self) -> Modifier {
        self.then_element(InsetsPaddingElement::system(
            WindowInsets::mandatory_system_gestures_of,
            "mandatorySystemGesturesPadding",
        ))
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The elements

/// What a padding pads by: insets given, or a kind of the host's, found where the node is attached (by its
/// getter, known by the modifier's name).
#[derive(Clone)]
enum InsetsSource {
    Given(WindowInsets),
    System(fn(&SystemInsetsHolder) -> WindowInsets),
}

struct InsetsPaddingElement {
    insets: InsetsSource,
    name: &'static str,
}

impl InsetsPaddingElement {
    fn system(getter: fn(&SystemInsetsHolder) -> WindowInsets, name: &'static str) -> InsetsPaddingElement {
        InsetsPaddingElement { insets: InsetsSource::System(getter), name }
    }
}

impl PartialEq for InsetsPaddingElement {
    fn eq(&self, other: &Self) -> bool {
        match (&self.insets, &other.insets) {
            (InsetsSource::Given(a), InsetsSource::Given(b)) => a == b,
            // (Compose's SystemInsetsPaddingModifierElement: the same getter; each kind's modifier has its name)
            (InsetsSource::System(_), InsetsSource::System(_)) => self.name == other.name,
            _ => false,
        }
    }
}

impl ModifierNodeElement for InsetsPaddingElement {
    type Node = InsetsConsumingNode;
    fn create(&self) -> InsetsConsumingNode {
        match &self.insets {
            InsetsSource::Given(insets) => InsetsConsumingNode::new(Kind::Padding(insets.clone())),
            InsetsSource::System(getter) => {
                let mut node = InsetsConsumingNode::new(Kind::Padding(WindowInsets::Zero));
                node.system_getter = Some(*getter);
                node
            }
        }
    }
    fn update(&self, node: &mut InsetsConsumingNode) {
        match &self.insets {
            InsetsSource::Given(given) => {
                node.system_getter = None;
                if let Kind::Padding(insets) = &node.kind
                    && insets != given
                {
                    node.kind = Kind::Padding(given.clone());
                    node.insets_invalidated_now();
                }
            }
            InsetsSource::System(getter) => {
                node.system_getter = Some(*getter);
                node.resolve_system_insets();
            }
        }
    }
    fn name(&self) -> &'static str {
        self.name
    }
}

#[derive(PartialEq)]
struct UnionInsetsConsumingElement {
    insets: WindowInsets,
}

impl ModifierNodeElement for UnionInsetsConsumingElement {
    type Node = InsetsConsumingNode;
    fn create(&self) -> InsetsConsumingNode {
        InsetsConsumingNode::new(Kind::Union(self.insets.clone()))
    }
    fn update(&self, node: &mut InsetsConsumingNode) {
        if let Kind::Union(insets) = &node.kind
            && *insets != self.insets
        {
            node.kind = Kind::Union(self.insets.clone());
            node.insets_invalidated_now();
        }
    }
    fn name(&self) -> &'static str {
        "consumeWindowInsets"
    }
}

#[derive(PartialEq)]
struct PaddingValuesConsumingElement {
    padding_values: PaddingValues,
}

impl ModifierNodeElement for PaddingValuesConsumingElement {
    type Node = InsetsConsumingNode;
    fn create(&self) -> InsetsConsumingNode {
        InsetsConsumingNode::new(Kind::PaddingValues(self.padding_values.clone()))
    }
    fn update(&self, node: &mut InsetsConsumingNode) {
        if let Kind::PaddingValues(p) = &node.kind
            && *p != self.padding_values
        {
            node.kind = Kind::PaddingValues(self.padding_values.clone());
            node.insets_invalidated_now();
        }
    }
    fn name(&self) -> &'static str {
        "consumeWindowInsets"
    }
}

struct ConsumedInsetsElement {
    block: Rc<dyn Fn(&WindowInsets)>,
}

impl PartialEq for ConsumedInsetsElement {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.block, &other.block)
    }
}

impl ModifierNodeElement for ConsumedInsetsElement {
    type Node = InsetsConsumingNode;
    fn create(&self) -> InsetsConsumingNode {
        InsetsConsumingNode::new(Kind::OnConsumed(self.block.clone()))
    }
    fn update(&self, node: &mut InsetsConsumingNode) {
        if let Kind::OnConsumed(block) = &node.kind
            && !Rc::ptr_eq(block, &self.block)
        {
            node.kind = Kind::OnConsumed(self.block.clone());
            node.insets_invalidated_now();
        }
    }
    fn name(&self) -> &'static str {
        "onConsumedWindowInsetsChanged"
    }
}

#[derive(PartialEq)]
struct RecalculateWindowInsetsElement;

impl ModifierNodeElement for RecalculateWindowInsetsElement {
    type Node = InsetsConsumingNode;
    fn create(&self) -> InsetsConsumingNode {
        InsetsConsumingNode::new(Kind::Recalculate(Rc::new(Cell::new(Some([0, 0, 0, 0])))))
    }
    fn update(&self, _: &mut InsetsConsumingNode) {}
    fn name(&self) -> &'static str {
        "recalculateWindowInsets"
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The node

/// What the insets nodes are found by, up and down the tree (Compose's InsetsConsumingModifierNodeKey).
const InsetsConsumingModifierNodeKey: &str = "androidx.compose.foundation.layout.ConsumedInsetsProvider";

/// Which side of the window insets a size modifier takes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum InsetSide {
    Start,
    End,
    Top,
    Bottom,
}

/// What an insets node does (Compose's subclasses of InsetsConsumingModifierNode).
pub(super) enum Kind {
    /// Pads by its insets less what is consumed above, consumes them (InsetsPaddingModifierNode and the system
    /// insets' SystemInsetsPaddingModifierNode).
    Padding(WindowInsets),
    /// Consumes its insets (UnionInsetsConsumingModifierNode).
    Union(WindowInsets),
    /// Consumes padding values, added (PaddingValuesConsumingModifierNode).
    PaddingValues(PaddingValues),
    /// Hears what is consumed above (ConsumedInsetsModifierNode).
    OnConsumed(Rc<dyn Fn(&WindowInsets)>),
    /// Takes a side of its insets less what is consumed above as its size (DerivedWidthModifierNode and
    /// DerivedHeightModifierNode); `left` is what is left of them.
    Size { insets: WindowInsets, side: InsetSide, left: WindowInsets },
    /// Consumes what reaches past its bounds in the window, its sides once known (RecalculateWindowInsets…Node;
    /// none: as what is consumed above).
    Recalculate(Rc<Cell<Option<[i32; 4]>>>),
}

impl Kind {
    /// Whether it measures (is a layout node).
    fn measures(&self) -> bool {
        matches!(self, Kind::Padding(_) | Kind::Size { .. } | Kind::Recalculate(_))
    }
}

/// The node of the window insets modifiers (Compose's InsetsConsumingModifierNode and its subclasses, one type
/// here so the nodes find one another): what the nodes above consumed, what it consumes itself, and what it does
/// with them. A layout node for the kinds that measure.
pub(super) struct InsetsConsumingNode {
    pub(super) kind: Kind,
    /// What the insets nodes above consumed.
    ancestor_consumed_insets: WindowInsets,
    /// What it consumes, theirs included.
    consumed_insets: WindowInsets,
    /// A padding by a kind of the host's: what makes it from the host's insets, where it is attached.
    system_getter: Option<fn(&SystemInsetsHolder) -> WindowInsets>,
    handle: Option<NodeHandle>,
}

impl InsetsConsumingNode {
    pub(super) fn new(kind: Kind) -> InsetsConsumingNode {
        InsetsConsumingNode {
            kind,
            ancestor_consumed_insets: WindowInsets::Zero,
            consumed_insets: WindowInsets::Zero,
            system_getter: None,
            handle: None,
        }
    }

    /// A padding by a kind of the host's pads by that kind of the host its layout is in (Compose's
    /// SystemInsetsPaddingModifierNode `update`); before it is attached, nothing.
    fn resolve_system_insets(&mut self) {
        let Some(getter) = self.system_getter else { return };
        let Some(cx) = self.handle.as_ref().and_then(|h| h.cx()).filter(|cx| cx.is_attached()) else { return };
        let insets = getter(&cx.current_value_of(&LocalSystemInsets));
        if !matches!(&self.kind, Kind::Padding(old) if *old == insets) {
            self.kind = Kind::Padding(insets);
            self.insets_invalidated(&cx);
        }
    }

    /// What it consumes given what the nodes above did (Compose's `calculateInsets`).
    fn calculate_insets(&self, ancestor: &WindowInsets) -> WindowInsets {
        match &self.kind {
            Kind::Padding(insets) | Kind::Union(insets) => ancestor.union(insets),
            Kind::PaddingValues(p) => ancestor.add(&WindowInsets::of_padding_values(p.clone())),
            Kind::OnConsumed(block) => {
                block(ancestor);
                ancestor.clone()
            }
            Kind::Size { .. } => ancestor.clone(),
            Kind::Recalculate(sides) => match sides.get() {
                Some([left, top, right, bottom]) => WindowInsets::value("reset", Insets::new(left, top, right, bottom)),
                None => ancestor.clone(),
            },
        }
    }

    /// What it consumes is worked out again and handed down (Compose's `insetsInvalidated`), with its context.
    fn insets_invalidated(&mut self, cx: &NodeCx) {
        if let Kind::Size { insets, left, .. } = &mut self.kind {
            *left = insets.exclude(&self.ancestor_consumed_insets);
        }
        self.consumed_insets = self.calculate_insets(&self.ancestor_consumed_insets);
        self.invalidate_child_consumed_insets(cx);
        if matches!(self.kind, Kind::Padding(_) | Kind::Size { .. }) {
            cx.invalidate_measurement();
        }
    }

    /// The same where it is (an element's update); before it is attached only what it keeps changes.
    pub(super) fn insets_invalidated_now(&mut self) {
        match self.handle.as_ref().and_then(|h| h.cx()).filter(|cx| cx.is_attached()) {
            Some(cx) => self.insets_invalidated(&cx),
            None => {
                if let Kind::Size { insets, left, .. } = &mut self.kind {
                    *left = insets.exclude(&self.ancestor_consumed_insets);
                }
            }
        }
    }

    /// Tells the nearest insets nodes below what it consumes (Compose's `invalidateChildConsumedInsets`).
    fn invalidate_child_consumed_insets(&self, cx: &NodeCx) {
        let consumed = self.consumed_insets.clone();
        cx.traverse_descendants(&InsetsConsumingModifierNodeKey, &mut |child| {
            child.with::<InsetsConsumingNode, _>(|n| n.set_ancestor_consumed_insets(consumed.clone(), child));
            TraversalDecision::SkipSubtreeAndContinueTraversal
        });
    }

    /// What the nodes above consume is `insets` now (Compose's `setAncestorConsumedInsets`).
    fn set_ancestor_consumed_insets(&mut self, insets: WindowInsets, cx: &NodeCx) {
        if self.ancestor_consumed_insets != insets {
            self.ancestor_consumed_insets = insets;
            self.insets_invalidated(cx);
        }
    }
}

impl TraversableNode for InsetsConsumingNode {
    fn traverse_key(&self) -> &dyn Any {
        &InsetsConsumingModifierNodeKey
    }
}

impl ModifierNode for InsetsConsumingNode {
    node_capabilities!(traversable);

    fn as_layout(&mut self) -> Option<&mut dyn LayoutModifierNode> {
        if self.kind.measures() { Some(self) } else { None }
    }

    fn on_attach(&mut self, cx: &NodeCx) {
        self.handle = Some(cx.handle());
        if let Some(getter) = self.system_getter {
            self.kind = Kind::Padding(getter(&cx.current_value_of(&LocalSystemInsets)));
        }
        let mut above = None;
        cx.traverse_ancestors(&InsetsConsumingModifierNodeKey, &mut |parent| {
            above = parent.with::<InsetsConsumingNode, _>(|n| n.consumed_insets.clone());
            false
        });
        if let Some(above) = above {
            self.ancestor_consumed_insets = above;
        }
        self.insets_invalidated(cx);
    }

    fn on_detach(&mut self, cx: &NodeCx) {
        // Gone: the nodes below see what the ones above consumed.
        self.consumed_insets = self.ancestor_consumed_insets.clone();
        self.invalidate_child_consumed_insets(cx);
        self.handle = None;
    }

    fn on_reset(&mut self) {
        self.ancestor_consumed_insets = WindowInsets::Zero;
    }

    fn should_auto_invalidate(&self) -> bool {
        // (it invalidates its measurement itself when what it pads by changes)
        !matches!(self.kind, Kind::Recalculate(_))
    }
}

impl LayoutModifierNode for InsetsConsumingNode {
    fn measure(
        &mut self,
        scope: &MeasureScope,
        measurable: &dyn Measurable,
        constraints: Constraints,
    ) -> MeasureResult {
        let direction = scope.layout_direction();
        match &self.kind {
            Kind::Padding(_) => {
                let (consumed, ancestor) = (&self.consumed_insets, &self.ancestor_consumed_insets);
                let left = consumed.get_left(scope, direction) - ancestor.get_left(scope, direction);
                let top = consumed.get_top(scope) - ancestor.get_top(scope);
                let right = consumed.get_right(scope, direction) - ancestor.get_right(scope, direction);
                let bottom = consumed.get_bottom(scope) - ancestor.get_bottom(scope);
                let (horizontal, vertical) = (left + right, top + bottom);
                let placeable = measurable.measure(offset(constraints, -horizontal, -vertical));
                let width = constraints.constrain_width(placeable.width() + horizontal);
                let height = constraints.constrain_height(placeable.height() + vertical);
                scope.layout(width, height, move |p| p.place(&placeable, left, top))
            }
            Kind::Size { side, left, .. } => {
                let size = match side {
                    InsetSide::Start | InsetSide::End => {
                        let start = (*side == InsetSide::Start) == (direction == LayoutDirection::Ltr);
                        if start { left.get_left(scope, direction) } else { left.get_right(scope, direction) }
                    }
                    InsetSide::Top => left.get_top(scope),
                    InsetSide::Bottom => left.get_bottom(scope),
                };
                if size == 0 {
                    return scope.layout(0, 0, |_| {});
                }
                if matches!(side, InsetSide::Start | InsetSide::End) {
                    let placeable = measurable.measure(Constraints::new(
                        size,
                        size,
                        constraints.min_height,
                        constraints.max_height,
                    ));
                    scope.layout(size, placeable.height(), move |p| p.place_relative(&placeable, 0, 0))
                } else {
                    let placeable =
                        measurable.measure(Constraints::new(constraints.min_width, constraints.max_width, size, size));
                    scope.layout(placeable.width(), size, move |p| p.place_relative(&placeable, 0, 0))
                }
            }
            Kind::Recalculate(sides) => {
                let sides = sides.clone();
                if scope.is_intrinsic() {
                    // An intrinsic size: its content's (Compose's overrides), nothing worked out.
                    let placeable = measurable.measure(constraints);
                    return scope.layout(placeable.width(), placeable.height(), move |p| p.place(&placeable, 0, 0));
                }
                if !constraints.has_fixed_width() || !constraints.has_fixed_height() {
                    // Nothing to work them out from: what is inside sees what the nodes above consumed.
                    if sides.get().is_some() {
                        sides.set(None);
                        if let Some(cx) = self.handle.as_ref().and_then(|h| h.cx()) {
                            self.insets_invalidated(&cx);
                        }
                    }
                    let placeable = measurable.measure(constraints);
                    return scope.layout(placeable.width(), placeable.height(), move |p| p.place(&placeable, 0, 0));
                }
                let (width, height) = (constraints.max_width, constraints.max_height);
                // (Compose measures the content in the placement, once the sides are known; here it is measured
                // now, and again if placing finds other sides)
                let placeable = measurable.measure(Constraints::fixed(width, height));
                let handle = self.handle.clone();
                scope.layout(width, height, move |p| {
                    if let Some(coordinates) = p.coordinates() {
                        let top_left = coordinates.position_in_root();
                        let size = coordinates.size();
                        let bottom_right =
                            coordinates.local_to_root(Offset::new(size.width as f32, size.height as f32));
                        // (the window's size: Compose's root fills its window, the host's root wraps its content)
                        let root = handle
                            .as_ref()
                            .map_or(IntSize::Zero, |h| h.current_value_of(&LocalWindowInfo).container_size());
                        let round = |v: f32| (v + 0.5).floor() as i32;
                        let now = [
                            round(top_left.x),
                            round(top_left.y),
                            root.width - round(bottom_right.x),
                            root.height - round(bottom_right.y),
                        ];
                        if sides.get() != Some(now) {
                            sides.set(Some(now));
                            if let Some(cx) = handle.as_ref().and_then(|h| h.cx()) {
                                cx.with::<InsetsConsumingNode, _>(|n| n.insets_invalidated(&cx));
                            }
                        }
                    }
                    p.place_at(&placeable, IntOffset::Zero, 0.0);
                })
            }
            // (not layout nodes: never measured)
            Kind::Union(_) | Kind::PaddingValues(_) | Kind::OnConsumed(_) => {
                let placeable = measurable.measure(constraints);
                scope.layout(placeable.width(), placeable.height(), move |p| p.place(&placeable, 0, 0))
            }
        }
    }
}
