//! Material's checkboxes, as Compose's Checkbox.kt: a box that is checked or not ([`Checkbox`]) or, standing for a
//! group, partly checked too ([`TriStateCheckbox`]) — an outline when unchecked, filled with a check mark drawn
//! along it when checked, the check bent into a dash when partly checked —, its colors easing from one state to
//! the next, the ripple in a 40 dp circle, at least 48 dp of room to touch it when it takes clicks itself.
//!
//! Compose's overloads taking the strokes are composables of their own here: [`StrokedCheckbox`] (Kotlin's
//! `Checkbox(checked, onCheckedChange, checkmarkStroke, outlineStroke, …)`) and [`StrokedTriStateCheckbox`].
//! [`ComposeMaterial3Flags::is_checkbox_styling_fix_enabled`] switches Compose 1.5's corrected styling on (off by
//! default, as Compose's).

use crate::color_scheme::ColorScheme;
use crate::flags::ComposeMaterial3Flags;
use crate::interactive_component_size::InteractiveComponentSizeModifiers;
use crate::material_theme::MaterialTheme;
use crate::ripples::ripple;
use crate::tokens::{CheckboxTokens, MotionSchemeKeyTokens};
use kompoz_animation::{
    AnimationSpec, FiniteAnimationSpec, Segment, SnapSpec, animate_color_as_state, update_transition,
};
use kompoz_foundation::prelude::*;
use kompoz_graphics::{Color, CornerRadius, Dp, Offset, Path, PathMeasure, Size, Stroke, StrokeCap};
use kompoz_runtime::{State, kompozable, kompozable_block, named, named_args, remember, remember_updated_state};
use kompoz_ui::draw::{DrawScope, DrawStyle, Fill};
use kompoz_ui::platform::LocalDensity;
use kompoz_ui::semantics::{Role, ToggleableState};
use kompoz_ui::{Alignment, Modifier};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

/// A checkbox (Compose's `Checkbox`): checked or not by `checked`, `on_checked_change` called with the other value
/// on a click (none: it takes no clicks — a row around it that is toggleable does, the checkbox only showing the
/// state —, and doesn't keep 48 dp of room). Not `enabled`: faded, taking no clicks. Its colors are `colors`'
/// ([`CheckboxDefaults::colors`]); its interactions are emitted to `interaction_source` (one of its own where
/// none). Its strokes are [`CheckboxDefaults::StrokeWidth`] wide in whole pixels, the check mark's ends square.
#[kompozable]
pub fn Checkbox(
    checked: bool,
    on_checked_change: Option<Rc<dyn Fn(bool)>>,
    #[default(Modifier)] modifier: Modifier,
    #[default(true)] enabled: bool,
    #[default(CheckboxDefaults.colors())] colors: CheckboxColors,
    #[default(None)] interaction_source: Option<MutableInteractionSource>,
) {
    let stroke_width_px = LocalDensity.current().to_px(CheckboxDefaults::StrokeWidth).floor();
    StrokedTriStateCheckbox(
        ToggleableState::of(checked),
        on_checked_change.map(|on_checked_change| Rc::new(move || on_checked_change(!checked)) as Rc<dyn Fn()>),
        Stroke(width = stroke_width_px, cap = StrokeCap::Square),
        Stroke(width = stroke_width_px),
        modifier = modifier,
        enabled = enabled,
        colors = colors,
        interaction_source = interaction_source,
    );
}

/// A checkbox drawn with strokes of its own (Compose's `Checkbox(checked, onCheckedChange, checkmarkStroke,
/// outlineStroke, …)`): the check mark stroked by `checkmark_stroke`, the outline by `outline_stroke` (Compose's
/// sample: round ends and joins). The rest is [`Checkbox`]'s.
///
/// Kotlin: `Checkbox(checked: Boolean, onCheckedChange: ((Boolean) -> Unit)?, checkmarkStroke: Stroke, outlineStroke: Stroke)`
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn StrokedCheckbox(
    checked: bool,
    on_checked_change: Option<Rc<dyn Fn(bool)>>,
    checkmark_stroke: Stroke,
    outline_stroke: Stroke,
    #[default(Modifier)] modifier: Modifier,
    #[default(true)] enabled: bool,
    #[default(CheckboxDefaults.colors())] colors: CheckboxColors,
    #[default(None)] interaction_source: Option<MutableInteractionSource>,
) {
    StrokedTriStateCheckbox(
        ToggleableState::of(checked),
        on_checked_change.map(|on_checked_change| Rc::new(move || on_checked_change(!checked)) as Rc<dyn Fn()>),
        checkmark_stroke,
        outline_stroke,
        modifier = modifier,
        enabled = enabled,
        colors = colors,
        interaction_source = interaction_source,
    );
}

