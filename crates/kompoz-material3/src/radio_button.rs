//! Material's radio buttons, as Compose's RadioButton.kt: a ring, a dot growing in it when selected, its color
//! easing between the states, the ripple in a 40 dp circle, at least 48 dp of room to touch it when it takes clicks
//! itself. Put a group's in a `selectable_group`.

use crate::color_scheme::ColorScheme;
use crate::interactive_component_size::InteractiveComponentSizeModifiers;
use crate::material_theme::MaterialTheme;
use crate::ripples::ripple;
use crate::tokens::{MotionSchemeKeyTokens, RadioButtonTokens};
use kompoz_animation::{animate_color_as_state, animate_dp_as_state};
use kompoz_foundation::prelude::*;
use kompoz_graphics::{Color, Dp, Stroke};
use kompoz_runtime::{State, kompozable, kompozable_block, named, named_args, remember_updated_state};
use kompoz_ui::draw::{DrawStyle, Fill};
use kompoz_ui::semantics::Role;
use kompoz_ui::{Alignment, Modifier};
use std::rc::Rc;

/// A radio button (Compose's `RadioButton`): `selected` or not, `on_click` called on a click (none: it takes no
/// clicks — a selectable row around it does, the button only showing the state —, and doesn't keep 48 dp of room).
/// Not `enabled`: faded, taking no clicks. Its colors are `colors`' ([`RadioButtonDefaults::colors`]); its
/// interactions are emitted to `interaction_source` (one of its own where none).
#[kompozable]
pub fn RadioButton(
    selected: bool,
    on_click: Option<Rc<dyn Fn()>>,
    #[default(Modifier)] modifier: Modifier,
    #[default(true)] enabled: bool,
    #[default(RadioButtonDefaults.colors())] colors: RadioButtonColors,
    #[default(None)] interaction_source: Option<MutableInteractionSource>,
) {
    let dot_radius = animate_dp_as_state(
        if selected { RadioButtonDotSize / 2.0 } else { Dp(0.0) },
        // TODO (Compose's) Load the motionScheme tokens from the component tokens file
        animation_spec = MotionSchemeKeyTokens::FastSpatial.value::<Dp>(),
    );
    let radio_color = colors.radio_color(enabled, selected);
    let clickable = on_click.is_some();
    let selectable_modifier = match on_click {
        Some(on_click) => Modifier.selectable_indication_with(
            selected,
            interaction_source,
            Some(ripple(bounded = false, radius = RadioButtonTokens::StateLayerSize / 2.0)),
            enabled,
            Some(Role::RadioButton),
            move || on_click(),
        ),
        None => Modifier,
    };
    Canvas(
        modifier
            .then(if clickable { Modifier.minimum_interactive_component_size() } else { Modifier })
            .then(selectable_modifier)
            .wrap_content_size(Alignment::Center)
            .padding(RadioButtonPadding)
            .required_size(RadioButtonTokens::IconSize),
        move |d| {
            // Draw the radio button
            let stroke_width = d.to_px(RadioStrokeWidth);
            let color = radio_color.value();
            let ring_radius = d.to_px(RadioButtonTokens::IconSize / 2.0) - stroke_width / 2.0;
            named_args!(d.draw_circle(
                color,
                radius = ring_radius,
                style = DrawStyle::Stroke(Stroke(width = stroke_width)),
            ));
            let dot = dot_radius.value();
            if dot > Dp(0.0) {
                let dot_px = d.to_px(dot) - stroke_width / 2.0;
                named_args!(d.draw_circle(color, radius = dot_px, style = Fill));
            }
        },
    );
}

/// The radio buttons' defaults (Compose's RadioButtonDefaults).
pub struct RadioButtonDefaults;

