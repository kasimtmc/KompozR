//! ui's layout modifiers, as Compose's: `layout {}` (LayoutModifier), `layout_id`, `on_size_changed`,
//! `on_globally_positioned`, `on_placed`, `z_index`.

use super::{
    GlobalPositionAwareModifierNode, LayoutAwareModifierNode, LayoutCoordinates, LayoutModifierNode, Measurable,
    MeasureResult, MeasureScope, ParentDataModifierNode,
};
use crate::modifier::{Modifier, ModifierNode, ModifierNodeElement};
use crate::node_capabilities;
use crate::unit::Constraints;
use kompoz_graphics::{Density, IntSize};
use std::any::Any;
use std::fmt;
use std::rc::Rc;

/// A `Modifier.layout` measure block: what it wraps, measured and placed.
pub type MeasureFn = Rc<dyn Fn(&MeasureScope, &dyn Measurable, Constraints) -> MeasureResult>;

impl Modifier {
    /// Measures and places what it wraps with `measure` (Compose's `Modifier.layout`): read state in its
    /// placement block and only the placement runs again when it changes.
    pub fn layout(
        self,
        measure: impl Fn(&MeasureScope, &dyn Measurable, Constraints) -> MeasureResult + 'static,
    ) -> Modifier {
        self.then_element(LayoutElement { measure: Rc::new(measure) })
    }

    /// Names the layout for its parent's measure policy (Compose's `Modifier.layoutId`): `measurable.layout_id()`.
    pub fn layout_id(self, id: impl Any + PartialEq + fmt::Debug) -> Modifier {
        self.then_element(LayoutIdElement { id: Rc::new(id) })
    }

    /// Calls `on_size_changed` when the layout is first measured and whenever its size changes after (Compose's
    /// `Modifier.onSizeChanged`).
    pub fn on_size_changed(self, on_size_changed: impl Fn(IntSize) + 'static) -> Modifier {
        self.then_element(OnSizeChangedElement { on_size_changed: Rc::new(on_size_changed) })
    }

    /// Calls `on_globally_positioned` with the layout's coordinates after each layout pass that may have moved or
    /// resized it (Compose's `Modifier.onGloballyPositioned`).
    pub fn on_globally_positioned(self, on_globally_positioned: impl Fn(&LayoutCoordinates) + 'static) -> Modifier {
        self.then_element(OnGloballyPositionedElement { on_globally_positioned: Rc::new(on_globally_positioned) })
    }

    /// Calls `on_placed` with the coordinates of what it is attached to when that is placed (Compose's
    /// `Modifier.onPlaced`).
    pub fn on_placed(self, on_placed: impl Fn(&LayoutCoordinates) + 'static) -> Modifier {
        self.then_element(OnPlacedElement { on_placed: Rc::new(on_placed) })
    }

    /// Draws the layout above (and hit tests it before) siblings of a lower z-index, as the parent placed it
    /// (Compose's `Modifier.zIndex`).
    pub fn z_index(self, z_index: f32) -> Modifier {
        self.then_element(ZIndexElement { z_index })
    }
}

// ---- layout {} ----

struct LayoutElement {
    measure: MeasureFn,
}

impl PartialEq for LayoutElement {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.measure, &other.measure)
    }
}

impl ModifierNodeElement for LayoutElement {
    type Node = LayoutModifierImpl;
    fn create(&self) -> LayoutModifierImpl {
        LayoutModifierImpl { measure: self.measure.clone() }
    }
    fn update(&self, node: &mut LayoutModifierImpl) {
        node.measure = self.measure.clone();
    }
    fn name(&self) -> &'static str {
        "layout"
    }
}

/// The node of `Modifier.layout` (Compose's LayoutModifierImpl).
pub(crate) struct LayoutModifierImpl {
    measure: MeasureFn,
}

impl ModifierNode for LayoutModifierImpl {
    node_capabilities!(layout);
}

