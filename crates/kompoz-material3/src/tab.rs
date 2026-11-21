//! Material's tabs, as Compose's Tab.kt: one choice of a tab row ([`PrimaryTabRow`](crate::PrimaryTabRow) and the
//! others) — a selectable column of content ([`Tab`]), a text and/or an icon laid out by their baselines
//! ([`TextIconTab`], Kotlin's `Tab(selected, onClick, text, icon)`), or an icon before its text ([`LeadingIconTab`]).
//! A tab's content takes the selected or the unselected content color, faded between them as it turns
//! (`TabTransition`); its ripple is always the selected color.

use crate::badge::badge_bounds;
use crate::content_color::LocalContentColor;
use crate::ripples::ripple;
use crate::text::ProvideTextStyle;
use crate::tokens::{MotionSchemeKeyTokens, PrimaryNavigationTabTokens};
use kompoz_animation::{FiniteAnimationSpec, Segment, update_transition};
use kompoz_foundation::interaction::MutableInteractionSource;
use kompoz_foundation::prelude::*;
use kompoz_graphics::{Color, Density, Dp, TextUnit, TextUnitType};
use kompoz_runtime::{KompozitionLocalProvider, kompozable, named_args};
use kompoz_text::TextAlign;
use kompoz_ui::layout::{FirstBaseline, LastBaseline, Layout, Placeable, PlacementScope, measure_policy};
use kompoz_ui::semantics::Role;
use kompoz_ui::{Alignment, Modifier};
use std::rc::Rc;
use std::sync::Arc;

/// A tab of a text and/or an icon (Compose's `Tab(selected, onClick, modifier, enabled, text, icon, …)`): a [`Tab`]
/// holding `text` (in the tab label's style, centered, 16 dp in from each side) over `icon`, laid out by the text's
/// baselines as Material says (48 dp tall with one of them, 72 with both). It gives a badge the bounds it keeps in
/// (a [`BadgedBox`](crate::BadgedBox) inside it stays within the tab). `unselected_content_color` None is Kotlin's
/// default, `selected_content_color` (by default the content color around).
///
/// Kotlin: `Tab(selected: Boolean, onClick: () -> Unit, modifier: Modifier = Modifier, enabled: Boolean = true, text: @Composable (() -> Unit)? = null, icon: @Composable (() -> Unit)? = null)`
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn TextIconTab(
    selected: bool,
    on_click: impl Fn() + 'static,
    #[default(Modifier)] modifier: Modifier,
    #[default(true)] enabled: bool,
    #[default(None)] text: Option<Rc<dyn Fn()>>,
    #[default(None)] icon: Option<Rc<dyn Fn()>>,
    #[default(LocalContentColor.current())] selected_content_color: Color,
    #[default(None)] unselected_content_color: Option<Color>,
    #[default(None)] interaction_source: Option<MutableInteractionSource>,
) {
    let styled_text: Option<Rc<dyn Fn()>> = text.map(|text| {
        Rc::new(move || {
            let style =
                named_args!(PrimaryNavigationTabTokens::LabelTextFont.value().copy(text_align = TextAlign::Center));
            let text = text.clone();
            ProvideTextStyle(style, move || text());
        }) as Rc<dyn Fn()>
    });
    Tab(
        selected,
        on_click,
        modifier = badge_bounds(modifier),
        enabled = enabled,
        selected_content_color = selected_content_color,
        unselected_content_color = unselected_content_color,
        interaction_source = interaction_source,
        content = move || TabBaselineLayout(styled_text.clone(), icon.clone()),
    );
}

/// A tab of an icon before its text (Compose's `LeadingIconTab`): `icon`, 8 dp, then `text` (in the tab label's
/// style) in a row centered in the tab, 48 dp tall, 16 dp in from each side; selected and colored as a [`Tab`].
/// `unselected_content_color` None is Kotlin's default, `selected_content_color`.
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn LeadingIconTab(
    selected: bool,
    on_click: impl Fn() + 'static,
    text: impl Fn() + 'static,
    icon: impl Fn() + 'static,
    #[default(Modifier)] modifier: Modifier,
    #[default(true)] enabled: bool,
    #[default(LocalContentColor.current())] selected_content_color: Color,
    #[default(None)] unselected_content_color: Option<Color>,
    #[default(None)] interaction_source: Option<MutableInteractionSource>,
) {
    let unselected_content_color = unselected_content_color.unwrap_or(selected_content_color);
    // The color of the Ripple should always the be selected color, as we want to show the color
    // before the item is considered selected, and hence before the new contentColor is
    // provided by TabTransition.
    let ripple = ripple(true, Dp::Unspecified, selected_content_color);
    let on_click: Rc<dyn Fn()> = Rc::new(on_click);
    let (text, icon): (Rc<dyn Fn()>, Rc<dyn Fn()>) = (Rc::new(text), Rc::new(icon));

    TabTransition(selected_content_color, unselected_content_color, selected, move || {
        let (text, icon, on_click) = (text.clone(), icon.clone(), on_click.clone());
        Row(
            modifier = modifier
                .clone()
                .height(SmallTabHeight)
                .selectable_indication_with(
                    selected,
                    interaction_source.clone(),
                    Some(ripple.clone()),
                    enabled,
                    Some(Role::Tab),
                    move || on_click(),
                )
                .padding_hv_with(HorizontalTextPadding, Dp(0.0))
                .fill_max_width(),
            horizontal_arrangement = Arrangement::Center,
            vertical_alignment = Alignment::CenterVertically,
            content = move || {
                icon();
                Spacer(Modifier.required_width(TextDistanceFromLeadingIcon));
                let style =
                    named_args!(PrimaryNavigationTabTokens::LabelTextFont.value().copy(text_align = TextAlign::Center));
                let text = text.clone();
                ProvideTextStyle(style, move || text());
            },
        );
    });
}

