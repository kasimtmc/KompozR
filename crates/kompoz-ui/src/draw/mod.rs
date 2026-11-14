//! Drawing, as Compose's `androidx.compose.ui.draw` over the [`GraphicsContext`]: draw modifier nodes draw in a
//! [`ContentDrawScope`] (the context, their size, layout direction and density, and `draw_content()` for what is
//! below them); a layout node draws through its coordinators, outer to inner, each translated to its position (and
//! through its layer), its children in z order, what it reads observed, and the node hooks a display list keeps
//! frames by around it (C++'s `LayoutNode::draw`).
//!
//! The core is here; the drawing API of the scope (`draw_rect` and the rest), the draw modifiers and the full
//! graphics layer drawing are the drawing package of this stage, in other files of this module (`impl DrawScope`).

use crate::graphics_context::{CornerRadii, GraphicsContext, LayerTransform, NodeRecording};
use crate::layout::GraphicsLayerScope;
use crate::modifier::{NodeKind, NodeRc};
use crate::node::{LayoutNode, NodeCoordinator};
use kompoz_graphics::{Density, LayoutDirection, Offset, Rect, Size};
use std::cell::Cell;
use std::rc::Rc;

/// A node that draws (Compose's DrawModifierNode): what it draws around `scope.draw_content()`, which draws what is
/// below it (the next draw nodes, the layout's content).
pub trait DrawModifierNode {
    fn draw(&mut self, scope: &mut ContentDrawScope);
    /// Its layout's size changed (Compose's `onMeasureResultChanged`).
    fn on_measure_result_changed(&mut self) {}
}

/// Where drawing happens (Compose's DrawScope): the context, the size drawn in, the layout direction and the
/// density (it derefs to it: `scope.to_px(1.dp())`). Its drawing functions are the drawing package's.
pub struct DrawScope<'a> {
    pub(crate) canvas: &'a mut dyn GraphicsContext,
    pub(crate) size: Size,
    pub(crate) layout_direction: LayoutDirection,
    pub(crate) density: Density,
}

impl<'a> DrawScope<'a> {
    pub fn new(
        canvas: &'a mut dyn GraphicsContext,
        size: Size,
        layout_direction: LayoutDirection,
        density: Density,
    ) -> Self {
        DrawScope { canvas, size, layout_direction, density }
    }
    /// The size drawn in, in px.
    pub fn size(&self) -> Size {
        self.size
    }
    /// Its middle.
    pub fn center(&self) -> Offset {
        self.size.center()
    }
    pub fn layout_direction(&self) -> LayoutDirection {
        self.layout_direction
    }
    pub fn density(&self) -> Density {
        self.density
    }
    /// The context drawn into.
    pub fn canvas(&mut self) -> &mut dyn GraphicsContext {
        &mut *self.canvas
    }
}

impl std::ops::Deref for DrawScope<'_> {
    type Target = Density;
    fn deref(&self) -> &Density {
        &self.density
    }
}

/// What a draw node draws in (Compose's ContentDrawScope): a [`DrawScope`] (it derefs to it) and
/// [`draw_content`](ContentDrawScope::draw_content).
pub struct ContentDrawScope<'a> {
    pub(crate) scope: DrawScope<'a>,
    pub(crate) next: ContentNext,
}

/// What `draw_content` draws: the draw nodes of a coordinator from `index` on, then what it holds.
pub(crate) struct ContentNext {
    pub(crate) coordinator: Rc<NodeCoordinator>,
    pub(crate) nodes: Rc<Vec<NodeRc>>,
    pub(crate) index: usize,
}

impl ContentDrawScope<'_> {
    /// Draws what is below the node: the draw nodes after it, then the layout's content (Compose's
    /// `drawContent`). Call it once, or not at all to draw nothing below.
    pub fn draw_content(&mut self) {
        let next = &self.next;
        if next.index < next.nodes.len() {
            next.coordinator.draw_node_at(&next.nodes, next.index, &mut *self.scope.canvas);
        } else {
            next.coordinator.perform_draw(&mut *self.scope.canvas);
        }
    }
}

impl<'a> std::ops::Deref for ContentDrawScope<'a> {
    type Target = DrawScope<'a>;
    fn deref(&self) -> &DrawScope<'a> {
        &self.scope
    }
}

impl std::ops::DerefMut for ContentDrawScope<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.scope
    }
}

thread_local! {
    /// Drawing into what a recording canvas keeps from one frame to the next (C++'s gKeptPass).
    static KEPT_PASS: Cell<bool> = const { Cell::new(false) };
}

