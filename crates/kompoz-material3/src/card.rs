//! Material's cards, as Compose's Card.kt: content about one subject in a column on a surface — filled
//! ([`Card`], the scheme's highest surface container), elevated ([`ElevatedCard`], a low surface container with a
//! shadow) or outlined ([`OutlinedCard`], the surface in a line of the outline variant) —, 12 dp round corners.
//! Their clickable kinds ([`ClickableCard`], [`ClickableElevatedCard`], [`ClickableOutlinedCard`]: Kotlin's
//! overloads taking `onClick`) take clicks, show the ripple, and raise or lower their shadow as they are pressed,
//! hovered, focused or dragged ([`CardElevation`]).

use crate::color_scheme::{ColorScheme, DisabledAlpha, content_color_for};
use crate::internal::elevation::AnimateElevation;
use crate::material_theme::MaterialTheme;
use crate::surface::{ClickableSurface, Surface};
use crate::tokens::{ElevatedCardTokens, FilledCardTokens, OutlinedCardTokens};
use kompoz_animation::{Animatable, AnimationVector1D, VectorConverter};
use kompoz_coroutines::LaunchedEffect;
use kompoz_foundation::interaction::{
    DragInteraction, FocusInteraction, HoverInteraction, Interaction, InteractionSource, MutableInteractionSource,
    PressInteraction,
};
use kompoz_foundation::prelude::*;
use kompoz_graphics::{BorderStroke, Color, Dp, Offset, Shape};
use kompoz_runtime::{
    KotlinProperties, SnapshotStateList, State, kompozable, kompozable_block, mutable_state_list_of, mutable_state_of,
    named, named_args, remember, remember_keyed,
};
use kompoz_ui::Modifier;
use std::sync::Arc;

/// A filled card (Compose's `Card(modifier, …)`): `content` in a Column on a [`Surface`] of `shape` in `colors`'
/// enabled container color, its content in their enabled content color, with the shadow of `elevation`'s default
/// elevation and `border` where given. It isn't clicked (see [`ClickableCard`]).
#[kompozable]
pub fn Card(
    #[default(Modifier)] modifier: Modifier,
    #[default(CardDefaults::shape())] shape: Arc<dyn Shape>,
    #[default(CardDefaults.card_colors())] colors: CardColors,
    #[default(CardDefaults.card_elevation())] elevation: CardElevation,
    #[default(None)] border: Option<BorderStroke>,
    content: impl Fn() + 'static,
) {
    let shadow_elevation = elevation.shadow_elevation(true, None).value();
    Surface(
        modifier = modifier,
        shape = shape,
        color = colors.container_color_for(true),
        content_color = Some(colors.content_color_for(true)),
        shadow_elevation = shadow_elevation,
        border = border,
        content = || Column(content = || content()),
    );
}

/// A filled card that is clicked (Compose's `Card(onClick, …)`): as [`Card`], on a [`ClickableSurface`] calling
/// `on_click` (not when `enabled` is false: its disabled colors then), its interactions emitted to
/// `interaction_source` (one of its own where none) raising or lowering its shadow as `elevation` says.
///
/// Kotlin: `Card(onClick: () -> Unit, modifier: Modifier = Modifier)`
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn ClickableCard(
    on_click: impl Fn() + 'static,
    #[default(Modifier)] modifier: Modifier,
    #[default(true)] enabled: bool,
    #[default(CardDefaults::shape())] shape: Arc<dyn Shape>,
    #[default(CardDefaults.card_colors())] colors: CardColors,
    #[default(CardDefaults.card_elevation())] elevation: CardElevation,
    #[default(None)] border: Option<BorderStroke>,
    #[default(None)] interaction_source: Option<MutableInteractionSource>,
    content: impl Fn() + 'static,
) {
    let interaction_source = interaction_source.unwrap_or_else(|| remember(MutableInteractionSource::new));
    let shadow_elevation =
        elevation.shadow_elevation(enabled, Some(&interaction_source.as_interaction_source())).value();
    ClickableSurface(
        on_click,
        modifier = modifier,
        enabled = enabled,
        shape = shape,
        color = colors.container_color_for(enabled),
        content_color = Some(colors.content_color_for(enabled)),
        shadow_elevation = shadow_elevation,
        border = border,
        interaction_source = Some(interaction_source),
        content = || Column(content = || content()),
    );
}