/// A tab of any content (Compose's `Tab(selected, onClick, …, content)`): `content` in a column centered both ways,
/// the whole tab selectable (the Tab role) calling `on_click` (not when `enabled` is false), with a bounded ripple of
/// `selected_content_color`; its content in `selected_content_color` while `selected`, else in
/// `unselected_content_color` (None is Kotlin's default, `selected_content_color`), faded between them as it turns.
/// Its interactions go to `interaction_source` (one of its own where none).
///
/// Kotlin: `Tab(selected: Boolean, onClick: () -> Unit, modifier: Modifier = Modifier, enabled: Boolean = true, selectedContentColor: Color = LocalContentColor.current, unselectedContentColor: Color = selectedContentColor, interactionSource: MutableInteractionSource? = null, content: @Composable ColumnScope.() -> Unit)`
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn Tab(
    selected: bool,
    on_click: impl Fn() + 'static,
    #[default(Modifier)] modifier: Modifier,
    #[default(true)] enabled: bool,
    #[default(LocalContentColor.current())] selected_content_color: Color,
    #[default(None)] unselected_content_color: Option<Color>,
    #[default(None)] interaction_source: Option<MutableInteractionSource>,
    content: impl Fn() + 'static,
) {
    let unselected_content_color = unselected_content_color.unwrap_or(selected_content_color);
    // The color of the Ripple should always the selected color, as we want to show the color
    // before the item is considered selected, and hence before the new contentColor is
    // provided by TabTransition.
    let ripple = ripple(true, Dp::Unspecified, selected_content_color);
    let on_click: Rc<dyn Fn()> = Rc::new(on_click);
    let content: Rc<dyn Fn()> = Rc::new(content);

    TabTransition(selected_content_color, unselected_content_color, selected, move || {
        let (content, on_click) = (content.clone(), on_click.clone());
        Column(
            modifier = modifier
                .clone()
                .selectable_indication_with(
                    selected,
                    interaction_source.clone(),
                    Some(ripple.clone()),
                    enabled,
                    Some(Role::Tab),
                    move || on_click(),
                )
                .fill_max_width(),
            horizontal_alignment = Alignment::CenterHorizontally,
            vertical_arrangement = Arrangement::Center,
            content = move || content(),
        );
    });
}

/// The content color of a tab, faded from one color to the other as it turns (Compose's private
/// `TabTransition`): in on the default effects spec, out on the fast one.
#[kompozable]
fn TabTransition(active_color: Color, inactive_color: Color, selected: bool, content: impl Fn() + 'static) {
    let transition = update_transition(selected, None);
    // TODO Load the motionScheme tokens from the component tokens file
    let color = transition.animate_color(
        move |segment: &Segment<bool>| -> Arc<dyn FiniteAnimationSpec<Color>> {
            if segment.is_transitioning_to(false, true) {
                // Fade-in
                MotionSchemeKeyTokens::DefaultEffects.value::<Color>()
            } else {
                // Fade-out
                MotionSchemeKeyTokens::FastEffects.value::<Color>()
            }
        },
        "ColorAnimation".to_string(),
        move |it: &bool| if *it { active_color } else { inactive_color },
    );
    KompozitionLocalProvider(&[LocalContentColor.provides(color.value())], content);
}