/// A checkbox of three states (Compose's `TriStateCheckbox`), for a group of checkboxes: on (all of them), off (none)
/// or [`ToggleableState::Indeterminate`] (some: a dash), `on_click` called on a click — deciding the next state —
/// (none: it takes no clicks). The rest is [`Checkbox`]'s.
#[kompozable]
pub fn TriStateCheckbox(
    state: ToggleableState,
    on_click: Option<Rc<dyn Fn()>>,
    #[default(Modifier)] modifier: Modifier,
    #[default(true)] enabled: bool,
    #[default(CheckboxDefaults.colors())] colors: CheckboxColors,
    #[default(None)] interaction_source: Option<MutableInteractionSource>,
) {
    let stroke_width_px = LocalDensity.current().to_px(CheckboxDefaults::StrokeWidth).floor();
    StrokedTriStateCheckbox(
        state,
        on_click,
        Stroke(width = stroke_width_px, cap = StrokeCap::Square),
        Stroke(width = stroke_width_px),
        modifier = modifier,
        enabled = enabled,
        colors = colors,
        interaction_source = interaction_source,
    );
}

/// A checkbox of three states drawn with strokes of its own (Compose's `TriStateCheckbox(state, onClick,
/// checkmarkStroke, outlineStroke, …)`): see [`TriStateCheckbox`] and [`StrokedCheckbox`]. With the styling fix
/// ([`ComposeMaterial3Flags::is_checkbox_styling_fix_enabled`]) its ripple takes the box's color and it is 18 dp
/// with no padding (else 20 dp in 2 dp of padding).
///
/// Kotlin: `TriStateCheckbox(state: ToggleableState, onClick: (() -> Unit)?, checkmarkStroke: Stroke, outlineStroke: Stroke)`
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn StrokedTriStateCheckbox(
    state: ToggleableState,
    on_click: Option<Rc<dyn Fn()>>,
    checkmark_stroke: Stroke,
    outline_stroke: Stroke,
    #[default(Modifier)] modifier: Modifier,
    #[default(true)] enabled: bool,
    #[default(CheckboxDefaults.colors())] colors: CheckboxColors,
    #[default(None)] interaction_source: Option<MutableInteractionSource>,
) {
    let is_checkbox_styling_fix_enabled = ComposeMaterial3Flags::is_checkbox_styling_fix_enabled();
    let indication = if is_checkbox_styling_fix_enabled {
        ripple(bounded = false, radius = CheckboxTokens::StateLayerSize / 2.0, color = colors.indicator_color(state))
    } else {
        ripple(bounded = false, radius = CheckboxTokens::StateLayerSize / 2.0)
    };
    let clickable = on_click.is_some();
    let toggleable_modifier = match on_click {
        Some(on_click) => Modifier.tri_state_toggleable_indication_with(
            state,
            interaction_source,
            Some(indication),
            enabled,
            Some(Role::Checkbox),
            move || on_click(),
        ),
        None => Modifier,
    };
    CheckboxImpl(
        enabled,
        state,
        modifier
            .then(if clickable { Modifier.minimum_interactive_component_size() } else { Modifier })
            .then(toggleable_modifier)
            .then(if is_checkbox_styling_fix_enabled { Modifier } else { Modifier.padding(CheckboxDefaultPadding) }),
        colors,
        checkmark_stroke,
        outline_stroke,
    );
}

/// The checkboxes' defaults (Compose's CheckboxDefaults).
pub struct CheckboxDefaults;

