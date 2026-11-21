//! What Material's list items are made of, as Compose's ListItemDefaults.kt: their defaults
//! ([`ListItemDefaults`]: the padding, elevation, shape and colors of a list item, the Expressive interactive and
//! segmented items' colors, shapes, elevations, gap and vertical alignment), their colors in each state
//! ([`ListItemColors`]: at rest, disabled, selected and dragged, for the container and each slot), the shapes an
//! interactive item morphs between ([`ListItemShapes`]) and its elevations ([`ListItemElevation`]).

// (the deprecated `ListItemColors.copy(containerColor, headlineColor, …)`'s own generated items name it)
#![allow(deprecated)]

use crate::button::corner_based_shape_of;
use crate::button::rounded_corner_shape_of;
use crate::color_scheme::ColorScheme;
use crate::internal::animated_shape::remember_animated_shape;
use crate::list_item::{
    InteractiveListBottomPadding, InteractiveListEndPadding, InteractiveListStartPadding, InteractiveListTopPadding,
    InteractiveListVerticalAlignmentBreakpoint,
};
use crate::material_theme::MaterialTheme;
use crate::shapes::Shapes;
use crate::tokens::{ListTokens, ReorderListTokens};
use kompoz_animation::FiniteAnimationSpec;
use kompoz_foundation::prelude::*;
use kompoz_graphics::{Color, Dp, Shape};
use kompoz_runtime::{
    KotlinProperties, key, kompozable, kompozable_block, named, named_args, remember, remember_keyed,
};
use kompoz_ui::Alignment;
use kompoz_ui::alignment::VerticalAlignment;
use kompoz_ui::platform::LocalDensity;
use std::cell::RefCell;
use std::fmt;
use std::rc::Rc;
use std::sync::Arc;

// ---- ListItemDefaults ------------------------------------------------------------------------------------------

/// The list items' defaults (Compose's `ListItemDefaults`): a list item's padding, elevation, shape and colors;
/// the Expressive interactive and segmented items' colors, shapes, elevations, the gap between segments and the
/// vertical alignment of an interactive item's parts. Its composables are methods of the value, as Kotlin calls them
/// (`ListItemDefaults.colors()`).
#[derive(Clone, Copy, Debug)]
pub struct ListItemDefaults;

impl ListItemDefaults {
    /// The tonal and shadow elevation of a list item: none (Compose's `ListItemDefaults.Elevation`).
    pub const Elevation: Dp = ListTokens::ItemContainerElevation;

    /// The gap between the items of a segmented list, 2 dp (Compose's Expressive `ListItemDefaults.SegmentedGap`).
    pub const SegmentedGap: Dp = ListTokens::SegmentedGap;

    /// The padding of an interactive list item's content: 16 dp at its sides, 10 dp above and below (Compose's
    /// `ListItemDefaults.ContentPadding`).
    pub fn ContentPadding() -> PaddingValues {
        PaddingValues::new(
            InteractiveListStartPadding,
            InteractiveListTopPadding,
            InteractiveListEndPadding,
            InteractiveListBottomPadding,
        )
    }

    /// A list item's shape: square corners (Compose's `ListItemDefaults.shape`).
    pub fn shape() -> Arc<dyn Shape> {
        ListTokens::ItemContainerShape.value()
    }

    /// A list item's container color: the scheme's surface (Compose's `ListItemDefaults.containerColor`).
    pub fn container_color() -> Color {
        ListTokens::ItemContainerColor.value()
    }

    /// A list item's content color: the scheme's on surface (Compose's `ListItemDefaults.contentColor`).
    pub fn content_color() -> Color {
        ListTokens::ItemLabelTextColor.value()
    }

    /// A list item's colors in `scheme` (Compose's internal `ColorScheme.defaultListItemColors`): on the surface,
    /// the headline on surface, the slots on surface variant; selected, the secondary container and the colors on
    /// it; disabled, on surface at 38% on the surface; dragged, the tertiary container and the colors on it.
    pub(crate) fn default_list_item_colors(scheme: &ColorScheme) -> ListItemColors {
        ListItemDefaults::list_item_colors_on(scheme, ListTokens::ItemContainerColor.into())
    }

    /// A segmented list item's colors in `scheme` (Compose's internal `ColorScheme.defaultSegmentedListItemColors`):
    /// a list item's, on the segmented container color.
    pub(crate) fn default_segmented_list_item_colors(scheme: &ColorScheme) -> ListItemColors {
        ListItemDefaults::list_item_colors_on(scheme, ListTokens::ItemSegmentedContainerColor.into())
    }

    /// The default colors of both on `container` (the two getters of Compose, which differ by their container's
    /// token only).
    fn list_item_colors_on(scheme: &ColorScheme, container: ContainerToken) -> ListItemColors {
        let token = |t| scheme.from_token(t);
        let faded = |t, alpha: f32| named_args!(scheme.from_token(t).copy(alpha = alpha));
        ListItemColors {
            // default
            container_color: token(container.0),
            content_color: token(ListTokens::ItemLabelTextColor),
            leading_content_color: token(ListTokens::ItemLeadingIconColor),
            trailing_content_color: token(ListTokens::ItemTrailingIconColor),
            overline_content_color: token(ListTokens::ItemOverlineColor),
            supporting_content_color: token(ListTokens::ItemSupportingTextColor),
            // selected
            selected_container_color: token(ListTokens::ItemSelectedContainerColor),
            selected_content_color: token(ListTokens::ItemSelectedLabelTextColor),
            selected_leading_content_color: token(ListTokens::ItemSelectedLeadingIconColor),
            selected_trailing_content_color: token(ListTokens::ItemSelectedTrailingIconColor),
            selected_overline_content_color: token(ListTokens::ItemSelectedOverlineColor),
            selected_supporting_content_color: token(ListTokens::ItemSelectedSupportingTextColor),
            // disabled
            disabled_container_color: token(container.0),
            disabled_content_color: faded(
                ListTokens::ItemDisabledLabelTextColor,
                ListTokens::ItemDisabledLabelTextOpacity,
            ),
            disabled_leading_content_color: faded(
                ListTokens::ItemDisabledLeadingIconColor,
                ListTokens::ItemDisabledLeadingIconOpacity,
            ),
            disabled_trailing_content_color: faded(
                ListTokens::ItemDisabledTrailingIconColor,
                ListTokens::ItemDisabledTrailingIconOpacity,
            ),
            disabled_overline_content_color: faded(
                ListTokens::ItemDisabledOverlineColor,
                ListTokens::ItemDisabledOverlineOpacity,
            ),
            disabled_supporting_content_color: faded(
                ListTokens::ItemDisabledSupportingTextColor,
                ListTokens::ItemDisabledSupportingTextOpacity,
            ),
            // dragged
            dragged_container_color: token(ReorderListTokens::ItemContainerColor),
            dragged_content_color: token(ReorderListTokens::ItemLabelTextColor),
            dragged_leading_content_color: token(ReorderListTokens::ItemLeadingIconColor),
            dragged_trailing_content_color: token(ReorderListTokens::ItemTrailingIconColor),
            dragged_overline_content_color: token(ReorderListTokens::ItemOverlineColor),
            dragged_supporting_content_color: token(ReorderListTokens::ItemSupportingTextColor),
        }
    }

