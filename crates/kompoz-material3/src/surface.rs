//! Material's Surface, as Compose's Surface.kt: content on a background of a shape — tinted by its tonal
//! elevation (the elevations of the surfaces it is on added: [`LocalAbsoluteTonalElevation`]), with a shadow of its
//! own elevation and a border where given — that gives its content a content color; one that is clicked, selected or
//! toggled, the ripple showing its interactions, at least 48 dp each way.
//!
//! Compose's overloads by their first parameters are composables of their own here: [`Surface`],
//! [`ClickableSurface`] (Kotlin's `Surface(onClick, …)`), [`SelectableSurface`] (`Surface(selected, onClick, …)`) and
//! [`ToggleableSurface`] (`Surface(checked, onCheckedChange, …)`).

use crate::color_scheme::content_color_for;
use crate::content_color::LocalContentColor;
use crate::interactive_component_size::InteractiveComponentSizeModifiers;
use crate::internal::child_parent_semantics::ChildParentSemanticsModifiers;
use crate::internal::shape_util::ClipShape;
use crate::material_theme::MaterialTheme;
use crate::ripples::ripple;
use kompoz_foundation::prelude::*;
use kompoz_graphics::{BorderStroke, Color, ColorProducer, Dp, RectangleShape, Shape};
use kompoz_runtime::{KompozitionLocal, KompozitionLocalProvider, kompozable, named_args, remember};
use kompoz_ui::Modifier;
use kompoz_ui::layout::GraphicsLayerScope;
use kompoz_ui::platform::LocalDensity;
use std::sync::Arc;

/// The tonal elevation of the surfaces around added up (Compose's LocalAbsoluteTonalElevation): what a surface's
/// own is added to, its background tinted by the sum.
pub static LocalAbsoluteTonalElevation: KompozitionLocal<Dp> = KompozitionLocal::new(|| Dp(0.0));

/// A surface (Compose's `Surface(modifier, shape, color, …)`): `content` (in a Box giving it the surface's min
/// constraints) on `color` in `shape` — the scheme's surface color tinted by its absolute tonal elevation
/// ([`LocalAbsoluteTonalElevation`] plus `tonal_elevation`) where tonal elevation is on — under `border`, with a
/// shadow of `shadow_elevation`, cut to the shape; the content in `content_color` (the `on` color of `color`, else
/// the one around; Kotlin's default `contentColorFor(color)`, here `None`). It takes the pointer's events (what is under it doesn't), and is a container to
/// accessibility.
#[kompozable(inline)]
#[allow(clippy::too_many_arguments)]
pub fn Surface(
    #[default(Modifier)] modifier: Modifier,
    #[default(Arc::new(RectangleShape))] shape: Arc<dyn Shape>,
    #[default(MaterialTheme::color_scheme().surface)] color: Color,
    #[default(None)] content_color: Option<Color>,
    #[default(Dp(0.0))] tonal_elevation: Dp,
    #[default(Dp(0.0))] shadow_elevation: Dp,
    #[default(None)] border: Option<BorderStroke>,
    content: impl FnOnce(),
) {
    let content_color = content_color.unwrap_or_else(|| content_color_for(color));
    let absolute_elevation = LocalAbsoluteTonalElevation.current() + tonal_elevation;
    let background_color = surface_color_at_elevation(color, absolute_elevation);
    let shadow = LocalDensity.current().to_px(shadow_elevation);
    KompozitionLocalProvider(
        &[LocalContentColor.provides(content_color), LocalAbsoluteTonalElevation.provides(absolute_elevation)],
        move || {
            let m = surface(modifier, shape, background_color, border, shadow)
                .semantics(|c| c.set_is_container(true))
                .pointer_input((), |_| async {});
            Box(modifier = m, propagate_min_constraints = true, content = content);
        },
    );
}

