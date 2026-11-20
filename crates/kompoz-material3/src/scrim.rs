//! Material's Scrim, as Compose's Scrim.kt: a color over the content behind a modal surface (a sheet, a drawer),
//! which a tap on dismisses.

use crate::tokens::ScrimTokens;
use kompoz_foundation::prelude::*;
use kompoz_graphics::Color;
use kompoz_runtime::{kompozable, named_args};
use kompoz_ui::Modifier;
use kompoz_ui::input::PointerInputScope;
use std::rc::Rc;

/// A scrim over all the room it has (Compose's `Scrim`), in `color` faded by `alpha()` (kept within 0..1; nothing
/// drawn for an unspecified color); `on_click` called on a tap and from accessibility's click (described as
/// `content_description`), none: it takes no clicks.
#[kompozable]
pub fn Scrim(
    content_description: Option<String>,
    #[default(Modifier)] modifier: Modifier,
    #[default(Some(Rc::new(|| {})))] on_click: Option<Rc<dyn Fn()>>,
    #[default(Rc::new(|| 1.0))] alpha: Rc<dyn Fn() -> f32>,
    #[default(ScrimDefaults::color())] color: Color,
) {
    if color.is_specified() {
        let dismiss_modifier = match on_click {
            Some(on_click) => {
                let (tap, action) = (on_click.clone(), on_click.clone());
                Modifier
                    .pointer_input(OnClickKey(on_click), move |scope: PointerInputScope| {
                        let tap = tap.clone();
                        async move {
                            scope.detect_tap_gestures().on_tap(move |_| tap()).await;
                        }
                    })
                    .semantics_with(true, move |c| {
                        c.set_traversal_index(1.0);
                        if let Some(description) = &content_description {
                            c.set_content_description(description.clone());
                        }
                        let action = action.clone();
                        named_args!(c.on_click(move || {
                            action();
                            true
                        }));
                    })
            }
            None => Modifier,
        };
        Canvas(modifier.fill_max_size().then(dismiss_modifier), move |d| {
            named_args!(d.draw_rect(color, alpha = alpha().clamp(0.0, 1.0)));
        });
    }
}

/// `pointerInput(onClick)`'s key: the callback, by its instance.
#[derive(Clone)]
struct OnClickKey(Rc<dyn Fn()>);

impl PartialEq for OnClickKey {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

/// The scrim's defaults (Compose's ScrimDefaults).
pub struct ScrimDefaults;

impl ScrimDefaults {
    /// The scheme's scrim color at 32% (Compose's `ScrimDefaults.color`).
    pub fn color() -> Color {
        named_args!(ScrimTokens::ContainerColor.value().copy(alpha = ScrimTokens::ContainerOpacity))
    }
}
