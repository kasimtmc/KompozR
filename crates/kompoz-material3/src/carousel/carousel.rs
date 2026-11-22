//! The carousels and their defaults (Compose's `Carousel.kt`): the multi-browse, uncontained and centered hero
//! carousels over the internal [`Carousel`], a pager whose page size and snap position its keylines give, each item
//! masked and moved by the keylines as it scrolls.

use super::carousel_item_scope::CarouselItemScope;
use super::carousel_state::{CarouselItemDrawInfo, CarouselState};
use super::keyline_list::{Keyline, KeylineList, lerp_keyline};
use super::keyline_snap_position::{KeylineSnapPosition, get_snap_position_offset};
use super::keylines::{hero_keyline_list, multi_browse_keyline_list, uncontained_keyline_list};
use super::strategy::Strategy;
use kompoz_animation::{AnimationSpec, DecayAnimationSpec, Spring, SpringSpec};
use kompoz_foundation::gestures::snapping::{SnapLayoutInfoProvider, remember_snap_fling_behavior};
use kompoz_foundation::gestures::{Orientation, TargetedFlingBehavior};
use kompoz_foundation::pager::{
    HorizontalPager, PageSize, PageSizeCalculator, PagerDefaults, PagerSnapDistance, PagerSnapDistanceCalculator,
    VerticalPager,
};
use kompoz_foundation::prelude::*;
use kompoz_graphics::{
    Color, Constraints, Density, Dp, LayoutDirection, Offset, Outline, OutlineShape, Rect, Shape, Size,
};
use kompoz_runtime::kt::KtFloat;
use kompoz_runtime::{MutableState, kompozable, mutable_state_of, named, named_args, remember, remember_keyed};
use kompoz_ui::Modifier;
use kompoz_ui::layout::GraphicsLayerScope;
use kompoz_ui::platform::{LocalDensity, LocalLayoutDirection};
use kompoz_ui::semantics::Role;
use std::rc::Rc;
use std::sync::Arc;

/// A carousel item's content: given its scope and its index among all the items (Compose's
/// `@Composable CarouselItemScope.(itemIndex: Int) -> Unit`).
pub type CarouselItemContent = Rc<dyn Fn(&CarouselItemScope, i32)>;

// ---------------------------------------------------------------------------------------------------------------
// The carousels

/// A horizontal carousel of many items at once for quick browsing of small content such as album art or photo
/// thumbnails ([Material Design carousel](https://m3.material.io/components/carousel/overview); Compose's
/// `HorizontalMultiBrowseCarousel`). It may adjust the items' sizes so large, medium and small items fit the space
/// exactly in a pleasing arrangement, lays the items out at the large size and masks them by their scroll offset
/// so they grow and shrink smoothly between the sizes.
///
/// `preferred_item_width` is what large items would like to be (adjusted to fit a whole number of items: small
/// items first, between `min_small_item_width` and `max_small_item_width`, then medium ones, then large ones);
/// `item_spacing` separates the items; `fling_behavior` (None: [`CarouselDefaults::single_advance_fling_behavior`]
/// of `state`, Kotlin's default naming it) runs after a drag; `user_scroll_enabled` allows scrolling by gesture and
/// accessibility; `content_padding` pads the content after it is clipped (before the first item, after the last).
/// Kotlin: `HorizontalMultiBrowseCarousel(state: CarouselState, preferredItemWidth: Dp, modifier: Modifier = Modifier, itemSpacing: Dp = 0.dp, flingBehavior: TargetedFlingBehavior = CarouselDefaults.singleAdvanceFlingBehavior(state = state), userScrollEnabled: Boolean = true, minSmallItemWidth: Dp = CarouselDefaults.MinSmallItemSize, maxSmallItemWidth: Dp = CarouselDefaults.MaxSmallItemSize, contentPadding: PaddingValues = PaddingValues(0.dp), content: @Composable CarouselItemScope.(itemIndex: Int) -> Unit)`
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn HorizontalMultiBrowseCarousel(
    state: CarouselState,
    preferred_item_width: Dp,
    #[default(Modifier)] modifier: Modifier,
    #[default(Dp(0.0))] item_spacing: Dp,
    #[default(None)] fling_behavior: Option<Rc<dyn TargetedFlingBehavior>>,
    #[default(true)] user_scroll_enabled: bool,
    #[default(CarouselDefaults::MinSmallItemSize)] min_small_item_width: Dp,
    #[default(CarouselDefaults::MaxSmallItemSize)] max_small_item_width: Dp,
    #[default(PaddingValues::all(Dp(0.0)))] content_padding: PaddingValues,
    content: impl Fn(&CarouselItemScope, i32) + 'static,
) {
    let fling_behavior = match fling_behavior {
        Some(f) => f,
        None => CarouselDefaults.single_advance_fling_behavior(&state),
    };
    let density = LocalDensity.current();
    // (Compose's compiler remembers the lambda by what it captures)
    let keys = (density, preferred_item_width, state.clone(), min_small_item_width, max_small_item_width);
    let s = state.clone();
    let keyline_list: KeylineListFn = remember_keyed(keys, move || {
        Rc::new(move |available_space: f32, item_spacing_px: f32| {
            multi_browse_keyline_list(
                &density,
                available_space,
                density.to_px(preferred_item_width),
                item_spacing_px,
                s.pager_state().page_count_state.get()(),
                Some(density.to_px(min_small_item_width)),
                Some(density.to_px(max_small_item_width)),
            )
        })
    });
    Carousel(
        state,
        Orientation::Horizontal,
        keyline_list,
        content_padding,
        // 2: the most medium and small items a multi-browse carousel has, so the most visible non-focal ones.
        2,
        modifier,
        item_spacing,
        Some(fling_behavior),
        user_scroll_enabled,
        Rc::new(content),
    );
}

