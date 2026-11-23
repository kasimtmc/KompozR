//! One content fading into another as a state changes (Compose's Crossfade.kt): [`Crossfade`] and
//! `Transition::Crossfade`. Its layouts are Box's (one over the other at the top start: KompozR's animation crate
//! doesn't depend on foundation, so the Box here is a private layout of Box's policy).

use crate::animated_content::{ContentKey, ContentKeys};
use crate::spec::FiniteAnimationSpec;
use crate::transition::{Transition, update_transition};
use crate::{AnimationValue, tween};
use kompoz_graphics::IntSize;
use kompoz_runtime::{SnapshotStateList, group, kompozable, mutable_confined_state_list_of, named, remember};
use kompoz_ui::layout::{Measurable, MeasurePolicy, MeasureResult, MeasureScope, PlacementScope, SlotKey, layout_into};
use kompoz_ui::{Alignment, Constraints, Modifier};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

fn default_crossfade_spec() -> Arc<dyn FiniteAnimationSpec<f32>> {
    Arc::new(tween::<f32>())
}

/// Shows `content` of `target_state`, crossfading to the new one's when it changes (Compose's `Crossfade`): the old
/// fades out as the new fades in along `animation_spec` (a tween of 300 ms by default); `label` names its
/// transition for tools.
///
/// Kotlin: `Crossfade(targetState: T, modifier: Modifier = Modifier, animationSpec: FiniteAnimationSpec<Float> = tween(), label: String = "Crossfade", content: @Composable (T) -> Unit)`
#[kompozable]
pub fn Crossfade<T: AnimationValue>(
    target_state: T,
    #[default(Modifier)] modifier: Modifier,
    #[default(default_crossfade_spec())] animation_spec: Arc<dyn FiniteAnimationSpec<f32>>,
    #[default("Crossfade".to_string())] label: String,
    content: impl Fn(T) + 'static,
) {
    let transition = update_transition(target_state, Some(label));
    crossfade_impl(&transition, modifier, animation_spec, None, Rc::new(content));
}

#[named]
impl<S: AnimationValue> Transition<S> {
    /// Crossfades between the contents of this transition's states (Compose's experimental
    /// `Transition<T>.Crossfade`): each content's alpha animates along it; `content_key` tells contents apart (by
    /// default the states). Composable.
    ///
    /// Kotlin: `Crossfade(modifier: Modifier = Modifier, animationSpec: FiniteAnimationSpec<Float> = tween(), contentKey: (targetState: T) -> Any? = { it }, content: @Composable (targetState: T) -> Unit)`
    #[track_caller]
    pub fn Crossfade(
        &self,
        #[default(Modifier)] modifier: Modifier,
        #[default(default_crossfade_spec())] animation_spec: Arc<dyn FiniteAnimationSpec<f32>>,
        #[default(None)] content_key: Option<Rc<dyn Fn(&S) -> SlotKey>>,
        content: impl Fn(S) + 'static,
    ) {
        kompoz_runtime::kompozable_block!({
            TransitionCrossfade(self.clone(), modifier, animation_spec, content_key, content);
        });
    }
}

/// The restartable body of `Transition::Crossfade` (a scope of its own, as Compose's composable extension functions
/// are).
#[kompozable]
fn TransitionCrossfade<S: AnimationValue>(
    transition: Transition<S>,
    modifier: Modifier,
    animation_spec: Arc<dyn FiniteAnimationSpec<f32>>,
    content_key: ContentKey<S>,
    content: impl Fn(S) + 'static,
) {
    crossfade_impl(&transition, modifier, animation_spec, content_key, Rc::new(content));
}

/// Compose's `Transition<T>.Crossfade` body.
#[track_caller]
fn crossfade_impl<S: AnimationValue>(
    transition: &Transition<S>,
    modifier: Modifier,
    animation_spec: Arc<dyn FiniteAnimationSpec<f32>>,
    content_key: ContentKey<S>,
    content: Rc<dyn Fn(S)>,
) {
    let this = transition.clone();
    group(move || {
        let current = this.current_state();
        let c = current.clone();
        let currently_visible: SnapshotStateList<S> = remember(move || mutable_confined_state_list_of([c]));
        let content_map = remember(|| Rc::new(RefCell::new(Vec::<S>::new())));
        let keys = remember(|| Rc::new(RefCell::new(ContentKeys::<S>::new())));
        keys.borrow_mut().set_key(content_key);
        let target = this.target_state();
        if current == target {
            // (at rest: only the current one; the others go)
            if currently_visible.len() != 1 || currently_visible.get(0).as_ref() != Some(&target) {
                currently_visible.retain(|s| *s == target);
                content_map.borrow_mut().clear();
            }
        }
        if !content_map.borrow().contains(&target) {
            // (one of the same key is replaced)
            let key = keys.borrow_mut().key_of(&target);
            let list = currently_visible.to_vec();
            match list.iter().position(|s| keys.borrow_mut().key_of(s) == key) {
                None => currently_visible.add(target.clone()),
                Some(id) => currently_visible.set(id, target.clone()),
            }
            *content_map.borrow_mut() = currently_visible.to_vec();
        }
        {
            let list = currently_visible.to_vec();
            keys.borrow_mut().retain(|s| list.contains(s) || *s == target);
        }
        let policy = remember(|| Rc::new(CrossfadeBoxPolicy) as Rc<dyn MeasurePolicy>);
        let t = this.clone();
        layout_into(modifier, policy.clone(), false, move || {
            for state_for_content in currently_visible.to_vec() {
                let key = keys.borrow_mut().key_of(&state_for_content);
                kompoz_runtime::key(&key, || {
                    if !content_map.borrow().contains(&state_for_content) {
                        return;
                    }
                    let (spec, s) = (animation_spec.clone(), state_for_content.clone());
                    let alpha = kompoz_runtime::named_args!(t.animate_float(
                        transition_spec = move |_: &crate::Segment<S>| spec.clone(),
                        target_value_by_state = move |it: &S| if *it == s { 1.0 } else { 0.0 }
                    ));
                    let content = content.clone();
                    let state = state_for_content.clone();
                    layout_into(
                        Modifier.graphics_layer_block(move |layer| layer.alpha = alpha.value()),
                        policy.clone(),
                        false,
                        move || content(state),
                    );
                });
            }
        });
    });
}

/// Box's policy without its children's own alignments (Compose's `Box` at the top start; its BoxScope isn't the
/// content's here): one over the other at the top start, as big as the biggest and at least the min constraints.
pub(crate) struct CrossfadeBoxPolicy;

impl MeasurePolicy for CrossfadeBoxPolicy {
    fn measure(
        &self,
        scope: &MeasureScope,
        measurables: &[&dyn Measurable],
        constraints: Constraints,
    ) -> MeasureResult {
        if measurables.is_empty() {
            return scope.layout(constraints.min_width, constraints.min_height, |_| {});
        }
        let content_constraints = constraints.copy_max_dimensions();
        let mut width = constraints.min_width;
        let mut height = constraints.min_height;
        let placeables: Vec<_> = measurables
            .iter()
            .map(|m| {
                let p = m.measure(content_constraints);
                width = width.max(p.width());
                height = height.max(p.height());
                p
            })
            .collect();
        let layout_direction = scope.layout_direction();
        scope.layout(width, height, move |p: &PlacementScope| {
            for placeable in &placeables {
                let at = Alignment::TopStart.align(
                    IntSize::new(placeable.width(), placeable.height()),
                    IntSize::new(width, height),
                    layout_direction,
                );
                p.place_at(placeable, at, 0.0);
            }
        })
    }
}
