//! Material's segmented buttons, as Compose's SegmentedButton.kt: a row of outlined buttons sharing their outline —
//! picked one at a time in a [`SingleChoiceSegmentedButtonRow`] (each a radio button to accessibility), toggled
//! each on its own in a [`MultiChoiceSegmentedButtonRow`] (each a checkbox) — the rows' scopes making their
//! [`SegmentedButton`](SingleChoiceSegmentedButtonRowScope::SegmentedButton)s; [`SegmentedButtonDefaults`] (the
//! colors, the shapes of the segments by their place in the row, the check mark sliding the label aside) and
//! [`SegmentedButtonColors`].
//!
//! Compose's scopes are interfaces extending `RowScope` (their `SegmentedButton`s extension functions of them); here
//! structs whose composables are methods (KompozR's `RowScope` members are `Modifier`'s own, so a scope carries
//! nothing else). The Kotlin syntax calls `SegmentedButton(…)` inside a row's lambda on its implicit receiver.

use crate::internal::icons::Icons;
use crate::material_theme::MaterialTheme;
use crate::shapes;
use crate::surface::{SelectableSurface, ToggleableSurface};
use crate::text::ProvideTextStyle;
use crate::tokens::{MotionSchemeKeyTokens, OutlinedSegmentedButtonTokens};
use crate::{ButtonDefaults, ColorScheme, VectorIcon};
use kompoz_animation::{
    Animatable, AnimatedVisibility, AnimationSpec, AnimationVector1D, Crossfade, ExitTransition, VectorConverter,
    fade_in, scale_in,
};
use kompoz_coroutines::{CoroutineScope, LaunchedEffect, remember_coroutine_scope};
use kompoz_foundation::interaction::{
    FocusInteraction, Interaction, InteractionSource, MutableInteractionSource, PressInteraction,
};
use kompoz_foundation::layout::{Arrangement, IntrinsicSize, PaddingValues};
use kompoz_foundation::prelude::*;
use kompoz_graphics::{BorderStroke, Color, Constraints, CornerBasedShape, CornerSize, Dp, RectangleShape, Shape};
use kompoz_runtime::{
    KotlinProperties, State, kompozable, kompozable_block, mutable_int_state_of, named, named_args, remember,
};
use kompoz_ui::layout::TransformOrigin;
use kompoz_ui::layout::{Measurable, MeasureResult, MeasureScope, MultiContentLayout, MultiContentMeasurePolicy};
use kompoz_ui::semantics::Role;
use kompoz_ui::{Alignment, Modifier};
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

// ---- The buttons -----------------------------------------------------------------------------------------------

/// The scope of a [`MultiChoiceSegmentedButtonRow`]'s content (Compose's interface `MultiChoiceSegmentedButtonRowScope
/// : RowScope`): where its toggleable [`SegmentedButton`](Self::SegmentedButton)s are made.
#[derive(Clone, Debug)]
pub struct MultiChoiceSegmentedButtonRowScope {
    _private: (),
}

