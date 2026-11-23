//! Material's list items, as Compose's ListItem.kt: a row of a list — its headline between leading content (an
//! icon, an avatar) and trailing content (meta text, an icon, a switch), an overline above and supporting content
//! below it — one, two or three lines tall (56, 72 or 88 dp).
//!
//! Compose's overloads are composables of their own here: [`ListItem`] (the classic one, no interaction),
//! [`ClickableListItem`] (Kotlin's Expressive `ListItem(onClick, …)`), [`SelectableListItem`] (`ListItem(selected,
//! onClick, …)`, a radio button to accessibility) and [`ToggleableListItem`] (`ListItem(checked, onCheckedChange,
//! …)`, a checkbox), and the segmented items of a card-like group, [`SegmentedListItem`] (Kotlin's
//! `SegmentedListItem(onClick, shapes, …)`), [`SelectableSegmentedListItem`] and [`ToggleableSegmentedListItem`].
//! The interactive ones show the ripple, morph their shape as they are pressed, selected, focused, hovered or
//! dragged ([`ListItemShapes`]), animate their colors between states ([`ListItemColors`]) and raise their shadow
//! while dragged ([`ListItemElevation`]).

use crate::content_color::LocalContentColor;
use crate::interactive_component_size::{InteractiveComponentSizeModifiers, LocalMinimumInteractiveComponentSize};
use crate::internal::float_producer::FloatProducer;
use crate::internal::layout_util::{PlaceableOrZero, SubtractConstraintSafely};
use crate::internal::provide_content_color_text_style::{
    ProvideContentColorTextStyle, ProvideContentColorTextStyleWithValues,
};
use crate::internal::shape_util::ClipShape;
use crate::list_item_defaults::{
    ListItemColors, ListItemDefaults, ListItemElevation, ListItemShapes, shape_for_interaction,
};
use crate::ripples::ripple;
use crate::surface::Surface;
use crate::tokens::{ListTokens, MotionSchemeKeyTokens, TypographyKeyTokens};
use kompoz_animation::{Segment, animate_dp_as_state, update_transition};
use kompoz_coroutines::LaunchedEffect;
use kompoz_foundation::interaction::{
    DragInteraction, FocusInteraction, HoverInteraction, MutableInteractionSource, PressInteraction,
};
use kompoz_foundation::prelude::*;
use kompoz_graphics::{Color, Constraints, Dp, UnitExt};
use kompoz_runtime::{KompozitionLocalProvider, MutableState, kompozable, mutable_state_of, named_args, remember};
use kompoz_ui::alignment::VerticalAlignment;
use kompoz_ui::layout::{
    FirstBaseline, GraphicsLayerScope, IntrinsicMeasurable, IntrinsicMeasureScope, LastBaseline, Measurable,
    MeasureResult, MeasureScope, MultiContentLayout, MultiContentMeasurePolicy, Placeable,
};
use kompoz_ui::platform::{LocalDensity, LocalLayoutDirection};
use kompoz_ui::semantics::{Role, SemanticsConfiguration};
use kompoz_ui::state::ToggleableState;
use kompoz_ui::{Alignment, Modifier};
use std::cell::RefCell;
use std::rc::Rc;

/// A nullable composable slot (Kotlin's `(@Composable () -> Unit)?`).
type Slot = Option<Rc<dyn Fn()>>;

// ---- ListItem --------------------------------------------------------------------------------------------------

/// A list item (Compose's `ListItem(headlineContent, …)`): `headline_content` (in the type scale's body large) with
/// `overline_content` above it (label small) and `supporting_content` below it (body medium), between
/// `leading_content` (16 dp before the headline) and `trailing_content` (16 dp after it, label small), on a
/// [`Surface`] of [`ListItemDefaults::shape`] in `colors`' container color with `tonal_elevation` and
/// `shadow_elevation`, 16 dp in at its sides. One line (the headline alone) is 56 dp tall, two (with an overline or
/// supporting content) 72 dp, three (with both, or supporting content over more than one line) 88 dp, its sides at
/// the top then (12 dp in), else centered. It is one node to accessibility, its texts together; it takes no
/// interaction (see [`ClickableListItem`], [`SelectableListItem`], [`ToggleableListItem`]).
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn ListItem(
    headline_content: impl Fn() + 'static,
    #[default(Modifier)] modifier: Modifier,
    #[default(None)] overline_content: Option<Rc<dyn Fn()>>,
    #[default(None)] supporting_content: Option<Rc<dyn Fn()>>,
    #[default(None)] leading_content: Option<Rc<dyn Fn()>>,
    #[default(None)] trailing_content: Option<Rc<dyn Fn()>>,
    #[default(ListItemDefaults.colors())] colors: ListItemColors,
    #[default(ListItemDefaults::Elevation)] tonal_elevation: Dp,
    #[default(ListItemDefaults::Elevation)] shadow_elevation: Dp,
) {
    let headline_content = Rc::new(headline_content);
    let decorated_headline_content: Rc<dyn Fn()> = {
        let headline_content = headline_content.clone();
        Rc::new(move || {
            let headline_content = headline_content.clone();
            ProvideTextStyleFromToken(
                colors.content_color_for(true, false, false),
                ListTokens::ItemLabelTextFont,
                move || headline_content(),
            );
        })
    };
    let decorated_supporting_content: Slot = supporting_content.map(|it| -> Rc<dyn Fn()> {
        Rc::new(move || {
            let it = it.clone();
            ProvideTextStyleFromToken(
                colors.supporting_content_color_for(true, false, false),
                ListTokens::ItemSupportingTextFont,
                move || it(),
            );
        })
    });
    let decorated_overline_content: Slot = overline_content.map(|it| -> Rc<dyn Fn()> {
        Rc::new(move || {
            let it = it.clone();
            ProvideTextStyleFromToken(
                colors.overline_content_color_for(true, false, false),
                ListTokens::ItemOverlineFont,
                move || it(),
            );
        })
    });
    let decorated_leading_content: Slot = leading_content.map(|it| -> Rc<dyn Fn()> {
        Rc::new(move || {
            let it = it.clone();
            Box(
                modifier = named_args!(Modifier.padding_sides(end = LeadingContentEndPadding)),
                content = move || {
                    let it = it.clone();
                    KompozitionLocalProvider(
                        &[LocalContentColor.provides(colors.leading_content_color_for(true, false, false))],
                        move || it(),
                    );
                },
            );
        })
    });
    let decorated_trailing_content: Slot = trailing_content.map(|it| -> Rc<dyn Fn()> {
        Rc::new(move || {
            let it = it.clone();
            Box(
                modifier = named_args!(Modifier.padding_sides(start = TrailingContentStartPadding)),
                content = move || {
                    let it = it.clone();
                    ProvideTextStyleFromToken(
                        colors.trailing_content_color_for(true, false, false),
                        ListTokens::ItemTrailingSupportingTextFont,
                        move || it(),
                    );
                },
            );
        })
    });

    Surface(
        modifier = named_args!(Modifier.semantics(merge_descendants = true, |_| {})).then(modifier),
        shape = ListItemDefaults::shape(),
        color = colors.container_color_for(true, false, false),
        content_color = Some(colors.content_color_for(true, false, false)),
        tonal_elevation = tonal_elevation,
        shadow_elevation = shadow_elevation,
        content = move || {
            ListItemLayout(
                decorated_leading_content,
                decorated_trailing_content,
                decorated_headline_content,
                decorated_overline_content,
                decorated_supporting_content,
            );
        },
    );
}

// ---- The Expressive interactive list items --------------------------------------------------------------------

