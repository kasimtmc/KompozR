//! Material's switch, as Compose's Switch.kt: a round track with a thumb at its start (off) or its end (on) — small
//! while off and plain, larger while on or showing content (an icon), larger still while held —, the thumb moving
//! and growing along the motion scheme's fast spatial spec, the ripple around the thumb, at least 48 dp of room to
//! touch it when it takes clicks itself.

use crate::content_color::LocalContentColor;
use crate::interactive_component_size::InteractiveComponentSizeModifiers;
use crate::material_theme::MaterialTheme;
use crate::ripples::ripple;
use crate::tokens::{MotionSchemeKeyTokens, SwitchTokens};
use kompoz_animation::{Animatable, AnimationVector1D, FiniteAnimationSpec, SnapSpec};
use kompoz_foundation::interaction::{Interaction, InteractionSource, PressInteraction};
use kompoz_foundation::prelude::*;
use kompoz_graphics::{Color, Dp};
use kompoz_runtime::{KompozitionLocalProvider, kompozable, named, named_args, remember};
use kompoz_ui::layout::LayoutModifierNode;
use kompoz_ui::semantics::Role;
use kompoz_ui::{
    Alignment, Constraints, Measurable, MeasureResult, MeasureScope, Modifier, ModifierNode, ModifierNodeElement,
    NodeCx, NodeHandle, node_capabilities,
};
use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;

/// A switch (Compose's `Switch`): on or off by `checked`, `on_checked_change` called with the other value on a
/// click (none: it takes no clicks — a toggleable row around it does, the switch only showing the state —, and
/// doesn't keep 48 dp of room). `thumb_content` is drawn in the thumb (an icon of [`SwitchDefaults::IconSize`], in
/// the colors' icon color): the thumb is then large while off too. Not `enabled`: faded, taking no clicks. Its
/// colors are `colors`' ([`SwitchDefaults::colors`]); its interactions are emitted to `interaction_source` (one of
/// its own where none), the thumb growing while it is pressed.
#[kompozable]
pub fn Switch(
    checked: bool,
    on_checked_change: Option<Rc<dyn Fn(bool)>>,
    #[default(Modifier)] modifier: Modifier,
    #[default(None)] thumb_content: Option<Rc<dyn Fn()>>,
    #[default(true)] enabled: bool,
    #[default(SwitchDefaults.colors())] colors: SwitchColors,
    #[default(None)] interaction_source: Option<MutableInteractionSource>,
) {
    let interaction_source = interaction_source.unwrap_or_else(|| remember(MutableInteractionSource::new));

    // TODO (Compose's): Add Swipeable modifier b/223797571
    let toggleable_modifier = match on_checked_change {
        Some(on_checked_change) => Modifier.minimum_interactive_component_size().toggleable_indication_with(
            checked,
            Some(interaction_source.clone()),
            None,
            enabled,
            Some(Role::Switch),
            move |value| on_checked_change(value),
        ),
        None => Modifier,
    };

    SwitchImpl(
        modifier
            .then(toggleable_modifier)
            .wrap_content_size(Alignment::Center)
            .required_size_wh(SwitchWidth, SwitchHeight),
        checked,
        enabled,
        colors,
        thumb_content,
        interaction_source.as_interaction_source(),
        SwitchTokens::HandleShape.value(),
    );
}

