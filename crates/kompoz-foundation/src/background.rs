//! `Modifier.background`, as Compose's: a color or a brush behind the content, in a shape.

use kompoz_graphics::{Brush, Color, LayoutDirection, Outline, RectangleShape, Shape, Size};
use kompoz_runtime::named;
use kompoz_ui::draw::{ContentDrawScope, DrawModifierNode};
use kompoz_ui::modifier::ObserverModifierNode;
use kompoz_ui::semantics::{SemanticsConfiguration, SemanticsModifierNode};
use kompoz_ui::{Modifier, ModifierNode, ModifierNodeElement, NodeCx, NodeHandle, node_capabilities};
use std::sync::Arc;

/// The background modifiers (Compose's `Modifier.background` overloads).
#[named(trait BackgroundModifiers)]
impl Modifier {
    /// `color` behind the content, in `shape` (Compose's `background(color, shape)`).
    fn background(self, color: Color, #[default(Arc::new(RectangleShape))] shape: Arc<dyn Shape>) -> Modifier {
        self.then_element(BackgroundElement { color, brush: None, alpha: 1.0, shape })
    }

    /// [`background`](BackgroundModifiers::background) with every parameter, for plain code.
    fn background_with(self, color: Color, shape: impl Shape + 'static) -> Modifier {
        self.then_element(BackgroundElement { color, brush: None, alpha: 1.0, shape: Arc::new(shape) })
    }

    /// `brush` behind the content, in `shape`, faded by `alpha` (Compose's `background(brush, shape, alpha)`).
    fn background_brush(
        self,
        brush: Brush,
        #[default(Arc::new(RectangleShape))] shape: Arc<dyn Shape>,
        #[default(1.0)] alpha: f32,
    ) -> Modifier {
        self.then_element(BackgroundElement { color: Color::Unspecified, brush: Some(brush), alpha, shape })
    }

    /// [`background_brush`](BackgroundModifiers::background_brush) with every parameter, for plain code.
    fn background_brush_with(self, brush: Brush, shape: impl Shape + 'static, alpha: f32) -> Modifier {
        self.then_element(BackgroundElement {
            color: Color::Unspecified,
            brush: Some(brush),
            alpha,
            shape: Arc::new(shape),
        })
    }
}

struct BackgroundElement {
    color: Color,
    brush: Option<Brush>,
    alpha: f32,
    shape: Arc<dyn Shape>,
}

impl PartialEq for BackgroundElement {
    fn eq(&self, o: &Self) -> bool {
        self.color == o.color && self.brush == o.brush && self.alpha == o.alpha && *self.shape == *o.shape
    }
}

impl ModifierNodeElement for BackgroundElement {
    type Node = BackgroundNode;
    fn create(&self) -> BackgroundNode {
        BackgroundNode {
            color: self.color,
            brush: self.brush.clone(),
            alpha: self.alpha,
            shape: self.shape.clone(),
            last: None,
            handle: None,
        }
    }
    fn update(&self, node: &mut BackgroundNode) {
        node.color = self.color;
        node.brush = self.brush.clone();
        node.alpha = self.alpha;
        if *node.shape != *self.shape {
            node.shape = self.shape.clone();
            node.last = None;
            if let Some(h) = &node.handle {
                h.invalidate_semantics();
            }
        }
        if let Some(h) = &node.handle {
            h.invalidate_draw();
        }
    }
    fn name(&self) -> &'static str {
        "background"
    }
}

/// The node of `Modifier.background` (Compose's BackgroundNode): a rect, or its shape's outline (kept for its size,
/// direction and shape; what making it reads observed), in its color and brush; its shape is its layout's `Shape`
/// semantics property.
struct BackgroundNode {
    color: Color,
    brush: Option<Brush>,
    alpha: f32,
    shape: Arc<dyn Shape>,
    last: Option<(Size, LayoutDirection, Outline)>,
    handle: Option<NodeHandle>,
}

impl BackgroundNode {
    fn outline(&mut self, size: Size, direction: LayoutDirection, density: kompoz_graphics::Density) -> Outline {
        if let Some((s, d, o)) = &self.last
            && *s == size
            && *d == direction
        {
            return o.clone();
        }
        let shape = self.shape.clone();
        let make = || shape.create_outline(size, direction, &density);
        let outline = match &self.handle {
            Some(h) => h.observe_reads(make),
            None => make(),
        };
        self.last = Some((size, direction, outline.clone()));
        outline
    }
}

impl ModifierNode for BackgroundNode {
    node_capabilities!(draw, observer, semantics);
    fn on_attach(&mut self, cx: &NodeCx) {
        self.handle = Some(cx.handle());
    }
    fn should_auto_invalidate(&self) -> bool {
        false
    }
}

impl SemanticsModifierNode for BackgroundNode {
    fn apply_semantics(&mut self, config: &mut SemanticsConfiguration) {
        config.set_shape(self.shape.clone());
    }
}

impl ObserverModifierNode for BackgroundNode {
    fn on_observed_reads_changed(&mut self) {
        self.last = None;
        if let Some(h) = &self.handle {
            h.invalidate_draw();
        }
    }
}

impl DrawModifierNode for BackgroundNode {
    fn draw(&mut self, scope: &mut ContentDrawScope) {
        if self.shape.shape_as_any().is::<kompoz_graphics::shape::RectangleShape>() {
            if self.color.is_specified() {
                scope.draw_rect(self.color);
            }
            if let Some(brush) = &self.brush {
                let size = scope.size();
                scope.draw_rect_with(
                    brush.clone(),
                    kompoz_graphics::Offset::Zero,
                    size,
                    self.alpha,
                    kompoz_ui::draw::Fill,
                    None,
                    kompoz_ui::draw::DefaultBlendMode,
                );
            }
        } else {
            let outline = self.outline(scope.size(), scope.layout_direction(), scope.density());
            if self.color.is_specified() {
                scope.draw_outline(&outline, self.color);
            }
            if let Some(brush) = &self.brush {
                scope.draw_outline_with(
                    &outline,
                    brush.clone(),
                    self.alpha,
                    kompoz_ui::draw::Fill,
                    None,
                    kompoz_ui::draw::DefaultBlendMode,
                );
            }
        }
        scope.draw_content();
    }
}