    /// An interactive list item's shapes in `shapes` (Compose's internal `Shapes.defaultListItemShapes`): extra
    /// small corners at rest, large ones selected, pressed, focused and dragged, medium ones hovered.
    pub(crate) fn default_list_item_shapes(shapes: &Shapes) -> ListItemShapes {
        ListItemShapes::new(
            shapes.from_token(ListTokens::ItemContainerExpressiveShape),
            shapes.from_token(ListTokens::ItemSelectedContainerExpressiveShape),
            shapes.from_token(ListTokens::ItemPressedContainerExpressiveShape),
            shapes.from_token(ListTokens::ItemFocusedContainerExpressiveShape),
            shapes.from_token(ListTokens::ItemHoveredContainerExpressiveShape),
            shapes.from_token(ReorderListTokens::ItemShape),
        )
    }
}

/// A container color's token (what the two default colors differ by).
struct ContainerToken(crate::tokens::ColorSchemeKeyTokens);

impl From<crate::tokens::ColorSchemeKeyTokens> for ContainerToken {
    fn from(token: crate::tokens::ColorSchemeKeyTokens) -> ContainerToken {
        ContainerToken(token)
    }
}

#[named]
impl ListItemDefaults {
    /// A list item's colors (Compose's `ListItemDefaults.colors()` and `colors(containerColor, …)` of every
    /// state): the theme's ([`default_list_item_colors`](Self::default_list_item_colors)), each color given other
    /// than unspecified instead. None given, the theme's as they are (Kotlin's overload without parameters).
    ///
    /// Kotlin: `colors(containerColor: Color = Color.Unspecified, contentColor: Color = Color.Unspecified, leadingContentColor: Color = Color.Unspecified, trailingContentColor: Color = Color.Unspecified, overlineContentColor: Color = Color.Unspecified, supportingContentColor: Color = Color.Unspecified, disabledContainerColor: Color = Color.Unspecified, disabledContentColor: Color = Color.Unspecified, disabledLeadingContentColor: Color = Color.Unspecified, disabledTrailingContentColor: Color = Color.Unspecified, disabledOverlineContentColor: Color = Color.Unspecified, disabledSupportingContentColor: Color = Color.Unspecified, selectedContainerColor: Color = Color.Unspecified, selectedContentColor: Color = Color.Unspecified, selectedLeadingContentColor: Color = Color.Unspecified, selectedTrailingContentColor: Color = Color.Unspecified, selectedOverlineContentColor: Color = Color.Unspecified, selectedSupportingContentColor: Color = Color.Unspecified, draggedContainerColor: Color = Color.Unspecified, draggedContentColor: Color = Color.Unspecified, draggedLeadingContentColor: Color = Color.Unspecified, draggedTrailingContentColor: Color = Color.Unspecified, draggedOverlineContentColor: Color = Color.Unspecified, draggedSupportingContentColor: Color = Color.Unspecified): ListItemColors`
    #[allow(clippy::too_many_arguments)]
    pub fn colors(
        &self,
        // default
        #[default(Color::Unspecified)] container_color: Color,
        #[default(Color::Unspecified)] content_color: Color,
        #[default(Color::Unspecified)] leading_content_color: Color,
        #[default(Color::Unspecified)] trailing_content_color: Color,
        #[default(Color::Unspecified)] overline_content_color: Color,
        #[default(Color::Unspecified)] supporting_content_color: Color,
        // disabled
        #[default(Color::Unspecified)] disabled_container_color: Color,
        #[default(Color::Unspecified)] disabled_content_color: Color,
        #[default(Color::Unspecified)] disabled_leading_content_color: Color,
        #[default(Color::Unspecified)] disabled_trailing_content_color: Color,
        #[default(Color::Unspecified)] disabled_overline_content_color: Color,
        #[default(Color::Unspecified)] disabled_supporting_content_color: Color,
        // selected
        #[default(Color::Unspecified)] selected_container_color: Color,
        #[default(Color::Unspecified)] selected_content_color: Color,
        #[default(Color::Unspecified)] selected_leading_content_color: Color,
        #[default(Color::Unspecified)] selected_trailing_content_color: Color,
        #[default(Color::Unspecified)] selected_overline_content_color: Color,
        #[default(Color::Unspecified)] selected_supporting_content_color: Color,
        // dragged
        #[default(Color::Unspecified)] dragged_container_color: Color,
        #[default(Color::Unspecified)] dragged_content_color: Color,
        #[default(Color::Unspecified)] dragged_leading_content_color: Color,
        #[default(Color::Unspecified)] dragged_trailing_content_color: Color,
        #[default(Color::Unspecified)] dragged_overline_content_color: Color,
        #[default(Color::Unspecified)] dragged_supporting_content_color: Color,
    ) -> ListItemColors {
        ListItemDefaults::default_list_item_colors(&MaterialTheme::color_scheme()).copy_with(ListItemColors {
            container_color,
            content_color,
            leading_content_color,
            trailing_content_color,
            overline_content_color,
            supporting_content_color,
            disabled_container_color,
            disabled_content_color,
            disabled_leading_content_color,
            disabled_trailing_content_color,
            disabled_overline_content_color,
            disabled_supporting_content_color,
            selected_container_color,
            selected_content_color,
            selected_leading_content_color,
            selected_trailing_content_color,
            selected_overline_content_color,
            selected_supporting_content_color,
            dragged_container_color,
            dragged_content_color,
            dragged_leading_content_color,
            dragged_trailing_content_color,
            dragged_overline_content_color,
            dragged_supporting_content_color,
        })
    }