impl LayoutModifierNode for LayoutModifierImpl {
    fn measure(
        &mut self,
        scope: &MeasureScope,
        measurable: &dyn Measurable,
        constraints: Constraints,
    ) -> MeasureResult {
        (self.measure)(scope, measurable, constraints)
    }
}

// ---- layout_id ----

/// A layout id: any value that compares and prints.
pub trait LayoutIdKey: Any + fmt::Debug {
    fn eq_key(&self, other: &dyn Any) -> bool;
    fn as_any(&self) -> &dyn Any;
}

impl<T: Any + PartialEq + fmt::Debug> LayoutIdKey for T {
    fn eq_key(&self, other: &dyn Any) -> bool {
        other.downcast_ref::<T>().is_some_and(|o| self == o)
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl dyn LayoutIdKey {
    /// Whether the id is `value` (of the same type and equal).
    pub fn is<T: Any>(&self, value: &T) -> bool {
        self.eq_key(value)
    }
    /// The id as a `T`, if it is one.
    pub fn downcast_ref<T: Any>(&self) -> Option<&T> {
        self.as_any().downcast_ref::<T>()
    }
}

/// What `Modifier.layout_id` tells the parent (Compose's LayoutIdParentData).
#[derive(Clone, Debug)]
pub struct LayoutIdParentData {
    pub id: Rc<dyn LayoutIdKey>,
}

struct LayoutIdElement {
    id: Rc<dyn LayoutIdKey>,
}

impl PartialEq for LayoutIdElement {
    fn eq(&self, other: &Self) -> bool {
        self.id.eq_key(other.id.as_any())
    }
}

impl ModifierNodeElement for LayoutIdElement {
    type Node = LayoutIdNode;
    fn create(&self) -> LayoutIdNode {
        LayoutIdNode { id: self.id.clone() }
    }
    fn update(&self, node: &mut LayoutIdNode) {
        node.id = self.id.clone();
    }
    fn name(&self) -> &'static str {
        "layoutId"
    }
}

/// The node of `Modifier.layout_id` (Compose's LayoutIdModifier): its parent data is its id, whatever the nodes
/// after it said.
pub(crate) struct LayoutIdNode {
    id: Rc<dyn LayoutIdKey>,
}

impl ModifierNode for LayoutIdNode {
    node_capabilities!(parent_data);
}

impl ParentDataModifierNode for LayoutIdNode {
    fn modify_parent_data(&mut self, _: &Density, _: Option<Box<dyn Any>>) -> Option<Box<dyn Any>> {
        Some(Box::new(LayoutIdParentData { id: self.id.clone() }))
    }
}

// ---- on_size_changed ----

struct OnSizeChangedElement {
    on_size_changed: Rc<dyn Fn(IntSize)>,
}

impl PartialEq for OnSizeChangedElement {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.on_size_changed, &other.on_size_changed)
    }
}

impl ModifierNodeElement for OnSizeChangedElement {
    type Node = OnSizeChangedNode;
    fn create(&self) -> OnSizeChangedNode {
        OnSizeChangedNode { on_size_changed: self.on_size_changed.clone(), previous: None }
    }
    fn update(&self, node: &mut OnSizeChangedNode) {
        node.on_size_changed = self.on_size_changed.clone();
        // (a new callback is told the size again: Compose resets the previous size)
        node.previous = None;
    }
    fn name(&self) -> &'static str {
        "onSizeChanged"
    }
}

/// The node of `Modifier.on_size_changed` (Compose's OnSizeChangedNode).
pub struct OnSizeChangedNode {
    on_size_changed: Rc<dyn Fn(IntSize)>,
    previous: Option<IntSize>,
}

impl ModifierNode for OnSizeChangedNode {
    node_capabilities!(layout_aware);
    fn should_auto_invalidate(&self) -> bool {
        false
    }
}