/// A surface that is clicked (Compose's `Surface(onClick, …)`): as [`Surface`], at least the minimum interactive size
/// each way, `on_click` called on a click (not when `enabled` is false), its interactions shown by the ripple and
/// emitted to `interaction_source` (one of its own where none), its content's semantics merged into its own.
///
/// Kotlin: `Surface(onClick: () -> Unit, modifier: Modifier = Modifier)`
#[kompozable(inline)]
#[allow(clippy::too_many_arguments)]
pub fn ClickableSurface(
    on_click: impl Fn() + 'static,
    #[default(Modifier)] modifier: Modifier,
    #[default(true)] enabled: bool,
    #[default(Arc::new(RectangleShape))] shape: Arc<dyn Shape>,
    #[default(MaterialTheme::color_scheme().surface)] color: Color,
    #[default(None)] content_color: Option<Color>,
    #[default(Dp(0.0))] tonal_elevation: Dp,
    #[default(Dp(0.0))] shadow_elevation: Dp,
    #[default(None)] border: Option<BorderStroke>,
    #[default(None)] interaction_source: Option<MutableInteractionSource>,
    content: impl FnOnce(),
) {
    let interaction_source = interaction_source.unwrap_or_else(|| remember(MutableInteractionSource::new));
    let content_color = content_color.unwrap_or_else(|| content_color_for(color));
    let absolute_elevation = LocalAbsoluteTonalElevation.current() + tonal_elevation;
    let background_color = surface_color_at_elevation(color, absolute_elevation);
    let shadow = LocalDensity.current().to_px(shadow_elevation);
    let indication = ripple();
    KompozitionLocalProvider(
        &[LocalContentColor.provides(content_color), LocalAbsoluteTonalElevation.provides(absolute_elevation)],
        move || {
            let m = surface(modifier.minimum_interactive_component_size(), shape, background_color, border, shadow)
                .clickable_indication_with(Some(interaction_source), Some(indication), enabled, None, None, on_click)
                .child_semantics();
            Box(modifier = m, propagate_min_constraints = true, content = content);
        },
    );
}

/// A surface that is selected (Compose's `Surface(selected, onClick, …)`): as [`ClickableSurface`], `selected` told
/// to accessibility.
///
/// Kotlin: `Surface(selected: Boolean, onClick: () -> Unit, modifier: Modifier = Modifier)`
#[kompozable(inline)]
#[allow(clippy::too_many_arguments)]
pub fn SelectableSurface(
    selected: bool,
    on_click: impl Fn() + 'static,
    #[default(Modifier)] modifier: Modifier,
    #[default(true)] enabled: bool,
    #[default(Arc::new(RectangleShape))] shape: Arc<dyn Shape>,
    #[default(MaterialTheme::color_scheme().surface)] color: Color,
    #[default(None)] content_color: Option<Color>,
    #[default(Dp(0.0))] tonal_elevation: Dp,
    #[default(Dp(0.0))] shadow_elevation: Dp,
    #[default(None)] border: Option<BorderStroke>,
    #[default(None)] interaction_source: Option<MutableInteractionSource>,
    content: impl FnOnce(),
) {
    let interaction_source = interaction_source.unwrap_or_else(|| remember(MutableInteractionSource::new));
    let content_color = content_color.unwrap_or_else(|| content_color_for(color));
    let absolute_elevation = LocalAbsoluteTonalElevation.current() + tonal_elevation;
    let background_color = surface_color_at_elevation(color, absolute_elevation);
    let shadow = LocalDensity.current().to_px(shadow_elevation);
    let indication = ripple();
    KompozitionLocalProvider(
        &[LocalContentColor.provides(content_color), LocalAbsoluteTonalElevation.provides(absolute_elevation)],
        move || {
            let m = surface(modifier.minimum_interactive_component_size(), shape, background_color, border, shadow)
                .selectable_indication_with(
                    selected,
                    Some(interaction_source),
                    Some(indication),
                    enabled,
                    None,
                    on_click,
                )
                .child_semantics();
            Box(modifier = m, propagate_min_constraints = true, content = content);
        },
    );
}

