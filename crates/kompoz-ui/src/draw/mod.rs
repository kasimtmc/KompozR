//! Drawing, as Compose's `androidx.compose.ui.draw` over the [`GraphicsContext`]: draw modifier nodes draw in a
//! [`ContentDrawScope`] (the context, their size, layout direction and density, and `draw_content()` for what is
//! below them); a layout node draws through its coordinators, outer to inner, each translated to its position (and
//! through its layer), its children in z order, what it reads observed, and the node hooks a display list keeps
//! frames by around it (C++'s `LayoutNode::draw`).
//!
//! The drawing API of the scope (`draw_rect` and the rest: [`scope`]; laid out text, `draw_text`: `text.rs`),
//! Compose's ui.graphics [`Canvas`] and
//! [`Paint`], the draw modifiers ([`modifiers`]), [`GraphicsLayer`] recording, painters ([`painter`]) and
//! composable vector content ([`vector`]) are in this module's other files.

mod bitmap;
mod canvas;
mod graphics_layer;
mod layer_draw;
pub mod modifiers;
pub mod painter;
mod picture;
pub mod scope;
mod style;
mod text;
pub mod vector;

pub use bitmap::{BitmapRenderer, draw_into_bitmap, pending_draws, set_bitmap_renderer};
pub use canvas::{Canvas, CanvasState, Paint, PaintingStyle};
pub use graphics_layer::{GraphicsLayer, GraphicsLayerProperties, remember_graphics_layer};
pub use modifiers::{
    BlurScope, BlurType, BlurredEdgeTreatment, CacheDrawModifierNode, CacheDrawScope, DrawResult, ShadowScope,
};
pub use painter::{
    BitmapPainter, BrushPainter, ColorPainter, DrawPainter, Painter, VectorPainter, draw_vector,
    remember_vector_painter, vector_drawn_into,
};
pub use picture::Picture;
pub use scope::{DrawContext, DrawTransform};
pub use style::{DefaultAlpha, DefaultBlendMode, DrawStyle, Fill, PointMode, Stroke};
pub use vector::remember_vector_painter_content;

// What drawing through a graphics layer is made of, for a backend's tests (C++'s detail::drawThroughLayer).
#[doc(hidden)]
pub use layer_draw::{LayerDraw, draw_layer, draw_through_layer};

use crate::graphics_context::{GraphicsContext, NodeRecording};
use crate::modifier::{NodeKind, NodeRc};
use crate::node::{LayoutNode, NodeCoordinator};
use canvas::CanvasSlot;
use kompoz_graphics::{Density, LayoutDirection, Offset, Size};
use layer_draw::DRAW_PART;
use std::cell::Cell;
use std::rc::Rc;

/// A node that draws (Compose's DrawModifierNode): what it draws around `scope.draw_content()`, which draws what is
/// below it (the next draw nodes, the layout's content).
pub trait DrawModifierNode {
    fn draw(&mut self, scope: &mut ContentDrawScope);
    /// Its layout's size changed (Compose's `onMeasureResultChanged`).
    fn on_measure_result_changed(&mut self) {}
}

/// Where drawing happens (Compose's DrawScope): the context, where the scope is in it and its size, the layout
/// direction and the density (it derefs to it: `scope.to_px(1.dp())`). Its drawing functions are [`scope`]'s.
pub struct DrawScope<'a> {
    pub(crate) canvas: &'a mut dyn GraphicsContext,
    /// Its top left in the context (an inset moves it).
    pub(crate) origin: Offset,
    pub(crate) size: Size,
    pub(crate) layout_direction: LayoutDirection,
    pub(crate) density: Density,
    /// Its canvas's stack, made when asked for (or the canvas's it draws over).
    pub(crate) canvas_state: CanvasSlot<'a>,
}

impl<'a> DrawScope<'a> {
    /// A scope of `size` over `canvas`, from its origin.
    pub fn new(
        canvas: &'a mut dyn GraphicsContext,
        size: Size,
        layout_direction: LayoutDirection,
        density: Density,
    ) -> Self {
        DrawScope::at(canvas, Offset::Zero, size, layout_direction, density)
    }

    /// A scope of `size` over `canvas`, its top left at `origin` there (C++'s DrawScope over bounds).
    pub fn at(
        canvas: &'a mut dyn GraphicsContext,
        origin: Offset,
        size: Size,
        layout_direction: LayoutDirection,
        density: Density,
    ) -> Self {
        DrawScope { canvas, origin, size, layout_direction, density, canvas_state: CanvasSlot::Own(None) }
    }

