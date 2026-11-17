//! The ripple node, as Compose's Ripple.kt and CommonRipple.kt (the common path: the desktop's and the web's,
//! Android's RippleDrawable aside): ripples spreading from presses, and a state layer for hovers, focuses and drags.

use crate::ripple_animation::{RippleAnimation, get_ripple_end_radius};
use kompoz_animation::{Animatable, AnimationSpec, AnimationVector1D, LinearEasing, TweenSpec};
use kompoz_coroutines::CoroutineScope;
use kompoz_foundation::interaction::{
    DragInteraction, FocusInteraction, HoverInteraction, Interaction, InteractionId, InteractionSource,
    PressInteraction,
};
use kompoz_graphics::{Color, ColorProducer, Density, Dp, IntSize, Size};
use kompoz_runtime::named_args;
use kompoz_ui::draw::{ContentDrawScope, DrawModifierNode, DrawScope};
use kompoz_ui::layout::LayoutAwareModifierNode;
use kompoz_ui::{ModifierNode, NodeCx, NodeHandle, node_capabilities};
use std::cell::{Cell, RefCell};
use std::fmt;
use std::rc::Rc;
use std::sync::Arc;

/// How opaque a ripple and its state layer are for each interaction (Compose's RippleAlpha): a design system's.
#[derive(Clone, Copy, PartialEq, Default)]
pub struct RippleAlpha {
    pub dragged_alpha: f32,
    pub focused_alpha: f32,
    pub hovered_alpha: f32,
    pub pressed_alpha: f32,
}

impl RippleAlpha {
    /// Kotlin: `RippleAlpha(draggedAlpha: Float, focusedAlpha: Float, hoveredAlpha: Float, pressedAlpha: Float)`
    pub const fn new(dragged_alpha: f32, focused_alpha: f32, hovered_alpha: f32, pressed_alpha: f32) -> RippleAlpha {
        RippleAlpha { dragged_alpha, focused_alpha, hovered_alpha, pressed_alpha }
    }
}

/// Compose's `RippleAlpha(draggedAlpha, focusedAlpha, hoveredAlpha, pressedAlpha)`.
pub fn RippleAlpha(dragged_alpha: f32, focused_alpha: f32, hovered_alpha: f32, pressed_alpha: f32) -> RippleAlpha {
    RippleAlpha::new(dragged_alpha, focused_alpha, hovered_alpha, pressed_alpha)
}

/// Compose's `toString`.
impl fmt::Display for RippleAlpha {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "RippleAlpha(draggedAlpha={}, focusedAlpha={}, hoveredAlpha={}, pressedAlpha={})",
            kt_float(self.dragged_alpha),
            kt_float(self.focused_alpha),
            kt_float(self.hovered_alpha),
            kt_float(self.pressed_alpha)
        )
    }
}

impl fmt::Debug for RippleAlpha {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

/// A float as Kotlin writes it (`1.0`, `0.12`).
fn kt_float(v: f32) -> String {
    if v.fract() == 0.0 && v.is_finite() { format!("{v:.1}") } else { format!("{v}") }
}

/// A ripple node (Compose's `createRippleModifierNode`), what a design system's ripple indication makes for each
/// component (its `IndicationNodeFactory::create`, or a node of its own delegating to it): `interaction_source`'s
/// presses spread ripples (from the press when `bounded`, clipped to the bounds; else from the middle, unclipped),
/// to `radius` (unspecified: to the corners, and 10 dp further when bounded) in `color` (read as it draws) at
/// `ripple_alpha`'s pressed alpha; its hovers, focuses and drags show a state layer of that radius at their alphas.
pub fn create_ripple_modifier_node(
    interaction_source: InteractionSource,
    bounded: bool,
    radius: Dp,
    color: ColorProducer,
    ripple_alpha: impl Fn() -> RippleAlpha + 'static,
) -> Box<dyn ModifierNode> {
    Box::new(RippleNode {
        interaction_source,
        shared: Rc::new(RippleShared {
            bounded,
            radius,
            color,
            ripple_alpha: Rc::new(ripple_alpha),
            state_layer: RefCell::new(None),
            target_radius: Cell::new(0.0),
            ripple_size: Cell::new(Size::Zero),
            has_valid_size: Cell::new(false),
            pending_interactions: RefCell::new(Vec::new()),
            ripples: RefCell::new(Vec::new()),
            handle: RefCell::new(None),
            density: Cell::new(Density::default()),
        }),
    })
}

/// A press interaction, as the node keeps it.
#[derive(Clone, Copy)]
enum PressKind {
    Press(PressInteraction::Press),
    Release(PressInteraction::Press),
    Cancel(PressInteraction::Press),
}

/// What the node and its coroutines share (Compose's RippleNode and CommonRippleNode fields).
struct RippleShared {
    bounded: bool,
    radius: Dp,
    color: ColorProducer,
    ripple_alpha: Rc<dyn Fn() -> RippleAlpha>,
    state_layer: RefCell<Option<Rc<StateLayer>>>,
    /// Worked out as it is measured; a ripple added before then waits (Compose's `targetRadius`).
    target_radius: Cell<f32>,
    ripple_size: Cell<Size>,
    has_valid_size: Cell<bool>,
    /// The presses that came before it had a size (Compose's `pendingInteractions`).
    pending_interactions: RefCell<Vec<PressKind>>,
    /// The ripples by their presses, in the order they came (Compose's `ripples` map).
    ripples: RefCell<Vec<(InteractionId, Rc<RippleAnimation>)>>,
    handle: RefCell<Option<NodeHandle>>,
    density: Cell<Density>,
}

impl RippleShared {
    fn invalidate_draw(&self) {
        if let Some(h) = self.handle.borrow().as_ref() {
            h.invalidate_draw();
        }
    }