/// A list item that is clicked (Compose's Expressive `ListItem(onClick, …)`): `content` (its headline, body large)
/// with `overline_content` above and `supporting_content` below it, between `leading_content` and
/// `trailing_content` (12 dp from it), padded by `content_padding` (16 dp at its sides, 10 dp above and below), its
/// parts placed in its height by `vertical_alignment` (centered in a short item, at the top in a tall one), at least
/// 56 dp tall. It calls `on_click` (`on_long_click` on a long press, labelled `on_long_click_label` to
/// accessibility), shows the ripple, and emits its interactions to `interaction_source` (one of its own where
/// none); its shape morphs between `shapes`' as it is pressed, focused or hovered, its colors (`colors`) animate
/// between its states, its shadow is `elevation`'s (raised while dragged). Not `enabled`, it isn't clicked and takes
/// its disabled colors. It is one node to accessibility.
///
/// Kotlin: `ListItem(onClick: () -> Unit, modifier: Modifier = Modifier)`
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn ClickableListItem(
    on_click: impl Fn() + 'static,
    #[default(Modifier)] modifier: Modifier,
    #[default(true)] enabled: bool,
    #[default(None)] leading_content: Option<Rc<dyn Fn()>>,
    #[default(None)] trailing_content: Option<Rc<dyn Fn()>>,
    #[default(None)] overline_content: Option<Rc<dyn Fn()>>,
    #[default(None)] supporting_content: Option<Rc<dyn Fn()>>,
    #[default(ListItemDefaults.vertical_alignment())] vertical_alignment: VerticalAlignment,
    #[default(None)] on_long_click: Option<Rc<dyn Fn()>>,
    #[default(None)] on_long_click_label: Option<String>,
    #[default(ListItemDefaults.shapes())] shapes: ListItemShapes,
    #[default(ListItemDefaults.colors())] colors: ListItemColors,
    #[default(ListItemDefaults.elevation())] elevation: ListItemElevation,
    #[default(ListItemDefaults::ContentPadding())] content_padding: PaddingValues,
    #[default(None)] interaction_source: Option<MutableInteractionSource>,
    content: impl Fn() + 'static,
) {
    InteractiveListItem(InteractiveListItemArgs {
        modifier,
        content: Rc::new(content),
        leading_content,
        trailing_content,
        overline_content,
        supporting_content,
        vertical_alignment,
        enabled,
        selected: false,
        apply_semantics: Rc::new(|_| {}),
        on_click: Rc::new(on_click),
        on_long_click,
        on_long_click_label,
        interaction_source,
        colors,
        shapes,
        elevation,
        content_padding,
    });
}

/// A list item of a single choice (Compose's Expressive `ListItem(selected, onClick, …)`, as a [`RadioButton`]'s
/// row): a [`ClickableListItem`] in its selected colors and shape while `selected`, selected and a radio button to
/// accessibility.
///
/// Kotlin: `ListItem(selected: Boolean, onClick: () -> Unit, modifier: Modifier = Modifier)`
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn SelectableListItem(
    selected: bool,
    on_click: impl Fn() + 'static,
    #[default(Modifier)] modifier: Modifier,
    #[default(true)] enabled: bool,
    #[default(None)] leading_content: Option<Rc<dyn Fn()>>,
    #[default(None)] trailing_content: Option<Rc<dyn Fn()>>,
    #[default(None)] overline_content: Option<Rc<dyn Fn()>>,
    #[default(None)] supporting_content: Option<Rc<dyn Fn()>>,
    #[default(ListItemDefaults.vertical_alignment())] vertical_alignment: VerticalAlignment,
    #[default(None)] on_long_click: Option<Rc<dyn Fn()>>,
    #[default(None)] on_long_click_label: Option<String>,
    #[default(ListItemDefaults.shapes())] shapes: ListItemShapes,
    #[default(ListItemDefaults.colors())] colors: ListItemColors,
    #[default(ListItemDefaults.elevation())] elevation: ListItemElevation,
    #[default(ListItemDefaults::ContentPadding())] content_padding: PaddingValues,
    #[default(None)] interaction_source: Option<MutableInteractionSource>,
    content: impl Fn() + 'static,
) {
    InteractiveListItem(InteractiveListItemArgs {
        modifier,
        content: Rc::new(content),
        leading_content,
        trailing_content,
        overline_content,
        supporting_content,
        vertical_alignment,
        enabled,
        selected,
        apply_semantics: selectable_semantics(selected),
        on_click: Rc::new(on_click),
        on_long_click,
        on_long_click_label,
        interaction_source,
        colors,
        shapes,
        elevation,
        content_padding,
    });
}

/// A list item of a multiple choice (Compose's Expressive `ListItem(checked, onCheckedChange, …)`, as a
/// [`Checkbox`]'s row): a [`ClickableListItem`] calling `on_checked_change` with the other state, in its selected
/// colors and shape while `checked`, a checkbox of that state to accessibility.
///
/// Kotlin: `ListItem(checked: Boolean, onCheckedChange: (Boolean) -> Unit, modifier: Modifier = Modifier)`
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn ToggleableListItem(
    checked: bool,
    on_checked_change: impl Fn(bool) + 'static,
    #[default(Modifier)] modifier: Modifier,
    #[default(true)] enabled: bool,
    #[default(None)] leading_content: Option<Rc<dyn Fn()>>,
    #[default(None)] trailing_content: Option<Rc<dyn Fn()>>,
    #[default(None)] overline_content: Option<Rc<dyn Fn()>>,
    #[default(None)] supporting_content: Option<Rc<dyn Fn()>>,
    #[default(ListItemDefaults.vertical_alignment())] vertical_alignment: VerticalAlignment,
    #[default(None)] on_long_click: Option<Rc<dyn Fn()>>,
    #[default(None)] on_long_click_label: Option<String>,
    #[default(ListItemDefaults.shapes())] shapes: ListItemShapes,
    #[default(ListItemDefaults.colors())] colors: ListItemColors,
    #[default(ListItemDefaults.elevation())] elevation: ListItemElevation,
    #[default(ListItemDefaults::ContentPadding())] content_padding: PaddingValues,
    #[default(None)] interaction_source: Option<MutableInteractionSource>,
    content: impl Fn() + 'static,
) {
    InteractiveListItem(InteractiveListItemArgs {
        modifier,
        content: Rc::new(content),
        leading_content,
        trailing_content,
        overline_content,
        supporting_content,
        vertical_alignment,
        enabled,
        selected: checked,
        apply_semantics: toggleable_semantics(checked),
        on_click: Rc::new(move || on_checked_change(!checked)),
        on_long_click,
        on_long_click_label,
        interaction_source,
        colors,
        shapes,
        elevation,
        content_padding,
    });
}

/// An item of a segmented list that is clicked (Compose's Expressive `SegmentedListItem(onClick, shapes, …)`): a
/// [`ClickableListItem`] of `shapes` (typically [`ListItemDefaults.segmented_shapes`] of its place in the list, the
/// list's ends rounded as a card's) in the segmented colors; the items are put
/// [`ListItemDefaults::SegmentedGap`] apart.
///
/// Kotlin: `SegmentedListItem(onClick: () -> Unit, shapes: ListItemShapes, modifier: Modifier = Modifier)`
///
/// [`ListItemDefaults.segmented_shapes`]: ListItemDefaults::segmented_shapes
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn SegmentedListItem(
    on_click: impl Fn() + 'static,
    shapes: ListItemShapes,
    #[default(Modifier)] modifier: Modifier,
    #[default(true)] enabled: bool,
    #[default(None)] leading_content: Option<Rc<dyn Fn()>>,
    #[default(None)] trailing_content: Option<Rc<dyn Fn()>>,
    #[default(None)] overline_content: Option<Rc<dyn Fn()>>,
    #[default(None)] supporting_content: Option<Rc<dyn Fn()>>,
    #[default(ListItemDefaults.vertical_alignment())] vertical_alignment: VerticalAlignment,
    #[default(None)] on_long_click: Option<Rc<dyn Fn()>>,
    #[default(None)] on_long_click_label: Option<String>,
    #[default(ListItemDefaults.segmented_colors())] colors: ListItemColors,
    #[default(ListItemDefaults.elevation())] elevation: ListItemElevation,
    #[default(ListItemDefaults::ContentPadding())] content_padding: PaddingValues,
    #[default(None)] interaction_source: Option<MutableInteractionSource>,
    content: impl Fn() + 'static,
) {
    InteractiveListItem(InteractiveListItemArgs {
        modifier,
        content: Rc::new(content),
        leading_content,
        trailing_content,
        overline_content,
        supporting_content,
        vertical_alignment,
        enabled,
        selected: false,
        apply_semantics: Rc::new(|_| {}),
        on_click: Rc::new(on_click),
        on_long_click,
        on_long_click_label,
        interaction_source,
        colors,
        shapes,
        elevation,
        content_padding,
    });
}