/// The track and the thumb (Compose's private `SwitchImpl`).
#[kompozable]
fn SwitchImpl(
    modifier: Modifier,
    checked: bool,
    enabled: bool,
    colors: SwitchColors,
    thumb_content: Option<Rc<dyn Fn()>>,
    interaction_source: InteractionSource,
    thumb_shape: Arc<dyn kompoz_graphics::Shape>,
) {
    let track_color = colors.track_color(enabled, checked);
    let resolved_thumb_color = colors.thumb_color(enabled, checked);
    let track_shape = SwitchTokens::TrackShape.value();

    Box(
        modifier = modifier
            .border(TrackOutlineWidth, colors.border_color(enabled, checked), track_shape.clone())
            .background(track_color, track_shape),
        content = move || {
            Box(
                modifier = Modifier
                    .align(Alignment::CenterStart)
                    .then_element(ThumbElement {
                        interaction_source: interaction_source.clone(),
                        checked,
                        // TODO (Compose's) Load the motionScheme tokens from the component tokens file
                        animation_spec: MotionSchemeKeyTokens::FastSpatial.value::<f32>(),
                    })
                    .indication(
                        interaction_source.clone(),
                        Some(ripple(bounded = false, radius = SwitchTokens::StateLayerSize / 2.0)),
                    )
                    .background(resolved_thumb_color, thumb_shape.clone()),
                content_alignment = Alignment::Center,
                content = move || {
                    if let Some(thumb_content) = thumb_content.clone() {
                        let icon_color = colors.icon_color(enabled, checked);
                        KompozitionLocalProvider(&[LocalContentColor.provides(icon_color)], move || thumb_content());
                    }
                },
            );
        },
    );
}

/// The thumb's element (Compose's private data class `ThumbElement`): equal for an equal source, state and spec.
struct ThumbElement {
    interaction_source: InteractionSource,
    checked: bool,
    animation_spec: Arc<dyn FiniteAnimationSpec<f32>>,
}

impl PartialEq for ThumbElement {
    fn eq(&self, other: &Self) -> bool {
        self.interaction_source == other.interaction_source
            && self.checked == other.checked
            && *self.animation_spec == *other.animation_spec
    }
}

impl ModifierNodeElement for ThumbElement {
    type Node = ThumbNode;

    fn create(&self) -> ThumbNode {
        ThumbNode {
            interaction_source: self.interaction_source.clone(),
            checked: self.checked,
            animation_spec: self.animation_spec.clone(),
            is_pressed: Rc::new(Cell::new(false)),
            offset_anim: None,
            size_anim: None,
            initial_offset: f32::NAN,
            initial_size: f32::NAN,
            handle: None,
        }
    }

    fn update(&self, node: &mut ThumbNode) {
        node.interaction_source = self.interaction_source.clone();
        if node.checked != self.checked
            && let Some(handle) = &node.handle
        {
            handle.invalidate_measurement();
        }
        node.checked = self.checked;
        node.animation_spec = self.animation_spec.clone();
        node.update();
    }

    fn name(&self) -> &'static str {
        "switchThumb"
    }
}

/// The thumb's node (Compose's private `ThumbNode`): its size and place by the state — off and plain 16 dp at the
/// start, on or with content 24 dp at the end, held 28 dp —, animated from the second measure on (snapped while
/// held); it counts its source's presses.
struct ThumbNode {
    interaction_source: InteractionSource,
    checked: bool,
    animation_spec: Arc<dyn FiniteAnimationSpec<f32>>,
    /// Shared with the coroutine counting the presses.
    is_pressed: Rc<Cell<bool>>,
    offset_anim: Option<Animatable<f32, AnimationVector1D>>,
    size_anim: Option<Animatable<f32, AnimationVector1D>>,
    initial_offset: f32,
    initial_size: f32,
    handle: Option<NodeHandle>,
}

impl ThumbNode {
    /// The animations made once there are values to start them from (Compose's `update`, called as its element
    /// updates it).
    fn update(&mut self) {
        if self.size_anim.is_none() && !self.initial_size.is_nan() {
            self.size_anim = Some(Animatable(self.initial_size));
        }
        if self.offset_anim.is_none() && !self.initial_offset.is_nan() {
            self.offset_anim = Some(Animatable(self.initial_offset));
        }
    }
}

impl ModifierNode for ThumbNode {
    node_capabilities!(layout);

    fn should_auto_invalidate(&self) -> bool {
        false
    }