/// An elevated card (Compose's `ElevatedCard(modifier, …)`): a [`Card`] in the elevated card's colors and
/// elevation (a low surface container, a shadow of 1 dp), without a border.
#[kompozable]
pub fn ElevatedCard(
    #[default(Modifier)] modifier: Modifier,
    #[default(CardDefaults::elevated_shape())] shape: Arc<dyn Shape>,
    #[default(CardDefaults.elevated_card_colors())] colors: CardColors,
    #[default(CardDefaults.elevated_card_elevation())] elevation: CardElevation,
    content: impl Fn() + 'static,
) {
    Card(
        modifier = modifier,
        shape = shape,
        border = None,
        elevation = elevation,
        colors = colors,
        content = || content(),
    );
}

/// An elevated card that is clicked (Compose's `ElevatedCard(onClick, …)`): a [`ClickableCard`] in the elevated
/// card's colors and elevation, without a border.
///
/// Kotlin: `ElevatedCard(onClick: () -> Unit, modifier: Modifier = Modifier)`
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn ClickableElevatedCard(
    on_click: impl Fn() + 'static,
    #[default(Modifier)] modifier: Modifier,
    #[default(true)] enabled: bool,
    #[default(CardDefaults::elevated_shape())] shape: Arc<dyn Shape>,
    #[default(CardDefaults.elevated_card_colors())] colors: CardColors,
    #[default(CardDefaults.elevated_card_elevation())] elevation: CardElevation,
    #[default(None)] interaction_source: Option<MutableInteractionSource>,
    content: impl Fn() + 'static,
) {
    ClickableCard(
        on_click,
        modifier = modifier,
        enabled = enabled,
        shape = shape,
        colors = colors,
        elevation = elevation,
        border = None,
        interaction_source = interaction_source,
        content = || content(),
    );
}

/// An outlined card (Compose's `OutlinedCard(modifier, …)`): a [`Card`] in the outlined card's colors and
/// elevation (the surface, no shadow) inside `border` (by default a line of the outline variant, 1 dp).
#[kompozable]
pub fn OutlinedCard(
    #[default(Modifier)] modifier: Modifier,
    #[default(CardDefaults::outlined_shape())] shape: Arc<dyn Shape>,
    #[default(CardDefaults.outlined_card_colors())] colors: CardColors,
    #[default(CardDefaults.outlined_card_elevation())] elevation: CardElevation,
    #[default(CardDefaults.outlined_card_border())] border: BorderStroke,
    content: impl Fn() + 'static,
) {
    Card(
        modifier = modifier,
        shape = shape,
        colors = colors,
        elevation = elevation,
        border = Some(border),
        content = || content(),
    );
}

/// An outlined card that is clicked (Compose's `OutlinedCard(onClick, …)`): a [`ClickableCard`] in the outlined
/// card's colors and elevation inside `border` (Kotlin's default `CardDefaults.outlinedCardBorder(enabled)`, here
/// `None`: the outline variant, or the outline at 12% over the low surface container when disabled).
///
/// Kotlin: `OutlinedCard(onClick: () -> Unit, modifier: Modifier = Modifier)`
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn ClickableOutlinedCard(
    on_click: impl Fn() + 'static,
    #[default(Modifier)] modifier: Modifier,
    #[default(true)] enabled: bool,
    #[default(CardDefaults::outlined_shape())] shape: Arc<dyn Shape>,
    #[default(CardDefaults.outlined_card_colors())] colors: CardColors,
    #[default(CardDefaults.outlined_card_elevation())] elevation: CardElevation,
    #[default(None)] border: Option<BorderStroke>,
    #[default(None)] interaction_source: Option<MutableInteractionSource>,
    content: impl Fn() + 'static,
) {
    let border = border.unwrap_or_else(|| CardDefaults.outlined_card_border_with(enabled));
    ClickableCard(
        on_click,
        modifier = modifier,
        enabled = enabled,
        shape = shape,
        colors = colors,
        elevation = elevation,
        border = Some(border),
        interaction_source = interaction_source,
        content = || content(),
    );
}

/// The cards' defaults (Compose's `CardDefaults`): their shapes, colors, elevations and the outlined card's
/// border. Its composables are methods of the value, as Kotlin calls them (`CardDefaults.card_colors()`).
#[derive(Clone, Copy, Debug)]
pub struct CardDefaults;