/// An item of a segmented list of a single choice (Compose's Expressive `SegmentedListItem(selected, onClick,
/// shapes, …)`): a [`SelectableListItem`] of `shapes` in the segmented colors.
///
/// Kotlin: `SegmentedListItem(selected: Boolean, onClick: () -> Unit, shapes: ListItemShapes, modifier: Modifier = Modifier)`
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn SelectableSegmentedListItem(
    selected: bool,
    on_click: impl Fn() + 'static,
    shapes: ListItemShapes,
    #[default(Modifier)] modifier: Modifier,
    #[default(true)] enabled: bool,
    #[default(None)] leading_content: Option<Rc<dyn Fn()>>,
    #[default(None)] trailing_content: Option<Rc<dyn Fn()>>,
    #[default(None)] overline_content: Option<Rc<dyn Fn()>>,
    #[default(None)] supporting_content: Option<Rc<dyn Fn()>>,
    #[default(ListItemDefaults.vertical_alignment())] vertical_alignment: VerticalAlignment,
    #[default(None)] on_long_click: Option<Rc<dyn Fn()>>,
    #[default(None)] on_long_click_label: Option<String>,
    #[default(ListItemDefaults.segmented_colors())] colors: ListItemColors,
    #[default(ListItemDefaults.elevation())] elevation: ListItemElevation,
    #[default(ListItemDefaults::ContentPadding())] content_padding: PaddingValues,
    #[default(None)] interaction_source: Option<MutableInteractionSource>,
    content: impl Fn() + 'static,
) {
    InteractiveListItem(InteractiveListItemArgs {
        modifier,
        content: Rc::new(content),
        leading_content,
        trailing_content,
        overline_content,
        supporting_content,
        vertical_alignment,
        enabled,
        selected,
        apply_semantics: selectable_semantics(selected),
        on_click: Rc::new(on_click),
        on_long_click,
        on_long_click_label,
        interaction_source,
        colors,
        shapes,
        elevation,
        content_padding,
    });
}

/// An item of a segmented list of a multiple choice (Compose's Expressive `SegmentedListItem(checked,
/// onCheckedChange, shapes, …)`): a [`ToggleableListItem`] of `shapes` in the segmented colors.
///
/// Kotlin: `SegmentedListItem(checked: Boolean, onCheckedChange: (Boolean) -> Unit, shapes: ListItemShapes, modifier: Modifier = Modifier)`
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn ToggleableSegmentedListItem(
    checked: bool,
    on_checked_change: impl Fn(bool) + 'static,
    shapes: ListItemShapes,
    #[default(Modifier)] modifier: Modifier,
    #[default(true)] enabled: bool,
    #[default(None)] leading_content: Option<Rc<dyn Fn()>>,
    #[default(None)] trailing_content: Option<Rc<dyn Fn()>>,
    #[default(None)] overline_content: Option<Rc<dyn Fn()>>,
    #[default(None)] supporting_content: Option<Rc<dyn Fn()>>,
    #[default(ListItemDefaults.vertical_alignment())] vertical_alignment: VerticalAlignment,
    #[default(None)] on_long_click: Option<Rc<dyn Fn()>>,
    #[default(None)] on_long_click_label: Option<String>,
    #[default(ListItemDefaults.segmented_colors())] colors: ListItemColors,
    #[default(ListItemDefaults.elevation())] elevation: ListItemElevation,
    #[default(ListItemDefaults::ContentPadding())] content_padding: PaddingValues,
    #[default(None)] interaction_source: Option<MutableInteractionSource>,
    content: impl Fn() + 'static,
) {
    InteractiveListItem(InteractiveListItemArgs {
        modifier,
        content: Rc::new(content),
        leading_content,
        trailing_content,
        overline_content,
        supporting_content,
        vertical_alignment,
        enabled,
        selected: checked,
        apply_semantics: toggleable_semantics(checked),
        on_click: Rc::new(move || on_checked_change(!checked)),
        on_long_click,
        on_long_click_label,
        interaction_source,
        colors,
        shapes,
        elevation,
        content_padding,
    });
}

/// What a single choice's item tells accessibility: selected or not, a radio button.
fn selectable_semantics(selected: bool) -> Rc<dyn Fn(&mut SemanticsConfiguration)> {
    Rc::new(move |c| {
        c.set_selected(selected);
        c.set_role(Role::RadioButton);
    })
}

/// What a multiple choice's item tells accessibility: its state, a checkbox.
fn toggleable_semantics(checked: bool) -> Rc<dyn Fn(&mut SemanticsConfiguration)> {
    Rc::new(move |c| {
        c.set_toggleable_state(ToggleableState::from(checked));
        c.set_role(Role::Checkbox);
    })
}

// ---- The classic item's layout ---------------------------------------------------------------------------------

/// The classic item's layout (Compose's private `ListItemLayout`): its slots as contents in the order the policy
/// takes them, a missing one empty.
#[kompozable]
fn ListItemLayout(leading: Slot, trailing: Slot, headline: Rc<dyn Fn()>, overline: Slot, supporting: Slot) {
    let measure_policy = remember(|| Rc::new(ListItemMeasurePolicy) as Rc<dyn MultiContentMeasurePolicy>);
    let empty = || -> Rc<dyn Fn()> { Rc::new(|| {}) };
    MultiContentLayout(
        contents = vec![
            headline,
            overline.unwrap_or_else(empty),
            supporting.unwrap_or_else(empty),
            leading.unwrap_or_else(empty),
            trailing.unwrap_or_else(empty),
        ],
        measure_policy = measure_policy,
    );
}

/// The classic item's measure policy (Compose's private `ListItemMeasurePolicy`).
struct ListItemMeasurePolicy;