    fn on_attach(&mut self, cx: &NodeCx) {
        let handle = cx.handle();
        self.handle = Some(handle.clone());
        let is_pressed = self.is_pressed.clone();
        let mut press_count = 0;
        cx.coroutine_scope().launch(self.interaction_source.interactions().collect(
            move |interaction: Arc<dyn Interaction>| {
                if interaction.is::<PressInteraction::Press>() {
                    press_count += 1;
                } else if interaction.is::<PressInteraction::Release>() || interaction.is::<PressInteraction::Cancel>()
                {
                    press_count -= 1;
                }
                let pressed = press_count > 0;
                if is_pressed.get() != pressed {
                    is_pressed.set(pressed);
                    handle.invalidate_measurement();
                }
            },
        ));
    }

    fn on_reset(&mut self) {
        self.offset_anim = None;
        self.size_anim = None;
        self.initial_size = f32::NAN;
        self.initial_offset = f32::NAN;
    }
}

impl LayoutModifierNode for ThumbNode {
    fn measure(
        &mut self,
        scope: &MeasureScope,
        measurable: &dyn Measurable,
        constraints: Constraints,
    ) -> MeasureResult {
        let density = scope.density();
        let is_pressed = self.is_pressed.get();
        let has_content = measurable.max_intrinsic_height(constraints.max_width) != 0
            && measurable.max_intrinsic_width(constraints.max_height) != 0;
        let size = density.to_px(if is_pressed {
            SwitchTokens::PressedHandleWidth
        } else if has_content || self.checked {
            ThumbDiameter
        } else {
            UncheckedThumbDiameter
        });

        let actual_size = self.size_anim.as_ref().map_or(size, |a| a.value()) as i32;
        let placeable = measurable.measure(Constraints::fixed(actual_size, actual_size));
        let thumb_padding_start = (SwitchHeight - density.to_dp(size)) / 2.0;
        let min_bound = density.to_px(thumb_padding_start);
        let thumb_path_length = (SwitchWidth - ThumbDiameter) - ThumbPadding;
        let max_bound = density.to_px(thumb_path_length);
        let offset = if is_pressed && self.checked {
            max_bound - density.to_px(TrackOutlineWidth)
        } else if is_pressed && !self.checked {
            density.to_px(TrackOutlineWidth)
        } else if self.checked {
            max_bound
        } else {
            min_bound
        };

        let spec = |pressed: bool,
                    spec: &Arc<dyn FiniteAnimationSpec<f32>>|
         -> Arc<dyn kompoz_animation::AnimationSpec<f32>> {
            if pressed { Arc::new(SnapSpec::<f32>::new(0)) } else { spec.clone() }
        };
        let scope_of = || self.handle.as_ref().and_then(|h| h.coroutine_scope());
        if self.size_anim.as_ref().map(|a| a.target_value()) != Some(size)
            && let Some(coroutines) = scope_of()
        {
            let (anim, spec) = (self.size_anim.clone(), spec(is_pressed, &self.animation_spec));
            coroutines.launch(async move {
                if let Some(anim) = anim {
                    named_args!(anim.animate_to(size, spec)).await;
                }
            });
        }

        if self.offset_anim.as_ref().map(|a| a.target_value()) != Some(offset)
            && let Some(coroutines) = scope_of()
        {
            let (anim, spec) = (self.offset_anim.clone(), spec(is_pressed, &self.animation_spec));
            coroutines.launch(async move {
                if let Some(anim) = anim {
                    named_args!(anim.animate_to(offset, spec)).await;
                }
            });
        }

        if self.initial_size.is_nan() && self.initial_offset.is_nan() {
            self.initial_size = size;
            self.initial_offset = offset;
        }

        let x = self.offset_anim.as_ref().map_or(offset, |a| a.value()) as i32;
        scope.layout(actual_size, actual_size, move |p| p.place_relative(&placeable, x, 0))
    }
}

/// The switches' defaults (Compose's SwitchDefaults).
pub struct SwitchDefaults;