#[named]
impl MultiChoiceSegmentedButtonRowScope {
    /// A toggleable segmented button (Compose's `MultiChoiceSegmentedButtonRowScope.SegmentedButton(checked,
    /// onCheckedChange, …)`), also known as the outlined segmented button: an equal share of the row (its weight 1),
    /// at least 58 by 40 dp (the button's least size), a toggleable [`Surface`](crate::Surface) of `shape` (see
    /// [`SegmentedButtonDefaults::item_shape`]) in its container color by `colors`, outlined by `border` (`None`:
    /// Kotlin's default, [`SegmentedButtonDefaults::border_stroke`] of its border color by `colors`). Its content,
    /// padded by `content_padding`, in its content color and the label large style: `icon` (`None`: Kotlin's default,
    /// [`SegmentedButtonDefaults::Icon`]`(checked)`, the check mark while checked) then `label`, 8 dp apart; with no
    /// icon showing the label slides half of the icon's room back, so it stays centered. `on_checked_change` is called
    /// with the other state when it is clicked, unless not `enabled` (dimmed then, and disabled to accessibility).
    /// Pressed or focused, and checked, it is drawn over its neighbours (its outline whole). Its interactions go to
    /// `interaction_source` (one of its own where none). Composable.
    ///
    /// Kotlin: `SegmentedButton(checked: Boolean, onCheckedChange: (Boolean) -> Unit, shape: Shape, modifier: Modifier = Modifier, enabled: Boolean = true, colors: SegmentedButtonColors = SegmentedButtonDefaults.colors(), border: BorderStroke = SegmentedButtonDefaults.borderStroke(colors.borderColor(enabled, checked)), contentPadding: PaddingValues = SegmentedButtonDefaults.ContentPadding, interactionSource: MutableInteractionSource? = null, icon: @Composable () -> Unit = { SegmentedButtonDefaults.Icon(checked) }, label: @Composable () -> Unit)`
    #[track_caller]
    #[allow(clippy::too_many_arguments)]
    pub fn SegmentedButton(
        &self,
        checked: bool,
        on_checked_change: impl Fn(bool) + 'static,
        shape: Arc<dyn Shape>,
        #[default(Modifier)] modifier: Modifier,
        #[default(true)] enabled: bool,
        #[default(SegmentedButtonDefaults.colors())] colors: SegmentedButtonColors,
        #[default(None)] border: Option<BorderStroke>,
        #[default(SegmentedButtonDefaults::ContentPadding())] content_padding: PaddingValues,
        #[default(None)] interaction_source: Option<MutableInteractionSource>,
        #[default(None)] icon: Option<Rc<dyn Fn()>>,
        label: impl Fn() + 'static,
    ) {
        kompozable_block!({
            ToggleableSegmentedButton(
                checked,
                Rc::new(on_checked_change),
                shape,
                modifier,
                enabled,
                colors,
                border,
                content_padding,
                interaction_source,
                icon,
                Rc::new(label),
            );
        });
    }
}

/// The body of [`MultiChoiceSegmentedButtonRowScope::SegmentedButton`] (a composable of its own: restartable as
/// Compose's).
#[kompozable]
#[allow(clippy::too_many_arguments)]
fn ToggleableSegmentedButton(
    checked: bool,
    on_checked_change: Rc<dyn Fn(bool)>,
    shape: Arc<dyn Shape>,
    modifier: Modifier,
    enabled: bool,
    colors: SegmentedButtonColors,
    border: Option<BorderStroke>,
    content_padding: PaddingValues,
    interaction_source: Option<MutableInteractionSource>,
    icon: Option<Rc<dyn Fn()>>,
    label: Rc<dyn Fn()>,
) {
    let border = border.unwrap_or_else(|| {
        SegmentedButtonDefaults
            .border_stroke(colors.border_color(enabled, checked), SegmentedButtonDefaults::BorderWidth)
    });
    let icon = icon.unwrap_or_else(|| Rc::new(move || SegmentedButtonDefaults.Icon(checked)));
    let interaction_source = interaction_source.unwrap_or_else(|| remember(MutableInteractionSource::new));
    let container_color = colors.container_color(enabled, checked);
    let content_color = colors.content_color(enabled, checked);
    let interaction_count = interaction_count_as_state(interaction_source.clone().into());

    ToggleableSurface(
        checked,
        move |on| on_checked_change(on),
        modifier = interaction_z_index(modifier.weight_with(1.0, true), checked, interaction_count)
            .default_min_size_with(ButtonDefaults::MinWidth, ButtonDefaults::MinHeight()),
        enabled = enabled,
        shape = shape,
        color = container_color,
        content_color = Some(content_color),
        border = Some(border),
        interaction_source = Some(interaction_source),
        content = move || SegmentedButtonContent(icon, label, content_padding),
    );
}

/// The scope of a [`SingleChoiceSegmentedButtonRow`]'s content (Compose's interface
/// `SingleChoiceSegmentedButtonRowScope : RowScope`): where its selectable [`SegmentedButton`](Self::SegmentedButton)s
/// are made.
#[derive(Clone, Debug)]
pub struct SingleChoiceSegmentedButtonRowScope {
    _private: (),
}