/// The first of a content's layouts, if any (Kotlin's `firstOrNull()`).
fn first<'a, M: ?Sized>(list: &'a [&'a M]) -> Option<&'a M> {
    list.first().copied()
}

impl MultiContentMeasurePolicy for ListItemMeasurePolicy {
    fn measure(
        &self,
        scope: &MeasureScope,
        measurables: &[Vec<&dyn Measurable>],
        constraints: Constraints,
    ) -> MeasureResult {
        let [headline_measurable, overline_measurable, supporting_measurable, leading_measurable, trailing_measurable] =
            measurables
        else {
            panic!("ListItemLayout: five contents");
        };
        let mut current_total_width = 0;

        let loose_constraints = named_args!(constraints.copy(min_width = 0, min_height = 0));
        let start_padding = ListItemStartPadding;
        let end_padding = ListItemEndPadding;
        let horizontal_padding = scope.round_to_px(start_padding + end_padding);

        // ListItem layout has a cycle in its dependencies which we use
        // intrinsic measurements to break:
        // 1. Intrinsic leading/trailing width
        // 2. Intrinsic supporting height
        // 3. Intrinsic vertical padding
        // 4. Actual leading/trailing measurement
        // 5. Actual supporting measurement
        // 6. Actual vertical padding
        let intrinsic_leading_width =
            first(leading_measurable).map_or(0, |m| m.min_intrinsic_width(constraints.max_height));
        let intrinsic_trailing_width =
            first(trailing_measurable).map_or(0, |m| m.min_intrinsic_width(constraints.max_height));
        let intrinsic_supporting_width_constraint = loose_constraints
            .max_width
            .subtract_constraint_safely(intrinsic_leading_width + intrinsic_trailing_width + horizontal_padding);
        let intrinsic_supporting_height =
            first(supporting_measurable).map_or(0, |m| m.min_intrinsic_height(intrinsic_supporting_width_constraint));
        let intrinsic_is_supporting_multiline = is_supporting_multiline_heuristic(scope, intrinsic_supporting_height);
        let intrinsic_list_item_type = ListItemType::of(
            first(overline_measurable).is_some(),
            first(supporting_measurable).is_some(),
            intrinsic_is_supporting_multiline,
        );
        let intrinsic_vertical_padding = scope.round_to_px(vertical_padding(intrinsic_list_item_type) * 2.0);

        let padded_loose_constraints = loose_constraints.offset(-horizontal_padding, -intrinsic_vertical_padding);

        let leading_placeable = first(leading_measurable).map(|m| m.measure(padded_loose_constraints));
        current_total_width += leading_placeable.width_or_zero();

        let trailing_placeable =
            first(trailing_measurable).map(|m| m.measure(padded_loose_constraints.offset(-current_total_width, 0)));
        current_total_width += trailing_placeable.width_or_zero();

        let mut current_total_height = 0;

        let headline_placeable =
            first(headline_measurable).map(|m| m.measure(padded_loose_constraints.offset(-current_total_width, 0)));
        current_total_height += headline_placeable.height_or_zero();

        let supporting_placeable = first(supporting_measurable)
            .map(|m| m.measure(padded_loose_constraints.offset(-current_total_width, -current_total_height)));
        current_total_height += supporting_placeable.height_or_zero();
        let is_supporting_multiline =
            supporting_placeable.as_ref().is_some_and(|p| p.get(FirstBaseline) != p.get(LastBaseline));

        let overline_placeable = first(overline_measurable)
            .map(|m| m.measure(padded_loose_constraints.offset(-current_total_width, -current_total_height)));

        let list_item_type =
            ListItemType::of(overline_placeable.is_some(), supporting_placeable.is_some(), is_supporting_multiline);
        let top_padding = vertical_padding(list_item_type);
        let vertical_padding = top_padding * 2.0;

        let width = calculate_width(
            leading_placeable.width_or_zero(),
            trailing_placeable.width_or_zero(),
            headline_placeable.width_or_zero(),
            overline_placeable.width_or_zero(),
            supporting_placeable.width_or_zero(),
            horizontal_padding,
            constraints,
        );
        let height = calculate_height(
            scope,
            leading_placeable.height_or_zero(),
            trailing_placeable.height_or_zero(),
            headline_placeable.height_or_zero(),
            overline_placeable.height_or_zero(),
            supporting_placeable.height_or_zero(),
            list_item_type,
            scope.round_to_px(vertical_padding),
            constraints,
        );

        place(
            scope,
            width,
            height,
            leading_placeable,
            trailing_placeable,
            headline_placeable,
            overline_placeable,
            supporting_placeable,
            list_item_type == ListItemType::ThreeLine,
            scope.round_to_px(start_padding),
            scope.round_to_px(end_padding),
            scope.round_to_px(top_padding),
        )
    }

    fn max_intrinsic_height(
        &self,
        scope: &IntrinsicMeasureScope,
        measurables: &[Vec<&dyn IntrinsicMeasurable>],
        width: i32,
    ) -> i32 {
        calculate_intrinsic_height(scope, measurables, width, |m, w| m.max_intrinsic_height(w))
    }

    fn max_intrinsic_width(
        &self,
        scope: &IntrinsicMeasureScope,
        measurables: &[Vec<&dyn IntrinsicMeasurable>],
        height: i32,
    ) -> i32 {
        calculate_intrinsic_width(scope, measurables, height, |m, h| m.max_intrinsic_width(h))
    }

    fn min_intrinsic_height(
        &self,
        scope: &IntrinsicMeasureScope,
        measurables: &[Vec<&dyn IntrinsicMeasurable>],
        width: i32,
    ) -> i32 {
        calculate_intrinsic_height(scope, measurables, width, |m, w| m.min_intrinsic_height(w))
    }

    fn min_intrinsic_width(
        &self,
        scope: &IntrinsicMeasureScope,
        measurables: &[Vec<&dyn IntrinsicMeasurable>],
        height: i32,
    ) -> i32 {
        calculate_intrinsic_width(scope, measurables, height, |m, h| m.min_intrinsic_width(h))
    }
}

/// The classic item's intrinsic width (Compose's private `calculateIntrinsicWidth`).
fn calculate_intrinsic_width(
    scope: &IntrinsicMeasureScope,
    measurables: &[Vec<&dyn IntrinsicMeasurable>],
    height: i32,
    intrinsic_measure: impl Fn(&dyn IntrinsicMeasurable, i32) -> i32,
) -> i32 {
    let [headline_measurable, overline_measurable, supporting_measurable, leading_measurable, trailing_measurable] =
        measurables
    else {
        panic!("ListItemLayout: five contents");
    };
    let measure = |list: &[&dyn IntrinsicMeasurable]| first(list).map_or(0, |m| intrinsic_measure(m, height));
    calculate_width(
        measure(leading_measurable),
        measure(trailing_measurable),
        measure(headline_measurable),
        measure(overline_measurable),
        measure(supporting_measurable),
        scope.round_to_px(ListItemStartPadding + ListItemEndPadding),
        Constraints::default(),
    )
}

/// The classic item's intrinsic height (Compose's private `calculateIntrinsicHeight`).
fn calculate_intrinsic_height(
    scope: &IntrinsicMeasureScope,
    measurables: &[Vec<&dyn IntrinsicMeasurable>],
    width: i32,
    intrinsic_measure: impl Fn(&dyn IntrinsicMeasurable, i32) -> i32,
) -> i32 {
    let [headline_measurable, overline_measurable, supporting_measurable, leading_measurable, trailing_measurable] =
        measurables
    else {
        panic!("ListItemLayout: five contents");
    };

    let mut remaining_width =
        width.subtract_constraint_safely(scope.round_to_px(ListItemStartPadding + ListItemEndPadding));
    let leading_height = first(leading_measurable).map_or(0, |it| {
        let height = intrinsic_measure(it, remaining_width);
        remaining_width = remaining_width.subtract_constraint_safely(it.max_intrinsic_width(Constraints::Infinity));
        height
    });
    let trailing_height = first(trailing_measurable).map_or(0, |it| {
        let height = intrinsic_measure(it, remaining_width);
        remaining_width = remaining_width.subtract_constraint_safely(it.max_intrinsic_width(Constraints::Infinity));
        height
    });
    let overline_height = first(overline_measurable).map_or(0, |m| intrinsic_measure(m, remaining_width));
    let headline_height = first(headline_measurable).map_or(0, |m| intrinsic_measure(m, remaining_width));
    let supporting_height = first(supporting_measurable).map_or(0, |m| intrinsic_measure(m, remaining_width));
    let is_supporting_multiline = is_supporting_multiline_heuristic(scope, supporting_height);
    let list_item_type = ListItemType::of(overline_height > 0, supporting_height > 0, is_supporting_multiline);

    calculate_height(
        scope,
        leading_height,
        trailing_height,
        headline_height,
        overline_height,
        supporting_height,
        list_item_type,
        scope.round_to_px(vertical_padding(list_item_type) * 2.0),
        Constraints::default(),
    )
}

/// The classic item's width (Compose's private `calculateWidth`): the constraints' bounded width, else its parts'
/// and padding's.
fn calculate_width(
    leading_width: i32,
    trailing_width: i32,
    headline_width: i32,
    overline_width: i32,
    supporting_width: i32,
    horizontal_padding: i32,
    constraints: Constraints,
) -> i32 {
    if constraints.has_bounded_width() {
        return constraints.max_width;
    }
    // Fallback behavior if width constraints are infinite
    let main_content_width = headline_width.max(overline_width).max(supporting_width);
    horizontal_padding + leading_width + main_content_width + trailing_width
}

/// The classic item's height (Compose's private `calculateHeight`): its lines' height at least, else its tallest
/// part and the padding, at most the constraints'.
#[allow(clippy::too_many_arguments)]
fn calculate_height(
    scope: &IntrinsicMeasureScope,
    leading_height: i32,
    trailing_height: i32,
    headline_height: i32,
    overline_height: i32,
    supporting_height: i32,
    list_item_type: ListItemType,
    vertical_padding: i32,
    constraints: Constraints,
) -> i32 {
    let default_min_height = match list_item_type {
        ListItemType::OneLine => ListTokens::ItemOneLineContainerHeight,
        ListItemType::TwoLine => ListTokens::ItemTwoLineContainerHeight,
        ListItemType::ThreeLine => ListTokens::ItemThreeLineContainerHeight,
    };
    let min_height = constraints.min_height.max(scope.round_to_px(default_min_height));

    let main_content_height = headline_height + overline_height + supporting_height;

    min_height
        .max(vertical_padding + leading_height.max(main_content_height).max(trailing_height))
        .min(constraints.max_height)
}

/// Places the classic item's parts (Compose's private `place`): the leading and trailing content at its sides
/// (centered, or at the top of a three line item), the overline, headline and supporting content in a column
/// between them (centered together, or at the top).
#[allow(clippy::too_many_arguments)]
fn place(
    scope: &MeasureScope,
    width: i32,
    height: i32,
    leading_placeable: Option<Placeable>,
    trailing_placeable: Option<Placeable>,
    headline_placeable: Option<Placeable>,
    overline_placeable: Option<Placeable>,
    supporting_placeable: Option<Placeable>,
    is_three_line: bool,
    start_padding: i32,
    end_padding: i32,
    top_padding: i32,
) -> MeasureResult {
    scope.layout(width, height, move |p| {
        let center_vertically = |size: i32| Alignment::CenterVertically.align(size, height);
        if let Some(it) = &leading_placeable {
            p.place_relative(
                it,
                start_padding,
                if is_three_line { top_padding } else { center_vertically(it.height()) },
            );
        }

        let main_content_x = start_padding + leading_placeable.width_or_zero();
        let main_content_y = if is_three_line {
            top_padding
        } else {
            let total_height = headline_placeable.height_or_zero()
                + overline_placeable.height_or_zero()
                + supporting_placeable.height_or_zero();
            center_vertically(total_height)
        };
        let mut current_y = main_content_y;

        if let Some(it) = &overline_placeable {
            p.place_relative(it, main_content_x, current_y);
        }
        current_y += overline_placeable.height_or_zero();

        if let Some(it) = &headline_placeable {
            p.place_relative(it, main_content_x, current_y);
        }
        current_y += headline_placeable.height_or_zero();

        if let Some(it) = &supporting_placeable {
            p.place_relative(it, main_content_x, current_y);
        }

        if let Some(it) = &trailing_placeable {
            p.place_relative(
                it,
                width - end_padding - it.width(),
                if is_three_line { top_padding } else { center_vertically(it.height()) },
            );
        }
    })
}

/// `content` in `color` and the style of `text_token` (Compose's private `ProvideTextStyleFromToken`).
#[kompozable]
fn ProvideTextStyleFromToken(color: Color, text_token: TypographyKeyTokens, content: impl Fn() + 'static) {
    ProvideContentColorTextStyle(color, text_token.value(), content);
}

/// How many lines a classic item is (Compose's private value class `ListItemType`), ordered by them.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
#[allow(clippy::enum_variant_names)] // (Compose's names)
enum ListItemType {
    OneLine = 1,
    TwoLine = 2,
    ThreeLine = 3,
}

impl ListItemType {
    /// Three lines with an overline and supporting content or supporting content over several lines, two with
    /// either, else one (Compose's `ListItemType(hasOverline, hasSupporting, isSupportingMultiline)`).
    fn of(has_overline: bool, has_supporting: bool, is_supporting_multiline: bool) -> ListItemType {
        if (has_overline && has_supporting) || is_supporting_multiline {
            ListItemType::ThreeLine
        } else if has_overline || has_supporting {
            ListItemType::TwoLine
        } else {
            ListItemType::OneLine
        }
    }
}

// Container related defaults
// TODO: Make sure these values stay up to date until replaced with tokens.
pub(crate) const ListItemVerticalPadding: Dp = Dp(8.0);

pub(crate) const ListItemThreeLineVerticalPadding: Dp = Dp(12.0);

pub(crate) const ListItemStartPadding: Dp = Dp(16.0);

pub(crate) const ListItemEndPadding: Dp = Dp(16.0);

// Icon related defaults.
// TODO: Make sure these values stay up to date until replaced with tokens.
pub(crate) const LeadingContentEndPadding: Dp = Dp(16.0);

// Trailing related defaults.
// TODO: Make sure these values stay up to date until replaced with tokens.
pub(crate) const TrailingContentStartPadding: Dp = Dp(16.0);

/// Whether supporting content of this height is more than one line (Compose's private
/// `isSupportingMultilineHeuristic`): in the actual layout phase, we can query supporting baselines, but for an
/// intrinsic measurement pass, we have to estimate — taller than 30 sp.
fn is_supporting_multiline_heuristic(density: &IntrinsicMeasureScope, estimated_supporting_height: i32) -> bool {
    estimated_supporting_height > density.round_to_px(density.text_to_dp(30.sp()))
}

/// The padding above and below a classic item of these lines (Compose's private `verticalPadding`): 12 dp for
/// three, else 8 dp.
fn vertical_padding(list_item_type: ListItemType) -> Dp {
    match list_item_type {
        ListItemType::ThreeLine => ListItemThreeLineVerticalPadding,
        _ => ListItemVerticalPadding,
    }
}

// ---- The interactive item's slots ------------------------------------------------------------------------------

/// The leading content of an interactive item (Compose's private `LeadingDecorator`): in a Box `InteractiveListInternalSpacing` before the headline, in `color` and `text_style`, the minimum
/// interactive size less the padding around it (which is part of its touch target).
#[kompozable]
fn LeadingDecorator(start_padding: Dp, color: Color, text_style: TypographyKeyTokens, content: Slot) {
    if let Some(content) = content {
        Box(
            modifier = named_args!(Modifier.padding_sides(end = InteractiveListInternalSpacing)),
            content = move || {
                let horizontal_padding = start_padding + InteractiveListInternalSpacing;
                // Padding contributes to content's touch target, so we can reduce enforcement value
                let mics = (LocalMinimumInteractiveComponentSize.current().take_or_else(|| Dp(0.0))
                    - horizontal_padding)
                    .coerce_at_least(Dp(0.0));
                let content = content.clone();
                ProvideContentColorTextStyleWithValues(
                    color,
                    text_style.value(),
                    vec![LocalMinimumInteractiveComponentSize.provides(mics)],
                    move || content(),
                );
            },
        );
    }
}

/// The trailing content of an interactive item (Compose's private `TrailingDecorator`): as the leading one's, after
/// the headline.
#[kompozable]
fn TrailingDecorator(end_padding: Dp, color: Color, text_style: TypographyKeyTokens, content: Slot) {
    if let Some(content) = content {
        Box(
            modifier = named_args!(Modifier.padding_sides(start = InteractiveListInternalSpacing)),
            content = move || {
                let horizontal_padding = end_padding + InteractiveListInternalSpacing;
                // Padding contributes to content's touch target, so we can reduce enforcement value
                let mics = (LocalMinimumInteractiveComponentSize.current().take_or_else(|| Dp(0.0))
                    - horizontal_padding)
                    .coerce_at_least(Dp(0.0));
                let content = content.clone();
                ProvideContentColorTextStyleWithValues(
                    color,
                    text_style.value(),
                    vec![LocalMinimumInteractiveComponentSize.provides(mics)],
                    move || content(),
                );
            },
        );
    }
}

/// The overline of an interactive item (Compose's private `OverlineDecorator`).
#[kompozable]
fn OverlineDecorator(color: Color, text_style: TypographyKeyTokens, content: Slot) {
    if let Some(content) = content {
        Box(content = move || {
            let content = content.clone();
            ProvideContentColorTextStyle(color, text_style.value(), move || content());
        });
    }
}

/// The supporting content of an interactive item (Compose's private `SupportingDecorator`).
#[kompozable]
fn SupportingDecorator(color: Color, text_style: TypographyKeyTokens, content: Slot) {
    if let Some(content) = content {
        Box(content = move || {
            let content = content.clone();
            ProvideContentColorTextStyle(color, text_style.value(), move || content());
        });
    }
}

/// The headline of an interactive item (Compose's private `ContentDecorator`).
#[kompozable]
fn ContentDecorator(color: Color, text_style: TypographyKeyTokens, content: Rc<dyn Fn()>) {
    Box(content = move || {
        let content = content.clone();
        ProvideContentColorTextStyle(color, text_style.value(), move || content());
    });
}

/// Keeps states of whether `source` is pressed, focused, hovered and dragged (Compose's private
/// `InteractionSource.CollectInteractionsAsState`): each the interactions started and not yet ended, those of a
/// state not given not kept.
#[kompozable]
fn CollectInteractionsAsState(
    source: MutableInteractionSource,
    pressed_state: Option<MutableState<bool>>,
    focused_state: Option<MutableState<bool>>,
    hovered_state: Option<MutableState<bool>>,
    dragged_state: Option<MutableState<bool>>,
) {
    let interactions = source.interactions();
    LaunchedEffect(source, async move {
        let press_interactions = pressed_state.as_ref().map(|_| RefCell::new(Vec::<PressInteraction::Press>::new()));
        let focus_interactions = focused_state.as_ref().map(|_| RefCell::new(Vec::<FocusInteraction::Focus>::new()));
        let hover_interactions = hovered_state.as_ref().map(|_| RefCell::new(Vec::<HoverInteraction::Enter>::new()));
        let drag_interactions = dragged_state.as_ref().map(|_| RefCell::new(Vec::<DragInteraction::Start>::new()));

        fn remove<T: PartialEq>(list: &Option<RefCell<Vec<T>>>, item: &T) {
            if let Some(list) = list {
                let mut list = list.borrow_mut();
                if let Some(index) = list.iter().position(|i| i == item) {
                    list.remove(index);
                }
            }
        }
        fn add<T>(list: &Option<RefCell<Vec<T>>>, item: &T)
        where
            T: Clone,
        {
            if let Some(list) = list {
                list.borrow_mut().push(item.clone());
            }
        }
        fn tell<T>(state: &Option<MutableState<bool>>, list: &Option<RefCell<Vec<T>>>) {
            if let (Some(state), Some(list)) = (state, list) {
                state.set(!list.borrow().is_empty());
            }
        }

        interactions
            .collect(move |interaction| {
                // press
                if let Some(i) = interaction.downcast_ref::<PressInteraction::Press>() {
                    add(&press_interactions, i);
                } else if let Some(i) = interaction.downcast_ref::<PressInteraction::Release>() {
                    remove(&press_interactions, &i.press);
                } else if let Some(i) = interaction.downcast_ref::<PressInteraction::Cancel>() {
                    remove(&press_interactions, &i.press);
                // focus
                } else if let Some(i) = interaction.downcast_ref::<FocusInteraction::Focus>() {
                    add(&focus_interactions, i);
                } else if let Some(i) = interaction.downcast_ref::<FocusInteraction::Unfocus>() {
                    remove(&focus_interactions, &i.focus);
                // hover
                } else if let Some(i) = interaction.downcast_ref::<HoverInteraction::Enter>() {
                    add(&hover_interactions, i);
                } else if let Some(i) = interaction.downcast_ref::<HoverInteraction::Exit>() {
                    remove(&hover_interactions, &i.enter);
                // drag
                } else if let Some(i) = interaction.downcast_ref::<DragInteraction::Start>() {
                    add(&drag_interactions, i);
                } else if let Some(i) = interaction.downcast_ref::<DragInteraction::Stop>() {
                    remove(&drag_interactions, &i.start);
                } else if let Some(i) = interaction.downcast_ref::<DragInteraction::Cancel>() {
                    remove(&drag_interactions, &i.start);
                }
                tell(&pressed_state, &press_interactions);
                tell(&focused_state, &focus_interactions);
                tell(&hovered_state, &hover_interactions);
                tell(&dragged_state, &drag_interactions);
            })
            .await;
    });
}

/// The states an interactive item's colors animate between (Compose's private `InteractiveListColorState`).
#[derive(Clone, Copy, PartialEq, Debug)]
struct InteractiveListColorState {
    enabled: bool,
    selected: bool,
    dragged: bool,
}

/// The parameters of [`InteractiveListItem`] (Compose's private function's), each overload's.
#[derive(Clone)]
struct InteractiveListItemArgs {
    modifier: Modifier,
    content: Rc<dyn Fn()>,
    leading_content: Slot,
    trailing_content: Slot,
    overline_content: Slot,
    supporting_content: Slot,
    vertical_alignment: VerticalAlignment,
    enabled: bool,
    selected: bool,
    apply_semantics: Rc<dyn Fn(&mut SemanticsConfiguration)>,
    on_click: Rc<dyn Fn()>,
    on_long_click: Slot,
    on_long_click_label: Option<String>,
    interaction_source: Option<MutableInteractionSource>,
    colors: ListItemColors,
    shapes: ListItemShapes,
    elevation: ListItemElevation,
    content_padding: PaddingValues,
}

/// An interactive list item (Compose's private `InteractiveListItem`): its shape, colors and shadow as its
/// interactions and state say, the ripple, clicks and long clicks, its slots in [`InteractiveListItemLayout`].
#[kompozable]
fn InteractiveListItem(args: InteractiveListItemArgs) {
    let InteractiveListItemArgs {
        modifier,
        content,
        leading_content,
        trailing_content,
        overline_content,
        supporting_content,
        vertical_alignment,
        enabled,
        selected,
        apply_semantics,
        on_click,
        on_long_click,
        on_long_click_label,
        interaction_source,
        colors,
        shapes,
        elevation,
        content_padding,
    } = args;
    let interaction_source = match interaction_source {
        Some(source) => source,
        None => remember(MutableInteractionSource::new),
    };

    let pressed = remember(|| mutable_state_of(false));
    let focused = remember(|| mutable_state_of(false));
    let hovered = remember(|| mutable_state_of(false));
    let dragged = remember(|| mutable_state_of(false));

    CollectInteractionsAsState(
        interaction_source.clone(),
        Some(pressed.clone()),
        Some(focused.clone()),
        Some(hovered.clone()),
        Some(dragged.clone()),
    );

    // TODO: Load the motionScheme tokens from the component tokens file
    let color_animation_spec = MotionSchemeKeyTokens::DefaultEffects.value::<Color>();
    let shape_animation_spec = MotionSchemeKeyTokens::FastSpatial.value::<f32>();
    let elevation_animation_spec = MotionSchemeKeyTokens::FastSpatial.value::<Dp>();

    let shape = shape_for_interaction(
        shapes,
        selected,
        pressed.get(),
        focused.get(),
        hovered.get(),
        dragged.get(),
        shape_animation_spec,
    );

    let color_state = InteractiveListColorState { enabled, selected, dragged: dragged.get() };
    let transition = update_transition(color_state, Some("ListColor".to_string()));

    let animate_color = |color: fn(&ListItemColors, bool, bool, bool) -> Color| {
        let spec = color_animation_spec.clone();
        transition.animate_color(
            move |_: &Segment<InteractiveListColorState>| spec.clone(),
            "ColorAnimation".to_string(),
            move |state: &InteractiveListColorState| color(&colors, state.enabled, state.selected, state.dragged),
        )
    };
    let container_color = animate_color(ListItemColors::container_color_for);
    let content_color = animate_color(ListItemColors::content_color_for);
    let leading_color = animate_color(ListItemColors::leading_content_color_for);
    let trailing_color = animate_color(ListItemColors::trailing_content_color_for);
    let overline_color = animate_color(ListItemColors::overline_content_color_for);
    let supporting_color = animate_color(ListItemColors::supporting_content_color_for);

    let leading_text_style = ListTokens::ItemLeadingAvatarLabelFont;
    let trailing_text_style = ListTokens::ItemTrailingSupportingTextFont;
    let overline_text_style = ListTokens::ItemOverlineFont;
    let supporting_text_style = ListTokens::ItemSupportingTextFont;
    let content_text_style = ListTokens::ItemLabelTextFont;

    let target_elevation = if dragged.get() { elevation.dragged_elevation } else { elevation.elevation };
    let shadow_elevation = animate_dp_as_state(target_elevation, elevation_animation_spec);
    let density = LocalDensity.current();
    let layout_direction = LocalLayoutDirection.current();

    let (content_color, leading_color, trailing_color, overline_color, supporting_color) = (
        content_color.value(),
        leading_color.value(),
        trailing_color.value(),
        overline_color.value(),
        supporting_color.value(),
    );
    let shadow_for_z_index = shadow_elevation.clone();
    let layer_shape = shape.clone();
    let item_modifier = modifier
        .semantics_with(true, move |c| apply_semantics(c))
        .default_min_size_with(Dp::Unspecified, ListTokens::ItemOneLineContainerHeight)
        .minimum_interactive_component_size()
        .z_index_lambda(FloatProducer::new(move || if shadow_for_z_index.value() > Dp(0.0) { 1.0 } else { 0.0 }))
        .graphics_layer_block(move |l: &mut GraphicsLayerScope| {
            l.shadow_elevation = density.to_px(shadow_elevation.value());
            l.shape = layer_shape.clone();
            l.clip = false;
        })
        .background(container_color.value(), shape.clone())
        .clip(ClipShape(shape))
        .combined_clickable_indication_with(
            Some(interaction_source),
            Some(ripple(true, Dp::Unspecified, Color::Unspecified)),
            enabled,
            None,
            None,
            on_long_click_label,
            on_long_click,
            None,
            true,
            move || on_click(),
        )
        .padding_values(content_padding.clone());

    KompozitionLocalProvider(&[LocalContentColor.provides(content_color)], move || {
        let (leading_content, trailing_content, overline_content, supporting_content, content) = (
            leading_content.clone(),
            trailing_content.clone(),
            overline_content.clone(),
            supporting_content.clone(),
            content.clone(),
        );
        InteractiveListItemLayout(
            item_modifier.clone(),
            vertical_alignment.clone(),
            Rc::new(move || {
                LeadingDecorator(
                    content_padding.calculate_start_padding(layout_direction),
                    leading_color,
                    leading_text_style,
                    leading_content.clone(),
                )
            }),
            Rc::new(move || {
                TrailingDecorator(
                    content_padding.calculate_end_padding(layout_direction),
                    trailing_color,
                    trailing_text_style,
                    trailing_content.clone(),
                )
            }),
            Rc::new(move || OverlineDecorator(overline_color, overline_text_style, overline_content.clone())),
            Rc::new(move || SupportingDecorator(supporting_color, supporting_text_style, supporting_content.clone())),
            Rc::new(move || ContentDecorator(content_color, content_text_style, content.clone())),
        );
    });
}

/// An interactive item's layout (Compose's private `InteractiveListItemLayout`): its slots as contents, measured
/// by a policy remembered for `vertical_alignment`.
#[kompozable]
fn InteractiveListItemLayout(
    modifier: Modifier,
    vertical_alignment: VerticalAlignment,
    leading: Rc<dyn Fn()>,
    trailing: Rc<dyn Fn()>,
    overline: Rc<dyn Fn()>,
    supporting: Rc<dyn Fn()>,
    content: Rc<dyn Fn()>,
) {
    let measure_policy = kompoz_runtime::remember_keyed(vertical_alignment.clone(), move || {
        Rc::new(InteractiveListItemMeasurePolicy { vertical_alignment }) as Rc<dyn MultiContentMeasurePolicy>
    });
    MultiContentLayout(
        contents = vec![leading, trailing, overline, supporting, content],
        modifier = modifier,
        measure_policy = measure_policy,
    );
}

/// An interactive item's measure policy (Compose's private `InteractiveListItemMeasurePolicy`): the leading and
/// trailing content at its sides, the overline, headline and supporting content in a column between them, each
/// put in its height by `vertical_alignment`.
struct InteractiveListItemMeasurePolicy {
    vertical_alignment: VerticalAlignment,
}

impl MultiContentMeasurePolicy for InteractiveListItemMeasurePolicy {
    fn measure(
        &self,
        scope: &MeasureScope,
        measurables: &[Vec<&dyn Measurable>],
        constraints: Constraints,
    ) -> MeasureResult {
        let [leading_measurable, trailing_measurable, overline_measurable, supporting_measurable, content_measurable] =
            measurables
        else {
            panic!("InteractiveListItemLayout: five contents");
        };

        let loose_constraints = named_args!(constraints.copy(min_width = 0, min_height = 0));
        let mut constraint_offset_x = 0;
        let mut constraint_offset_y = 0;

        let leading_placeable = first(leading_measurable).map(|m| m.measure(loose_constraints));
        constraint_offset_x += leading_placeable.width_or_zero();

        let trailing_placeable =
            first(trailing_measurable).map(|m| m.measure(loose_constraints.offset(-constraint_offset_x, 0)));
        constraint_offset_x += trailing_placeable.width_or_zero();

        let overline_placeable =
            first(overline_measurable).map(|m| m.measure(loose_constraints.offset(-constraint_offset_x, 0)));
        constraint_offset_y += overline_placeable.height_or_zero();

        let content_placeable = first(content_measurable)
            .map(|m| m.measure(loose_constraints.offset(-constraint_offset_x, -constraint_offset_y)));
        constraint_offset_y += content_placeable.height_or_zero();

        let supporting_placeable = first(supporting_measurable)
            .map(|m| m.measure(loose_constraints.offset(-constraint_offset_x, -constraint_offset_y)));

        let width = interactive_width(
            leading_placeable.width_or_zero(),
            trailing_placeable.width_or_zero(),
            overline_placeable.width_or_zero(),
            supporting_placeable.width_or_zero(),
            content_placeable.width_or_zero(),
            constraints,
        );
        let height = interactive_height(
            leading_placeable.height_or_zero(),
            trailing_placeable.height_or_zero(),
            overline_placeable.height_or_zero(),
            supporting_placeable.height_or_zero(),
            content_placeable.height_or_zero(),
            constraints,
        );

        let vertical_alignment = self.vertical_alignment.clone();
        scope.layout(width, height, move |p| {
            if let Some(leading_placeable) = &leading_placeable {
                p.place_relative(leading_placeable, 0, vertical_alignment.align(leading_placeable.height(), height));
            }

            let main_content_x = leading_placeable.width_or_zero();
            let main_content_total_height = content_placeable.height_or_zero()
                + overline_placeable.height_or_zero()
                + supporting_placeable.height_or_zero();
            let main_content_y = vertical_alignment.align(main_content_total_height, height);
            let mut curr_y = main_content_y;

            if let Some(it) = &overline_placeable {
                p.place_relative(it, main_content_x, curr_y);
            }
            curr_y += overline_placeable.height_or_zero();

            if let Some(it) = &content_placeable {
                p.place_relative(it, main_content_x, curr_y);
            }
            curr_y += content_placeable.height_or_zero();

            if let Some(it) = &supporting_placeable {
                p.place_relative(it, main_content_x, curr_y);
            }

            if let Some(trailing_placeable) = &trailing_placeable {
                p.place_relative(
                    trailing_placeable,
                    width - trailing_placeable.width(),
                    vertical_alignment.align(trailing_placeable.height(), height),
                );
            }
        })
    }

    fn max_intrinsic_height(
        &self,
        _: &IntrinsicMeasureScope,
        measurables: &[Vec<&dyn IntrinsicMeasurable>],
        width: i32,
    ) -> i32 {
        interactive_intrinsic_height(measurables, width, |m, w| m.max_intrinsic_height(w))
    }

    fn max_intrinsic_width(
        &self,
        _: &IntrinsicMeasureScope,
        measurables: &[Vec<&dyn IntrinsicMeasurable>],
        height: i32,
    ) -> i32 {
        interactive_intrinsic_width(measurables, height, |m, h| m.max_intrinsic_width(h))
    }

    fn min_intrinsic_height(
        &self,
        _: &IntrinsicMeasureScope,
        measurables: &[Vec<&dyn IntrinsicMeasurable>],
        width: i32,
    ) -> i32 {
        interactive_intrinsic_height(measurables, width, |m, w| m.min_intrinsic_height(w))
    }

    fn min_intrinsic_width(
        &self,
        _: &IntrinsicMeasureScope,
        measurables: &[Vec<&dyn IntrinsicMeasurable>],
        height: i32,
    ) -> i32 {
        interactive_intrinsic_width(measurables, height, |m, h| m.min_intrinsic_width(h))
    }
}

/// An interactive item's intrinsic width (its policy's private `calculateIntrinsicWidth`).
fn interactive_intrinsic_width(
    measurables: &[Vec<&dyn IntrinsicMeasurable>],
    height: i32,
    intrinsic_measure: impl Fn(&dyn IntrinsicMeasurable, i32) -> i32,
) -> i32 {
    let [leading_measurable, trailing_measurable, overline_measurable, supporting_measurable, content_measurable] =
        measurables
    else {
        panic!("InteractiveListItemLayout: five contents");
    };
    let measure = |list: &[&dyn IntrinsicMeasurable]| first(list).map_or(0, |m| intrinsic_measure(m, height));
    interactive_width(
        measure(leading_measurable),
        measure(trailing_measurable),
        measure(overline_measurable),
        measure(supporting_measurable),
        measure(content_measurable),
        Constraints::default(),
    )
}

/// An interactive item's intrinsic height (its policy's private `calculateIntrinsicHeight`).
fn interactive_intrinsic_height(
    measurables: &[Vec<&dyn IntrinsicMeasurable>],
    width: i32,
    intrinsic_measure: impl Fn(&dyn IntrinsicMeasurable, i32) -> i32,
) -> i32 {
    let [leading_measurable, trailing_measurable, overline_measurable, supporting_measurable, content_measurable] =
        measurables
    else {
        panic!("InteractiveListItemLayout: five contents");
    };

    let mut remaining_width = width;

    let leading_height = first(leading_measurable).map_or(0, |it| {
        let height = intrinsic_measure(it, remaining_width);
        remaining_width = remaining_width.subtract_constraint_safely(it.max_intrinsic_width(Constraints::Infinity));
        height
    });
    let trailing_height = first(trailing_measurable).map_or(0, |it| {
        let height = intrinsic_measure(it, remaining_width);
        remaining_width = remaining_width.subtract_constraint_safely(it.max_intrinsic_width(Constraints::Infinity));
        height
    });
    let overline_height = first(overline_measurable).map_or(0, |m| intrinsic_measure(m, remaining_width));
    let supporting_height = first(supporting_measurable).map_or(0, |m| intrinsic_measure(m, remaining_width));
    let content_height = first(content_measurable).map_or(0, |m| intrinsic_measure(m, remaining_width));

    interactive_height(
        leading_height,
        trailing_height,
        overline_height,
        supporting_height,
        content_height,
        Constraints::default(),
    )
}

/// An interactive item's width (its policy's private `calculateWidth`): the constraints' bounded width, else its
/// parts'.
fn interactive_width(
    leading_width: i32,
    trailing_width: i32,
    overline_width: i32,
    supporting_width: i32,
    content_width: i32,
    constraints: Constraints,
) -> i32 {
    if constraints.has_bounded_width() {
        return constraints.max_width;
    }
    // Fallback behavior if width constraints are infinite
    let main_content_width = content_width.max(overline_width).max(supporting_width);
    leading_width + main_content_width + trailing_width
}

/// An interactive item's height (its policy's private `calculateHeight`): its tallest part, in the constraints.
fn interactive_height(
    leading_height: i32,
    trailing_height: i32,
    overline_height: i32,
    supporting_height: i32,
    content_height: i32,
    constraints: Constraints,
) -> i32 {
    let main_content_height = content_height + overline_height + supporting_height;

    constraints.constrain_height(leading_height.max(main_content_height).max(trailing_height))
}

/// Compose's private `Modifier.zIndexLambda`.
trait ZIndexLambda {
    /// What it wraps placed at the z index `z_index` gives as it is placed (Compose's private
    /// `Modifier.zIndexLambda`).
    fn z_index_lambda(self, z_index: FloatProducer) -> Modifier;
}

impl ZIndexLambda for Modifier {
    fn z_index_lambda(self, z_index: FloatProducer) -> Modifier {
        self.layout(move |scope, measurable, constraints| {
            let placeable = measurable.measure(constraints);
            let z_index = z_index.clone();
            scope.layout(placeable.width(), placeable.height(), move |p| {
                named_args!(p.place(&placeable, 0, 0, z_index = z_index.invoke()))
            })
        })
    }
}

pub(crate) const InteractiveListStartPadding: Dp = ListTokens::ItemLeadingSpace;
pub(crate) const InteractiveListEndPadding: Dp = ListTokens::ItemTrailingSpace;
pub(crate) const InteractiveListTopPadding: Dp = ListTokens::ItemTopSpace;
pub(crate) const InteractiveListBottomPadding: Dp = ListTokens::ItemBottomSpace;
pub(crate) const InteractiveListInternalSpacing: Dp = ListTokens::ItemBetweenSpace;

/// The height under which an interactive item's parts are centered, at the top from there: halfway between a two
/// and a three line item's, less the padding above and below (Compose's internal
/// `InteractiveListVerticalAlignmentBreakpoint`).
pub(crate) const InteractiveListVerticalAlignmentBreakpoint: Dp =
    Dp((ListTokens::ItemThreeLineContainerHeight.0 + ListTokens::ItemTwoLineContainerHeight.0) / 2.0
        - InteractiveListTopPadding.0
        - InteractiveListBottomPadding.0);