#[named]
impl SwitchDefaults {
    /// The colors of a switch (Compose's `SwitchDefaults.colors()` and `colors(checkedThumbColor, …)`): the
    /// scheme's — on, the primary track and the primary's content color thumb; off, the highest surface container
    /// track in an outline, an outline thumb; disabled, faded over the surface — each given one in its place.
    /// (Compose's `colors()` gives the scheme's cached `defaultSwitchColors`, the same colors: one function here.)
    #[allow(clippy::too_many_arguments)]
    pub fn colors(
        &self,
        #[default(SwitchTokens::SelectedHandleColor.value())] checked_thumb_color: Color,
        #[default(SwitchTokens::SelectedTrackColor.value())] checked_track_color: Color,
        #[default(Color::Transparent)] checked_border_color: Color,
        #[default(SwitchTokens::SelectedIconColor.value())] checked_icon_color: Color,
        #[default(SwitchTokens::UnselectedHandleColor.value())] unchecked_thumb_color: Color,
        #[default(SwitchTokens::UnselectedTrackColor.value())] unchecked_track_color: Color,
        #[default(SwitchTokens::UnselectedFocusTrackOutlineColor.value())] unchecked_border_color: Color,
        #[default(SwitchTokens::UnselectedIconColor.value())] unchecked_icon_color: Color,
        #[default(faded(SwitchTokens::DisabledSelectedHandleColor.value(), SwitchTokens::DisabledSelectedHandleOpacity, MaterialTheme::color_scheme().surface))]
        disabled_checked_thumb_color: Color,
        #[default(faded(SwitchTokens::DisabledSelectedTrackColor.value(), SwitchTokens::DisabledTrackOpacity, MaterialTheme::color_scheme().surface))]
        disabled_checked_track_color: Color,
        #[default(Color::Transparent)] disabled_checked_border_color: Color,
        #[default(faded(SwitchTokens::DisabledSelectedIconColor.value(), SwitchTokens::DisabledSelectedIconOpacity, MaterialTheme::color_scheme().surface))]
        disabled_checked_icon_color: Color,
        #[default(faded(SwitchTokens::DisabledUnselectedHandleColor.value(), SwitchTokens::DisabledUnselectedHandleOpacity, MaterialTheme::color_scheme().surface))]
        disabled_unchecked_thumb_color: Color,
        #[default(faded(SwitchTokens::DisabledUnselectedTrackColor.value(), SwitchTokens::DisabledTrackOpacity, MaterialTheme::color_scheme().surface))]
        disabled_unchecked_track_color: Color,
        #[default(faded(SwitchTokens::DisabledUnselectedTrackOutlineColor.value(), SwitchTokens::DisabledTrackOpacity, MaterialTheme::color_scheme().surface))]
        disabled_unchecked_border_color: Color,
        #[default(faded(SwitchTokens::DisabledUnselectedIconColor.value(), SwitchTokens::DisabledUnselectedIconOpacity, MaterialTheme::color_scheme().surface))]
        disabled_unchecked_icon_color: Color,
    ) -> SwitchColors {
        SwitchColors(
            checked_thumb_color,
            checked_track_color,
            checked_border_color,
            checked_icon_color,
            unchecked_thumb_color,
            unchecked_track_color,
            unchecked_border_color,
            unchecked_icon_color,
            disabled_checked_thumb_color,
            disabled_checked_track_color,
            disabled_checked_border_color,
            disabled_checked_icon_color,
            disabled_unchecked_thumb_color,
            disabled_unchecked_track_color,
            disabled_unchecked_border_color,
            disabled_unchecked_icon_color,
        )
    }
}

#[allow(non_snake_case)]
impl SwitchDefaults {
    /// The size of an icon in a switch's thumb (Compose's `SwitchDefaults.IconSize`): 16 dp.
    pub const IconSize: Dp = Dp(16.0);
}

/// `color` at `alpha` over `surface` (the disabled colors' `color.copy(alpha = …).compositeOver(surface)`).
fn faded(color: Color, alpha: f32, surface: Color) -> Color {
    named_args!(color.copy(alpha = alpha)).composite_over(surface)
}