#[named]
#[allow(non_snake_case)]
impl CheckboxDefaults {
    /// The colors of a checkbox (Compose's `CheckboxDefaults.colors()` and `colors(checkedColor, …)`): the scheme's
    /// (the primary box and border checked, the check mark in the primary's content color, the variant of the
    /// surface's content color the outline unchecked; the surface's content color at 38% disabled), each given
    /// one in its place: `checked_color` the box and border checked, `unchecked_color` the outline unchecked,
    /// `checkmark_color` the check mark (disabled too), the disabled ones theirs (the box unchecked transparent).
    pub fn colors(
        &self,
        #[default(Color::Unspecified)] checked_color: Color,
        #[default(Color::Unspecified)] unchecked_color: Color,
        #[default(Color::Unspecified)] checkmark_color: Color,
        #[default(Color::Unspecified)] disabled_checked_color: Color,
        #[default(Color::Unspecified)] disabled_unchecked_color: Color,
        #[default(Color::Unspecified)] disabled_indeterminate_color: Color,
    ) -> CheckboxColors {
        named_args!(default_checkbox_colors(&MaterialTheme::color_scheme()).copy(
            checked_checkmark_color = checkmark_color,
            unchecked_checkmark_color = Color::Transparent,
            disabled_checkmark_color = checkmark_color,
            checked_box_color = checked_color,
            unchecked_box_color = Color::Transparent,
            disabled_checked_box_color = disabled_checked_color,
            disabled_unchecked_box_color = Color::Transparent,
            disabled_indeterminate_box_color = disabled_indeterminate_color,
            checked_border_color = checked_color,
            unchecked_border_color = unchecked_color,
            disabled_border_color = disabled_checked_color,
            disabled_unchecked_border_color = disabled_unchecked_color,
            disabled_indeterminate_border_color = disabled_indeterminate_color,
        ))
    }

    /// The colors of a checkbox, each part's given in its place (Compose's `CheckboxDefaults.colors(
    /// checkedCheckmarkColor, …)`): the check mark's, the box's and the border's, checked, unchecked, disabled and
    /// partly checked; the scheme's where unspecified.
    ///
    /// Kotlin: `colors(checkedCheckmarkColor: Color = Color.Unspecified, uncheckedCheckmarkColor: Color = Color.Unspecified, disabledCheckmarkColor: Color = Color.Unspecified, checkedBoxColor: Color = Color.Unspecified, uncheckedBoxColor: Color = Color.Unspecified, disabledCheckedBoxColor: Color = Color.Unspecified, disabledUncheckedBoxColor: Color = Color.Unspecified, disabledIndeterminateBoxColor: Color = Color.Unspecified, checkedBorderColor: Color = Color.Unspecified, uncheckedBorderColor: Color = Color.Unspecified, disabledBorderColor: Color = Color.Unspecified, disabledUncheckedBorderColor: Color = Color.Unspecified, disabledIndeterminateBorderColor: Color = Color.Unspecified)`
    #[allow(clippy::too_many_arguments)]
    pub fn colors_of_parts(
        &self,
        #[default(Color::Unspecified)] checked_checkmark_color: Color,
        #[default(Color::Unspecified)] unchecked_checkmark_color: Color,
        #[default(Color::Unspecified)] disabled_checkmark_color: Color,
        #[default(Color::Unspecified)] checked_box_color: Color,
        #[default(Color::Unspecified)] unchecked_box_color: Color,
        #[default(Color::Unspecified)] disabled_checked_box_color: Color,
        #[default(Color::Unspecified)] disabled_unchecked_box_color: Color,
        #[default(Color::Unspecified)] disabled_indeterminate_box_color: Color,
        #[default(Color::Unspecified)] checked_border_color: Color,
        #[default(Color::Unspecified)] unchecked_border_color: Color,
        #[default(Color::Unspecified)] disabled_border_color: Color,
        #[default(Color::Unspecified)] disabled_unchecked_border_color: Color,
        #[default(Color::Unspecified)] disabled_indeterminate_border_color: Color,
    ) -> CheckboxColors {
        default_checkbox_colors(&MaterialTheme::color_scheme()).copy_with(
            checked_checkmark_color,
            unchecked_checkmark_color,
            checked_box_color,
            unchecked_box_color,
            disabled_checked_box_color,
            disabled_unchecked_box_color,
            disabled_indeterminate_box_color,
            checked_border_color,
            unchecked_border_color,
            disabled_border_color,
            disabled_unchecked_border_color,
            disabled_indeterminate_border_color,
            disabled_checkmark_color,
        )
    }
}