#[named]
impl SingleChoiceSegmentedButtonRowScope {
    /// A selectable segmented button (Compose's `SingleChoiceSegmentedButtonRowScope.SegmentedButton(selected,
    /// onClick, …)`), also known as the outlined segmented button: as the toggleable one
    /// ([`MultiChoiceSegmentedButtonRowScope::SegmentedButton`]) but selectable — `on_click` called when it is
    /// clicked — and a radio button to accessibility, its `icon` (`None`: Kotlin's default,
    /// [`SegmentedButtonDefaults::Icon`]`(selected)`) and colors by `selected`. Composable.
    ///
    /// Kotlin: `SegmentedButton(selected: Boolean, onClick: () -> Unit, shape: Shape, modifier: Modifier = Modifier, enabled: Boolean = true, colors: SegmentedButtonColors = SegmentedButtonDefaults.colors(), border: BorderStroke = SegmentedButtonDefaults.borderStroke(colors.borderColor(enabled, selected)), contentPadding: PaddingValues = SegmentedButtonDefaults.ContentPadding, interactionSource: MutableInteractionSource? = null, icon: @Composable () -> Unit = { SegmentedButtonDefaults.Icon(selected) }, label: @Composable () -> Unit)`
    #[track_caller]
    #[allow(clippy::too_many_arguments)]
    pub fn SegmentedButton(
        &self,
        selected: bool,
        on_click: impl Fn() + 'static,
        shape: Arc<dyn Shape>,
        #[default(Modifier)] modifier: Modifier,
        #[default(true)] enabled: bool,
        #[default(SegmentedButtonDefaults.colors())] colors: SegmentedButtonColors,
        #[default(None)] border: Option<BorderStroke>,
        #[default(SegmentedButtonDefaults::ContentPadding())] content_padding: PaddingValues,
        #[default(None)] interaction_source: Option<MutableInteractionSource>,
        #[default(None)] icon: Option<Rc<dyn Fn()>>,
        label: impl Fn() + 'static,
    ) {
        kompozable_block!({
            SelectableSegmentedButton(
                selected,
                Rc::new(on_click),
                shape,
                modifier,
                enabled,
                colors,
                border,
                content_padding,
                interaction_source,
                icon,
                Rc::new(label),
            );
        });
    }
}

/// The body of [`SingleChoiceSegmentedButtonRowScope::SegmentedButton`].
#[kompozable]
#[allow(clippy::too_many_arguments)]
fn SelectableSegmentedButton(
    selected: bool,
    on_click: Rc<dyn Fn()>,
    shape: Arc<dyn Shape>,
    modifier: Modifier,
    enabled: bool,
    colors: SegmentedButtonColors,
    border: Option<BorderStroke>,
    content_padding: PaddingValues,
    interaction_source: Option<MutableInteractionSource>,
    icon: Option<Rc<dyn Fn()>>,
    label: Rc<dyn Fn()>,
) {
    let border = border.unwrap_or_else(|| {
        SegmentedButtonDefaults
            .border_stroke(colors.border_color(enabled, selected), SegmentedButtonDefaults::BorderWidth)
    });
    let icon = icon.unwrap_or_else(|| Rc::new(move || SegmentedButtonDefaults.Icon(selected)));
    let interaction_source = interaction_source.unwrap_or_else(|| remember(MutableInteractionSource::new));
    let container_color = colors.container_color(enabled, selected);
    let content_color = colors.content_color(enabled, selected);
    let interaction_count = interaction_count_as_state(interaction_source.clone().into());

    SelectableSurface(
        selected,
        move || on_click(),
        modifier = interaction_z_index(modifier.weight_with(1.0, true), selected, interaction_count)
            .default_min_size_with(ButtonDefaults::MinWidth, ButtonDefaults::MinHeight())
            .semantics(|s| s.set_role(Role::RadioButton)),
        enabled = enabled,
        shape = shape,
        color = container_color,
        content_color = Some(content_color),
        border = Some(border),
        interaction_source = Some(interaction_source),
        content = move || SegmentedButtonContent(icon, label, content_padding),
    );
}

// ---- The rows --------------------------------------------------------------------------------------------------

/// A row of selectable segmented buttons, one picked at a time (Compose's `SingleChoiceSegmentedButtonRow`): `content`
/// (in a [`SingleChoiceSegmentedButtonRowScope`], its
/// [`SegmentedButton`](SingleChoiceSegmentedButtonRowScope::SegmentedButton)s) in a row as wide as its widest
/// button's share (each as wide as the widest, unless `modifier` sizes it), at least 40 dp tall, its buttons centered
/// across and overlapping by `space` (their outlines one), a selectable group to accessibility.
#[kompozable]
pub fn SingleChoiceSegmentedButtonRow(
    #[default(Modifier)] modifier: Modifier,
    #[default(SegmentedButtonDefaults::BorderWidth)] space: Dp,
    content: impl Fn(&SingleChoiceSegmentedButtonRowScope) + 'static,
) {
    Row(
        modifier = modifier
            .selectable_group()
            .default_min_size_with(Dp::Unspecified, OutlinedSegmentedButtonTokens::ContainerHeight)
            .width_intrinsic(IntrinsicSize::Min),
        horizontal_arrangement = Arrangement::spaced_by(-space),
        vertical_alignment = Alignment::CenterVertically,
        content = move || {
            let scope = remember(|| SingleChoiceSegmentedButtonRowScope { _private: () });
            content(&scope);
        },
    );
}