/// The colors of a switch in its states (Compose's SwitchColors): the thumb's, the track's, its border's and the
/// icon's, on or off, enabled or not. Make them with [`SwitchDefaults::colors`] or
/// [`SwitchColors()`](fn@SwitchColors); change some with [`copy`](SwitchColors::copy).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, kompoz_runtime::KotlinProperties)]
pub struct SwitchColors {
    pub checked_thumb_color: Color,
    pub checked_track_color: Color,
    pub checked_border_color: Color,
    pub checked_icon_color: Color,
    pub unchecked_thumb_color: Color,
    pub unchecked_track_color: Color,
    pub unchecked_border_color: Color,
    pub unchecked_icon_color: Color,
    pub disabled_checked_thumb_color: Color,
    pub disabled_checked_track_color: Color,
    pub disabled_checked_border_color: Color,
    pub disabled_checked_icon_color: Color,
    pub disabled_unchecked_thumb_color: Color,
    pub disabled_unchecked_track_color: Color,
    pub disabled_unchecked_border_color: Color,
    pub disabled_unchecked_icon_color: Color,
}

/// A switch's colors, each given (Compose's `SwitchColors(…)` constructor).
#[named(type)]
#[allow(clippy::too_many_arguments)]
pub fn SwitchColors(
    checked_thumb_color: Color,
    checked_track_color: Color,
    checked_border_color: Color,
    checked_icon_color: Color,
    unchecked_thumb_color: Color,
    unchecked_track_color: Color,
    unchecked_border_color: Color,
    unchecked_icon_color: Color,
    disabled_checked_thumb_color: Color,
    disabled_checked_track_color: Color,
    disabled_checked_border_color: Color,
    disabled_checked_icon_color: Color,
    disabled_unchecked_thumb_color: Color,
    disabled_unchecked_track_color: Color,
    disabled_unchecked_border_color: Color,
    disabled_unchecked_icon_color: Color,
) -> SwitchColors {
    SwitchColors {
        checked_thumb_color,
        checked_track_color,
        checked_border_color,
        checked_icon_color,
        unchecked_thumb_color,
        unchecked_track_color,
        unchecked_border_color,
        unchecked_icon_color,
        disabled_checked_thumb_color,
        disabled_checked_track_color,
        disabled_checked_border_color,
        disabled_checked_icon_color,
        disabled_unchecked_thumb_color,
        disabled_unchecked_track_color,
        disabled_unchecked_border_color,
        disabled_unchecked_icon_color,
    }
}