/// A tab's text and icon laid out by the text's baselines (Compose's private `TabBaselineLayout`): each alone
/// centered in a tab of 48 dp (or what it needs), both in one of 72 dp with the text's last baseline a set distance
/// above the indicator and the icon 20 sp above its first baseline.
#[kompozable]
fn TabBaselineLayout(text: Option<Rc<dyn Fn()>>, icon: Option<Rc<dyn Fn()>>) {
    let (has_text, has_icon) = (text.is_some(), icon.is_some());
    Layout(
        content = move || {
            if let Some(text) = text.clone() {
                Box(
                    modifier = Modifier.layout_id("text").padding_hv_with(HorizontalTextPadding, Dp(0.0)),
                    content = move || text(),
                );
            }
            if let Some(icon) = icon.clone() {
                Box(modifier = Modifier.layout_id("icon"), content = move || icon());
            }
        },
        measure_policy = measure_policy(move |scope, measurables, constraints| {
            let text_placeable = has_text.then(|| {
                measurables
                    .iter()
                    .find(|m| m.layout_id().is_some_and(|id| id.is(&"text")))
                    .expect("TabBaselineLayout: its text")
                    .measure(
                        // Measure with loose constraints for height as we don't want the text to
                        // take up more
                        // space than it needs
                        named_args!(constraints.copy(min_height = 0)),
                    )
            });

            let icon_placeable = has_icon.then(|| {
                measurables
                    .iter()
                    .find(|m| m.layout_id().is_some_and(|id| id.is(&"icon")))
                    .expect("TabBaselineLayout: its icon")
                    .measure(constraints)
            });

            let tab_width =
                text_placeable.as_ref().map_or(0, |p| p.width()).max(icon_placeable.as_ref().map_or(0, |p| p.width()));

            let spec_height = scope.round_to_px(if text_placeable.is_some() && icon_placeable.is_some() {
                LargeTabHeight
            } else {
                SmallTabHeight
            });

            let tab_height = spec_height.max(
                icon_placeable.as_ref().map_or(0, |p| p.height())
                    + text_placeable.as_ref().map_or(0, |p| p.height())
                    + round_sp_to_px(scope, IconDistanceFromBaseline),
            );

            let first_baseline = text_placeable.as_ref().map(|p| p.get(FirstBaseline));
            let last_baseline = text_placeable.as_ref().map(|p| p.get(LastBaseline));
            let density: Density = **scope;

            scope.layout(tab_width, tab_height, move |p| match (&text_placeable, &icon_placeable) {
                (Some(text_placeable), Some(icon_placeable)) => place_text_and_icon(
                    p,
                    &density,
                    text_placeable,
                    icon_placeable,
                    tab_width,
                    tab_height,
                    first_baseline.expect("a text's first baseline"),
                    last_baseline.expect("a text's last baseline"),
                ),
                (Some(text_placeable), None) => place_text_or_icon(p, text_placeable, tab_height),
                (None, Some(icon_placeable)) => place_text_or_icon(p, icon_placeable, tab_height),
                (None, None) => {}
            })
        }),
    );
}

/// `sp` rounded to whole pixels (Compose's `TextUnit.roundToPx()`).
fn round_sp_to_px(density: &Density, sp: TextUnit) -> i32 {
    density.round_to_px(density.text_to_dp(sp))
}

/// A text or an icon alone, centered in the tab's height (Compose's private `placeTextOrIcon`).
fn place_text_or_icon(p: &PlacementScope, text_or_icon_placeable: &Placeable, tab_height: i32) {
    let content_y = (tab_height - text_or_icon_placeable.height()) / 2;
    p.place_relative(text_or_icon_placeable, 0, content_y);
}

/// A text and an icon (Compose's private `placeTextAndIcon`): the text's last baseline a set distance above the
/// indicator (14 dp for one line, 6 for two), the icon above the text by 20 sp to its first baseline; both centered.
#[allow(clippy::too_many_arguments)]
fn place_text_and_icon(
    p: &PlacementScope,
    density: &Density,
    text_placeable: &Placeable,
    icon_placeable: &Placeable,
    tab_width: i32,
    tab_height: i32,
    first_baseline: i32,
    last_baseline: i32,
) {
    let baseline_offset =
        if first_baseline == last_baseline { SingleLineTextBaselineWithIcon } else { DoubleLineTextBaselineWithIcon };

    // Total offset between the last text baseline and the bottom of the Tab layout
    let text_offset =
        density.round_to_px(baseline_offset) + density.round_to_px(PrimaryNavigationTabTokens::ActiveIndicatorHeight);

    // How much space there is between the top of the icon (essentially the top of this layout)
    // and the top of the text layout's bounding box (not baseline)
    let icon_offset = icon_placeable.height() + round_sp_to_px(density, IconDistanceFromBaseline) - first_baseline;

    let text_placeable_x = (tab_width - text_placeable.width()) / 2;
    let text_placeable_y = tab_height - last_baseline - text_offset;
    p.place_relative(text_placeable, text_placeable_x, text_placeable_y);

    let icon_placeable_x = (tab_width - icon_placeable.width()) / 2;
    let icon_placeable_y = text_placeable_y - icon_offset;
    p.place_relative(icon_placeable, icon_placeable_x, icon_placeable_y);
}

// Tab specifications
const SmallTabHeight: Dp = PrimaryNavigationTabTokens::ContainerHeight;
const LargeTabHeight: Dp = Dp(72.0);

// The horizontal padding on the left and right of text
pub(crate) const HorizontalTextPadding: Dp = Dp(16.0);

// Distance from the top of the indicator to the text baseline when there is one line of text and an
// icon
const SingleLineTextBaselineWithIcon: Dp = Dp(14.0);

// Distance from the top of the indicator to the last text baseline when there are two lines of text
// and an icon
const DoubleLineTextBaselineWithIcon: Dp = Dp(6.0);

// Distance from the first text baseline to the bottom of the icon in a combined tab
const IconDistanceFromBaseline: TextUnit = TextUnit::new(20.0, TextUnitType::Sp);

// Distance from the end of the leading icon to the start of the text
const TextDistanceFromLeadingIcon: Dp = Dp(8.0);