impl CheckboxDefaults {
    /// The width of a checkbox's strokes (Compose's `CheckboxDefaults.StrokeWidth`): 2 dp.
    pub const StrokeWidth: Dp = Dp(2.0);
}

/// A scheme's default checkbox colors (Compose's internal `ColorScheme.defaultCheckboxColors`; its cache isn't
/// ported: the colors are plain values, made where asked).
pub(crate) fn default_checkbox_colors(scheme: &ColorScheme) -> CheckboxColors {
    let selected_disabled = named_args!(
        scheme
            .from_token(CheckboxTokens::SelectedDisabledContainerColor)
            .copy(alpha = CheckboxTokens::SelectedDisabledContainerOpacity)
    );
    CheckboxColors(
        scheme.from_token(CheckboxTokens::SelectedIconColor),
        Color::Transparent,
        scheme.from_token(CheckboxTokens::SelectedContainerColor),
        Color::Transparent,
        selected_disabled,
        Color::Transparent,
        selected_disabled,
        scheme.from_token(CheckboxTokens::SelectedContainerColor),
        scheme.from_token(CheckboxTokens::UnselectedOutlineColor),
        selected_disabled,
        named_args!(
            scheme
                .from_token(CheckboxTokens::UnselectedDisabledOutlineColor)
                .copy(alpha = CheckboxTokens::UnselectedDisabledContainerOpacity)
        ),
        selected_disabled,
        scheme.from_token(CheckboxTokens::SelectedDisabledIconColor),
    )
}

/// The box and its check mark (Compose's private `CheckboxImpl`): the check drawn along as it is checked, at once
/// gone 100 ms after it is unchecked, bent into a dash toward partly checked; the colors eased.
#[kompozable]
fn CheckboxImpl(
    enabled: bool,
    value: ToggleableState,
    modifier: Modifier,
    colors: CheckboxColors,
    checkmark_stroke: Stroke,
    outline_stroke: Stroke,
) {
    let is_checkbox_styling_fix_enabled = ComposeMaterial3Flags::is_checkbox_styling_fix_enabled();
    let transition = update_transition(value);
    let default_animation_spec = MotionSchemeKeyTokens::DefaultSpatial.value::<f32>();
    let draw_spec = default_animation_spec.clone();
    let check_draw_fraction = transition.animate_float(
        transition_spec = move |s: &Segment<ToggleableState>| -> Arc<dyn FiniteAnimationSpec<f32>> {
            // TODO (Compose's) Load the motionScheme tokens from the component tokens file
            if s.initial_state() == ToggleableState::Off {
                draw_spec.clone()
            } else if s.target_state() == ToggleableState::Off {
                Arc::new(SnapSpec::new(SnapAnimationDelay))
            } else {
                draw_spec.clone()
            }
        },
        target_value_by_state = |it: &ToggleableState| match it {
            ToggleableState::On => 1.0,
            ToggleableState::Off => 0.0,
            ToggleableState::Indeterminate => 1.0,
        },
    );
    let check_center_gravitation_shift_fraction = transition.animate_float(
        transition_spec = move |s: &Segment<ToggleableState>| -> Arc<dyn FiniteAnimationSpec<f32>> {
            // TODO (Compose's) Load the motionScheme tokens from the component tokens file
            if s.initial_state() == ToggleableState::Off {
                Arc::new(SnapSpec::new(0))
            } else if s.target_state() == ToggleableState::Off {
                Arc::new(SnapSpec::new(SnapAnimationDelay))
            } else {
                default_animation_spec.clone()
            }
        },
        target_value_by_state = |it: &ToggleableState| match it {
            ToggleableState::On => 0.0,
            ToggleableState::Off => 0.0,
            ToggleableState::Indeterminate => 1.0,
        },
    );
    let check_cache = remember(|| Rc::new(RefCell::new(CheckDrawingCache::default())));
    let check_color = if is_checkbox_styling_fix_enabled {
        colors.checkmark_color_enabled(enabled, value)
    } else {
        colors.checkmark_color(value)
    };
    let box_color = colors.box_color(enabled, value);
    let border_color = colors.border_color(enabled, value);
    let container_size = if is_checkbox_styling_fix_enabled { CheckboxTokens::ContainerSize } else { CheckboxSize };
    Canvas(modifier.wrap_content_size(Alignment::Center).required_size(container_size), move |d| {
        let radius = d.to_px(RadiusSize);
        draw_box(d, box_color.value(), border_color.value(), radius, &outline_stroke);
        draw_check(
            d,
            check_color.value(),
            check_draw_fraction.value(),
            check_center_gravitation_shift_fraction.value(),
            &checkmark_stroke,
            &mut check_cache.borrow_mut(),
        );
    });
}