    fn coroutine_scope(&self) -> Option<CoroutineScope> {
        self.handle.borrow().as_ref().and_then(|h| h.coroutine_scope())
    }

    fn handle_press_interaction(self: &Rc<Self>, press: PressKind) {
        match press {
            PressKind::Press(p) => self.add_ripple(p),
            PressKind::Release(p) | PressKind::Cancel(p) => self.remove_ripple(p),
        }
    }

    /// The ripples before finished, a new one spreading from the press (Compose's CommonRippleNode.addRipple).
    fn add_ripple(self: &Rc<Self>, interaction: PressInteraction::Press) {
        for (_, ripple) in self.ripples.borrow().iter() {
            ripple.finish();
        }
        let origin = if self.bounded { Some(interaction.press_position) } else { None };
        let ripple_animation = Rc::new(RippleAnimation::new(origin, self.target_radius.get(), self.bounded));
        let id = interaction.id();
        self.ripples.borrow_mut().push((id, ripple_animation.clone()));
        if let Some(scope) = self.coroutine_scope() {
            let shared = Rc::downgrade(self);
            scope.launch(async move {
                // (Kotlin's `finally`: gone once it ends, or is cancelled)
                let _remove = RemoveRipple(shared, id);
                ripple_animation.animate().await;
            });
        }
        self.invalidate_draw();
    }

    fn remove_ripple(&self, interaction: PressInteraction::Press) {
        let ripple = self.ripples.borrow().iter().find(|(id, _)| *id == interaction.id()).map(|(_, r)| r.clone());
        if let Some(ripple) = ripple {
            ripple.finish();
        }
    }

    /// The state layer told of `interaction` (made at its first, drawn from then on).
    fn update_state_layer(&self, interaction: &dyn Interaction) {
        let state_layer = self.state_layer.borrow().clone();
        let state_layer = match state_layer {
            Some(s) => s,
            None => {
                let s = Rc::new(StateLayer::new(self.bounded, self.ripple_alpha.clone()));
                // (drawn from now on)
                self.invalidate_draw();
                *self.state_layer.borrow_mut() = Some(s.clone());
                s
            }
        };
        if let Some(scope) = self.coroutine_scope() {
            state_layer.handle_interaction(interaction, &scope);
        }
    }
}

/// Removes a ripple from its node as its coroutine ends.
struct RemoveRipple(std::rc::Weak<RippleShared>, InteractionId);

impl Drop for RemoveRipple {
    fn drop(&mut self) {
        if let Some(shared) = self.0.upgrade() {
            shared.ripples.borrow_mut().retain(|(id, _)| *id != self.1);
            shared.invalidate_draw();
        }
    }
}

/// The ripple node (Compose's RippleNode with CommonRippleNode, the common platforms' ripples): it doesn't redraw
/// for being updated, only for its ripples and state layer.
struct RippleNode {
    interaction_source: InteractionSource,
    shared: Rc<RippleShared>,
}

impl ModifierNode for RippleNode {
    node_capabilities!(draw, layout_aware);