/// A horizontal carousel of items of `item_width` but one cut off at the end (Compose's
/// `HorizontalUncontainedCarousel`): as many items as fit at that width (at most the carousel's), the last cut-off
/// item sized so items moving off the edge still move. `fling_behavior` (None: [`CarouselDefaults::no_snap_fling_behavior`])
/// and the others as [`HorizontalMultiBrowseCarousel`]'s.
/// Kotlin: `HorizontalUncontainedCarousel(state: CarouselState, itemWidth: Dp, modifier: Modifier = Modifier, itemSpacing: Dp = 0.dp, flingBehavior: TargetedFlingBehavior = CarouselDefaults.noSnapFlingBehavior(), userScrollEnabled: Boolean = true, contentPadding: PaddingValues = PaddingValues(0.dp), content: @Composable CarouselItemScope.(itemIndex: Int) -> Unit)`
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn HorizontalUncontainedCarousel(
    state: CarouselState,
    item_width: Dp,
    #[default(Modifier)] modifier: Modifier,
    #[default(Dp(0.0))] item_spacing: Dp,
    #[default(CarouselDefaults.no_snap_fling_behavior())] fling_behavior: Rc<dyn TargetedFlingBehavior>,
    #[default(true)] user_scroll_enabled: bool,
    #[default(PaddingValues::all(Dp(0.0)))] content_padding: PaddingValues,
    content: impl Fn(&CarouselItemScope, i32) + 'static,
) {
    let density = LocalDensity.current();
    let keyline_list: KeylineListFn = remember_keyed((density, item_width), move || {
        Rc::new(move |available_space: f32, item_spacing_px: f32| {
            uncontained_keyline_list(&density, available_space, density.to_px(item_width), item_spacing_px)
        })
    });
    Carousel(
        state,
        Orientation::Horizontal,
        keyline_list,
        content_padding,
        // Only one item masks as it moves in and out of view: no more non-focal items.
        0,
        modifier,
        item_spacing,
        Some(fling_behavior),
        user_scroll_enabled,
        Rc::new(content),
    );
}