impl CardDefaults {
    /// A card's shape: the theme's medium corners, 12 dp (Compose's `CardDefaults.shape`).
    pub fn shape() -> Arc<dyn Shape> {
        FilledCardTokens::ContainerShape.value()
    }

    /// An elevated card's shape (Compose's `CardDefaults.elevatedShape`).
    pub fn elevated_shape() -> Arc<dyn Shape> {
        ElevatedCardTokens::ContainerShape.value()
    }

    /// An outlined card's shape (Compose's `CardDefaults.outlinedShape`).
    pub fn outlined_shape() -> Arc<dyn Shape> {
        OutlinedCardTokens::ContainerShape.value()
    }

    /// A card's colors in `scheme` (Compose's internal `ColorScheme.defaultCardColors`): the highest surface
    /// container and its content color; disabled, the surface variant at 38% over it, the content at 38%.
    pub(crate) fn default_card_colors(scheme: &ColorScheme) -> CardColors {
        let container = scheme.from_token(FilledCardTokens::ContainerColor);
        CardColors::new(
            container,
            scheme.content_color_for(container),
            named_args!(
                scheme
                    .from_token(FilledCardTokens::DisabledContainerColor)
                    .copy(alpha = FilledCardTokens::DisabledContainerOpacity)
            )
            .composite_over(container),
            named_args!(scheme.content_color_for(container).copy(alpha = DisabledAlpha)),
        )
    }

    /// An elevated card's colors in `scheme` (Compose's internal `ColorScheme.defaultElevatedCardColors`): the low
    /// surface container; disabled, the surface at 38% over itself (as Compose's).
    pub(crate) fn default_elevated_card_colors(scheme: &ColorScheme) -> CardColors {
        let container = scheme.from_token(ElevatedCardTokens::ContainerColor);
        let disabled = scheme.from_token(ElevatedCardTokens::DisabledContainerColor);
        CardColors::new(
            container,
            scheme.content_color_for(container),
            named_args!(disabled.copy(alpha = ElevatedCardTokens::DisabledContainerOpacity)).composite_over(disabled),
            named_args!(scheme.content_color_for(container).copy(alpha = DisabledAlpha)),
        )
    }

    /// An outlined card's colors in `scheme` (Compose's internal `ColorScheme.defaultOutlinedCardColors`): the
    /// surface, enabled or not; the content at 38% when disabled.
    pub(crate) fn default_outlined_card_colors(scheme: &ColorScheme) -> CardColors {
        let container = scheme.from_token(OutlinedCardTokens::ContainerColor);
        CardColors::new(
            container,
            scheme.content_color_for(container),
            container,
            named_args!(scheme.content_color_for(container).copy(alpha = DisabledAlpha)),
        )
    }
}

/// The colors `container_color`, `content_color`, `disabled_container_color` and `disabled_content_color` make
/// of `defaults` (the body of Compose's `CardDefaults.cardColors(containerColor, …)` and its two siblings): none
/// given, the defaults as they are (Kotlin's overload without parameters); else each one given over the
/// defaults' (an unspecified one keeping the default), the content color `content_color_for(container)` where
/// not given, the disabled content color `disabled_content(content)` where not given.
fn card_colors_of(
    defaults: CardColors,
    container_color: Option<Color>,
    content_color: Option<Color>,
    disabled_container_color: Option<Color>,
    disabled_content_color: Option<Color>,
    disabled_content: impl FnOnce(Color, Color) -> Color,
) -> CardColors {
    if container_color.is_none()
        && content_color.is_none()
        && disabled_container_color.is_none()
        && disabled_content_color.is_none()
    {
        return defaults;
    }
    let container_color = container_color.unwrap_or(Color::Unspecified);
    let content_color = content_color.unwrap_or_else(|| content_color_for(container_color));
    let disabled_container_color = disabled_container_color.unwrap_or(Color::Unspecified);
    let disabled_content_color =
        disabled_content_color.unwrap_or_else(|| disabled_content(container_color, content_color));
    named_args!(defaults.copy(
        container_color = container_color,
        content_color = content_color,
        disabled_container_color = disabled_container_color,
        disabled_content_color = disabled_content_color,
    ))
}