    /// A list item's colors by their older names (Compose's `ListItemDefaults.colors(containerColor, headlineColor,
    /// …)`, which Kotlin calls for the colors both have): the theme's, each given other than unspecified instead
    /// (`headline_color` the content color, the icon colors the leading and trailing content's).
    ///
    /// Kotlin: `colors(containerColor: Color = Color.Unspecified, headlineColor: Color = Color.Unspecified, leadingIconColor: Color = Color.Unspecified, overlineColor: Color = Color.Unspecified, supportingColor: Color = Color.Unspecified, trailingIconColor: Color = Color.Unspecified, disabledHeadlineColor: Color = Color.Unspecified, disabledLeadingIconColor: Color = Color.Unspecified, disabledTrailingIconColor: Color = Color.Unspecified): ListItemColors`
    #[allow(clippy::too_many_arguments)]
    pub fn colors_with_headline_color(
        &self,
        #[default(Color::Unspecified)] container_color: Color,
        #[default(Color::Unspecified)] headline_color: Color,
        #[default(Color::Unspecified)] leading_icon_color: Color,
        #[default(Color::Unspecified)] overline_color: Color,
        #[default(Color::Unspecified)] supporting_color: Color,
        #[default(Color::Unspecified)] trailing_icon_color: Color,
        #[default(Color::Unspecified)] disabled_headline_color: Color,
        #[default(Color::Unspecified)] disabled_leading_icon_color: Color,
        #[default(Color::Unspecified)] disabled_trailing_icon_color: Color,
    ) -> ListItemColors {
        let u = Color::Unspecified;
        ListItemDefaults::default_list_item_colors(&MaterialTheme::color_scheme()).copy_with(ListItemColors {
            container_color,
            content_color: headline_color,
            leading_content_color: leading_icon_color,
            overline_content_color: overline_color,
            supporting_content_color: supporting_color,
            trailing_content_color: trailing_icon_color,
            disabled_content_color: disabled_headline_color,
            disabled_leading_content_color: disabled_leading_icon_color,
            disabled_trailing_content_color: disabled_trailing_icon_color,
            ..ListItemColors::all(u)
        })
    }

    /// A segmented list item's colors (Compose's Expressive `ListItemDefaults.segmentedColors()` and
    /// `segmentedColors(containerColor, …)`): the theme's
    /// ([`default_segmented_list_item_colors`](Self::default_segmented_list_item_colors)), each color given other
    /// than unspecified instead.
    ///
    /// Kotlin: `segmentedColors(containerColor: Color = Color.Unspecified, contentColor: Color = Color.Unspecified, leadingContentColor: Color = Color.Unspecified, trailingContentColor: Color = Color.Unspecified, overlineContentColor: Color = Color.Unspecified, supportingContentColor: Color = Color.Unspecified, disabledContainerColor: Color = Color.Unspecified, disabledContentColor: Color = Color.Unspecified, disabledLeadingContentColor: Color = Color.Unspecified, disabledTrailingContentColor: Color = Color.Unspecified, disabledOverlineContentColor: Color = Color.Unspecified, disabledSupportingContentColor: Color = Color.Unspecified, selectedContainerColor: Color = Color.Unspecified, selectedContentColor: Color = Color.Unspecified, selectedLeadingContentColor: Color = Color.Unspecified, selectedTrailingContentColor: Color = Color.Unspecified, selectedOverlineContentColor: Color = Color.Unspecified, selectedSupportingContentColor: Color = Color.Unspecified, draggedContainerColor: Color = Color.Unspecified, draggedContentColor: Color = Color.Unspecified, draggedLeadingContentColor: Color = Color.Unspecified, draggedTrailingContentColor: Color = Color.Unspecified, draggedOverlineContentColor: Color = Color.Unspecified, draggedSupportingContentColor: Color = Color.Unspecified): ListItemColors`
    #[allow(clippy::too_many_arguments)]
    pub fn segmented_colors(
        &self,
        // default
        #[default(Color::Unspecified)] container_color: Color,
        #[default(Color::Unspecified)] content_color: Color,
        #[default(Color::Unspecified)] leading_content_color: Color,
        #[default(Color::Unspecified)] trailing_content_color: Color,
        #[default(Color::Unspecified)] overline_content_color: Color,
        #[default(Color::Unspecified)] supporting_content_color: Color,
        // disabled
        #[default(Color::Unspecified)] disabled_container_color: Color,
        #[default(Color::Unspecified)] disabled_content_color: Color,
        #[default(Color::Unspecified)] disabled_leading_content_color: Color,
        #[default(Color::Unspecified)] disabled_trailing_content_color: Color,
        #[default(Color::Unspecified)] disabled_overline_content_color: Color,
        #[default(Color::Unspecified)] disabled_supporting_content_color: Color,
        // selected
        #[default(Color::Unspecified)] selected_container_color: Color,
        #[default(Color::Unspecified)] selected_content_color: Color,
        #[default(Color::Unspecified)] selected_leading_content_color: Color,
        #[default(Color::Unspecified)] selected_trailing_content_color: Color,
        #[default(Color::Unspecified)] selected_overline_content_color: Color,
        #[default(Color::Unspecified)] selected_supporting_content_color: Color,
        // dragged
        #[default(Color::Unspecified)] dragged_container_color: Color,
        #[default(Color::Unspecified)] dragged_content_color: Color,
        #[default(Color::Unspecified)] dragged_leading_content_color: Color,
        #[default(Color::Unspecified)] dragged_trailing_content_color: Color,
        #[default(Color::Unspecified)] dragged_overline_content_color: Color,
        #[default(Color::Unspecified)] dragged_supporting_content_color: Color,
    ) -> ListItemColors {
        ListItemDefaults::default_segmented_list_item_colors(&MaterialTheme::color_scheme()).copy_with(ListItemColors {
            container_color,
            content_color,
            leading_content_color,
            trailing_content_color,
            overline_content_color,
            supporting_content_color,
            disabled_container_color,
            disabled_content_color,
            disabled_leading_content_color,
            disabled_trailing_content_color,
            disabled_overline_content_color,
            disabled_supporting_content_color,
            selected_container_color,
            selected_content_color,
            selected_leading_content_color,
            selected_trailing_content_color,
            selected_overline_content_color,
            selected_supporting_content_color,
            dragged_container_color,
            dragged_content_color,
            dragged_leading_content_color,
            dragged_trailing_content_color,
            dragged_overline_content_color,
            dragged_supporting_content_color,
        })
    }