/// A row of toggleable segmented buttons, each checked on its own (Compose's `MultiChoiceSegmentedButtonRow`): as
/// [`SingleChoiceSegmentedButtonRow`], its `content` in a [`MultiChoiceSegmentedButtonRowScope`], no selectable
/// group.
#[kompozable]
pub fn MultiChoiceSegmentedButtonRow(
    #[default(Modifier)] modifier: Modifier,
    #[default(SegmentedButtonDefaults::BorderWidth)] space: Dp,
    content: impl Fn(&MultiChoiceSegmentedButtonRowScope) + 'static,
) {
    Row(
        modifier = modifier
            .default_min_size_with(Dp::Unspecified, OutlinedSegmentedButtonTokens::ContainerHeight)
            .width_intrinsic(IntrinsicSize::Min),
        horizontal_arrangement = Arrangement::spaced_by(-space),
        vertical_alignment = Alignment::CenterVertically,
        content = move || {
            let scope = remember(|| MultiChoiceSegmentedButtonRowScope { _private: () });
            content(&scope);
        },
    );
}

// ---- The content -----------------------------------------------------------------------------------------------

/// A segmented button's content (Compose's private `SegmentedButtonContent`): `icon` and `content` side by side by
/// [`SegmentedButtonContentMeasurePolicy`], as tall as the taller of them, in the label's style, padded by
/// `content_padding`, centered.
#[kompozable]
fn SegmentedButtonContent(icon: Rc<dyn Fn()>, content: Rc<dyn Fn()>, content_padding: PaddingValues) {
    Box(
        content_alignment = Alignment::Center,
        modifier = Modifier.padding_values(content_padding),
        content = move || {
            let typography = OutlinedSegmentedButtonTokens::LabelTextFont.value();
            // TODO Load the motionScheme tokens from the component tokens file
            let animation_spec = MotionSchemeKeyTokens::FastSpatial.value::<i32>();
            let (icon, content) = (icon.clone(), content.clone());
            ProvideTextStyle(typography, move || {
                let scope = remember_coroutine_scope();
                let animation_spec = animation_spec.clone();
                let measure_policy: Rc<dyn MultiContentMeasurePolicy> = remember(move || {
                    Rc::new(SegmentedButtonContentMeasurePolicy::new(scope, animation_spec))
                        as Rc<dyn MultiContentMeasurePolicy>
                });
                MultiContentLayout(
                    vec![icon.clone(), content.clone()],
                    modifier = Modifier.height_intrinsic(IntrinsicSize::Min),
                    measure_policy = measure_policy,
                );
            });
        },
    );
}

/// How a segmented button's icon and label are laid out (Compose's internal `SegmentedButtonContentMeasurePolicy`):
/// the icon at the start, centered across, the label after the icon's room (at least the icon size) and 8 dp; with
/// no icon showing (none of any width) the label is half of that room back, so it is centered. The label's moves
/// from then on (an icon showing or going) are animated along `animation_spec` in `scope`; the first place is
/// taken at once.
pub(crate) struct SegmentedButtonContentMeasurePolicy {
    scope: CoroutineScope,
    animation_spec: Arc<dyn AnimationSpec<i32>>,
    animatable: RefCell<Option<Animatable<i32, AnimationVector1D>>>,
    initial_offset: Cell<Option<i32>>,
}

impl SegmentedButtonContentMeasurePolicy {
    pub(crate) fn new(
        scope: CoroutineScope,
        animation_spec: Arc<dyn AnimationSpec<i32>>,
    ) -> SegmentedButtonContentMeasurePolicy {
        SegmentedButtonContentMeasurePolicy {
            scope,
            animation_spec,
            animatable: RefCell::new(None),
            initial_offset: Cell::new(None),
        }
    }
}