/// A surface that is toggled (Compose's `Surface(checked, onCheckedChange, …)`): as [`ClickableSurface`], a click
/// calling `on_checked_change` with the other state, `checked` told to accessibility.
///
/// Kotlin: `Surface(checked: Boolean, onCheckedChange: (Boolean) -> Unit, modifier: Modifier = Modifier)`
#[kompozable(inline)]
#[allow(clippy::too_many_arguments)]
pub fn ToggleableSurface(
    checked: bool,
    on_checked_change: impl Fn(bool) + 'static,
    #[default(Modifier)] modifier: Modifier,
    #[default(true)] enabled: bool,
    #[default(Arc::new(RectangleShape))] shape: Arc<dyn Shape>,
    #[default(MaterialTheme::color_scheme().surface)] color: Color,
    #[default(None)] content_color: Option<Color>,
    #[default(Dp(0.0))] tonal_elevation: Dp,
    #[default(Dp(0.0))] shadow_elevation: Dp,
    #[default(None)] border: Option<BorderStroke>,
    #[default(None)] interaction_source: Option<MutableInteractionSource>,
    content: impl FnOnce(),
) {
    let interaction_source = interaction_source.unwrap_or_else(|| remember(MutableInteractionSource::new));
    let content_color = content_color.unwrap_or_else(|| content_color_for(color));
    let absolute_elevation = LocalAbsoluteTonalElevation.current() + tonal_elevation;
    let background_color = surface_color_at_elevation(color, absolute_elevation);
    let shadow = LocalDensity.current().to_px(shadow_elevation);
    let indication = ripple();
    KompozitionLocalProvider(
        &[LocalContentColor.provides(content_color), LocalAbsoluteTonalElevation.provides(absolute_elevation)],
        move || {
            let m = surface(modifier.minimum_interactive_component_size(), shape, background_color, border, shadow)
                .toggleable_indication_with(
                    checked,
                    Some(interaction_source),
                    Some(indication),
                    enabled,
                    None,
                    on_checked_change,
                )
                .child_semantics();
            Box(modifier = m, propagate_min_constraints = true, content = content);
        },
    );
}

/// A surface whose background `background_color` gives as it draws (Compose's internal `Surface(backgroundColor:
/// ColorProducer, …)`): its color, or the scheme's surface tinted by `tonal_elevation` where it is that, behind
/// `content`; a shadow of `shadow_elevation`.
#[kompozable]
pub(crate) fn ColorProducerSurface(
    background_color: ColorProducer,
    modifier: Modifier,
    #[default(Dp(0.0))] tonal_elevation: Dp,
    #[default(Dp(0.0))] shadow_elevation: Dp,
    content: impl Fn() + 'static,
) {
    let absolute_elevation = LocalAbsoluteTonalElevation.current() + tonal_elevation;
    let tonal_elevation_enabled = crate::color_scheme::LocalTonalElevationEnabled.current();
    let color_scheme = MaterialTheme::color_scheme();
    let content = Rc::new(content);
    KompozitionLocalProvider(&[LocalAbsoluteTonalElevation.provides(absolute_elevation)], move || {
        let shadow = if shadow_elevation.0 > 0.0 {
            Modifier.graphics_layer_block(move |l: &mut GraphicsLayerScope| {
                l.shadow_elevation = l.density().to_px(shadow_elevation);
                l.clip = false;
            })
        } else {
            Modifier
        };
        let background_color = background_color.clone();
        let m = modifier.then(shadow).draw_behind(move |d| {
            let background_color_resolved = background_color.get();
            let color = if background_color_resolved == color_scheme.surface && tonal_elevation_enabled {
                color_scheme.surface_color_at_elevation(tonal_elevation)
            } else {
                background_color_resolved
            };
            d.draw_rect(color);
        });
        let content = content.clone();
        Box(modifier = m, content = move || content());
    });
}

/// The surface's background: its shadow, its border, its color, cut to its shape (Compose's private
/// `Modifier.surface`).
fn surface(
    modifier: Modifier,
    shape: Arc<dyn Shape>,
    background_color: Color,
    border: Option<BorderStroke>,
    shadow_elevation: f32,
) -> Modifier {
    let shadow = if shadow_elevation > 0.0 {
        let mut layer = GraphicsLayerScope::default();
        layer.shadow_elevation = shadow_elevation;
        layer.shape = shape.clone();
        layer.clip = false;
        Modifier.graphics_layer_with(layer)
    } else {
        Modifier
    };
    let border = match border {
        Some(border) => named_args!(Modifier.border_stroke(border, shape = shape.clone())),
        None => Modifier,
    };
    let shape_for_clip = ClipShape(shape.clone());
    named_args!(modifier.then(shadow).then(border).background(background_color, shape = shape)).clip(shape_for_clip)
}

/// The background of a surface's color at an elevation (Compose's private `surfaceColorAtElevation`): the
/// scheme's tonal elevation applied.
fn surface_color_at_elevation(color: Color, elevation: Dp) -> Color {
    MaterialTheme::color_scheme().apply_tonal_elevation(color, elevation)
}