    /// An interactive list item's shapes (Compose's Expressive `ListItemDefaults.shapes()` and `shapes(shape, …)`):
    /// the theme's ([`default_list_item_shapes`](Self::default_list_item_shapes)), each one given instead (`None`
    /// keeps the theme's).
    ///
    /// Kotlin: `shapes(shape: Shape? = null, selectedShape: Shape? = null, pressedShape: Shape? = null, focusedShape: Shape? = null, hoveredShape: Shape? = null, draggedShape: Shape? = null): ListItemShapes`
    pub fn shapes(
        &self,
        #[default(None)] shape: Option<Arc<dyn Shape>>,
        #[default(None)] selected_shape: Option<Arc<dyn Shape>>,
        #[default(None)] pressed_shape: Option<Arc<dyn Shape>>,
        #[default(None)] focused_shape: Option<Arc<dyn Shape>>,
        #[default(None)] hovered_shape: Option<Arc<dyn Shape>>,
        #[default(None)] dragged_shape: Option<Arc<dyn Shape>>,
    ) -> ListItemShapes {
        named_args!(ListItemDefaults::default_list_item_shapes(&MaterialTheme::shapes()).copy(
            shape = shape,
            selected_shape = selected_shape,
            pressed_shape = pressed_shape,
            focused_shape = focused_shape,
            hovered_shape = hovered_shape,
            dragged_shape = dragged_shape,
        ))
    }

    /// The shapes of the item at `index` of `count` in a segmented list (Compose's Expressive
    /// `ListItemDefaults.segmentedShapes`): `default_shapes` (the theme's by default), the first item's top corners
    /// and the last item's bottom corners those of a list's container (large) where both are corner based shapes;
    /// a lone item's as they are. Remembered for these.
    ///
    /// Kotlin: `segmentedShapes(index: Int, count: Int, defaultShapes: ListItemShapes = shapes()): ListItemShapes`
    #[track_caller]
    pub fn segmented_shapes(
        &self,
        index: i32,
        count: i32,
        #[default(named_args!(ListItemDefaults.shapes()))] default_shapes: ListItemShapes,
    ) -> ListItemShapes {
        let override_shape = ListTokens::ContainerShape.value();
        kompozable_block!({
            remember_keyed((index, count, default_shapes.clone(), ShapeKey(override_shape.clone())), move || {
                let base = || (corner_based_shape_of(&*default_shapes.shape), corner_based_shape_of(&*override_shape));
                if count == 1 {
                    default_shapes
                } else if index == 0 {
                    match base() {
                        (Some(default_base_shape), Some(override_shape)) => {
                            let shape = named_args!(default_base_shape.copy(
                                top_start = override_shape.top_start().clone(),
                                top_end = override_shape.top_end().clone(),
                            ));
                            named_args!(default_shapes.copy(shape = Some(Arc::new(shape) as Arc<dyn Shape>)))
                        }
                        _ => default_shapes,
                    }
                } else if index == count - 1 {
                    match base() {
                        (Some(default_base_shape), Some(override_shape)) => {
                            let shape = named_args!(default_base_shape.copy(
                                bottom_start = override_shape.bottom_start().clone(),
                                bottom_end = override_shape.bottom_end().clone(),
                            ));
                            named_args!(default_shapes.copy(shape = Some(Arc::new(shape) as Arc<dyn Shape>)))
                        }
                        _ => default_shapes,
                    }
                } else {
                    default_shapes
                }
            })
        })
    }

    /// An interactive list item's elevations (Compose's Expressive `ListItemDefaults.elevation`): none at rest, 8 dp
    /// (level 4) dragged.
    ///
    /// Kotlin: `elevation(elevation: Dp = ListTokens.ItemContainerElevation, draggedElevation: Dp = ListTokens.ItemDraggedContainerElevation): ListItemElevation`
    pub fn elevation(
        &self,
        #[default(ListTokens::ItemContainerElevation)] elevation: Dp,
        #[default(ListTokens::ItemDraggedContainerElevation)] dragged_elevation: Dp,
    ) -> ListItemElevation {
        ListItemElevation::new(elevation, dragged_elevation)
    }
}

impl ListItemDefaults {
    /// How an interactive list item puts its parts in its height (Compose's Expressive
    /// `ListItemDefaults.verticalAlignment()`): centered while the room is shorter than between a two and a three
    /// line item's (less its padding), at the top from there. Remembered for the density.
    #[track_caller]
    pub fn vertical_alignment(&self) -> VerticalAlignment {
        let density = LocalDensity.current();
        kompozable_block!({
            remember_keyed(density, move || {
                VerticalAlignment::custom(move |size, space| {
                    let breakpoint = density.round_to_px(InteractiveListVerticalAlignmentBreakpoint);
                    let base_alignment = if space < breakpoint { Alignment::CenterVertically } else { Alignment::Top };
                    base_alignment.align(size, space)
                })
            })
        })
    }
}

/// A shape compared as it is (a remembered value's key).
struct ShapeKey(Arc<dyn Shape>);

impl PartialEq for ShapeKey {
    fn eq(&self, other: &Self) -> bool {
        *self.0 == *other.0
    }
}

// ---- ListItemColors --------------------------------------------------------------------------------------------

/// A list item's colors (Compose's `ListItemColors`): its container's and each slot's — the headline (`content`),
/// the leading and trailing content, the overline and the supporting content — at rest, disabled, selected and
/// dragged. Equal by its colors.
#[derive(Clone, Copy, PartialEq, Debug, KotlinProperties)]
pub struct ListItemColors {
    // default
    pub container_color: Color,
    pub content_color: Color,
    pub leading_content_color: Color,
    pub trailing_content_color: Color,
    pub overline_content_color: Color,
    pub supporting_content_color: Color,
    // disabled
    pub disabled_container_color: Color,
    pub disabled_content_color: Color,
    pub disabled_leading_content_color: Color,
    pub disabled_trailing_content_color: Color,
    pub disabled_overline_content_color: Color,
    pub disabled_supporting_content_color: Color,
    // selected
    pub selected_container_color: Color,
    pub selected_content_color: Color,
    pub selected_leading_content_color: Color,
    pub selected_trailing_content_color: Color,
    pub selected_overline_content_color: Color,
    pub selected_supporting_content_color: Color,
    // dragged
    pub dragged_container_color: Color,
    pub dragged_content_color: Color,
    pub dragged_leading_content_color: Color,
    pub dragged_trailing_content_color: Color,
    pub dragged_overline_content_color: Color,
    pub dragged_supporting_content_color: Color,
}