impl MultiContentMeasurePolicy for SegmentedButtonContentMeasurePolicy {
    fn measure(
        &self,
        scope: &MeasureScope,
        measurables: &[Vec<&dyn Measurable>],
        constraints: Constraints,
    ) -> MeasureResult {
        let (icon_measurables, content_measurables) = (&measurables[0], &measurables[1]);
        let icon_placeables: Vec<_> = icon_measurables.iter().map(|m| m.measure(constraints)).collect();
        let icon_width = icon_placeables.iter().map(|p| p.width()).max().unwrap_or(0);
        let content_placeables: Vec<_> = content_measurables.iter().map(|m| m.measure(constraints)).collect();
        let content_width = content_placeables.iter().map(|p| p.width()).max();
        let height = content_placeables.iter().map(|p| p.height()).max().unwrap_or(0);
        let icon_size = scope.round_to_px(SegmentedButtonDefaults::IconSize);
        let icon_spacing = scope.round_to_px(IconSpacing);
        let width = icon_size.max(icon_width) + icon_spacing + content_width.unwrap_or(0);
        let offset_x = if icon_width == 0 { -(icon_size + icon_spacing) / 2 } else { 0 };

        match self.initial_offset.get() {
            None => self.initial_offset.set(Some(offset_x)),
            Some(initial_offset) => {
                let anim = self
                    .animatable
                    .borrow_mut()
                    .get_or_insert_with(|| Animatable((initial_offset, i32::VectorConverter)))
                    .clone();
                if anim.target_value() != offset_x {
                    let spec = self.animation_spec.clone();
                    named_args!(self.scope.launch(async move {
                        named_args!(anim.animate_to(offset_x, animation_spec = spec)).await;
                    }));
                }
            }
        }

        let animatable = self.animatable.borrow().clone();
        scope.layout(width, height, move |p| {
            for placeable in &icon_placeables {
                p.place(placeable, 0, (height - placeable.height()) / 2);
            }

            let content_offset_x = icon_size + icon_spacing + animatable.as_ref().map_or(offset_x, |a| a.value());

            for placeable in &content_placeables {
                p.place(placeable, content_offset_x, (height - placeable.height()) / 2);
            }
        })
    }
}

/// How many presses and focuses `source` has going on (Compose's private `InteractionSource.interactionCountAsState`):
/// one added as one starts, taken away as one ends (released, unfocused, cancelled).
#[kompozable]
fn interaction_count_as_state(source: InteractionSource) -> State<i32> {
    let interaction_count = remember(|| mutable_int_state_of(0));
    let count = interaction_count.clone();
    let collecting = source.interactions().collect(move |interaction: Arc<dyn Interaction>| {
        if interaction.is::<PressInteraction::Press>() || interaction.is::<FocusInteraction::Focus>() {
            count.set(count.peek() + 1);
        } else if interaction.is::<PressInteraction::Release>()
            || interaction.is::<FocusInteraction::Unfocus>()
            || interaction.is::<PressInteraction::Cancel>()
        {
            count.set(count.peek() - 1);
        }
    });
    LaunchedEffect(source, collecting);

    interaction_count.into()
}

/// `modifier` placing what it wraps above its neighbours by how many interactions go on in it, and more while
/// `checked` (Compose's private `Modifier.interactionZIndex`): a pressed or focused segment's outline, and the checked
/// one's, drawn whole.
fn interaction_z_index(modifier: Modifier, checked: bool, interaction_count: State<i32>) -> Modifier {
    modifier.layout(move |scope, measurable, constraints| {
        let placeable = measurable.measure(constraints);
        let interaction_count = interaction_count.clone();
        scope.layout(placeable.width(), placeable.height(), move |p| {
            let z_index = interaction_count.value() as f32 + if checked { CheckedZIndexFactor } else { 0.0 };
            named_args!(p.place(&placeable, 0, 0, z_index = z_index));
        })
    })
}

const CheckedZIndexFactor: f32 = 5.0;
const IconSpacing: Dp = Dp(8.0);

// ---- Defaults --------------------------------------------------------------------------------------------------

/// The segmented buttons' defaults (Compose's `SegmentedButtonDefaults`): their colors, the shapes of the segments
/// by their place in a row, sizes, the padding, the icon. Plain values are associated consts (or functions making a
/// value); what Compose reads in composition or computes from arguments are methods of the value
/// (`SegmentedButtonDefaults.colors()`).
#[derive(Clone, Copy, Debug)]
pub struct SegmentedButtonDefaults;