/// A horizontal carousel centering at least one large item between two small ones (Compose's
/// `HorizontalCenteredHeroCarousel`). `max_item_width`: [`Dp::Unspecified`] lets one large item fill the viewport
/// less two small items' room; another width adds large items as space allows (for an aspect ratio of at most 2:1,
/// the carousel's height × 2). The others as [`HorizontalMultiBrowseCarousel`]'s.
/// Kotlin: `HorizontalCenteredHeroCarousel(state: CarouselState, modifier: Modifier = Modifier, maxItemWidth: Dp = Dp.Unspecified, itemSpacing: Dp = 0.dp, flingBehavior: TargetedFlingBehavior = CarouselDefaults.singleAdvanceFlingBehavior(state = state), userScrollEnabled: Boolean = true, minSmallItemWidth: Dp = CarouselDefaults.MinSmallItemSize, maxSmallItemWidth: Dp = CarouselDefaults.MaxSmallItemSize, contentPadding: PaddingValues = PaddingValues(0.dp), content: @Composable CarouselItemScope.(itemIndex: Int) -> Unit)`
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn HorizontalCenteredHeroCarousel(
    state: CarouselState,
    #[default(Modifier)] modifier: Modifier,
    #[default(Dp::Unspecified)] max_item_width: Dp,
    #[default(Dp(0.0))] item_spacing: Dp,
    #[default(None)] fling_behavior: Option<Rc<dyn TargetedFlingBehavior>>,
    #[default(true)] user_scroll_enabled: bool,
    #[default(CarouselDefaults::MinSmallItemSize)] min_small_item_width: Dp,
    #[default(CarouselDefaults::MaxSmallItemSize)] max_small_item_width: Dp,
    #[default(PaddingValues::all(Dp(0.0)))] content_padding: PaddingValues,
    content: impl Fn(&CarouselItemScope, i32) + 'static,
) {
    let fling_behavior = match fling_behavior {
        Some(f) => f,
        None => CarouselDefaults.single_advance_fling_behavior(&state),
    };
    let density = LocalDensity.current();
    // (Dp's NaN Unspecified compared by its bits: Kotlin's Dp is equal to itself)
    let keys = (density, max_item_width.0.to_bits(), state.clone(), min_small_item_width, max_small_item_width);
    let s = state.clone();
    let keyline_list: KeylineListFn = remember_keyed(keys, move || {
        Rc::new(move |available_space: f32, item_spacing_px: f32| {
            hero_keyline_list(
                &density,
                available_space,
                if max_item_width.is_specified() { Some(density.to_px(max_item_width)) } else { None },
                item_spacing_px,
                s.pager_state().page_count_state.get()(),
                true,
                Some(density.to_px(min_small_item_width)),
                Some(density.to_px(max_small_item_width)),
            )
        })
    });
    Carousel(
        state,
        Orientation::Horizontal,
        keyline_list,
        content_padding,
        // 2: the most medium and small items a centered hero carousel has.
        2,
        modifier,
        item_spacing,
        Some(fling_behavior),
        user_scroll_enabled,
        Rc::new(content),
    );
}

/// The keylines of a carousel for the space along it and the item spacing, px (Compose's `keylineList:
/// (availableSpace: Float, itemSpacing: Float) -> KeylineList`).
pub(crate) type KeylineListFn = Rc<dyn Fn(f32, f32) -> KeylineList>;