impl ListItemColors {
    /// Kotlin: `ListItemColors(containerColor: Color, contentColor: Color, leadingContentColor: Color, trailingContentColor: Color, overlineContentColor: Color, supportingContentColor: Color, disabledContainerColor: Color, disabledContentColor: Color, disabledLeadingContentColor: Color, disabledTrailingContentColor: Color, disabledOverlineContentColor: Color, disabledSupportingContentColor: Color, selectedContainerColor: Color, selectedContentColor: Color, selectedLeadingContentColor: Color, selectedTrailingContentColor: Color, selectedOverlineContentColor: Color, selectedSupportingContentColor: Color, draggedContainerColor: Color, draggedContentColor: Color, draggedLeadingContentColor: Color, draggedTrailingContentColor: Color, draggedOverlineContentColor: Color, draggedSupportingContentColor: Color)`
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        container_color: Color,
        content_color: Color,
        leading_content_color: Color,
        trailing_content_color: Color,
        overline_content_color: Color,
        supporting_content_color: Color,
        disabled_container_color: Color,
        disabled_content_color: Color,
        disabled_leading_content_color: Color,
        disabled_trailing_content_color: Color,
        disabled_overline_content_color: Color,
        disabled_supporting_content_color: Color,
        selected_container_color: Color,
        selected_content_color: Color,
        selected_leading_content_color: Color,
        selected_trailing_content_color: Color,
        selected_overline_content_color: Color,
        selected_supporting_content_color: Color,
        dragged_container_color: Color,
        dragged_content_color: Color,
        dragged_leading_content_color: Color,
        dragged_trailing_content_color: Color,
        dragged_overline_content_color: Color,
        dragged_supporting_content_color: Color,
    ) -> ListItemColors {
        ListItemColors {
            container_color,
            content_color,
            leading_content_color,
            trailing_content_color,
            overline_content_color,
            supporting_content_color,
            disabled_container_color,
            disabled_content_color,
            disabled_leading_content_color,
            disabled_trailing_content_color,
            disabled_overline_content_color,
            disabled_supporting_content_color,
            selected_container_color,
            selected_content_color,
            selected_leading_content_color,
            selected_trailing_content_color,
            selected_overline_content_color,
            selected_supporting_content_color,
            dragged_container_color,
            dragged_content_color,
            dragged_leading_content_color,
            dragged_trailing_content_color,
            dragged_overline_content_color,
            dragged_supporting_content_color,
        }
    }

    /// Colors by their older names (Compose's secondary constructor): `headline_color` the content color, the icon
    /// colors the leading and trailing content's; the disabled container, overline and supporting colors and all
    /// selected and dragged ones unspecified.
    ///
    /// Kotlin: `ListItemColors(containerColor: Color, headlineColor: Color, leadingIconColor: Color, overlineColor: Color, supportingTextColor: Color, trailingIconColor: Color, disabledHeadlineColor: Color, disabledLeadingIconColor: Color, disabledTrailingIconColor: Color)`
    #[allow(clippy::too_many_arguments)]
    pub fn new_with_headline_color(
        container_color: Color,
        headline_color: Color,
        leading_icon_color: Color,
        overline_color: Color,
        supporting_text_color: Color,
        trailing_icon_color: Color,
        disabled_headline_color: Color,
        disabled_leading_icon_color: Color,
        disabled_trailing_icon_color: Color,
    ) -> ListItemColors {
        ListItemColors {
            // default
            container_color,
            content_color: headline_color,
            leading_content_color: leading_icon_color,
            trailing_content_color: trailing_icon_color,
            overline_content_color: overline_color,
            supporting_content_color: supporting_text_color,
            // disabled
            disabled_content_color: disabled_headline_color,
            disabled_leading_content_color: disabled_leading_icon_color,
            disabled_trailing_content_color: disabled_trailing_icon_color,
            ..ListItemColors::all(Color::Unspecified)
        }
    }

    /// Every color `color`.
    fn all(color: Color) -> ListItemColors {
        ListItemColors {
            container_color: color,
            content_color: color,
            leading_content_color: color,
            trailing_content_color: color,
            overline_content_color: color,
            supporting_content_color: color,
            disabled_container_color: color,
            disabled_content_color: color,
            disabled_leading_content_color: color,
            disabled_trailing_content_color: color,
            disabled_overline_content_color: color,
            disabled_supporting_content_color: color,
            selected_container_color: color,
            selected_content_color: color,
            selected_leading_content_color: color,
            selected_trailing_content_color: color,
            selected_overline_content_color: color,
            selected_supporting_content_color: color,
            dragged_container_color: color,
            dragged_content_color: color,
            dragged_leading_content_color: color,
            dragged_trailing_content_color: color,
            dragged_overline_content_color: color,
            dragged_supporting_content_color: color,
        }
    }

    /// The color of a state: disabled first, then dragged, then selected, else at rest (each of Compose's color
    /// functions).
    fn by_state(enabled: bool, selected: bool, dragged: bool, colors: [Color; 4]) -> Color {
        let [rest, disabled, selected_color, dragged_color] = colors;
        if !enabled {
            disabled
        } else if dragged {
            dragged_color
        } else if selected {
            selected_color
        } else {
            rest
        }
    }

    /// The container's color in a state (Compose's `containerColor(enabled, selected, dragged)`; named apart from
    /// the property's getter here).
    ///
    /// Kotlin: `containerColor(enabled: Boolean, selected: Boolean, dragged: Boolean): Color`
    pub fn container_color_for(&self, enabled: bool, selected: bool, dragged: bool) -> Color {
        let c = self;
        ListItemColors::by_state(
            enabled,
            selected,
            dragged,
            [c.container_color, c.disabled_container_color, c.selected_container_color, c.dragged_container_color],
        )
    }

    /// The headline's color in a state (Compose's `contentColor(enabled, selected, dragged)`).
    ///
    /// Kotlin: `contentColor(enabled: Boolean, selected: Boolean, dragged: Boolean): Color`
    pub fn content_color_for(&self, enabled: bool, selected: bool, dragged: bool) -> Color {
        let c = self;
        ListItemColors::by_state(
            enabled,
            selected,
            dragged,
            [c.content_color, c.disabled_content_color, c.selected_content_color, c.dragged_content_color],
        )
    }

    /// The leading content's color in a state (Compose's `leadingContentColor(enabled, selected, dragged)`).
    ///
    /// Kotlin: `leadingContentColor(enabled: Boolean, selected: Boolean, dragged: Boolean): Color`
    pub fn leading_content_color_for(&self, enabled: bool, selected: bool, dragged: bool) -> Color {
        let c = self;
        ListItemColors::by_state(
            enabled,
            selected,
            dragged,
            [
                c.leading_content_color,
                c.disabled_leading_content_color,
                c.selected_leading_content_color,
                c.dragged_leading_content_color,
            ],
        )
    }

    /// The trailing content's color in a state (Compose's `trailingContentColor(enabled, selected, dragged)`).
    ///
    /// Kotlin: `trailingContentColor(enabled: Boolean, selected: Boolean, dragged: Boolean): Color`
    pub fn trailing_content_color_for(&self, enabled: bool, selected: bool, dragged: bool) -> Color {
        let c = self;
        ListItemColors::by_state(
            enabled,
            selected,
            dragged,
            [
                c.trailing_content_color,
                c.disabled_trailing_content_color,
                c.selected_trailing_content_color,
                c.dragged_trailing_content_color,
            ],
        )
    }

    /// The overline's color in a state (Compose's `overlineContentColor(enabled, selected, dragged)`).
    ///
    /// Kotlin: `overlineContentColor(enabled: Boolean, selected: Boolean, dragged: Boolean): Color`
    pub fn overline_content_color_for(&self, enabled: bool, selected: bool, dragged: bool) -> Color {
        let c = self;
        ListItemColors::by_state(
            enabled,
            selected,
            dragged,
            [
                c.overline_content_color,
                c.disabled_overline_content_color,
                c.selected_overline_content_color,
                c.dragged_overline_content_color,
            ],
        )
    }

    /// The supporting content's color in a state (Compose's `supportingContentColor(enabled, selected, dragged)`).
    ///
    /// Kotlin: `supportingContentColor(enabled: Boolean, selected: Boolean, dragged: Boolean): Color`
    pub fn supporting_content_color_for(&self, enabled: bool, selected: bool, dragged: bool) -> Color {
        let c = self;
        ListItemColors::by_state(
            enabled,
            selected,
            dragged,
            [
                c.supporting_content_color,
                c.disabled_supporting_content_color,
                c.selected_supporting_content_color,
                c.dragged_supporting_content_color,
            ],
        )
    }

    /// These colors, each of `other`'s other than unspecified instead (the body of Compose's `copy`).
    pub(crate) fn copy_with(&self, other: ListItemColors) -> ListItemColors {
        let pick = |given: Color, own: Color| given.take_or_else(|| own);
        let (o, s) = (other, self);
        ListItemColors {
            container_color: pick(o.container_color, s.container_color),
            content_color: pick(o.content_color, s.content_color),
            leading_content_color: pick(o.leading_content_color, s.leading_content_color),
            trailing_content_color: pick(o.trailing_content_color, s.trailing_content_color),
            overline_content_color: pick(o.overline_content_color, s.overline_content_color),
            supporting_content_color: pick(o.supporting_content_color, s.supporting_content_color),
            disabled_container_color: pick(o.disabled_container_color, s.disabled_container_color),
            disabled_content_color: pick(o.disabled_content_color, s.disabled_content_color),
            disabled_leading_content_color: pick(o.disabled_leading_content_color, s.disabled_leading_content_color),
            disabled_trailing_content_color: pick(o.disabled_trailing_content_color, s.disabled_trailing_content_color),
            disabled_overline_content_color: pick(o.disabled_overline_content_color, s.disabled_overline_content_color),
            disabled_supporting_content_color: pick(
                o.disabled_supporting_content_color,
                s.disabled_supporting_content_color,
            ),
            selected_container_color: pick(o.selected_container_color, s.selected_container_color),
            selected_content_color: pick(o.selected_content_color, s.selected_content_color),
            selected_leading_content_color: pick(o.selected_leading_content_color, s.selected_leading_content_color),
            selected_trailing_content_color: pick(o.selected_trailing_content_color, s.selected_trailing_content_color),
            selected_overline_content_color: pick(o.selected_overline_content_color, s.selected_overline_content_color),
            selected_supporting_content_color: pick(
                o.selected_supporting_content_color,
                s.selected_supporting_content_color,
            ),
            dragged_container_color: pick(o.dragged_container_color, s.dragged_container_color),
            dragged_content_color: pick(o.dragged_content_color, s.dragged_content_color),
            dragged_leading_content_color: pick(o.dragged_leading_content_color, s.dragged_leading_content_color),
            dragged_trailing_content_color: pick(o.dragged_trailing_content_color, s.dragged_trailing_content_color),
            dragged_overline_content_color: pick(o.dragged_overline_content_color, s.dragged_overline_content_color),
            dragged_supporting_content_color: pick(
                o.dragged_supporting_content_color,
                s.dragged_supporting_content_color,
            ),
        }
    }

    /// The content color (Compose's deprecated `headlineColor`, renamed to `contentColor`).
    #[deprecated(note = "Renamed to contentColor")]
    pub fn headline_color(&self) -> Color {
        self.content_color
    }

    /// The leading content's color (Compose's deprecated `leadingIconColor`).
    #[deprecated(note = "Renamed to leadingContentColor")]
    pub fn leading_icon_color(&self) -> Color {
        self.leading_content_color
    }

    /// The overline's color (Compose's deprecated `overlineColor`).
    #[deprecated(note = "Renamed to overlineContentColor")]
    pub fn overline_color(&self) -> Color {
        self.overline_content_color
    }

    /// The supporting content's color (Compose's deprecated `supportingTextColor`).
    #[deprecated(note = "Renamed to supportingContentColor")]
    pub fn supporting_text_color(&self) -> Color {
        self.supporting_content_color
    }

    /// The trailing content's color (Compose's deprecated `trailingIconColor`).
    #[deprecated(note = "Renamed to trailingContentColor")]
    pub fn trailing_icon_color(&self) -> Color {
        self.trailing_content_color
    }

    /// The disabled content color (Compose's deprecated `disabledHeadlineColor`).
    #[deprecated(note = "Renamed to disabledContentColor")]
    pub fn disabled_headline_color(&self) -> Color {
        self.disabled_content_color
    }

    /// The disabled leading content's color (Compose's deprecated `disabledLeadingIconColor`).
    #[deprecated(note = "Renamed to disabledLeadingContentColor")]
    pub fn disabled_leading_icon_color(&self) -> Color {
        self.disabled_leading_content_color
    }

    /// The disabled trailing content's color (Compose's deprecated `disabledTrailingIconColor`).
    #[deprecated(note = "Renamed to disabledTrailingContentColor")]
    pub fn disabled_trailing_icon_color(&self) -> Color {
        self.disabled_trailing_content_color
    }
}