impl LayoutAwareModifierNode for OnSizeChangedNode {
    fn on_remeasured(&mut self, size: IntSize) {
        if self.previous != Some(size) {
            self.previous = Some(size);
            (self.on_size_changed)(size);
        }
    }
}

// ---- on_globally_positioned ----

struct OnGloballyPositionedElement {
    on_globally_positioned: Rc<dyn Fn(&LayoutCoordinates)>,
}

impl PartialEq for OnGloballyPositionedElement {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.on_globally_positioned, &other.on_globally_positioned)
    }
}

impl ModifierNodeElement for OnGloballyPositionedElement {
    type Node = OnGloballyPositionedNode;
    fn create(&self) -> OnGloballyPositionedNode {
        OnGloballyPositionedNode { callback: self.on_globally_positioned.clone() }
    }
    fn update(&self, node: &mut OnGloballyPositionedNode) {
        node.callback = self.on_globally_positioned.clone();
    }
    fn name(&self) -> &'static str {
        "onGloballyPositioned"
    }
}

/// The node of `Modifier.on_globally_positioned` (Compose's OnGloballyPositionedNode).
pub struct OnGloballyPositionedNode {
    callback: Rc<dyn Fn(&LayoutCoordinates)>,
}

impl ModifierNode for OnGloballyPositionedNode {
    node_capabilities!(global_position_aware);
}

impl GlobalPositionAwareModifierNode for OnGloballyPositionedNode {
    fn on_globally_positioned(&mut self, coordinates: &LayoutCoordinates) {
        (self.callback)(coordinates)
    }
}

// ---- on_placed ----

struct OnPlacedElement {
    on_placed: Rc<dyn Fn(&LayoutCoordinates)>,
}

impl PartialEq for OnPlacedElement {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.on_placed, &other.on_placed)
    }
}

impl ModifierNodeElement for OnPlacedElement {
    type Node = OnPlacedNode;
    fn create(&self) -> OnPlacedNode {
        OnPlacedNode { callback: self.on_placed.clone() }
    }
    fn update(&self, node: &mut OnPlacedNode) {
        node.callback = self.on_placed.clone();
    }
    fn name(&self) -> &'static str {
        "onPlaced"
    }
}

/// The node of `Modifier.on_placed` (Compose's OnPlacedNode).
pub struct OnPlacedNode {
    callback: Rc<dyn Fn(&LayoutCoordinates)>,
}

impl ModifierNode for OnPlacedNode {
    node_capabilities!(layout_aware);
}

impl LayoutAwareModifierNode for OnPlacedNode {
    fn on_placed(&mut self, coordinates: &LayoutCoordinates) {
        (self.callback)(coordinates)
    }
    fn observes_remeasure(&self) -> bool {
        false
    }
}

// ---- z_index ----

#[derive(PartialEq)]
struct ZIndexElement {
    z_index: f32,
}

impl ModifierNodeElement for ZIndexElement {
    type Node = ZIndexNode;
    fn create(&self) -> ZIndexNode {
        ZIndexNode { z_index: self.z_index }
    }
    fn update(&self, node: &mut ZIndexNode) {
        node.z_index = self.z_index;
    }
    fn name(&self) -> &'static str {
        "zIndex"
    }
}

/// The node of `Modifier.z_index` (Compose's ZIndexNode): what it wraps placed with its z-index.
pub struct ZIndexNode {
    z_index: f32,
}

impl ModifierNode for ZIndexNode {
    node_capabilities!(layout);
}

impl LayoutModifierNode for ZIndexNode {
    fn measure(
        &mut self,
        scope: &MeasureScope,
        measurable: &dyn Measurable,
        constraints: Constraints,
    ) -> MeasureResult {
        let placeable = measurable.measure(constraints);
        let z = self.z_index;
        scope.layout(placeable.width(), placeable.height(), move |p| {
            p.place_at(&placeable, kompoz_graphics::IntOffset::Zero, z)
        })
    }
}