/// A carousel of items changing size by their place and the keylines (Compose's internal `Carousel`): a
/// horizontal or vertical pager over `state`'s pager, its page size and snap position the keylines' (from
/// `keyline_list`), only the cross-axis part of `content_padding` given to the pager (the main-axis part is the
/// strategy's), `max_non_focal_visible_item_count` pages composed beyond the viewport, each page a box masked and
/// moved by [`carousel_item`] with `content` in it. `fling_behavior` None:
/// [`CarouselDefaults::single_advance_fling_behavior`] of `state`.
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub(crate) fn Carousel(
    state: CarouselState,
    orientation: Orientation,
    keyline_list: KeylineListFn,
    content_padding: PaddingValues,
    max_non_focal_visible_item_count: i32,
    #[default(Modifier)] modifier: Modifier,
    #[default(Dp(0.0))] item_spacing: Dp,
    #[default(None)] fling_behavior: Option<Rc<dyn TargetedFlingBehavior>>,
    #[default(true)] user_scroll_enabled: bool,
    content: CarouselItemContent,
) {
    let fling_behavior = match fling_behavior {
        Some(f) => f,
        None => CarouselDefaults.single_advance_fling_behavior(&state),
    };
    let before_content_padding = calculate_before_content_padding(&content_padding, orientation);
    let after_content_padding = calculate_after_content_padding(&content_padding, orientation);
    let kl = keyline_list.clone();
    let page_size = remember_keyed(Rc::as_ptr(&keyline_list) as *const () as usize, move || {
        Rc::new(CarouselPageSize::new(kl, before_content_padding, after_content_padding))
    });

    let snap_position = KeylineSnapPosition(page_size.clone());
    let layout_direction = LocalLayoutDirection.current();

    let page_content = {
        let state = state.clone();
        let page_size = page_size.clone();
        move |page: i32| {
            let carousel_item_info = remember(CarouselItemDrawInfo::new);
            let info = carousel_item_info.clone();
            let scope = remember(move || CarouselItemScope::new(info));
            let info = carousel_item_info.clone();
            let clip_shape: Arc<dyn Shape> = remember(move || {
                let shape: Arc<dyn Shape> = Arc::new(OutlineShape(move |_: Size, _: LayoutDirection, _: &Density| {
                    Outline::Rectangle(info.mask_rect())
                }));
                shape
            });

            let page_size = page_size.clone();
            let content = content.clone();
            Box(
                modifier = Modifier.carousel_item(
                    page,
                    state.clone(),
                    Rc::new(move || page_size.strategy()),
                    carousel_item_info,
                    clip_shape,
                ),
                content = move || content(&scope, page),
            );
        }
    };

    let semantics_modifier = modifier.semantics(|s| s.set_role(Role::Carousel));
    let page_size: Rc<dyn PageSizeCalculator> = page_size;
    if orientation == Orientation::Horizontal {
        HorizontalPager(
            state = state.pager_state().pager_state.clone(),
            // Only the cross-axis padding: the main-axis padding is the strategy's.
            content_padding = PaddingValues::new(
                Dp(0.0),
                content_padding.calculate_top_padding(),
                Dp(0.0),
                content_padding.calculate_bottom_padding(),
            ),
            page_size = PageSize::Custom(page_size.clone()),
            page_spacing = item_spacing,
            beyond_viewport_page_count = max_non_focal_visible_item_count,
            snap_position = snap_position,
            fling_behavior = Some(fling_behavior),
            user_scroll_enabled = user_scroll_enabled,
            modifier = semantics_modifier,
            page_content = page_content,
        );
    } else if orientation == Orientation::Vertical {
        VerticalPager(
            state = state.pager_state().pager_state.clone(),
            // Only the cross-axis padding: the main-axis padding is the strategy's.
            content_padding = PaddingValues::new(
                content_padding.calculate_start_padding(layout_direction),
                Dp(0.0),
                content_padding.calculate_end_padding(layout_direction),
                Dp(0.0),
            ),
            page_size = PageSize::Custom(page_size.clone()),
            page_spacing = item_spacing,
            beyond_viewport_page_count = max_non_focal_visible_item_count,
            snap_position = snap_position,
            fling_behavior = Some(fling_behavior),
            user_scroll_enabled = user_scroll_enabled,
            modifier = semantics_modifier,
            page_content = page_content,
        );
    }
}

/// The padding before the content along `orientation`, px (Compose's composable
/// `PaddingValues.calculateBeforeContentPadding`).
#[kompozable]
fn calculate_before_content_padding(padding: &PaddingValues, orientation: Orientation) -> f32 {
    let dp_value = if orientation == Orientation::Vertical {
        padding.calculate_top_padding()
    } else {
        padding.calculate_start_padding(LocalLayoutDirection.current())
    };
    LocalDensity.current().to_px(dp_value)
}

/// The padding after the content along `orientation`, px (Compose's `PaddingValues.calculateAfterContentPadding`).
#[kompozable]
fn calculate_after_content_padding(padding: &PaddingValues, orientation: Orientation) -> f32 {
    let dp_value = if orientation == Orientation::Vertical {
        padding.calculate_bottom_padding()
    } else {
        padding.calculate_end_padding(LocalLayoutDirection.current())
    };
    LocalDensity.current().to_px(dp_value)
}

// ---------------------------------------------------------------------------------------------------------------
// CarouselPageSize

/// A page size keeping a strategy up to date with the container's latest space (Compose's internal
/// `CarouselPageSize`): the strategy's item size when it is valid, else the whole space (as a pager's default).
pub(crate) struct CarouselPageSize {
    keyline_list: KeylineListFn,
    before_content_padding: f32,
    after_content_padding: f32,
    /// Compose's `strategyState` (structural equality: Strategy's own).
    strategy_state: MutableState<Arc<Strategy>>,
}