impl SegmentedButtonDefaults {
    /// The width of a segmented button's outline: 1 dp (Compose's `SegmentedButtonDefaults.BorderWidth`), also how
    /// much the segments of a row overlap.
    pub const BorderWidth: Dp = OutlinedSegmentedButtonTokens::OutlineWidth;

    /// The size of a segmented button's icon: 18 dp (Compose's `SegmentedButtonDefaults.IconSize`).
    pub const IconSize: Dp = OutlinedSegmentedButtonTokens::IconSize;

    /// The shape the segments' shapes are cut from: fully rounded (Compose's composable
    /// `SegmentedButtonDefaults.baseShape`; the theme's shapes read).
    pub fn base_shape() -> CornerBasedShape {
        let shape = OutlinedSegmentedButtonTokens::Shape.value();
        crate::internal::animated_shape::as_corner_based_shape(&shape)
            .expect("the segmented button's shape token is a corner based shape")
    }

    /// The padding of a segmented button's content: 12 dp at the sides, the button's above and below (Compose's
    /// `SegmentedButtonDefaults.ContentPadding`).
    pub fn ContentPadding() -> PaddingValues {
        let button = ButtonDefaults::ContentPadding();
        PaddingValues::new(Dp(12.0), button.calculate_top_padding(), Dp(12.0), button.calculate_bottom_padding())
    }

    /// The theme's segmented button colors (Compose's internal `ColorScheme.defaultSegmentedButtonColors`; made
    /// afresh, no cache): an active segment in the secondary container color and its content color, an inactive one
    /// transparent with on-surface content, both outlined in the outline color; disabled, the content and outline
    /// faded.
    pub(crate) fn default_segmented_button_colors(scheme: &ColorScheme) -> SegmentedButtonColors {
        use OutlinedSegmentedButtonTokens as T;
        let faded = |token, alpha: f32| -> Color { named_args!(scheme.from_token(token).copy(alpha = alpha)) };
        SegmentedButtonColors::new(
            scheme.from_token(T::SelectedContainerColor),
            scheme.from_token(T::SelectedLabelTextColor),
            scheme.from_token(T::OutlineColor),
            Color::Transparent,
            scheme.from_token(T::UnselectedLabelTextColor),
            scheme.from_token(T::OutlineColor),
            scheme.from_token(T::SelectedContainerColor),
            faded(T::DisabledLabelTextColor, T::DisabledLabelTextOpacity),
            faded(T::OutlineColor, T::DisabledOutlineOpacity),
            Color::Transparent,
            faded(T::DisabledLabelTextColor, T::DisabledLabelTextOpacity),
            faded(T::OutlineColor, T::DisabledOutlineOpacity),
        )
    }
}

#[named]
impl SegmentedButtonDefaults {
    /// The segmented button colors, each given one other than unspecified in place of the theme's (Compose's
    /// `colors()` and `colors(activeContainerColor, …)`).
    #[allow(clippy::too_many_arguments)]
    pub fn colors(
        &self,
        #[default(Color::Unspecified)] active_container_color: Color,
        #[default(Color::Unspecified)] active_content_color: Color,
        #[default(Color::Unspecified)] active_border_color: Color,
        #[default(Color::Unspecified)] inactive_container_color: Color,
        #[default(Color::Unspecified)] inactive_content_color: Color,
        #[default(Color::Unspecified)] inactive_border_color: Color,
        #[default(Color::Unspecified)] disabled_active_container_color: Color,
        #[default(Color::Unspecified)] disabled_active_content_color: Color,
        #[default(Color::Unspecified)] disabled_active_border_color: Color,
        #[default(Color::Unspecified)] disabled_inactive_container_color: Color,
        #[default(Color::Unspecified)] disabled_inactive_content_color: Color,
        #[default(Color::Unspecified)] disabled_inactive_border_color: Color,
    ) -> SegmentedButtonColors {
        named_args!(Self::default_segmented_button_colors(&MaterialTheme::color_scheme()).copy(
            active_container_color = active_container_color,
            active_content_color = active_content_color,
            active_border_color = active_border_color,
            inactive_container_color = inactive_container_color,
            inactive_content_color = inactive_content_color,
            inactive_border_color = inactive_border_color,
            disabled_active_container_color = disabled_active_container_color,
            disabled_active_content_color = disabled_active_content_color,
            disabled_active_border_color = disabled_active_border_color,
            disabled_inactive_container_color = disabled_inactive_container_color,
            disabled_inactive_content_color = disabled_inactive_content_color,
            disabled_inactive_border_color = disabled_inactive_border_color,
        ))
    }

