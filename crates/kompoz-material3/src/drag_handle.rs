//! Material's vertical drag handle, as Compose's DragHandle.kt: the handle a pane is resized by
//! ([`VerticalDragHandle`]), larger and darker while pressed or dragged ([`DragHandleSizes`], [`DragHandleColors`],
//! [`DragHandleShapes`], [`VerticalDragHandleDefaults`]).

use crate::color_scheme::ColorScheme;
use crate::interactive_component_size::InteractiveComponentSizeModifiers;
use crate::material_theme::MaterialTheme;
use crate::ripples::ripple;
use crate::shapes::Shapes;
use crate::tokens::DragHandleTokens;
use kompoz_foundation::gestures::{AwaitPointerEventScopeGestures, PointerInputScopeGestures};
use kompoz_foundation::prelude::*;
use kompoz_graphics::{Color, DpSize, Shape};
use kompoz_runtime::{KotlinProperties, kompozable, mutable_state_of, named, named_args, remember};
use kompoz_ui::input::{PointerEventPass, PointerInputScope};
use kompoz_ui::{Constraints, Modifier};
use std::fmt;
use std::sync::Arc;

/// A vertical drag handle (Compose's `VerticalDragHandle`): a pill of `sizes`' size, `shapes`' shape and
/// `colors`' color, larger, squarer and darker while pressed, more so while dragged (a drag interaction in
/// `interaction_source`, one of its own where none; the drag itself is what `modifier` gives, a
/// `Modifier.draggable`, say), hovered and showing the ripple through that source, in at least the minimum
/// interactive size.
#[kompozable]
pub fn VerticalDragHandle(
    #[default(Modifier)] modifier: Modifier,
    #[default(VerticalDragHandleDefaults.sizes())] sizes: DragHandleSizes,
    #[default(VerticalDragHandleDefaults.colors())] colors: DragHandleColors,
    #[default(VerticalDragHandleDefaults.shapes())] shapes: DragHandleShapes,
    #[default(None)] interaction_source: Option<MutableInteractionSource>,
) {
    let interaction_source = interaction_source.unwrap_or_else(|| remember(MutableInteractionSource::new));
    let is_dragged = interaction_source.collect_is_dragged_as_state();
    let is_pressed = remember(|| mutable_state_of(false));
    let (layer_dragged, layer_pressed) = (is_dragged.clone(), is_pressed.clone());
    let (layout_dragged, layout_pressed) = (is_dragged.clone(), is_pressed.clone());
    let (draw_dragged, draw_pressed) = (is_dragged.clone(), is_pressed.clone());
    let (on_pressed, on_released) = (is_pressed.clone(), is_pressed.clone());
    Box(modifier = modifier
        .minimum_interactive_component_size()
        .hoverable(interaction_source.clone())
        .pressable(interaction_source.clone(), move || on_pressed.set(true), move || on_released.set(false))
        .graphics_layer_block(move |scope| {
            scope.shape = if layer_dragged.get() {
                shapes.dragged_shape.clone()
            } else if layer_pressed.get() {
                shapes.pressed_shape.clone()
            } else {
                shapes.shape.clone()
            };
            scope.clip = true;
        })
        .layout(move |scope, measurable, _| {
            let drag_handle_size = if layout_dragged.get() {
                sizes.dragged_size
            } else if layout_pressed.get() {
                sizes.pressed_size
            } else {
                sizes.size
            };
            // set constraints here to be the size needed
            let placeable = measurable.measure(Constraints::fixed(
                fast_round_to_int(scope.to_px(drag_handle_size.width)),
                fast_round_to_int(scope.to_px(drag_handle_size.height)),
            ));
            let (w, h) = (placeable.width(), placeable.height());
            scope.layout(w, h, move |p| named_args!(p.place_relative(&placeable, 0, 0)))
        })
        .draw_behind(move |d| {
            let color = if draw_dragged.get() {
                colors.dragged_color
            } else if draw_pressed.get() {
                colors.pressed_color
            } else {
                colors.color
            };
            named_args!(d.draw_rect(color));
        })
        .indication(interaction_source, Some(named_args!(ripple()))));
}