/// The box: filled where its fill is its border's color, else filled inside the border and stroked along it
/// (Compose's private `DrawScope.drawBox`).
fn draw_box(d: &mut DrawScope, box_color: Color, border_color: Color, radius: f32, stroke: &Stroke) {
    let half_stroke_width = stroke.width / 2.0;
    let checkbox_size = d.size().width;
    if box_color == border_color {
        d.draw_round_rect_with(
            box_color,
            Offset::Zero,
            Size::new(checkbox_size, checkbox_size),
            CornerRadius::circular(radius),
            1.0,
            Fill,
            None,
            Default::default(),
        );
    } else {
        d.draw_round_rect_with(
            box_color,
            Offset::new(stroke.width, stroke.width),
            Size::new(checkbox_size - stroke.width * 2.0, checkbox_size - stroke.width * 2.0),
            CornerRadius::circular((radius - stroke.width).max(0.0)),
            1.0,
            Fill,
            None,
            Default::default(),
        );
        d.draw_round_rect_with(
            border_color,
            Offset::new(half_stroke_width, half_stroke_width),
            Size::new(checkbox_size - stroke.width, checkbox_size - stroke.width),
            CornerRadius::circular(radius - half_stroke_width),
            1.0,
            DrawStyle::Stroke(stroke.clone()),
            None,
            Default::default(),
        );
    }
}

/// The check mark, `check_fraction` of it drawn, gravitated toward a dash across the middle by
/// `cross_center_gravitation` (Compose's private `DrawScope.drawCheck`).
fn draw_check(
    d: &mut DrawScope,
    check_color: Color,
    check_fraction: f32,
    cross_center_gravitation: f32,
    stroke: &Stroke,
    drawing_cache: &mut CheckDrawingCache,
) {
    let is_checkbox_styling_fix_enabled = ComposeMaterial3Flags::is_checkbox_styling_fix_enabled();
    let width = d.size().width;
    let check_cross_x = 0.4;
    let check_cross_y = if is_checkbox_styling_fix_enabled { 0.65 } else { 0.7 };
    let left_x = if is_checkbox_styling_fix_enabled { 0.25 } else { 0.2 };
    let left_y = 0.5;
    let right_x = if is_checkbox_styling_fix_enabled { 0.75 } else { 0.8 };
    let right_y = 0.3;

    let gravitated_cross_x = lerp(check_cross_x, 0.5, cross_center_gravitation);
    let gravitated_cross_y = lerp(check_cross_y, 0.5, cross_center_gravitation);
    // gravitate only Y for end to achieve center line
    let gravitated_left_y = lerp(left_y, 0.5, cross_center_gravitation);
    let gravitated_right_y = lerp(right_y, 0.5, cross_center_gravitation);

    let CheckDrawingCache { check_path, path_measure, path_to_draw } = drawing_cache;
    check_path.rewind();
    check_path.move_to(width * left_x, width * gravitated_left_y);
    check_path.line_to(width * gravitated_cross_x, width * gravitated_cross_y);
    check_path.line_to(width * right_x, width * gravitated_right_y);
    path_measure.set_path(check_path, false);
    path_to_draw.rewind();
    path_measure.get_segment(0.0, path_measure.length() * check_fraction, path_to_draw, true);
    d.draw_path_with(path_to_draw, check_color, 1.0, DrawStyle::Stroke(stroke.clone()), None, Default::default());
}

