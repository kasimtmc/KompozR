//! `Modifier.animate_content_size`, as Compose's `AnimationModifier.kt`: a layout that animates its own size to its
//! content's as the content's changes, clipped to it; in a lookahead scope, to its content's lookahead size, its
//! content measured at the lookahead constraints all along.

use crate::{Animatable, AnimationEndReason, AnimationSpec, AnimationVector2D, FiniteAnimationSpec, Spring};
use crate::{VectorConverter, VisibilityThreshold, spring};
use kompoz_graphics::IntSize;
use kompoz_runtime::{named, named_args};
use kompoz_ui::layout::{IntrinsicMeasurable, IntrinsicMeasureScope, LayoutModifierNode, Measurable, MeasureResult};
use kompoz_ui::layout::{MeasureScope, PlacementScope};
use kompoz_ui::node_capabilities;
use kompoz_ui::{Alignment, Constraints, Modifier, ModifierNode, ModifierNodeElement, NodeCx, NodeHandle};
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

/// What `animate_content_size` tells when an animation of its size ends (Compose's `finishedListener`): the size it
/// started from (where it was when interrupted, if it was) and the one it reached.
pub type SizeAnimationFinishedListener = Rc<dyn Fn(IntSize, IntSize)>;

/// The spec `animate_content_size` animates by unless given one (Compose's default): a spring, medium-low stiff,
/// there within a pixel.
fn default_size_spec() -> Arc<dyn FiniteAnimationSpec<IntSize>> {
    Arc::new(named_args!(spring(
        stiffness = Spring::StiffnessMediumLow,
        visibility_threshold = Some(IntSize::VisibilityThreshold)
    )))
}

/// The modifiers animating a layout's size (Compose's `Modifier.animateContentSize` overloads).
#[named(trait ContentSizeModifiers)]
impl Modifier {
    /// Animates its own size to its content's whenever the content's changes (the modifier after it, or the
    /// layout's own content at the end of the chain), so its parent sees the size change gradually (Compose's
    /// `animateContentSize(animationSpec, alignment, finishedListener)`): along `animation_spec`, the content placed
    /// by `alignment` in the size along the way and clipped to it; `finished_listener` told the start and the end
    /// sizes of each animation that ends (not of one interrupted: the next starts from where it was). Its first
    /// size isn't animated. In a [`LookaheadScope`](kompoz_ui::LookaheadScope), it animates to its content's
    /// lookahead size, the content measured with the lookahead constraints throughout.
    ///
    /// Kotlin: `animateContentSize(animationSpec: FiniteAnimationSpec<IntSize> = spring(stiffness = Spring.StiffnessMediumLow, visibilityThreshold = IntSize.VisibilityThreshold), alignment: Alignment = Alignment.TopStart, finishedListener: ((initialValue: IntSize, targetValue: IntSize) -> Unit)? = null)`
    fn animate_content_size(
        self,
        #[default(default_size_spec())] animation_spec: Arc<dyn FiniteAnimationSpec<IntSize>>,
        #[default(Alignment::TopStart)] alignment: Alignment,
        #[default(None)] finished_listener: Option<SizeAnimationFinishedListener>,
    ) -> Modifier {
        self.clip_to_bounds().then_element(SizeAnimationModifierElement {
            animation_spec,
            alignment,
            finished_listener,
        })
    }

    /// The same at the top start (Compose's `animateContentSize(animationSpec, finishedListener)`, the overload
    /// without an alignment).
    ///
    /// Kotlin: `animateContentSize(animationSpec: FiniteAnimationSpec<IntSize> = spring(stiffness = Spring.StiffnessMediumLow, visibilityThreshold = IntSize.VisibilityThreshold), finishedListener: ((initialValue: IntSize, targetValue: IntSize) -> Unit)? = null)`
    fn animate_content_size_top_start(
        self,
        #[default(default_size_spec())] animation_spec: Arc<dyn FiniteAnimationSpec<IntSize>>,
        #[default(None)] finished_listener: Option<SizeAnimationFinishedListener>,
    ) -> Modifier {
        self.clip_to_bounds().then_element(SizeAnimationModifierElement {
            animation_spec,
            alignment: Alignment::TopStart,
            finished_listener,
        })
    }
}

/// `animate_content_size`'s element (Compose's SizeAnimationModifierElement): equal with an equal spec and alignment
/// and the same listener.
struct SizeAnimationModifierElement {
    animation_spec: Arc<dyn FiniteAnimationSpec<IntSize>>,
    alignment: Alignment,
    finished_listener: Option<SizeAnimationFinishedListener>,
}

impl PartialEq for SizeAnimationModifierElement {
    fn eq(&self, other: &Self) -> bool {
        *self.animation_spec == *other.animation_spec
            && self.alignment == other.alignment
            && match (&self.finished_listener, &other.finished_listener) {
                (None, None) => true,
                (Some(a), Some(b)) => Rc::ptr_eq(a, b),
                _ => false,
            }
    }
}

impl ModifierNodeElement for SizeAnimationModifierElement {
    type Node = SizeAnimationModifierNode;
    fn create(&self) -> SizeAnimationModifierNode {
        SizeAnimationModifierNode {
            animation_spec: self.animation_spec.clone(),
            alignment: self.alignment.clone(),
            listener: Rc::new(RefCell::new(self.finished_listener.clone())),
            lookahead_size: None,
            lookahead_constraints: None,
            anim_data: None,
            handle: None,
        }
    }
    fn update(&self, node: &mut SizeAnimationModifierNode) {
        node.animation_spec = self.animation_spec.clone();
        *node.listener.borrow_mut() = self.finished_listener.clone();
        node.alignment = self.alignment.clone();
    }
    fn name(&self) -> &'static str {
        "animateContentSize"
    }
}