impl LayoutNode {
    /// Draws it (C++'s `LayoutNode::draw`, Compose's `LayoutNode.draw`): asked first whether the canvas keeps what it
    /// drew last frame (then nothing more), then between `begin_node` and `end_node`, through its coordinators,
    /// what it reads observed (a change draws it again).
    pub fn draw(&self, canvas: &mut dyn GraphicsContext) {
        let origin = if self.is_attached() { self.coordinates().position_in_root() } else { Offset::Zero };
        let recording = canvas.record_node(self.id, origin, !self.draw_dirty.get());
        if recording == NodeRecording::Kept {
            return;
        }
        let kept = recording == NodeRecording::Fresh || (recording == NodeRecording::Other && KEPT_PASS.get());
        if kept {
            self.draw_dirty.set(false);
        }
        let outer_pass = KEPT_PASS.replace(kept);
        canvas.begin_node(self.id);
        let outer = self.outer_coordinator();
        match self.owner() {
            Some(owner) => {
                let scope = self.reader_scope(&self.draw_reads, |n| n.invalidate_draw());
                owner.snapshot_observer().observe(&scope, || outer.draw(canvas));
            }
            None => outer.draw(canvas),
        }
        canvas.end_node();
        KEPT_PASS.set(outer_pass);
    }
}

impl NodeCoordinator {
    /// Draws it at its position (Compose's `NodeCoordinator.draw`): through its layer if it has one.
    pub(crate) fn draw(&self, canvas: &mut dyn GraphicsContext) {
        let position = self.position.get().to_offset();
        let moved = position != Offset::Zero;
        if moved {
            canvas.push_translation(position);
        }
        match self.layer_properties() {
            Some(layer) => {
                let size = self.measured_size.get().to_size();
                draw_through_layer(canvas, &layer, size, &mut |c| self.draw_contained_draw_modifiers(c));
            }
            None => self.draw_contained_draw_modifiers(canvas),
        }
        if moved {
            canvas.pop_translation();
        }
    }

    /// Its draw nodes, the first given what is after it (Compose's `drawContainedDrawModifiers`).
    fn draw_contained_draw_modifiers(&self, canvas: &mut dyn GraphicsContext) {
        let mut nodes = Vec::new();
        self.visit_nodes(NodeKind::Draw, &mut |cell| nodes.push(cell.clone()));
        if nodes.is_empty() {
            self.perform_draw(canvas);
        } else {
            self.draw_node_at(&Rc::new(nodes), 0, canvas);
        }
    }

    /// Draws the draw node at `index` of `nodes`.
    pub(crate) fn draw_node_at(&self, nodes: &Rc<Vec<NodeRc>>, index: usize, canvas: &mut dyn GraphicsContext) {
        let Some(layout_node) = self.layout_node() else { return };
        let cell = nodes[index].clone();
        let mut scope = ContentDrawScope {
            scope: DrawScope {
                canvas,
                size: self.measured_size.get().to_size(),
                layout_direction: layout_node.layout_direction(),
                density: layout_node.density(),
            },
            next: ContentNext { coordinator: self.rc(), nodes: nodes.clone(), index: index + 1 },
        };
        let mut node = cell.node.borrow_mut();
        if let Some(draw) = node.as_draw() {
            draw.draw(&mut scope);
        }
    }

    /// Draws what it holds (Compose's `performDraw`): what a layout modifier wraps, or the layout's placed
    /// children in z order.
    pub(crate) fn perform_draw(&self, canvas: &mut dyn GraphicsContext) {
        if self.is_inner() {
            let Some(node) = self.layout_node() else { return };
            for child in node.z_sorted_children() {
                if child.is_placed() {
                    child.draw(canvas);
                }
            }
        } else if let Some(wrapped) = self.wrapped() {
            wrapped.draw(canvas);
        }
    }

    /// Tells its draw nodes its size changed.
    pub(crate) fn on_draw_size_changed(&self) {
        self.visit_nodes(NodeKind::Draw, &mut |cell| {
            if let Some(d) = cell.node.borrow_mut().as_draw() {
                d.on_measure_result_changed();
            }
        });
    }
}

/// Draws `content` through a graphics layer of `size`: through its matrix, faded by its alpha (in a layer), cut to
/// its shape when it clips. The drawing package of this stage draws the rest as C++'s `drawThroughLayer` does
/// (its elevation shadow, render effect, blend mode, color filter and compositing strategy).
pub(crate) fn draw_through_layer(
    canvas: &mut dyn GraphicsContext,
    layer: &GraphicsLayerScope,
    size: Size,
    content: &mut dyn FnMut(&mut dyn GraphicsContext),
) {
    let alpha = layer.alpha.clamp(0.0, 1.0);
    if alpha <= 0.0 || layer.scale_x == 0.0 || layer.scale_y == 0.0 {
        return;
    }
    let matrix = layer.matrix();
    let moved = !matrix.is_identity();
    if moved {
        canvas.push_transform(&matrix);
    }
    let bounds = Rect::new(0.0, 0.0, size.width, size.height);
    let faded = alpha < 1.0;
    if faded {
        canvas.push_layer(bounds, CornerRadii::Zero, alpha, LayerTransform::default());
    }
    if layer.clip {
        canvas.push_clip_shape(
            bounds,
            &*layer.shape,
            layer.layout_direction,
            &layer.density,
            crate::graphics_context::ClipOp::Intersect,
        );
    }
    content(canvas);
    if layer.clip {
        canvas.pop_clip();
    }
    if faded {
        canvas.pop_layer();
    }
    if moved {
        canvas.pop_transform();
    }
}