#[named]
impl RadioButtonDefaults {
    /// The colors of a radio button (Compose's `RadioButtonDefaults.colors()` and `colors(selectedColor, …)`): the
    /// scheme's (the primary selected, the variant of the surface's content color unselected, the surface's content
    /// color at 38% disabled), each given one in its place.
    pub fn colors(
        &self,
        #[default(Color::Unspecified)] selected_color: Color,
        #[default(Color::Unspecified)] unselected_color: Color,
        #[default(Color::Unspecified)] disabled_selected_color: Color,
        #[default(Color::Unspecified)] disabled_unselected_color: Color,
    ) -> RadioButtonColors {
        named_args!(default_radio_button_colors(&MaterialTheme::color_scheme()).copy(
            selected_color = selected_color,
            unselected_color = unselected_color,
            disabled_selected_color = disabled_selected_color,
            disabled_unselected_color = disabled_unselected_color,
        ))
    }
}

/// A scheme's default radio button colors (Compose's internal `ColorScheme.defaultRadioButtonColors`; its cache
/// isn't ported: the colors are plain values, made where asked).
pub(crate) fn default_radio_button_colors(scheme: &ColorScheme) -> RadioButtonColors {
    RadioButtonColors(
        scheme.from_token(RadioButtonTokens::SelectedIconColor),
        scheme.from_token(RadioButtonTokens::UnselectedIconColor),
        named_args!(
            scheme
                .from_token(RadioButtonTokens::DisabledSelectedIconColor)
                .copy(alpha = RadioButtonTokens::DisabledSelectedIconOpacity)
        ),
        named_args!(
            scheme
                .from_token(RadioButtonTokens::DisabledUnselectedIconColor)
                .copy(alpha = RadioButtonTokens::DisabledUnselectedIconOpacity)
        ),
    )
}

/// The colors of a radio button in its states (Compose's RadioButtonColors): selected or not, enabled or not. Make
/// them with [`RadioButtonDefaults::colors`] or [`RadioButtonColors()`](fn@RadioButtonColors); change some with
/// [`copy`](RadioButtonColors::copy).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, kompoz_runtime::KotlinProperties)]
pub struct RadioButtonColors {
    pub selected_color: Color,
    pub unselected_color: Color,
    pub disabled_selected_color: Color,
    pub disabled_unselected_color: Color,
}

/// A radio button's colors, each given (Compose's `RadioButtonColors(…)` constructor).
#[named(type)]
pub fn RadioButtonColors(
    selected_color: Color,
    unselected_color: Color,
    disabled_selected_color: Color,
    disabled_unselected_color: Color,
) -> RadioButtonColors {
    RadioButtonColors { selected_color, unselected_color, disabled_selected_color, disabled_unselected_color }
}

#[named]
impl RadioButtonColors {
    /// The same with what is given replaced (Compose's `copy`): an unspecified color keeps this one's.
    pub fn copy(
        &self,
        #[default(self.selected_color)] selected_color: Color,
        #[default(self.unselected_color)] unselected_color: Color,
        #[default(self.disabled_selected_color)] disabled_selected_color: Color,
        #[default(self.disabled_unselected_color)] disabled_unselected_color: Color,
    ) -> RadioButtonColors {
        RadioButtonColors(
            selected_color.take_or_else(|| self.selected_color),
            unselected_color.take_or_else(|| self.unselected_color),
            disabled_selected_color.take_or_else(|| self.disabled_selected_color),
            disabled_unselected_color.take_or_else(|| self.disabled_unselected_color),
        )
    }
}

impl RadioButtonColors {
    /// The ring's and the dot's color (Compose's internal `radioColor`): eased while enabled, the disabled one at
    /// once (no animation between enabled and disabled).
    pub(crate) fn radio_color(&self, enabled: bool, selected: bool) -> State<Color> {
        let target = match (enabled, selected) {
            (true, true) => self.selected_color,
            (true, false) => self.unselected_color,
            (false, true) => self.disabled_selected_color,
            (false, false) => self.disabled_unselected_color,
        };
        // If not enabled 'snap' to the disabled state, as there should be no animations between
        // enabled / disabled.
        kompozable_block!({
            if enabled {
                // TODO (Compose's) Load the motionScheme tokens from the component tokens file
                animate_color_as_state(target, MotionSchemeKeyTokens::DefaultEffects.value::<Color>())
            } else {
                remember_updated_state!(target)
            }
        })
    }
}

const RadioButtonPadding: Dp = Dp(2.0);
const RadioButtonDotSize: Dp = Dp(12.0);
const RadioStrokeWidth: Dp = Dp(2.0);