/// Compose's `lerp(start, stop, fraction)` of floats.
fn lerp(start: f32, stop: f32, fraction: f32) -> f32 {
    (1.0 - fraction) * start + fraction * stop
}

/// What the check mark is drawn with, kept between draws (Compose's private `CheckDrawingCache`).
#[derive(Default)]
struct CheckDrawingCache {
    check_path: Path,
    path_measure: PathMeasure,
    path_to_draw: Path,
}

/// The colors of a checkbox in its states (Compose's CheckboxColors): the check mark's, the box's and the
/// border's, checked, unchecked, disabled and partly checked. Make them with [`CheckboxDefaults::colors`] or
/// [`CheckboxColors()`](fn@CheckboxColors); change some with [`copy`](CheckboxColors::copy).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, kompoz_runtime::KotlinProperties)]
pub struct CheckboxColors {
    pub checked_checkmark_color: Color,
    pub unchecked_checkmark_color: Color,
    pub checked_box_color: Color,
    pub unchecked_box_color: Color,
    pub disabled_checked_box_color: Color,
    pub disabled_unchecked_box_color: Color,
    pub disabled_indeterminate_box_color: Color,
    pub checked_border_color: Color,
    pub unchecked_border_color: Color,
    pub disabled_border_color: Color,
    pub disabled_unchecked_border_color: Color,
    pub disabled_indeterminate_border_color: Color,
    pub disabled_checkmark_color: Color,
}

/// A checkbox's colors, each given (Compose's `CheckboxColors(…)` constructor).
#[named(type)]
#[allow(clippy::too_many_arguments)]
pub fn CheckboxColors(
    checked_checkmark_color: Color,
    unchecked_checkmark_color: Color,
    checked_box_color: Color,
    unchecked_box_color: Color,
    disabled_checked_box_color: Color,
    disabled_unchecked_box_color: Color,
    disabled_indeterminate_box_color: Color,
    checked_border_color: Color,
    unchecked_border_color: Color,
    disabled_border_color: Color,
    disabled_unchecked_border_color: Color,
    disabled_indeterminate_border_color: Color,
    disabled_checkmark_color: Color,
) -> CheckboxColors {
    CheckboxColors {
        checked_checkmark_color,
        unchecked_checkmark_color,
        checked_box_color,
        unchecked_box_color,
        disabled_checked_box_color,
        disabled_unchecked_box_color,
        disabled_indeterminate_box_color,
        checked_border_color,
        unchecked_border_color,
        disabled_border_color,
        disabled_unchecked_border_color,
        disabled_indeterminate_border_color,
        disabled_checkmark_color,
    }
}

/// A checkbox's colors without the disabled check mark's (Compose's deprecated constructor without
/// `disabledCheckmarkColor`): the checked check mark's color is the disabled one's.
///
/// Kotlin: `CheckboxColors(checkedCheckmarkColor: Color, uncheckedCheckmarkColor: Color, checkedBoxColor: Color, uncheckedBoxColor: Color, disabledCheckedBoxColor: Color, disabledUncheckedBoxColor: Color, disabledIndeterminateBoxColor: Color, checkedBorderColor: Color, uncheckedBorderColor: Color, disabledBorderColor: Color, disabledUncheckedBorderColor: Color, disabledIndeterminateBorderColor: Color)`
#[deprecated(
    note = "This constructor is deprecated. Use the primary constructor that includes 'disabledCheckmarkColor'"
)]
#[allow(clippy::too_many_arguments)]
pub fn CheckboxColorsWithoutDisabledCheckmark(
    checked_checkmark_color: Color,
    unchecked_checkmark_color: Color,
    checked_box_color: Color,
    unchecked_box_color: Color,
    disabled_checked_box_color: Color,
    disabled_unchecked_box_color: Color,
    disabled_indeterminate_box_color: Color,
    checked_border_color: Color,
    unchecked_border_color: Color,
    disabled_border_color: Color,
    disabled_unchecked_border_color: Color,
    disabled_indeterminate_border_color: Color,
) -> CheckboxColors {
    CheckboxColors(
        checked_checkmark_color,
        unchecked_checkmark_color,
        checked_box_color,
        unchecked_box_color,
        disabled_checked_box_color,
        disabled_unchecked_box_color,
        disabled_indeterminate_box_color,
        checked_border_color,
        unchecked_border_color,
        disabled_border_color,
        disabled_unchecked_border_color,
        disabled_indeterminate_border_color,
        checked_checkmark_color,
    )
}