#[named]
impl SwitchColors {
    /// The same with what is given replaced (Compose's `copy`): an unspecified color keeps this one's.
    #[allow(clippy::too_many_arguments)]
    pub fn copy(
        &self,
        #[default(self.checked_thumb_color)] checked_thumb_color: Color,
        #[default(self.checked_track_color)] checked_track_color: Color,
        #[default(self.checked_border_color)] checked_border_color: Color,
        #[default(self.checked_icon_color)] checked_icon_color: Color,
        #[default(self.unchecked_thumb_color)] unchecked_thumb_color: Color,
        #[default(self.unchecked_track_color)] unchecked_track_color: Color,
        #[default(self.unchecked_border_color)] unchecked_border_color: Color,
        #[default(self.unchecked_icon_color)] unchecked_icon_color: Color,
        #[default(self.disabled_checked_thumb_color)] disabled_checked_thumb_color: Color,
        #[default(self.disabled_checked_track_color)] disabled_checked_track_color: Color,
        #[default(self.disabled_checked_border_color)] disabled_checked_border_color: Color,
        #[default(self.disabled_checked_icon_color)] disabled_checked_icon_color: Color,
        #[default(self.disabled_unchecked_thumb_color)] disabled_unchecked_thumb_color: Color,
        #[default(self.disabled_unchecked_track_color)] disabled_unchecked_track_color: Color,
        #[default(self.disabled_unchecked_border_color)] disabled_unchecked_border_color: Color,
        #[default(self.disabled_unchecked_icon_color)] disabled_unchecked_icon_color: Color,
    ) -> SwitchColors {
        SwitchColors(
            checked_thumb_color.take_or_else(|| self.checked_thumb_color),
            checked_track_color.take_or_else(|| self.checked_track_color),
            checked_border_color.take_or_else(|| self.checked_border_color),
            checked_icon_color.take_or_else(|| self.checked_icon_color),
            unchecked_thumb_color.take_or_else(|| self.unchecked_thumb_color),
            unchecked_track_color.take_or_else(|| self.unchecked_track_color),
            unchecked_border_color.take_or_else(|| self.unchecked_border_color),
            unchecked_icon_color.take_or_else(|| self.unchecked_icon_color),
            disabled_checked_thumb_color.take_or_else(|| self.disabled_checked_thumb_color),
            disabled_checked_track_color.take_or_else(|| self.disabled_checked_track_color),
            disabled_checked_border_color.take_or_else(|| self.disabled_checked_border_color),
            disabled_checked_icon_color.take_or_else(|| self.disabled_checked_icon_color),
            disabled_unchecked_thumb_color.take_or_else(|| self.disabled_unchecked_thumb_color),
            disabled_unchecked_track_color.take_or_else(|| self.disabled_unchecked_track_color),
            disabled_unchecked_border_color.take_or_else(|| self.disabled_unchecked_border_color),
            disabled_unchecked_icon_color.take_or_else(|| self.disabled_unchecked_icon_color),
        )
    }
}

impl SwitchColors {
    /// The thumb's color (Compose's internal `thumbColor`).
    pub(crate) fn thumb_color(&self, enabled: bool, checked: bool) -> Color {
        if enabled {
            if checked { self.checked_thumb_color } else { self.unchecked_thumb_color }
        } else if checked {
            self.disabled_checked_thumb_color
        } else {
            self.disabled_unchecked_thumb_color
        }
    }

    /// The track's color (Compose's internal `trackColor`).
    pub(crate) fn track_color(&self, enabled: bool, checked: bool) -> Color {
        if enabled {
            if checked { self.checked_track_color } else { self.unchecked_track_color }
        } else if checked {
            self.disabled_checked_track_color
        } else {
            self.disabled_unchecked_track_color
        }
    }

    /// The track's border's color (Compose's internal `borderColor`).
    pub(crate) fn border_color(&self, enabled: bool, checked: bool) -> Color {
        if enabled {
            if checked { self.checked_border_color } else { self.unchecked_border_color }
        } else if checked {
            self.disabled_checked_border_color
        } else {
            self.disabled_unchecked_border_color
        }
    }

    /// The thumb content's (icon's) color (Compose's internal `iconColor`).
    pub(crate) fn icon_color(&self, enabled: bool, checked: bool) -> Color {
        if enabled {
            if checked { self.checked_icon_color } else { self.unchecked_icon_color }
        } else if checked {
            self.disabled_checked_icon_color
        } else {
            self.disabled_unchecked_icon_color
        }
    }
}

/// The thumb's diameter on (Compose's internal `ThumbDiameter`): 24 dp.
const ThumbDiameter: Dp = SwitchTokens::SelectedHandleWidth;
/// The thumb's diameter off and plain (Compose's internal `UncheckedThumbDiameter`): 16 dp.
const UncheckedThumbDiameter: Dp = SwitchTokens::UnselectedHandleWidth;

const SwitchWidth: Dp = SwitchTokens::TrackWidth;
const SwitchHeight: Dp = SwitchTokens::TrackHeight;
const ThumbPadding: Dp = Dp((SwitchHeight.0 - ThumbDiameter.0) / 2.0);
const TrackOutlineWidth: Dp = SwitchTokens::TrackOutlineWidth;