impl CarouselPageSize {
    pub(crate) fn new(
        keyline_list: KeylineListFn,
        before_content_padding: f32,
        after_content_padding: f32,
    ) -> CarouselPageSize {
        CarouselPageSize {
            keyline_list,
            before_content_padding,
            after_content_padding,
            strategy_state: mutable_state_of(Arc::new(Strategy::empty())),
        }
    }

    /// The strategy of the last measure (a state read).
    pub(crate) fn strategy(&self) -> Arc<Strategy> {
        self.strategy_state.get()
    }
}

impl PageSizeCalculator for CarouselPageSize {
    fn calculate_main_axis_page_size(&self, _density: &Density, available_space: i32, page_spacing: i32) -> i32 {
        let keylines = (self.keyline_list)(available_space as f32, page_spacing as f32);
        self.strategy_state.set(Arc::new(Strategy::new(
            keylines,
            available_space as f32,
            page_spacing as f32,
            self.before_content_padding,
            self.after_content_padding,
        )));

        // A valid strategy's item size; else a full-size item, as a pager's default.
        let strategy = self.strategy();
        if strategy.is_valid { strategy.item_main_axis_size().round_to_int() } else { available_space }
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The item modifier

/// Masks and moves a carousel item as it moves along the carousel (Compose's internal `Modifier.carouselItem`).
trait CarouselItemModifier {
    /// The item at `index`: measured at the strategy's item size along the main axis (nothing shown without a
    /// valid strategy), the current item in front and the others behind the lower ones, and in its layer: the
    /// keylines for the scroll offset, the keyline interpolated at the item's center giving its mask (written to
    /// `carousel_item_draw_info`, clipped to by `clip_shape`, a rect of the mask's bounds — corners and the like are
    /// the content's, by [`CarouselItemScope::mask_clip`] and [`CarouselItemScope::mask_border`]) and its
    /// translation pinning the masked items' edges together.
    fn carousel_item(
        self,
        index: i32,
        state: CarouselState,
        strategy: Rc<dyn Fn() -> Arc<Strategy>>,
        carousel_item_draw_info: CarouselItemDrawInfo,
        clip_shape: Arc<dyn Shape>,
    ) -> Modifier;
}

impl CarouselItemModifier for Modifier {
    fn carousel_item(
        self,
        index: i32,
        state: CarouselState,
        strategy: Rc<dyn Fn() -> Arc<Strategy>>,
        carousel_item_draw_info: CarouselItemDrawInfo,
        clip_shape: Arc<dyn Shape>,
    ) -> Modifier {
        self.layout(move |scope, measurable, constraints| {
            let strategy_result = strategy();
            if !strategy_result.is_valid {
                // No strategy: no content shown.
                return named_args!(scope.layout(0, 0, |_| {}));
            }

            let pager = state.pager_state().pager_state.clone();
            let is_vertical = pager.layout_info().orientation() == Orientation::Vertical;
            let is_rtl = scope.layout_direction() == LayoutDirection::Rtl;

            // The item takes the strategy's item size along its main axis.
            let main_axis_size = strategy_result.item_main_axis_size();
            let item_constraints = if is_vertical {
                Constraints {
                    min_height: main_axis_size.round_to_int(),
                    max_height: main_axis_size.round_to_int(),
                    ..constraints
                }
            } else {
                Constraints {
                    min_width: main_axis_size.round_to_int(),
                    max_width: main_axis_size.round_to_int(),
                    ..constraints
                }
            };

            let placeable = measurable.measure(item_constraints);
            // The current item in front; the others in reverse order of placement (higher indices behind lower ones).
            let item_z_index = if index == pager.current_page() {
                1.0
            } else if index == 0 {
                0.0
            } else {
                1.0 / index as f32
            };

            let (width, height) = (placeable.width(), placeable.height());
            let state = state.clone();
            let info = carousel_item_draw_info.clone();
            let clip_shape = clip_shape.clone();
            named_args!(scope.layout(width, height, move |p| {
                let state = state.clone();
                let strategy_result = strategy_result.clone();
                let info = info.clone();
                let clip_shape = clip_shape.clone();
                named_args!(p.place_with_layer(
                    &placeable,
                    0,
                    0,
                    z_index = item_z_index,
                    layer_block = move |layer: &mut GraphicsLayerScope| {
                        update_item_layer(
                            layer,
                            index,
                            &state,
                            &strategy_result,
                            &info,
                            &clip_shape,
                            is_vertical,
                            is_rtl,
                        );
                    }
                ));
            }))
        })
    }
}

/// The item's layer block of [`CarouselItemModifier::carousel_item`].
#[allow(clippy::too_many_arguments)]
fn update_item_layer(
    layer: &mut GraphicsLayerScope,
    index: i32,
    state: &CarouselState,
    strategy_result: &Strategy,
    carousel_item_draw_info: &CarouselItemDrawInfo,
    clip_shape: &Arc<dyn Shape>,
    is_vertical: bool,
    is_rtl: bool,
) {
    let size = layer.size();
    let scroll_offset = calculate_current_scroll_offset(state, strategy_result);
    let max_scroll_offset = calculate_max_scroll_offset(state, strategy_result);
    // (Compose's TODO: work keylines out fewer times)
    let keylines = strategy_result.get_keyline_list_for_scroll_offset(scroll_offset, max_scroll_offset, false);
    let rounded_keylines = strategy_result.get_keyline_list_for_scroll_offset(scroll_offset, max_scroll_offset, true);

    // The center of the item at this index.
    let item_size_with_spacing = strategy_result.item_main_axis_size() + strategy_result.item_spacing;
    let unadjusted_center =
        (index as f32 * item_size_with_spacing) + (strategy_result.item_main_axis_size() / 2.0) - scroll_offset;

    // The keylines before and after the item's center, interpolated into the keyline its mask and offset are of.
    let keyline_before = keylines.get_keyline_before(unadjusted_center);
    let keyline_after = keylines.get_keyline_after(unadjusted_center);
    let progress = get_progress(&keyline_before, &keyline_after, unadjusted_center);
    let interpolated_keyline = lerp_keyline(&keyline_before, &keyline_after, progress);
    let is_out_of_keyline_bounds = keyline_before == keyline_after;

    let center_x = if is_vertical { size.height / 2.0 } else { strategy_result.item_main_axis_size() / 2.0 };
    let center_y = if is_vertical { strategy_result.item_main_axis_size() / 2.0 } else { size.height / 2.0 };
    let half_mask_width = if is_vertical { size.width / 2.0 } else { interpolated_keyline.size / 2.0 };
    let half_mask_height = if is_vertical { interpolated_keyline.size / 2.0 } else { size.height / 2.0 };
    let mask_rect = Rect::new(
        center_x - half_mask_width,
        center_y - half_mask_height,
        center_x + half_mask_width,
        center_y + half_mask_height,
    );

    // The item's draw info.
    carousel_item_draw_info.size_state.set(interpolated_keyline.size);
    let min_size = rounded_keylines
        .iter()
        .min_by(|a, b| a.size.total_cmp(&b.size))
        .expect("Collection contains no element matching the predicate.")
        .size;
    carousel_item_draw_info.min_size_state.set(min_size);
    carousel_item_draw_info.max_size_state.set(rounded_keylines.first_focal().size);
    carousel_item_draw_info.mask_rect_state.set(mask_rect);

    // Clip the item.
    layer.clip = mask_rect != Rect::new(0.0, 0.0, size.width, size.height);
    layer.shape = clip_shape.clone();

    // Clipped items leave space between them: translated to pin their edges together.
    let mut translation = interpolated_keyline.offset - unadjusted_center;
    if is_out_of_keyline_bounds {
        // Beyond the first or last keyline: offset on by cutting its unadjusted offset by its masked size.
        let out_of_bounds_offset =
            (unadjusted_center - interpolated_keyline.unadjusted_offset) / interpolated_keyline.size;
        translation += out_of_bounds_offset;
    }
    if is_vertical {
        layer.translation_y = translation;
    } else {
        layer.translation_x = if is_rtl { -translation } else { translation };
    }
}

/// Draws the keylines over a carousel, to debug it (Compose's private `Modifier.drawDebugLines`; unused, as
/// Compose's).
#[allow(dead_code)]
fn draw_debug_lines(
    modifier: Modifier,
    state: CarouselState,
    page_size: Rc<CarouselPageSize>,
    stroke_color: Color,
    stroke_width: Dp,
) -> Modifier {
    modifier.draw_with_content(move |c| {
        c.draw_content();
        let strategy_result = page_size.strategy();
        let scroll_offset = calculate_current_scroll_offset(&state, &strategy_result);
        let max_scroll_offset = calculate_max_scroll_offset(&state, &strategy_result);
        let keylines = strategy_result.get_keyline_list_for_scroll_offset(scroll_offset, max_scroll_offset, false);
        let stroke_width_px = c.density().to_px(stroke_width);
        for keyline in keylines.iter() {
            named_args!(c.draw_line(
                stroke_color,
                Offset::new(keyline.offset, 0.0),
                Offset::new(keyline.offset, 100.0),
                stroke_width = stroke_width_px
            ));
        }
    })
}

/// The scroll offset now, from the current item, its offset fraction, the item size and spacing and the snap
/// position (Compose's internal `calculateCurrentScrollOffset`).
pub(crate) fn calculate_current_scroll_offset(state: &CarouselState, strategy: &Strategy) -> f32 {
    let pager = &state.pager_state().pager_state;
    let item_size_with_spacing = strategy.item_main_axis_size() + strategy.item_spacing;
    let current_item_scroll_offset = (pager.current_page() as f32 * item_size_with_spacing)
        + (pager.current_page_offset_fraction() * item_size_with_spacing);
    current_item_scroll_offset - get_snap_position_offset(strategy, pager.current_page(), pager.page_count()) as f32
}

/// The most the carousel scrolls, from the item count, size and spacing (Compose's internal
/// `calculateMaxScrollOffset`).
pub(crate) fn calculate_max_scroll_offset(state: &CarouselState, strategy: &Strategy) -> f32 {
    let item_count = state.pager_state().pager_state.page_count() as f32;
    let max_scroll_possible =
        (strategy.item_main_axis_size() * item_count) + (strategy.item_spacing * (item_count - 1.0));
    (max_scroll_possible - strategy.available_space).max(0.0)
}

/// How far (0 to 1) `unadjusted_offset` is from `before`'s unadjusted offset to `after`'s (1 when they are the
/// same keyline).
fn get_progress(before: &Keyline, after: &Keyline, unadjusted_offset: f32) -> f32 {
    if before == after {
        return 1.0;
    }
    let total = after.unadjusted_offset - before.unadjusted_offset;
    (unadjusted_offset - before.unadjusted_offset) / total
}

// ---------------------------------------------------------------------------------------------------------------
// CarouselAlignment

/// Where items are aligned along a carousel's main axis (Compose's internal value class `CarouselAlignment`): the
/// focal items at the start, the middle, or the end of the container.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CarouselAlignment {
    Start = -1,
    Center = 0,
    // (no carousel of Compose's aligns at the end)
    #[allow(dead_code)]
    End = 1,
}

// ---------------------------------------------------------------------------------------------------------------
// CarouselDefaults

/// The defaults of the carousels (Compose's `CarouselDefaults`): its composables are methods of the value, as
/// Kotlin calls them (`CarouselDefaults.single_advance_fling_behavior(&state)`).
#[derive(Clone, Copy, Debug)]
pub struct CarouselDefaults;

impl CarouselDefaults {
    /// The least a strategy may make small items (Compose's `MinSmallItemSize`).
    pub const MinSmallItemSize: Dp = Dp(40.0);
    /// The most a strategy may make small items (Compose's `MaxSmallItemSize`).
    pub const MaxSmallItemSize: Dp = Dp(56.0);
    /// The anchors' size, the keylines off the ends (Compose's internal `AnchorSize`).
    pub(crate) const AnchorSize: Dp = Dp(10.0);
    /// The part of a large item over which a medium one is too like it (Compose's internal
    /// `MediumLargeItemDiffThreshold`).
    pub(crate) const MediumLargeItemDiffThreshold: f32 = 0.85;
}

/// The default snap animation of the carousels' flings (Compose's `spring(stiffness = Spring.StiffnessMediumLow)`).
fn default_snap_animation_spec() -> Arc<dyn AnimationSpec<f32>> {
    Arc::new(SpringSpec::<f32>::new(Spring::DampingRatioNoBouncy, Spring::StiffnessMediumLow, None))
}

/// The page multi-browse flings go to: the one the velocity suggests (Compose's anonymous `PagerSnapDistance` of
/// `multiBrowseFlingBehavior`).
struct SuggestedTargetPage;

impl PagerSnapDistanceCalculator for SuggestedTargetPage {
    fn calculate_target_page(
        &self,
        _start_page: i32,
        suggested_target_page: i32,
        _velocity: f32,
        _page_size: i32,
        _page_spacing: i32,
    ) -> i32 {
        suggested_target_page
    }
}

/// Snaps nowhere: no offset (Compose's anonymous `SnapLayoutInfoProvider` of `noSnapFlingBehavior`).
struct NoSnapLayoutInfoProvider;

impl SnapLayoutInfoProvider for NoSnapLayoutInfoProvider {
    fn calculate_snap_offset(&self, _velocity: f32) -> f32 {
        0.0
    }
}

#[named]
impl CarouselDefaults {
    /// A fling of one item at most (Compose's composable `CarouselDefaults.singleAdvanceFlingBehavior`): the pager's
    /// fling of `state`'s pager at most one page away, snapping along `snap_animation_spec`.
    ///
    /// Kotlin: `singleAdvanceFlingBehavior(state: CarouselState, snapAnimationSpec: AnimationSpec<Float> = spring(stiffness = Spring.StiffnessMediumLow))`
    #[track_caller]
    pub fn single_advance_fling_behavior(
        &self,
        state: &CarouselState,
        #[default(default_snap_animation_spec())] snap_animation_spec: Arc<dyn AnimationSpec<f32>>,
    ) -> Rc<dyn TargetedFlingBehavior> {
        named_args!(PagerDefaults.fling_behavior(
            &state.pager_state().pager_state,
            pager_snap_distance = PagerSnapDistance::at_most(1),
            snap_animation_spec = snap_animation_spec
        ))
    }