#[named]
impl CardDefaults {
    /// A card's elevation (Compose's `CardDefaults.cardElevation`): none, 1 dp hovered, 6 dp dragged.
    pub fn card_elevation(
        &self,
        #[default(FilledCardTokens::ContainerElevation)] default_elevation: Dp,
        #[default(FilledCardTokens::PressedContainerElevation)] pressed_elevation: Dp,
        #[default(FilledCardTokens::FocusContainerElevation)] focused_elevation: Dp,
        #[default(FilledCardTokens::HoverContainerElevation)] hovered_elevation: Dp,
        #[default(FilledCardTokens::DraggedContainerElevation)] dragged_elevation: Dp,
        #[default(FilledCardTokens::DisabledContainerElevation)] disabled_elevation: Dp,
    ) -> CardElevation {
        CardElevation::new(
            default_elevation,
            pressed_elevation,
            focused_elevation,
            hovered_elevation,
            dragged_elevation,
            disabled_elevation,
        )
    }

    /// An elevated card's elevation (Compose's `CardDefaults.elevatedCardElevation`): 1 dp, 3 dp hovered, 8 dp
    /// dragged, 1 dp disabled.
    pub fn elevated_card_elevation(
        &self,
        #[default(ElevatedCardTokens::ContainerElevation)] default_elevation: Dp,
        #[default(ElevatedCardTokens::PressedContainerElevation)] pressed_elevation: Dp,
        #[default(ElevatedCardTokens::FocusContainerElevation)] focused_elevation: Dp,
        #[default(ElevatedCardTokens::HoverContainerElevation)] hovered_elevation: Dp,
        #[default(ElevatedCardTokens::DraggedContainerElevation)] dragged_elevation: Dp,
        #[default(ElevatedCardTokens::DisabledContainerElevation)] disabled_elevation: Dp,
    ) -> CardElevation {
        CardElevation::new(
            default_elevation,
            pressed_elevation,
            focused_elevation,
            hovered_elevation,
            dragged_elevation,
            disabled_elevation,
        )
    }

    /// An outlined card's elevation (Compose's `CardDefaults.outlinedCardElevation`): none but dragged (6 dp);
    /// pressed, focused and hovered the default one where not given (Kotlin's defaults `defaultElevation`, here
    /// `None`).
    pub fn outlined_card_elevation(
        &self,
        #[default(OutlinedCardTokens::ContainerElevation)] default_elevation: Dp,
        #[default(None)] pressed_elevation: Option<Dp>,
        #[default(None)] focused_elevation: Option<Dp>,
        #[default(None)] hovered_elevation: Option<Dp>,
        #[default(OutlinedCardTokens::DraggedContainerElevation)] dragged_elevation: Dp,
        #[default(OutlinedCardTokens::DisabledContainerElevation)] disabled_elevation: Dp,
    ) -> CardElevation {
        CardElevation::new(
            default_elevation,
            pressed_elevation.unwrap_or(default_elevation),
            focused_elevation.unwrap_or(default_elevation),
            hovered_elevation.unwrap_or(default_elevation),
            dragged_elevation,
            disabled_elevation,
        )
    }

    /// A card's colors (Compose's `CardDefaults.cardColors()` and `cardColors(containerColor, …)`): with nothing
    /// given, the theme's (the highest surface container, its content color; disabled, the surface variant at 38%
    /// over it and the content at 38%); else those given over them — `content_color` where not given
    /// `content_color_for(container_color)` (the content color around for an unspecified or a non-scheme
    /// container), `disabled_content_color` the content color at 38%; an unspecified one keeps the theme's.
    ///
    /// Kotlin: `cardColors(containerColor: Color = Color.Unspecified, contentColor: Color = contentColorFor(containerColor), disabledContainerColor: Color = Color.Unspecified, disabledContentColor: Color = contentColor.copy(DisabledAlpha)): CardColors`
    pub fn card_colors(
        &self,
        #[default(None)] container_color: Option<Color>,
        #[default(None)] content_color: Option<Color>,
        #[default(None)] disabled_container_color: Option<Color>,
        #[default(None)] disabled_content_color: Option<Color>,
    ) -> CardColors {
        card_colors_of(
            CardDefaults::default_card_colors(&MaterialTheme::color_scheme()),
            container_color,
            content_color,
            disabled_container_color,
            disabled_content_color,
            |_, content| named_args!(content.copy(alpha = DisabledAlpha)),
        )
    }