#[named]
impl ListItemColors {
    /// The same with each color given one other than unspecified (Compose's `copy`).
    #[allow(clippy::too_many_arguments)]
    pub fn copy(
        &self,
        // default
        #[default(self.container_color)] container_color: Color,
        #[default(self.content_color)] content_color: Color,
        #[default(self.leading_content_color)] leading_content_color: Color,
        #[default(self.trailing_content_color)] trailing_content_color: Color,
        #[default(self.overline_content_color)] overline_content_color: Color,
        #[default(self.supporting_content_color)] supporting_content_color: Color,
        // disabled
        #[default(self.disabled_container_color)] disabled_container_color: Color,
        #[default(self.disabled_content_color)] disabled_content_color: Color,
        #[default(self.disabled_leading_content_color)] disabled_leading_content_color: Color,
        #[default(self.disabled_trailing_content_color)] disabled_trailing_content_color: Color,
        #[default(self.disabled_overline_content_color)] disabled_overline_content_color: Color,
        #[default(self.disabled_supporting_content_color)] disabled_supporting_content_color: Color,
        // selected
        #[default(self.selected_container_color)] selected_container_color: Color,
        #[default(self.selected_content_color)] selected_content_color: Color,
        #[default(self.selected_leading_content_color)] selected_leading_content_color: Color,
        #[default(self.selected_trailing_content_color)] selected_trailing_content_color: Color,
        #[default(self.selected_overline_content_color)] selected_overline_content_color: Color,
        #[default(self.selected_supporting_content_color)] selected_supporting_content_color: Color,
        // dragged
        #[default(self.dragged_container_color)] dragged_container_color: Color,
        #[default(self.dragged_content_color)] dragged_content_color: Color,
        #[default(self.dragged_leading_content_color)] dragged_leading_content_color: Color,
        #[default(self.dragged_trailing_content_color)] dragged_trailing_content_color: Color,
        #[default(self.dragged_overline_content_color)] dragged_overline_content_color: Color,
        #[default(self.dragged_supporting_content_color)] dragged_supporting_content_color: Color,
    ) -> ListItemColors {
        self.copy_with(ListItemColors {
            container_color,
            content_color,
            leading_content_color,
            trailing_content_color,
            overline_content_color,
            supporting_content_color,
            disabled_container_color,
            disabled_content_color,
            disabled_leading_content_color,
            disabled_trailing_content_color,
            disabled_overline_content_color,
            disabled_supporting_content_color,
            selected_container_color,
            selected_content_color,
            selected_leading_content_color,
            selected_trailing_content_color,
            selected_overline_content_color,
            selected_supporting_content_color,
            dragged_container_color,
            dragged_content_color,
            dragged_leading_content_color,
            dragged_trailing_content_color,
            dragged_overline_content_color,
            dragged_supporting_content_color,
        })
    }
}