/// Kotlin's `Float.fastRoundToInt` (Math.round: a half up).
fn fast_round_to_int(value: f32) -> i32 {
    (value + 0.5).floor() as i32
}

/// The colors of a drag handle at rest, pressed and dragged (Compose's `DragHandleColors`).
#[derive(Clone, Copy, PartialEq, Debug, KotlinProperties)]
pub struct DragHandleColors {
    pub color: Color,
    pub pressed_color: Color,
    pub dragged_color: Color,
}

/// Compose's `DragHandleColors(color, pressedColor, draggedColor)`.
#[named(type)]
pub fn DragHandleColors(color: Color, pressed_color: Color, dragged_color: Color) -> DragHandleColors {
    DragHandleColors { color, pressed_color, dragged_color }
}

/// The shapes of a drag handle at rest, pressed and dragged (Compose's `DragHandleShapes`), equal when their
/// shapes are.
#[derive(Clone, KotlinProperties)]
pub struct DragHandleShapes {
    pub shape: Arc<dyn Shape>,
    pub pressed_shape: Arc<dyn Shape>,
    pub dragged_shape: Arc<dyn Shape>,
}

/// Compose's `DragHandleShapes(shape, pressedShape, draggedShape)`.
#[named(type)]
pub fn DragHandleShapes(
    shape: Arc<dyn Shape>,
    pressed_shape: Arc<dyn Shape>,
    dragged_shape: Arc<dyn Shape>,
) -> DragHandleShapes {
    DragHandleShapes { shape, pressed_shape, dragged_shape }
}

impl PartialEq for DragHandleShapes {
    fn eq(&self, other: &Self) -> bool {
        *self.shape == *other.shape
            && *self.pressed_shape == *other.pressed_shape
            && *self.dragged_shape == *other.dragged_shape
    }
}

impl fmt::Debug for DragHandleShapes {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("DragHandleShapes")
            .field("shape", &self.shape)
            .field("pressed_shape", &self.pressed_shape)
            .field("dragged_shape", &self.dragged_shape)
            .finish()
    }
}

/// The sizes of a drag handle at rest, pressed and dragged (Compose's `DragHandleSizes`).
#[derive(Clone, Copy, PartialEq, Debug, KotlinProperties)]
pub struct DragHandleSizes {
    pub size: DpSize,
    pub pressed_size: DpSize,
    pub dragged_size: DpSize,
}

/// Compose's `DragHandleSizes(size, pressedSize, draggedSize)`.
#[named(type)]
pub fn DragHandleSizes(size: DpSize, pressed_size: DpSize, dragged_size: DpSize) -> DragHandleSizes {
    DragHandleSizes { size, pressed_size, dragged_size }
}

/// The defaults of a [`VerticalDragHandle`] (Compose's `object VerticalDragHandleDefaults`; a unit value:
/// `VerticalDragHandleDefaults.colors()`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct VerticalDragHandleDefaults;

#[named]
impl VerticalDragHandleDefaults {
    /// Its colors, the scheme's (the outline at rest, on-surface pressed and dragged) where unspecified (Compose's
    /// `colors()` and `colors(color, pressedColor, draggedColor)`: one function, all unspecified the first).
    pub fn colors(
        &self,
        #[default(Color::Unspecified)] color: Color,
        #[default(Color::Unspecified)] pressed_color: Color,
        #[default(Color::Unspecified)] dragged_color: Color,
    ) -> DragHandleColors {
        let defaults = default_colors(&MaterialTheme::color_scheme());
        DragHandleColors {
            color: color.take_or_else(|| defaults.color),
            pressed_color: pressed_color.take_or_else(|| defaults.pressed_color),
            dragged_color: dragged_color.take_or_else(|| defaults.dragged_color),
        }
    }

    /// Its shapes, the theme's (full at rest, medium corners pressed and dragged) where None (Compose's `shapes()`
    /// and `shapes(shape, pressedShape, draggedShape)`).
    pub fn shapes(
        &self,
        #[default(None)] shape: Option<Arc<dyn Shape>>,
        #[default(None)] pressed_shape: Option<Arc<dyn Shape>>,
        #[default(None)] dragged_shape: Option<Arc<dyn Shape>>,
    ) -> DragHandleShapes {
        let defaults = default_shapes(&MaterialTheme::shapes());
        DragHandleShapes {
            shape: shape.unwrap_or(defaults.shape),
            pressed_shape: pressed_shape.unwrap_or(defaults.pressed_shape),
            dragged_shape: dragged_shape.unwrap_or(defaults.dragged_shape),
        }
    }