    /// An elevated card's colors (Compose's `CardDefaults.elevatedCardColors()` and
    /// `elevatedCardColors(containerColor, …)`): as [`card_colors`](Self::card_colors), over the elevated card's
    /// (the low surface container; disabled, the surface at 38%).
    ///
    /// Kotlin: `elevatedCardColors(containerColor: Color = Color.Unspecified, contentColor: Color = contentColorFor(containerColor), disabledContainerColor: Color = Color.Unspecified, disabledContentColor: Color = contentColor.copy(DisabledAlpha)): CardColors`
    pub fn elevated_card_colors(
        &self,
        #[default(None)] container_color: Option<Color>,
        #[default(None)] content_color: Option<Color>,
        #[default(None)] disabled_container_color: Option<Color>,
        #[default(None)] disabled_content_color: Option<Color>,
    ) -> CardColors {
        card_colors_of(
            CardDefaults::default_elevated_card_colors(&MaterialTheme::color_scheme()),
            container_color,
            content_color,
            disabled_container_color,
            disabled_content_color,
            |_, content| named_args!(content.copy(alpha = DisabledAlpha)),
        )
    }

    /// An outlined card's colors (Compose's `CardDefaults.outlinedCardColors()` and
    /// `outlinedCardColors(containerColor, …)`): as [`card_colors`](Self::card_colors), over the outlined card's
    /// (the surface), `disabled_content_color` where not given `content_color_for(container_color)` at 38% (not
    /// the content color given, as Compose's).
    ///
    /// Kotlin: `outlinedCardColors(containerColor: Color = Color.Unspecified, contentColor: Color = contentColorFor(containerColor), disabledContainerColor: Color = Color.Unspecified, disabledContentColor: Color = contentColorFor(containerColor).copy(DisabledAlpha)): CardColors`
    pub fn outlined_card_colors(
        &self,
        #[default(None)] container_color: Option<Color>,
        #[default(None)] content_color: Option<Color>,
        #[default(None)] disabled_container_color: Option<Color>,
        #[default(None)] disabled_content_color: Option<Color>,
    ) -> CardColors {
        card_colors_of(
            CardDefaults::default_outlined_card_colors(&MaterialTheme::color_scheme()),
            container_color,
            content_color,
            disabled_container_color,
            disabled_content_color,
            |container, _| named_args!(content_color_for(container).copy(alpha = DisabledAlpha)),
        )
    }

    /// An outlined card's border (Compose's `CardDefaults.outlinedCardBorder`): 1 dp of the outline variant; not
    /// `enabled`, the outline at 12% over the low surface container. Remembered while its color stays.
    #[track_caller]
    pub fn outlined_card_border(&self, #[default(true)] enabled: bool) -> BorderStroke {
        self.outlined_card_border_with(enabled)
    }
}

impl CardDefaults {
    /// [`outlined_card_border`](Self::outlined_card_border) with its parameter, for plain code.
    #[track_caller]
    pub fn outlined_card_border_with(&self, enabled: bool) -> BorderStroke {
        let color = if enabled {
            OutlinedCardTokens::OutlineColor.value()
        } else {
            named_args!(
                OutlinedCardTokens::DisabledOutlineColor
                    .value()
                    .copy(alpha = OutlinedCardTokens::DisabledOutlineOpacity)
            )
            .composite_over(ElevatedCardTokens::ContainerColor.value())
        };
        kompozable_block!(remember_keyed(color, || BorderStroke(OutlinedCardTokens::OutlineWidth, color)))
    }
}

// ---- CardElevation -----------------------------------------------------------------------------------------------

/// A card's elevations in its states (Compose's `CardElevation`): the one by default, pressed, focused, hovered,
/// dragged and disabled — the shadow a clickable card animates between as its interactions come and go. Made by
/// [`CardDefaults`]' elevation functions (its constructor is internal, as Compose's). Equal by its elevations but
/// the dragged one (as Compose's `equals`).
#[derive(Clone, Copy, Debug)]
pub struct CardElevation {
    default_elevation: Dp,
    pressed_elevation: Dp,
    focused_elevation: Dp,
    hovered_elevation: Dp,
    dragged_elevation: Dp,
    disabled_elevation: Dp,
}