#[named]
impl ListItemColors {
    /// The same with the colors of the older names given (Compose's deprecated `copy(containerColor, headlineColor,
    /// …)`): a new value of [`new_with_headline_color`](Self::new_with_headline_color) — its selected and dragged
    /// colors and the disabled container, overline and supporting ones unspecified, as Compose's.
    ///
    /// Kotlin: `copy(containerColor: Color = this.containerColor, headlineColor: Color = this.headlineColor, leadingIconColor: Color = this.leadingIconColor, overlineColor: Color = this.overlineColor, supportingTextColor: Color = this.supportingTextColor, trailingIconColor: Color = this.trailingIconColor, disabledHeadlineColor: Color = this.disabledHeadlineColor, disabledLeadingIconColor: Color = this.disabledLeadingIconColor, disabledTrailingIconColor: Color = this.disabledTrailingIconColor): ListItemColors`
    #[deprecated(note = "Use overload with parameters for selected and dragged colors")]
    #[allow(clippy::too_many_arguments)]
    pub fn copy_with_headline_color(
        &self,
        #[default(self.container_color)] container_color: Color,
        #[default(self.content_color)] headline_color: Color,
        #[default(self.leading_content_color)] leading_icon_color: Color,
        #[default(self.overline_content_color)] overline_color: Color,
        #[default(self.supporting_content_color)] supporting_text_color: Color,
        #[default(self.trailing_content_color)] trailing_icon_color: Color,
        #[default(self.disabled_content_color)] disabled_headline_color: Color,
        #[default(self.disabled_leading_content_color)] disabled_leading_icon_color: Color,
        #[default(self.disabled_trailing_content_color)] disabled_trailing_icon_color: Color,
    ) -> ListItemColors {
        ListItemColors::new_with_headline_color(
            container_color.take_or_else(|| self.container_color),
            headline_color.take_or_else(|| self.content_color),
            leading_icon_color.take_or_else(|| self.leading_content_color),
            overline_color.take_or_else(|| self.overline_content_color),
            supporting_text_color.take_or_else(|| self.supporting_content_color),
            trailing_icon_color.take_or_else(|| self.trailing_content_color),
            disabled_headline_color.take_or_else(|| self.disabled_content_color),
            disabled_leading_icon_color.take_or_else(|| self.disabled_leading_content_color),
            disabled_trailing_icon_color.take_or_else(|| self.disabled_trailing_content_color),
        )
    }
}

// ---- ListItemShapes --------------------------------------------------------------------------------------------