    /// The shape of the segment at `index` of `count` in a row (Compose's `SegmentedButtonDefaults.itemShape`):
    /// `base_shape` alone, its start corners for the first, its end corners for the last, square in between.
    pub fn item_shape(
        &self,
        index: i32,
        count: i32,
        #[default(SegmentedButtonDefaults::base_shape())] base_shape: CornerBasedShape,
    ) -> Arc<dyn Shape> {
        if count == 1 {
            return Arc::new(base_shape);
        }

        if index == 0 {
            Arc::new(shapes::start(&base_shape, CornerSize::dp(Dp(0.0))))
        } else if index == count - 1 {
            Arc::new(shapes::end(&base_shape, CornerSize::dp(Dp(0.0))))
        } else {
            Arc::new(RectangleShape)
        }
    }

    /// The check mark of an active segment (Compose's `SegmentedButtonDefaults.ActiveIcon`): `Icons.Filled.Check`,
    /// [`IconSize`](Self::IconSize), no description. Composable.
    #[track_caller]
    pub fn ActiveIcon(&self) {
        kompozable_block!({
            VectorIcon(Icons::Filled::Check(), None, modifier = Modifier.size(Self::IconSize));
        });
    }

    /// A segmented button's icon (Compose's `SegmentedButtonDefaults.Icon`): without `inactive_content`,
    /// `active_content` (the check mark by default) while `active`, coming in faded in and growing out of its bottom
    /// start, gone at once; with it, the two crossfaded as `active` changes. Composable.
    #[track_caller]
    pub fn Icon(
        &self,
        active: bool,
        #[default(Rc::new(|| SegmentedButtonDefaults.ActiveIcon()))] active_content: Rc<dyn Fn()>,
        #[default(None)] inactive_content: Option<Rc<dyn Fn()>>,
    ) {
        kompozable_block!(match inactive_content {
            None => {
                // TODO Load the motionScheme tokens from the component tokens file
                AnimatedVisibility(
                    active,
                    exit = ExitTransition::None,
                    enter = fade_in(MotionSchemeKeyTokens::DefaultEffects.value(), 0.0)
                        + named_args!(scale_in(
                            initial_scale = 0.0,
                            transform_origin = TransformOrigin::new(0.0, 1.0),
                            animation_spec = MotionSchemeKeyTokens::FastSpatial.value(),
                        )),
                    content = move |_| active_content(),
                );
            }
            Some(inactive_content) => {
                Crossfade(
                    active,
                    // TODO Load the motionScheme tokens from the component tokens file
                    animation_spec = MotionSchemeKeyTokens::DefaultEffects.value(),
                    content = move |it: bool| if it { active_content() } else { inactive_content() },
                );
            }
        });
    }

    /// A segmented button's outline: `width` (1 dp by default) of `color` (Compose's
    /// `SegmentedButtonDefaults.borderStroke`).
    pub fn border_stroke(
        &self,
        color: Color,
        #[default(SegmentedButtonDefaults::BorderWidth)] width: Dp,
    ) -> BorderStroke {
        BorderStroke(width, color)
    }
}

// ---- Colors ----------------------------------------------------------------------------------------------------

/// A segmented button's colors (Compose's `SegmentedButtonColors`): its container's, its content's and its
/// outline's, active (checked or selected) or not, enabled or not. Equal by its colors.
#[derive(Clone, Copy, PartialEq, Debug, KotlinProperties)]
pub struct SegmentedButtonColors {
    // enabled & active
    pub active_container_color: Color,
    pub active_content_color: Color,
    pub active_border_color: Color,
    // enabled & inactive
    pub inactive_container_color: Color,
    pub inactive_content_color: Color,
    pub inactive_border_color: Color,
    // disable & active
    pub disabled_active_container_color: Color,
    pub disabled_active_content_color: Color,
    pub disabled_active_border_color: Color,
    // disable & inactive
    pub disabled_inactive_container_color: Color,
    pub disabled_inactive_content_color: Color,
    pub disabled_inactive_border_color: Color,
}