impl PartialEq for CardElevation {
    fn eq(&self, other: &Self) -> bool {
        self.default_elevation == other.default_elevation
            && self.pressed_elevation == other.pressed_elevation
            && self.focused_elevation == other.focused_elevation
            && self.hovered_elevation == other.hovered_elevation
            && self.disabled_elevation == other.disabled_elevation
    }
}

impl CardElevation {
    /// Compose's internal constructor `CardElevation(defaultElevation, pressedElevation, focusedElevation,
    /// hoveredElevation, draggedElevation, disabledElevation)`.
    pub(crate) fn new(
        default_elevation: Dp,
        pressed_elevation: Dp,
        focused_elevation: Dp,
        hovered_elevation: Dp,
        dragged_elevation: Dp,
        disabled_elevation: Dp,
    ) -> CardElevation {
        CardElevation {
            default_elevation,
            pressed_elevation,
            focused_elevation,
            hovered_elevation,
            dragged_elevation,
            disabled_elevation,
        }
    }

    /// The shadow's elevation as state (Compose's internal composable `shadowElevation`): without an interaction
    /// source (a card that isn't clicked), the default one, remembered; else the disabled one where not
    /// `enabled`, or that of the last of the source's interactions going on (a press, a hover, a focus, a drag),
    /// or the default one, animated to as [`AnimateElevation`] says.
    #[track_caller]
    pub(crate) fn shadow_elevation(&self, enabled: bool, interaction_source: Option<&InteractionSource>) -> State<Dp> {
        let this = *self;
        let interaction_source = interaction_source.cloned();
        kompozable_block!({
            match interaction_source {
                None => State::from(remember(|| mutable_state_of(this.default_elevation))),
                Some(interaction_source) => this.animate_elevation(enabled, interaction_source),
            }
        })
    }

    /// Compose's private `animateElevation`.
    #[track_caller]
    fn animate_elevation(&self, enabled: bool, interaction_source: InteractionSource) -> State<Dp> {
        let this = *self;
        kompozable_block!({
            let interactions = remember(|| mutable_state_list_of(Vec::<Arc<dyn Interaction>>::new()));
            let collected = interactions.clone();
            let collecting = interaction_source.interactions().collect(move |interaction| {
                track_interaction(&collected, interaction);
            });
            LaunchedEffect(interaction_source.clone(), collecting);

            let interaction = interactions.with(|list| list.last().cloned());

            let target = if !enabled {
                this.disabled_elevation
            } else {
                match &interaction {
                    Some(i) if i.is::<PressInteraction::Press>() => this.pressed_elevation,
                    Some(i) if i.is::<HoverInteraction::Enter>() => this.hovered_elevation,
                    Some(i) if i.is::<FocusInteraction::Focus>() => this.focused_elevation,
                    Some(i) if i.is::<DragInteraction::Start>() => this.dragged_elevation,
                    _ => this.default_elevation,
                }
            };

            let animatable: Animatable<Dp, AnimationVector1D> =
                remember(|| Animatable::new(target, Dp::VectorConverter, None, "Animatable"));

            let a = animatable.clone();
            LaunchedEffect(target, async move {
                if a.target_value() != target {
                    if !enabled {
                        // No transition when moving to a disabled state.
                        a.snap_to(target).await;
                    } else {
                        let last_target = a.target_value();
                        let last_interaction: Option<Arc<dyn Interaction>> = if last_target == this.pressed_elevation {
                            Some(Arc::new(PressInteraction::Press::new(Offset::Zero)) as Arc<dyn Interaction>)
                        } else if last_target == this.hovered_elevation {
                            Some(Arc::new(HoverInteraction::Enter::new()) as Arc<dyn Interaction>)
                        } else if last_target == this.focused_elevation {
                            Some(Arc::new(FocusInteraction::Focus::new()) as Arc<dyn Interaction>)
                        } else if last_target == this.dragged_elevation {
                            Some(Arc::new(DragInteraction::Start::new()) as Arc<dyn Interaction>)
                        } else {
                            None
                        };
                        a.animate_elevation(target, last_interaction.as_deref(), interaction.as_deref()).await;
                    }
                }
            });

            animatable.as_state()
        })
    }
}