    /// Its sizes, the tokens' (4 × 48 dp at rest, 12 × 52 dp pressed and dragged) where unspecified (Compose's
    /// `sizes()` and `sizes(size, pressedSize, draggedSize)`).
    pub fn sizes(
        &self,
        #[default(DpSize::Unspecified)] size: DpSize,
        #[default(DpSize::Unspecified)] pressed_size: DpSize,
        #[default(DpSize::Unspecified)] dragged_size: DpSize,
    ) -> DragHandleSizes {
        let defaults = DefaultSizes;
        DragHandleSizes {
            size: if size.is_specified() { size } else { defaults.size },
            pressed_size: if pressed_size.is_specified() { pressed_size } else { defaults.pressed_size },
            dragged_size: if dragged_size.is_specified() { dragged_size } else { defaults.dragged_size },
        }
    }
}

/// Compose's private `ColorScheme.colors` (no per-scheme cache: a fresh value).
fn default_colors(scheme: &ColorScheme) -> DragHandleColors {
    DragHandleColors {
        color: scheme.from_token(DragHandleTokens::Color),
        pressed_color: scheme.from_token(DragHandleTokens::PressedColor),
        dragged_color: scheme.from_token(DragHandleTokens::DraggedColor),
    }
}

/// Compose's private `Shapes.shapes` (no per-shapes cache: a fresh value).
fn default_shapes(shapes: &Shapes) -> DragHandleShapes {
    DragHandleShapes {
        shape: shapes.from_token(DragHandleTokens::Shape),
        pressed_shape: shapes.from_token(DragHandleTokens::PressedShape),
        dragged_shape: shapes.from_token(DragHandleTokens::DraggedShape),
    }
}

/// Compose's private `sizes`.
const DefaultSizes: DragHandleSizes = DragHandleSizes {
    size: DpSize { width: DragHandleTokens::Width, height: DragHandleTokens::Height },
    pressed_size: DpSize { width: DragHandleTokens::PressedWidth, height: DragHandleTokens::PressedHeight },
    dragged_size: DpSize { width: DragHandleTokens::DraggedWidth, height: DragHandleTokens::DraggedHeight },
};

/// Compose's private `Modifier.pressable`.
trait PressableModifier {
    /// Calls `on_pressed` at each gesture's first press (seen in the initial pass) and
    /// `on_released_or_cancelled` at its end.
    fn pressable(
        self,
        interaction_source: MutableInteractionSource,
        on_pressed: impl Fn() + 'static,
        on_released_or_cancelled: impl Fn() + 'static,
    ) -> Modifier;
}

impl PressableModifier for Modifier {
    fn pressable(
        self,
        interaction_source: MutableInteractionSource,
        on_pressed: impl Fn() + 'static,
        on_released_or_cancelled: impl Fn() + 'static,
    ) -> Modifier {
        let on_pressed: std::rc::Rc<dyn Fn()> = std::rc::Rc::new(on_pressed);
        let on_released_or_cancelled: std::rc::Rc<dyn Fn()> = std::rc::Rc::new(on_released_or_cancelled);
        self.pointer_input(interaction_source, move |scope: PointerInputScope| {
            let (on_pressed, on_released_or_cancelled) = (on_pressed.clone(), on_released_or_cancelled.clone());
            async move {
                scope
                    .await_each_gesture(move |s| {
                        let (on_pressed, on_released_or_cancelled) =
                            (on_pressed.clone(), on_released_or_cancelled.clone());
                        async move {
                            s.await_first_down_with(true, PointerEventPass::Initial).await;
                            on_pressed();
                            s.wait_for_up_or_cancellation_with(PointerEventPass::Initial).await;
                            on_released_or_cancelled();
                        }
                    })
                    .await;
            }
        })
    }
}