#[named]
impl CheckboxColors {
    /// The same with what is given replaced (Compose's `copy`): an unspecified color keeps this one's. (Compose's
    /// hidden deprecated `copy` without `disabledCheckmarkColor` can't be called from Kotlin source: not here.)
    #[allow(clippy::too_many_arguments)]
    pub fn copy(
        &self,
        #[default(self.checked_checkmark_color)] checked_checkmark_color: Color,
        #[default(self.unchecked_checkmark_color)] unchecked_checkmark_color: Color,
        #[default(self.checked_box_color)] checked_box_color: Color,
        #[default(self.unchecked_box_color)] unchecked_box_color: Color,
        #[default(self.disabled_checked_box_color)] disabled_checked_box_color: Color,
        #[default(self.disabled_unchecked_box_color)] disabled_unchecked_box_color: Color,
        #[default(self.disabled_indeterminate_box_color)] disabled_indeterminate_box_color: Color,
        #[default(self.checked_border_color)] checked_border_color: Color,
        #[default(self.unchecked_border_color)] unchecked_border_color: Color,
        #[default(self.disabled_border_color)] disabled_border_color: Color,
        #[default(self.disabled_unchecked_border_color)] disabled_unchecked_border_color: Color,
        #[default(self.disabled_indeterminate_border_color)] disabled_indeterminate_border_color: Color,
        #[default(self.disabled_checkmark_color)] disabled_checkmark_color: Color,
    ) -> CheckboxColors {
        self.copy_with(
            checked_checkmark_color,
            unchecked_checkmark_color,
            checked_box_color,
            unchecked_box_color,
            disabled_checked_box_color,
            disabled_unchecked_box_color,
            disabled_indeterminate_box_color,
            checked_border_color,
            unchecked_border_color,
            disabled_border_color,
            disabled_unchecked_border_color,
            disabled_indeterminate_border_color,
            disabled_checkmark_color,
        )
    }
}

impl CheckboxColors {
    /// [`copy`](CheckboxColors::copy) with every color given, for plain code.
    #[allow(clippy::too_many_arguments)]
    pub fn copy_with(
        &self,
        checked_checkmark_color: Color,
        unchecked_checkmark_color: Color,
        checked_box_color: Color,
        unchecked_box_color: Color,
        disabled_checked_box_color: Color,
        disabled_unchecked_box_color: Color,
        disabled_indeterminate_box_color: Color,
        checked_border_color: Color,
        unchecked_border_color: Color,
        disabled_border_color: Color,
        disabled_unchecked_border_color: Color,
        disabled_indeterminate_border_color: Color,
        disabled_checkmark_color: Color,
    ) -> CheckboxColors {
        CheckboxColors(
            checked_checkmark_color.take_or_else(|| self.checked_checkmark_color),
            unchecked_checkmark_color.take_or_else(|| self.unchecked_checkmark_color),
            checked_box_color.take_or_else(|| self.checked_box_color),
            unchecked_box_color.take_or_else(|| self.unchecked_box_color),
            disabled_checked_box_color.take_or_else(|| self.disabled_checked_box_color),
            disabled_unchecked_box_color.take_or_else(|| self.disabled_unchecked_box_color),
            disabled_indeterminate_box_color.take_or_else(|| self.disabled_indeterminate_box_color),
            checked_border_color.take_or_else(|| self.checked_border_color),
            unchecked_border_color.take_or_else(|| self.unchecked_border_color),
            disabled_border_color.take_or_else(|| self.disabled_border_color),
            disabled_unchecked_border_color.take_or_else(|| self.disabled_unchecked_border_color),
            disabled_indeterminate_border_color.take_or_else(|| self.disabled_indeterminate_border_color),
            disabled_checkmark_color.take_or_else(|| self.disabled_checkmark_color),
        )
    }