/// An interaction's part in what goes on (the `when (interaction)` of Compose's elevation classes, a card's and a
/// chip's): a hover, a focus, a press or a drag added, its end taking it away.
pub(crate) fn track_interaction(
    interactions: &SnapshotStateList<Arc<dyn Interaction>>,
    interaction: Arc<dyn Interaction>,
) {
    if interaction.is::<HoverInteraction::Enter>()
        || interaction.is::<FocusInteraction::Focus>()
        || interaction.is::<PressInteraction::Press>()
        || interaction.is::<DragInteraction::Start>()
    {
        interactions.add(interaction);
    } else {
        interactions.update(|list| remove_ended(list, &*interaction));
    }
}

/// Takes the first interaction `end` ends out of `list` (Kotlin's `interactions.remove(interaction.enter)` and the
/// others).
fn remove_ended(list: &mut Vec<Arc<dyn Interaction>>, end: &dyn Interaction) {
    let started = |i: &Arc<dyn Interaction>| -> bool {
        if let Some(exit) = end.downcast_ref::<HoverInteraction::Exit>() {
            i.downcast_ref::<HoverInteraction::Enter>() == Some(&exit.enter)
        } else if let Some(unfocus) = end.downcast_ref::<FocusInteraction::Unfocus>() {
            i.downcast_ref::<FocusInteraction::Focus>() == Some(&unfocus.focus)
        } else if let Some(release) = end.downcast_ref::<PressInteraction::Release>() {
            i.downcast_ref::<PressInteraction::Press>() == Some(&release.press)
        } else if let Some(cancel) = end.downcast_ref::<PressInteraction::Cancel>() {
            i.downcast_ref::<PressInteraction::Press>() == Some(&cancel.press)
        } else if let Some(stop) = end.downcast_ref::<DragInteraction::Stop>() {
            i.downcast_ref::<DragInteraction::Start>() == Some(&stop.start)
        } else if let Some(cancel) = end.downcast_ref::<DragInteraction::Cancel>() {
            i.downcast_ref::<DragInteraction::Start>() == Some(&cancel.start)
        } else {
            false
        }
    };
    if let Some(index) = list.iter().position(started) {
        list.remove(index);
    }
}

// ---- CardColors --------------------------------------------------------------------------------------------------

/// A card's colors (Compose's `CardColors`): its container's and its content's, enabled and disabled. Equal by its
/// colors.
#[derive(Clone, Copy, PartialEq, Debug, KotlinProperties)]
pub struct CardColors {
    /// The container's color, enabled.
    pub container_color: Color,
    /// The content's color, enabled.
    pub content_color: Color,
    /// The container's color, disabled.
    pub disabled_container_color: Color,
    /// The content's color, disabled.
    pub disabled_content_color: Color,
}

impl CardColors {
    /// Colors of one's own (Compose's constructor).
    ///
    /// Kotlin: `CardColors(containerColor: Color, contentColor: Color, disabledContainerColor: Color, disabledContentColor: Color)`
    pub fn new(
        container_color: Color,
        content_color: Color,
        disabled_container_color: Color,
        disabled_content_color: Color,
    ) -> CardColors {
        CardColors { container_color, content_color, disabled_container_color, disabled_content_color }
    }

    /// The container's color, `enabled` or not (Compose's internal `containerColor(enabled)`).
    pub(crate) fn container_color_for(&self, enabled: bool) -> Color {
        if enabled { self.container_color } else { self.disabled_container_color }
    }

    /// The content's color, `enabled` or not (Compose's internal `contentColor(enabled)`).
    pub(crate) fn content_color_for(&self, enabled: bool) -> Color {
        if enabled { self.content_color } else { self.disabled_content_color }
    }
}

#[named]
impl CardColors {
    /// These colors with those given in place of their own (Compose's `copy`): an unspecified one keeps its own.
    pub fn copy(
        &self,
        #[default(Color::Unspecified)] container_color: Color,
        #[default(Color::Unspecified)] content_color: Color,
        #[default(Color::Unspecified)] disabled_container_color: Color,
        #[default(Color::Unspecified)] disabled_content_color: Color,
    ) -> CardColors {
        CardColors::new(
            container_color.take_or_else(|| self.container_color),
            content_color.take_or_else(|| self.content_color),
            disabled_container_color.take_or_else(|| self.disabled_container_color),
            disabled_content_color.take_or_else(|| self.disabled_content_color),
        )
    }
}
