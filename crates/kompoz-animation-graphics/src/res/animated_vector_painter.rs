//! rememberAnimatedVectorPainter (Compose's AnimatedVectorPainterResources.android.kt).

use crate::vector::AnimatedImageVector;
use crate::vector::animator::StateVectorConfig;
use kompoz_animation::update_transition;
use kompoz_graphics::VectorGroup;
use kompoz_runtime::kompozable;
use kompoz_ui::draw::vector::{VectorConfig, render_vector_group};
use kompoz_ui::draw::{VectorPainter, remember_vector_painter_content};
use std::collections::HashMap;
use std::rc::Rc;

/// A painter of `animated_image_vector` (Compose's `rememberAnimatedVectorPainter`): its vector at the start of
/// all its animations, or at their end if `at_end`; a change of `at_end` animates to the other (its animators'
/// specs played by an `update_transition` of `at_end`, backwards going back). Its vector's size, viewport, name and
/// tint; mirrored right to left (Compose's `autoMirror = true`).
#[kompozable]
pub fn remember_animated_vector_painter(animated_image_vector: AnimatedImageVector, at_end: bool) -> VectorPainter {
    remember_animated_vector_painter_with(animated_image_vector, at_end, |group, overrides| {
        render_vector_group(group, overrides)
    })
}

/// [`remember_animated_vector_painter`] drawing its vector's root group and the configs of its targets through
/// `render` (Compose's internal overload, for tests).
#[kompozable]
pub(crate) fn remember_animated_vector_painter_with(
    animated_image_vector: AnimatedImageVector,
    at_end: bool,
    render: impl Fn(&VectorGroup, &HashMap<String, Rc<dyn VectorConfig>>) + 'static,
) -> VectorPainter {
    let image_vector = animated_image_vector.image_vector();
    let render = Rc::new(render);
    let (name, root) = (image_vector.name().to_string(), image_vector.clone());
    remember_vector_painter_content(
        image_vector.default_width(),
        image_vector.default_height(),
        viewport_width = image_vector.viewport_width(),
        viewport_height = image_vector.viewport_height(),
        name = image_vector.name().to_string(),
        tint_color = image_vector.tint_color(),
        tint_blend_mode = image_vector.tint_blend_mode(),
        auto_mirror = true,
        // (composed in the vector's own composition, as Compose's: its transition's frames compose it, not the
        // caller)
        content = move |_, _| {
            let transition = update_transition(at_end, Some(name.clone()));
            let total_duration = animated_image_vector.total_duration();
            let mut map: Vec<(String, StateVectorConfig)> = Vec::new();
            for target in animated_image_vector.targets() {
                let config = target.animator.create_vector_config(&transition, total_duration);
                match map.iter_mut().find(|(name, _)| *name == target.name) {
                    Some((_, current)) => current.merge(config),
                    None => map.push((target.name.clone(), config)),
                }
            }
            let configs: HashMap<String, Rc<dyn VectorConfig>> =
                map.into_iter().map(|(name, config)| (name, Rc::new(config) as Rc<dyn VectorConfig>)).collect();
            render(root.root(), &configs);
        },
    )
}