    /// A scope of `size` over a [`Canvas`], drawing into it from its origin (Compose's CanvasDrawScope.draw):
    /// `canvas()` and `draw_into_canvas` give that canvas, what is pushed on it left there.
    pub fn over_canvas(
        canvas: &'a mut Canvas<'_>,
        size: Size,
        layout_direction: LayoutDirection,
        density: Density,
    ) -> Self {
        let (context, state) = canvas.parts();
        let origin = state.origin();
        DrawScope { canvas: context, origin, size, layout_direction, density, canvas_state: CanvasSlot::Of(state) }
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
    /// The context drawn into (its coordinates: this scope's top left is [`origin`](DrawScope::origin)).
    pub fn context(&mut self) -> &mut dyn GraphicsContext {
        &mut *self.canvas
    }
    /// Its top left in the context.
    pub fn origin(&self) -> Offset {
        self.origin
    }
}

/// What its canvas left pushed is popped as it goes.
impl Drop for DrawScope<'_> {
    fn drop(&mut self) {
        if let CanvasSlot::Own(Some(state)) = &mut self.canvas_state {
            state.restore_to(&mut *self.canvas, 0);
        }
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
#[derive(Clone)]
pub(crate) struct ContentNext {
    pub(crate) coordinator: Rc<NodeCoordinator>,
    pub(crate) nodes: Rc<Vec<NodeRc>>,
    pub(crate) index: usize,
}

impl ContentDrawScope<'_> {
    /// Draws what is below the node: the draw nodes after it, then the layout's content (Compose's
    /// `drawContent`), at the scope's top left (an inset or a translation moves it). Call it once, or not at all
    /// to draw nothing below.
    pub fn draw_content(&mut self) {
        let origin = self.scope.origin;
        let canvas = &mut *self.scope.canvas;
        let moved = origin != Offset::Zero;
        if moved {
            canvas.push_translation(origin);
        }
        let next = &self.next;
        if next.index < next.nodes.len() {
            next.coordinator.draw_node_at(&next.nodes, next.index, canvas);
        } else {
            next.coordinator.perform_draw(canvas);
        }
        if moved {
            canvas.pop_translation();
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
    pub(crate) static KEPT_PASS: Cell<bool> = const { Cell::new(false) };
}

impl LayoutNode {
    /// Draws it (C++'s `LayoutNode::draw`, Compose's `LayoutNode.draw`): asked first whether the canvas keeps what it
    /// drew last frame (then nothing more), then between `begin_node` and `end_node`, through its coordinators,
    /// what it reads observed (a change draws it again).
    pub fn draw(&self, canvas: &mut dyn GraphicsContext) {
        let origin = self.outer_coordinator().position.get().to_offset(); // (where it is in its parent)
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
                match &layer.record_into {
                    // Placed with a graphics layer (Compose's `placeWithLayer(offset, layer)`): what it draws is
                    // that layer's recording, drawn through it (and drawable again by whoever holds the layer).
                    Some(recorded) => {
                        let mut picture = Picture::new();
                        let outer = KEPT_PASS.replace(false);
                        self.draw_contained_draw_modifiers(&mut picture);
                        KEPT_PASS.set(outer);
                        recorded.keep_recording(picture, self.measured_size.get());
                        let picture = recorded.picture();
                        draw_through_layer(canvas, &layer, size, &mut |c| {
                            if let Some(p) = &picture {
                                p.replay(c);
                            }
                        });
                    }
                    None => draw_through_layer(canvas, &layer, size, &mut |c| self.draw_contained_draw_modifiers(c)),
                }
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
            scope: DrawScope::new(
                canvas,
                self.measured_size.get().to_size(),
                layout_node.layout_direction(),
                layout_node.density(),
            ),
            next: ContentNext { coordinator: self.rc(), nodes: nodes.clone(), index: index + 1 },
        };
        let mut node = cell.node.borrow_mut();
        if let Some(draw) = node.as_draw() {
            draw.draw(&mut scope);
        }
    }

    /// Draws what it holds (Compose's `performDraw`): what a layout modifier wraps, or the layout's placed
    /// children in z order (none while only its own drawing is drawn, and only them, onto the canvas under the
    /// state-only one, while only they are: a blur of the node's own drawing).
    pub(crate) fn perform_draw(&self, canvas: &mut dyn GraphicsContext) {
        if self.is_inner() {
            let Some(node) = self.layout_node() else { return };
            let part = DRAW_PART.with(|p| p.borrow().clone()).filter(|p| p.node == node.id);
            if part.as_ref().is_some_and(|p| p.own) {
                return;
            }
            let outer_part = part.as_ref().map(|p| {
                p.pass.set(true);
                DRAW_PART.with(|d| d.borrow_mut().take())
            });
            for child in node.z_sorted_children() {
                if child.is_placed() {
                    child.draw(canvas);
                }
            }
            if let (Some(p), Some(outer)) = (&part, outer_part) {
                p.pass.set(false);
                DRAW_PART.with(|d| *d.borrow_mut() = outer);
            }
        } else if let Some(wrapped) = self.wrapped() {
            wrapped.draw(canvas);
        }
    }

    /// Tells its draw nodes its size changed (Compose's `visitNodes(Nodes.Draw) { it.onMeasureResultChanged() }`):
    /// the layout node of the coordinator wrapping it too, which draws in its space — told once its `measure`, under
    /// way as it measures this one, has returned.
    pub(crate) fn on_draw_size_changed(&self) {
        self.visit_nodes(NodeKind::Draw, &mut |cell| match cell.node.try_borrow_mut() {
            Ok(mut node) => {
                if let Some(d) = node.as_draw() {
                    d.on_measure_result_changed();
                }
            }
            Err(_) => DRAW_SIZE_CHANGED_LATER.with(|l| l.borrow_mut().push(cell.clone())),
        });
    }
}

thread_local! {
    /// Draw nodes told of a size change while they were measuring (a layout node drawing in the space of what it
    /// wraps, which its own `measure` measures): told once it returns ([`tell_draw_size_changed_later`]).
    static DRAW_SIZE_CHANGED_LATER: std::cell::RefCell<Vec<NodeRc>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// Tells the draw nodes that were busy measuring when their size changed, those no longer busy.
pub(crate) fn tell_draw_size_changed_later() {
    let waiting = DRAW_SIZE_CHANGED_LATER.with(|l| std::mem::take(&mut *l.borrow_mut()));
    for cell in waiting {
        match cell.node.try_borrow_mut() {
            Ok(mut node) => {
                if let Some(d) = node.as_draw() {
                    d.on_measure_result_changed();
                }
            }
            Err(_) => DRAW_SIZE_CHANGED_LATER.with(|l| l.borrow_mut().push(cell.clone())),
        }
    }
}