impl SegmentedButtonColors {
    /// Kotlin: `SegmentedButtonColors(activeContainerColor: Color, activeContentColor: Color, activeBorderColor: Color, inactiveContainerColor: Color, inactiveContentColor: Color, inactiveBorderColor: Color, disabledActiveContainerColor: Color, disabledActiveContentColor: Color, disabledActiveBorderColor: Color, disabledInactiveContainerColor: Color, disabledInactiveContentColor: Color, disabledInactiveBorderColor: Color)`
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        active_container_color: Color,
        active_content_color: Color,
        active_border_color: Color,
        inactive_container_color: Color,
        inactive_content_color: Color,
        inactive_border_color: Color,
        disabled_active_container_color: Color,
        disabled_active_content_color: Color,
        disabled_active_border_color: Color,
        disabled_inactive_container_color: Color,
        disabled_inactive_content_color: Color,
        disabled_inactive_border_color: Color,
    ) -> SegmentedButtonColors {
        SegmentedButtonColors {
            active_container_color,
            active_content_color,
            active_border_color,
            inactive_container_color,
            inactive_content_color,
            inactive_border_color,
            disabled_active_container_color,
            disabled_active_content_color,
            disabled_active_border_color,
            disabled_inactive_container_color,
            disabled_inactive_content_color,
            disabled_inactive_border_color,
        }
    }

    /// The outline's color, `enabled` and `active` or not (Compose's internal `borderColor`).
    pub(crate) fn border_color(&self, enabled: bool, active: bool) -> Color {
        match (enabled, active) {
            (true, true) => self.active_border_color,
            (true, false) => self.inactive_border_color,
            (false, true) => self.disabled_active_border_color,
            (false, false) => self.disabled_inactive_border_color,
        }
    }

    /// The content's color, `enabled` and `checked` or not (Compose's internal `contentColor`).
    pub(crate) fn content_color(&self, enabled: bool, checked: bool) -> Color {
        match (enabled, checked) {
            (true, true) => self.active_content_color,
            (true, false) => self.inactive_content_color,
            (false, true) => self.disabled_active_content_color,
            (false, false) => self.disabled_inactive_content_color,
        }
    }

    /// The container's color, `enabled` and `active` or not (Compose's internal `containerColor`).
    pub(crate) fn container_color(&self, enabled: bool, active: bool) -> Color {
        match (enabled, active) {
            (true, true) => self.active_container_color,
            (true, false) => self.inactive_container_color,
            (false, true) => self.disabled_active_container_color,
            (false, false) => self.disabled_inactive_container_color,
        }
    }
}

#[named]
impl SegmentedButtonColors {
    /// The same with each color given one other than unspecified (Compose's `copy`).
    #[allow(clippy::too_many_arguments)]
    pub fn copy(
        &self,
        #[default(self.active_container_color)] active_container_color: Color,
        #[default(self.active_content_color)] active_content_color: Color,
        #[default(self.active_border_color)] active_border_color: Color,
        #[default(self.inactive_container_color)] inactive_container_color: Color,
        #[default(self.inactive_content_color)] inactive_content_color: Color,
        #[default(self.inactive_border_color)] inactive_border_color: Color,
        #[default(self.disabled_active_container_color)] disabled_active_container_color: Color,
        #[default(self.disabled_active_content_color)] disabled_active_content_color: Color,
        #[default(self.disabled_active_border_color)] disabled_active_border_color: Color,
        #[default(self.disabled_inactive_container_color)] disabled_inactive_container_color: Color,
        #[default(self.disabled_inactive_content_color)] disabled_inactive_content_color: Color,
        #[default(self.disabled_inactive_border_color)] disabled_inactive_border_color: Color,
    ) -> SegmentedButtonColors {
        SegmentedButtonColors::new(
            active_container_color.take_or_else(|| self.active_container_color),
            active_content_color.take_or_else(|| self.active_content_color),
            active_border_color.take_or_else(|| self.active_border_color),
            inactive_container_color.take_or_else(|| self.inactive_container_color),
            inactive_content_color.take_or_else(|| self.inactive_content_color),
            inactive_border_color.take_or_else(|| self.inactive_border_color),
            disabled_active_container_color.take_or_else(|| self.disabled_active_container_color),
            disabled_active_content_color.take_or_else(|| self.disabled_active_content_color),
            disabled_active_border_color.take_or_else(|| self.disabled_active_border_color),
            disabled_inactive_container_color.take_or_else(|| self.disabled_inactive_container_color),
            disabled_inactive_content_color.take_or_else(|| self.disabled_inactive_content_color),
            disabled_inactive_border_color.take_or_else(|| self.disabled_inactive_border_color),
        )
    }
}