    fn should_auto_invalidate(&self) -> bool {
        false
    }

    fn on_attach(&mut self, cx: &NodeCx) {
        *self.shared.handle.borrow_mut() = Some(cx.handle());
        self.shared.density.set(cx.density());
        let shared = self.shared.clone();
        let flow = self.interaction_source.interactions();
        cx.coroutine_scope().launch(flow.collect(move |interaction: Arc<dyn Interaction>| {
            let i = &*interaction;
            let press = if let Some(p) = i.downcast_ref::<PressInteraction::Press>() {
                Some(PressKind::Press(*p))
            } else if let Some(r) = i.downcast_ref::<PressInteraction::Release>() {
                Some(PressKind::Release(r.press))
            } else {
                i.downcast_ref::<PressInteraction::Cancel>().map(|c| PressKind::Cancel(c.press))
            };
            match press {
                Some(press) if shared.has_valid_size.get() => shared.handle_press_interaction(press),
                // (handled once it has a size)
                Some(press) => shared.pending_interactions.borrow_mut().push(press),
                None => shared.update_state_layer(i),
            }
        }));
    }

    fn on_detach(&mut self, _cx: &NodeCx) {
        self.shared.ripples.borrow_mut().clear();
        *self.shared.handle.borrow_mut() = None;
    }
}

impl LayoutAwareModifierNode for RippleNode {
    fn on_remeasured(&mut self, size: IntSize) {
        let shared = &self.shared;
        shared.has_valid_size.set(true);
        let density = shared.handle.borrow().as_ref().map_or(shared.density.get(), |h| h.density());
        let size = Size::new(size.width as f32, size.height as f32);
        shared.ripple_size.set(size);
        // (worked out here rather than Android's RippleDrawable.RADIUS_AUTO: the spec's)
        shared.target_radius.set(if shared.radius.is_unspecified() {
            get_ripple_end_radius(&density, shared.bounded, size)
        } else {
            density.to_px(shared.radius)
        });
        // The presses that waited for a size.
        let pending: Vec<PressKind> = shared.pending_interactions.borrow_mut().drain(..).collect();
        for press in pending {
            shared.handle_press_interaction(press);
        }
    }
}

impl DrawModifierNode for RippleNode {
    fn draw(&mut self, scope: &mut ContentDrawScope) {
        scope.draw_content();
        let shared = &self.shared;
        let color = shared.color.get();
        let state_layer = shared.state_layer.borrow().clone();
        if let Some(state_layer) = state_layer {
            state_layer.draw_state_layer(scope, shared.target_radius.get(), color);
        }
        // Compose's CommonRippleNode.drawRipples.
        let alpha = (shared.ripple_alpha)().pressed_alpha;
        if alpha != 0.0 {
            let ripples: Vec<Rc<RippleAnimation>> = shared.ripples.borrow().iter().map(|(_, r)| r.clone()).collect();
            for ripple in ripples {
                ripple.draw(scope, named_args!(color.copy(alpha = alpha)));
            }
        }
    }
}

/// An interaction the state layer shows, by its kind and identity.
#[derive(Clone, Copy, PartialEq, Debug)]
enum StateInteraction {
    Hover(InteractionId),
    Focus(InteractionId),
    Drag(InteractionId),
}

/// The layer showing a hover, a focus or a drag, the last that came on top (Compose's StateLayer): its alpha
/// animated to the interaction's.
struct StateLayer {
    bounded: bool,
    ripple_alpha: Rc<dyn Fn() -> RippleAlpha>,
    animated_alpha: Animatable<f32, AnimationVector1D>,
    interactions: RefCell<Vec<StateInteraction>>,
    current_interaction: Cell<Option<StateInteraction>>,
}

impl StateLayer {
    fn new(bounded: bool, ripple_alpha: Rc<dyn Fn() -> RippleAlpha>) -> StateLayer {
        StateLayer {
            bounded,
            ripple_alpha,
            animated_alpha: Animatable(0.0),
            interactions: RefCell::new(Vec::new()),
            current_interaction: Cell::new(None),
        }
    }