/// The size animation (Compose's AnimData): its Animatable and the size the running animation started from.
struct AnimData {
    anim: Animatable<IntSize, AnimationVector2D>,
    start_size: Rc<Cell<IntSize>>,
}

/// `animate_content_size`'s node (Compose's SizeAnimationModifierNode): measures what it wraps and animates the size
/// it reports to that size (its lookahead size in a lookahead scope).
struct SizeAnimationModifierNode {
    animation_spec: Arc<dyn FiniteAnimationSpec<IntSize>>,
    alignment: Alignment,
    /// The listener (the node's latest: an animation ending tells the one there is then).
    listener: Rc<RefCell<Option<SizeAnimationFinishedListener>>>,
    /// What it measured to in the lookahead pass (Compose's `lookaheadSize`, InvalidSize as none).
    lookahead_size: Option<IntSize>,
    /// The constraints of the lookahead pass (Compose's `lookaheadConstraints`, with `lookaheadConstraintsAvailable`).
    lookahead_constraints: Option<Constraints>,
    /// The animation, made with the first size (Compose's `animData`, a state its measure reads: here its measure
    /// reads the Animatable's value, and a reset node is measured again anyway).
    anim_data: Option<AnimData>,
    handle: Option<NodeHandle>,
}

impl ModifierNode for SizeAnimationModifierNode {
    node_capabilities!(layout);

    fn on_attach(&mut self, cx: &NodeCx) {
        self.handle = Some(cx.handle());
        // (attached again, it may be in a tree without a lookahead scope)
        self.lookahead_size = None;
        self.lookahead_constraints = None;
    }

    fn on_reset(&mut self) {
        // (reset: the node may be reused, its animation is stale)
        self.anim_data = None;
    }
}

impl SizeAnimationModifierNode {
    /// The constraints the content is measured with outside the lookahead pass: the lookahead's when there was one
    /// (Compose's `targetConstraints`), so it isn't measured again all along the animation.
    fn target_constraints(&self, default: Constraints) -> Constraints {
        self.lookahead_constraints.unwrap_or(default)
    }

    /// Its size now, on its way to `target_size` (Compose's `animateTo`): the first one as it is; a new target (or one
    /// it was stopped short of) animated to from where it is.
    fn animate_to(&mut self, target_size: IntSize) -> IntSize {
        let Some(data) = &self.anim_data else {
            let anim = Animatable::new(target_size, IntSize::VectorConverter, Some(IntSize::new(1, 1)), "Animatable");
            let value = anim.value();
            self.anim_data = Some(AnimData { anim, start_size: Rc::new(Cell::new(target_size)) });
            return value;
        };
        let anim = &data.anim;
        let was_interrupted = target_size != anim.value() && !anim.is_running();
        if target_size != anim.target_value() || was_interrupted {
            data.start_size.set(anim.value());
            if let Some(scope) = self.handle.as_ref().and_then(|h| h.coroutine_scope()) {
                let (anim, start, listener) = (anim.clone(), data.start_size.clone(), self.listener.clone());
                let spec: Arc<dyn AnimationSpec<IntSize>> = self.animation_spec.clone();
                scope.launch(async move {
                    let result = named_args!(anim.animate_to(target_size, animation_spec = spec)).await;
                    if result.end_reason == AnimationEndReason::Finished {
                        let listener = listener.borrow().clone();
                        if let Some(l) = listener {
                            l(start.get(), result.end_state.value());
                        }
                    }
                });
            }
        }
        data.anim.value()
    }
}

impl LayoutModifierNode for SizeAnimationModifierNode {
    fn measure(
        &mut self,
        scope: &MeasureScope,
        measurable: &dyn Measurable,
        constraints: Constraints,
    ) -> MeasureResult {
        let looking_ahead = scope.is_looking_ahead();
        let placeable = if looking_ahead {
            self.lookahead_constraints = Some(constraints);
            measurable.measure(constraints)
        } else {
            // (measured with the lookahead constraints when there are some: not again all along the animation)
            measurable.measure(self.target_constraints(constraints))
        };
        let measured_size = IntSize::new(placeable.width(), placeable.height());
        let size = if looking_ahead {
            self.lookahead_size = Some(measured_size);
            measured_size
        } else {
            let target = self.lookahead_size.unwrap_or(measured_size);
            // (within the constraints: its parent doesn't center it)
            constraints.constrain_size(self.animate_to(target))
        };
        let alignment = self.alignment.clone();
        let layout_direction = scope.layout_direction();
        scope.layout(size.width, size.height, move |p: &PlacementScope| {
            let offset = alignment.align(measured_size, size, layout_direction);
            p.place(&placeable, offset.x, offset.y);
        })
    }

    // (Compose's LayoutModifierNodeWithPassThroughIntrinsics: what it wraps answers)
    fn min_intrinsic_width(&mut self, _: &IntrinsicMeasureScope, m: &dyn IntrinsicMeasurable, height: i32) -> i32 {
        m.min_intrinsic_width(height)
    }
    fn min_intrinsic_height(&mut self, _: &IntrinsicMeasureScope, m: &dyn IntrinsicMeasurable, width: i32) -> i32 {
        m.min_intrinsic_height(width)
    }
    fn max_intrinsic_width(&mut self, _: &IntrinsicMeasureScope, m: &dyn IntrinsicMeasurable, height: i32) -> i32 {
        m.max_intrinsic_width(height)
    }
    fn max_intrinsic_height(&mut self, _: &IntrinsicMeasureScope, m: &dyn IntrinsicMeasurable, width: i32) -> i32 {
        m.max_intrinsic_height(width)
    }
}