    /// A fling as far as its velocity carries, snapping to the item it ends at (Compose's composable
    /// `CarouselDefaults.multiBrowseFlingBehavior`): decaying along `decay_animation_spec` (None: the remembered
    /// spline decay) when fast enough to reach its target item, then `snap_animation_spec`; slower, only the snap.
    ///
    /// Kotlin: `multiBrowseFlingBehavior(state: CarouselState, decayAnimationSpec: DecayAnimationSpec<Float> = rememberSplineBasedDecay(), snapAnimationSpec: AnimationSpec<Float> = spring(stiffness = Spring.StiffnessMediumLow))`
    #[track_caller]
    pub fn multi_browse_fling_behavior(
        &self,
        state: &CarouselState,
        #[default(None)] decay_animation_spec: Option<Arc<dyn DecayAnimationSpec<f32>>>,
        #[default(default_snap_animation_spec())] snap_animation_spec: Arc<dyn AnimationSpec<f32>>,
    ) -> Rc<dyn TargetedFlingBehavior> {
        // (a new distance each composition, as Compose's anonymous object)
        let pager_snap_distance = PagerSnapDistance::custom(Rc::new(SuggestedTargetPage));
        named_args!(PagerDefaults.fling_behavior(
            &state.pager_state().pager_state,
            pager_snap_distance = pager_snap_distance,
            decay_animation_spec = decay_animation_spec,
            snap_animation_spec = snap_animation_spec
        ))
    }

    /// A fling as far as its velocity carries, snapping nowhere after (Compose's composable
    /// `CarouselDefaults.noSnapFlingBehavior`).
    ///
    /// Kotlin: `noSnapFlingBehavior()`
    #[track_caller]
    pub fn no_snap_fling_behavior(&self) -> Rc<dyn TargetedFlingBehavior> {
        kompoz_runtime::kompozable_block!({
            let decay_layout_info_provider: Rc<dyn SnapLayoutInfoProvider> =
                remember(|| -> Rc<dyn SnapLayoutInfoProvider> { Rc::new(NoSnapLayoutInfoProvider) });
            remember_snap_fling_behavior(decay_layout_info_provider)
        })
    }
}