    /// Notes `interaction` and animates to what the latest one shows (Compose's `handleInteraction`).
    fn handle_interaction(&self, interaction: &dyn Interaction, scope: &CoroutineScope) {
        {
            let mut interactions = self.interactions.borrow_mut();
            let remove = |list: &mut Vec<StateInteraction>, gone: StateInteraction| list.retain(|i| *i != gone);
            if let Some(e) = interaction.downcast_ref::<HoverInteraction::Enter>() {
                interactions.push(StateInteraction::Hover(e.id()));
            } else if let Some(e) = interaction.downcast_ref::<HoverInteraction::Exit>() {
                remove(&mut interactions, StateInteraction::Hover(e.enter.id()));
            } else if let Some(f) = interaction.downcast_ref::<FocusInteraction::Focus>() {
                interactions.push(StateInteraction::Focus(f.id()));
            } else if let Some(u) = interaction.downcast_ref::<FocusInteraction::Unfocus>() {
                remove(&mut interactions, StateInteraction::Focus(u.focus.id()));
            } else if let Some(s) = interaction.downcast_ref::<DragInteraction::Start>() {
                interactions.push(StateInteraction::Drag(s.id()));
            } else if let Some(s) = interaction.downcast_ref::<DragInteraction::Stop>() {
                remove(&mut interactions, StateInteraction::Drag(s.start.id()));
            } else if let Some(c) = interaction.downcast_ref::<DragInteraction::Cancel>() {
                remove(&mut interactions, StateInteraction::Drag(c.start.id()));
            } else {
                return;
            }
        }
        // The latest is the one shown.
        let new_interaction = self.interactions.borrow().last().copied();
        if self.current_interaction.get() != new_interaction {
            let alpha = self.animated_alpha.clone();
            match new_interaction {
                Some(new) => {
                    let ripple_alpha = (self.ripple_alpha)();
                    let target_alpha = match new {
                        StateInteraction::Hover(_) => ripple_alpha.hovered_alpha,
                        StateInteraction::Focus(_) => ripple_alpha.focused_alpha,
                        StateInteraction::Drag(_) => ripple_alpha.dragged_alpha,
                    };
                    let spec = incoming_state_layer_animation_spec_for(new);
                    scope.launch(async move {
                        named_args!(alpha.animate_to(target_alpha, animation_spec = spec)).await;
                    });
                }
                None => {
                    let spec = outgoing_state_layer_animation_spec_for(self.current_interaction.get());
                    scope.launch(async move {
                        named_args!(alpha.animate_to(0.0, animation_spec = spec)).await;
                    });
                }
            }
            self.current_interaction.set(new_interaction);
        }
    }

    /// A circle of `radius` in `color` at the animated alpha, clipped to the bounds when bounded (Compose's
    /// `drawStateLayer`).
    fn draw_state_layer(&self, scope: &mut DrawScope, radius: f32, color: Color) {
        let alpha = self.animated_alpha.value();
        if alpha > 0.0 {
            let modulated_color = named_args!(color.copy(alpha = alpha));
            let center = scope.center();
            if self.bounded {
                let size = scope.size();
                scope.clip_rect_with(0.0, 0.0, size.width, size.height, Default::default(), |s| {
                    s.draw_circle_with(
                        modulated_color,
                        radius,
                        center,
                        1.0,
                        Default::default(),
                        None,
                        Default::default(),
                    )
                });
            } else {
                scope.draw_circle_with(
                    modulated_color,
                    radius,
                    center,
                    1.0,
                    Default::default(),
                    None,
                    Default::default(),
                );
            }
        }
    }
}

/// How a state layer comes in (Compose's `incomingStateLayerAnimationSpecFor`).
fn incoming_state_layer_animation_spec_for(interaction: StateInteraction) -> Arc<dyn AnimationSpec<f32>> {
    match interaction {
        StateInteraction::Hover(_) => default_tween_spec(),
        StateInteraction::Focus(_) | StateInteraction::Drag(_) => Arc::new(TweenSpec::new(45, 0, LinearEasing)),
    }
}

/// How a state layer goes (Compose's `outgoingStateLayerAnimationSpecFor`).
fn outgoing_state_layer_animation_spec_for(interaction: Option<StateInteraction>) -> Arc<dyn AnimationSpec<f32>> {
    match interaction {
        Some(StateInteraction::Drag(_)) => Arc::new(TweenSpec::new(150, 0, LinearEasing)),
        _ => default_tween_spec(),
    }
}

/// Compose's `DefaultTweenSpec`.
fn default_tween_spec() -> Arc<dyn AnimationSpec<f32>> {
    Arc::new(TweenSpec::new(15, 0, LinearEasing))
}