    /// The ripple's color in `state` (Compose's internal `indicatorColor`): the box's unchecked color when off,
    /// else its checked one.
    pub(crate) fn indicator_color(&self, state: ToggleableState) -> Color {
        if state == ToggleableState::Off { self.unchecked_box_color } else { self.checked_box_color }
    }

    /// The check mark's color in `state`, disabled too (Compose's internal `checkmarkColor(enabled, state)`, the
    /// styling fix's), eased.
    pub(crate) fn checkmark_color_enabled(&self, enabled: bool, state: ToggleableState) -> State<Color> {
        let target = if enabled {
            if state == ToggleableState::Off { self.unchecked_checkmark_color } else { self.checked_checkmark_color }
        } else {
            self.disabled_checkmark_color
        };
        kompozable_block!(animate_color_as_state(target, color_animation_spec_for_state(state)))
    }

    /// The check mark's color in `state` (Compose's internal `checkmarkColor(state)`): enabled or not, eased.
    pub(crate) fn checkmark_color(&self, state: ToggleableState) -> State<Color> {
        let target =
            if state == ToggleableState::Off { self.unchecked_checkmark_color } else { self.checked_checkmark_color };
        kompozable_block!(animate_color_as_state(target, color_animation_spec_for_state(state)))
    }

    /// The box's color in `state` (Compose's internal `boxColor`): eased while enabled, the disabled one at once
    /// (no animation between enabled and disabled).
    pub(crate) fn box_color(&self, enabled: bool, state: ToggleableState) -> State<Color> {
        let target = if enabled {
            match state {
                ToggleableState::On | ToggleableState::Indeterminate => self.checked_box_color,
                ToggleableState::Off => self.unchecked_box_color,
            }
        } else {
            match state {
                ToggleableState::On => self.disabled_checked_box_color,
                ToggleableState::Indeterminate => self.disabled_indeterminate_box_color,
                ToggleableState::Off => self.disabled_unchecked_box_color,
            }
        };
        // If not enabled 'snap' to the disabled state, as there should be no animations between
        // enabled / disabled.
        kompozable_block!({
            if enabled {
                animate_color_as_state(target, color_animation_spec_for_state(state))
            } else {
                remember_updated_state!(target)
            }
        })
    }

    /// The border's color in `state` (Compose's internal `borderColor`): as [`box_color`](Self::box_color).
    pub(crate) fn border_color(&self, enabled: bool, state: ToggleableState) -> State<Color> {
        let target = if enabled {
            match state {
                ToggleableState::On | ToggleableState::Indeterminate => self.checked_border_color,
                ToggleableState::Off => self.unchecked_border_color,
            }
        } else {
            match state {
                ToggleableState::Indeterminate => self.disabled_indeterminate_border_color,
                ToggleableState::On => self.disabled_border_color,
                ToggleableState::Off => self.disabled_unchecked_border_color,
            }
        };
        // If not enabled 'snap' to the disabled state, as there should be no animations between
        // enabled / disabled.
        kompozable_block!({
            if enabled {
                animate_color_as_state(target, color_animation_spec_for_state(state))
            } else {
                remember_updated_state!(target)
            }
        })
    }
}

/// How a color eases toward `state` (Compose's private `colorAnimationSpecForState`): the box out (off) by the
/// fast effects, in by the default effects.
fn color_animation_spec_for_state(state: ToggleableState) -> Arc<dyn AnimationSpec<Color>> {
    // TODO (Compose's) Load the motionScheme tokens from the component tokens file
    if state == ToggleableState::Off {
        // Box out
        MotionSchemeKeyTokens::FastEffects.value::<Color>()
    } else {
        // Box in
        MotionSchemeKeyTokens::DefaultEffects.value::<Color>()
    }
}

const SnapAnimationDelay: i32 = 100;

// TODO (Compose's, b/188529841) Update the padding and size when the Checkbox spec is finalized.
const CheckboxDefaultPadding: Dp = Dp(2.0);
const CheckboxSize: Dp = Dp(20.0);
const RadiusSize: Dp = Dp(2.0);