/// The shapes an Expressive interactive list item morphs between (Compose's Expressive `ListItemShapes`): at rest,
/// selected, pressed, focused, hovered and dragged. Equal by its shapes.
#[derive(Clone, KotlinProperties)]
pub struct ListItemShapes {
    pub shape: Arc<dyn Shape>,
    pub selected_shape: Arc<dyn Shape>,
    pub pressed_shape: Arc<dyn Shape>,
    pub focused_shape: Arc<dyn Shape>,
    pub hovered_shape: Arc<dyn Shape>,
    pub dragged_shape: Arc<dyn Shape>,
}

impl ListItemShapes {
    /// Kotlin: `ListItemShapes(shape: Shape, selectedShape: Shape, pressedShape: Shape, focusedShape: Shape, hoveredShape: Shape, draggedShape: Shape)`
    pub fn new(
        shape: Arc<dyn Shape>,
        selected_shape: Arc<dyn Shape>,
        pressed_shape: Arc<dyn Shape>,
        focused_shape: Arc<dyn Shape>,
        hovered_shape: Arc<dyn Shape>,
        dragged_shape: Arc<dyn Shape>,
    ) -> ListItemShapes {
        ListItemShapes { shape, selected_shape, pressed_shape, focused_shape, hovered_shape, dragged_shape }
    }

    /// Whether all its shapes are rounded corner shapes (Compose's private `hasRoundedCornerShapes`).
    fn has_rounded_corner_shapes(&self) -> bool {
        self.all().iter().all(|s| rounded_corner_shape_of(&***s).is_some())
    }

    /// Whether all its shapes are corner based (Compose's private `hasCornerBasedShapes`).
    fn has_corner_based_shapes(&self) -> bool {
        self.all().iter().all(|s| corner_based_shape_of(&***s).is_some())
    }

    fn all(&self) -> [&Arc<dyn Shape>; 6] {
        [
            &self.shape,
            &self.selected_shape,
            &self.pressed_shape,
            &self.focused_shape,
            &self.hovered_shape,
            &self.dragged_shape,
        ]
    }
}

#[named]
impl ListItemShapes {
    /// The same with the shapes given (Compose's `copy`: `None` keeps its own).
    pub fn copy(
        &self,
        #[default(None)] shape: Option<Arc<dyn Shape>>,
        #[default(None)] selected_shape: Option<Arc<dyn Shape>>,
        #[default(None)] pressed_shape: Option<Arc<dyn Shape>>,
        #[default(None)] focused_shape: Option<Arc<dyn Shape>>,
        #[default(None)] hovered_shape: Option<Arc<dyn Shape>>,
        #[default(None)] dragged_shape: Option<Arc<dyn Shape>>,
    ) -> ListItemShapes {
        ListItemShapes::new(
            shape.unwrap_or_else(|| self.shape.clone()),
            selected_shape.unwrap_or_else(|| self.selected_shape.clone()),
            pressed_shape.unwrap_or_else(|| self.pressed_shape.clone()),
            focused_shape.unwrap_or_else(|| self.focused_shape.clone()),
            hovered_shape.unwrap_or_else(|| self.hovered_shape.clone()),
            dragged_shape.unwrap_or_else(|| self.dragged_shape.clone()),
        )
    }
}

impl PartialEq for ListItemShapes {
    fn eq(&self, other: &Self) -> bool {
        self.all().iter().zip(other.all()).all(|(a, b)| ***a == **b)
    }
}

impl fmt::Debug for ListItemShapes {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("ListItemShapes")
            .field("shape", &self.shape)
            .field("selected_shape", &self.selected_shape)
            .field("pressed_shape", &self.pressed_shape)
            .field("focused_shape", &self.focused_shape)
            .field("hovered_shape", &self.hovered_shape)
            .field("dragged_shape", &self.dragged_shape)
            .finish()
    }
}

/// The shape of an interactive list item in its state (Compose's internal `ListItemShapes.shapeForInteraction`):
/// pressed first, then dragged, selected, focused, hovered, else at rest; morphing corner by corner along
/// `animation_spec` where all of `shapes` are corner based (rounded ones as rounded), a state of its own for each
/// `shapes` (Compose's `key(this)`); else the one of the state.
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub(crate) fn shape_for_interaction(
    shapes: ListItemShapes,
    selected: bool,
    pressed: bool,
    focused: bool,
    hovered: bool,
    dragged: bool,
    animation_spec: Arc<dyn FiniteAnimationSpec<f32>>,
) -> Arc<dyn Shape> {
    let shape = if pressed {
        shapes.pressed_shape.clone()
    } else if dragged {
        shapes.dragged_shape.clone()
    } else if selected {
        shapes.selected_shape.clone()
    } else if focused {
        shapes.focused_shape.clone()
    } else if hovered {
        shapes.hovered_shape.clone()
    } else {
        shapes.shape.clone()
    };

    // (Compose's `key(this)` compares the shapes by `equals`: an identity of each run of equal shapes here)
    let seen = remember(|| Rc::new(RefCell::new((shapes.clone(), 0u64))));
    let generation = {
        let mut seen = seen.borrow_mut();
        if seen.0 != shapes {
            *seen = (shapes.clone(), seen.1 + 1);
        }
        seen.1
    };
    if shapes.has_rounded_corner_shapes() {
        let rounded = rounded_corner_shape_of(&*shape).expect("a rounded corner shape");
        return key(&generation, || remember_animated_shape(rounded, animation_spec));
    } else if shapes.has_corner_based_shapes() {
        let corner_based = corner_based_shape_of(&*shape).expect("a corner based shape");
        return key(&generation, || remember_animated_shape(corner_based, animation_spec));
    }

    shape
}

// ---- ListItemElevation -----------------------------------------------------------------------------------------

/// An Expressive interactive list item's elevations (Compose's Expressive `ListItemElevation`): its shadow at rest
/// and while dragged. Equal by its values.
#[derive(Clone, Copy, PartialEq, Debug, KotlinProperties)]
pub struct ListItemElevation {
    pub elevation: Dp,
    pub dragged_elevation: Dp,
}

impl ListItemElevation {
    /// Kotlin: `ListItemElevation(elevation: Dp, draggedElevation: Dp)`
    pub fn new(elevation: Dp, dragged_elevation: Dp) -> ListItemElevation {
        ListItemElevation { elevation, dragged_elevation }
    }
}
