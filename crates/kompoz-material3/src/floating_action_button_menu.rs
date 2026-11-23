//! Material Expressive's FAB menu, as Compose's FloatingActionButtonMenu.kt: a [`ToggleFloatingActionButton`] (a FAB
//! growing round and turning primary as it is checked, its icon following by
//! [`ToggleFloatingActionButtonModifiers::animate_icon`]) opening a [`FloatingActionButtonMenu`] — a column of
//! [`FloatingActionButtonMenuItem`](FloatingActionButtonMenuScope::FloatingActionButtonMenuItem) pills above it, shown
//! one after the other from the button up, each widening and fading in — and [`ToggleFloatingActionButtonDefaults`]
//! (the button's colors and sizes as functions of the checked progress).
//!
//! Compose's scopes are interfaces (the item an extension function of the menu's); here structs whose composables
//! are methods. The Kotlin syntax calls `FloatingActionButtonMenuItem(…)` inside the menu's lambda on its implicit
//! receiver, and reads `checkedProgress` inside a toggleable FAB's (a lambda kept past it — a graphics layer's block —
//! holding a clone of the scope).

use crate::color_scheme::content_color_for;
use crate::interactive_component_size::LocalMinimumInteractiveComponentSize;
use crate::material_theme::MaterialTheme;
use crate::ripples::ripple;
use crate::surface::ClickableSurface;
use crate::text::LocalTextStyle;
use crate::tokens::{
    FabBaselineTokens, FabLargeTokens, FabMediumTokens, FabMenuBaselineTokens, FabPrimaryContainerTokens,
    MotionSchemeKeyTokens,
};
use kompoz_animation::{
    Animatable, AnimationVector1D, FiniteAnimationSpec, VectorConverter, animate_float_as_state, spring,
};
use kompoz_coroutines::remember_coroutine_scope;
use kompoz_foundation::layout::Arrangement;
use kompoz_foundation::prelude::*;
use kompoz_graphics::{
    BlendMode, Color, ColorFilter, Constraints, CornerRadius, Density, Dp, GenericShape, Path, PathDirection,
    RoundRect, Size, lerp_color, lerp_dp,
};
use kompoz_runtime::{
    KompozitionLocalProvider, MutableState, kompozable, kompozable_block, mutable_confined_state_of,
    mutable_int_state_of, mutable_state_of, named, named_args, remember, remember_keyed,
};
use kompoz_ui::focus::FocusRequester;
use kompoz_ui::input::{Key, KeyEvent, KeyEventType};
use kompoz_ui::layout::{HorizontalRuler, Layout, ParentDataModifierNode, measure_policy};
use kompoz_ui::platform::LocalDensity;
use kompoz_ui::semantics::{SemanticsConfiguration, SemanticsModifierNode};
use kompoz_ui::{Alignment, HorizontalAlignment, Modifier, ModifierNode, ModifierNodeElement, node_capabilities};
use std::any::Any;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

// ---- The menu --------------------------------------------------------------------------------------------------

/// A FAB menu (Compose's Expressive `FloatingActionButtonMenu`): `button` (typically a [`ToggleFloatingActionButton`]
/// checked while `expanded`) 16 dp above its bottom, and above it `content` (in a [`FloatingActionButtonMenuScope`]:
/// typically [`FloatingActionButtonMenuItem`](FloatingActionButtonMenuScope::FloatingActionButtonMenuItem)s) in a
/// column 4 dp apart, 8 dp above the button, each line placed by `horizontal_alignment` (at the end by default), as
/// the button is. As it expands the items show one after the other from the button up (closing, from the top down),
/// each widening and fading in; the column scrolls when it is taller than there is room for, and is a traversal group
/// coming before the button. With `modifier`, 16 dp in at the sides. Tab (Shift not held) or Down from the button
/// while expanded moves the focus to the first item.
#[kompozable]
pub fn FloatingActionButtonMenu(
    expanded: bool,
    button: impl Fn() + 'static,
    #[default(Modifier)] modifier: Modifier,
    #[default(Alignment::End)] horizontal_alignment: HorizontalAlignment,
    content: impl Fn(&FloatingActionButtonMenuScope) + 'static,
) {
    let button_height = remember(|| mutable_int_state_of(0));
    let focus_requester = remember(FocusRequester::new);

    let column_height = button_height.clone();
    let column_alignment = horizontal_alignment.clone();
    let content: Rc<dyn Fn(&FloatingActionButtonMenuScope)> = Rc::new(content);
    let button: Rc<dyn Fn()> = Rc::new(button);
    let measured_height = button_height.clone();
    let placed_alignment = horizontal_alignment;
    Layout(
        modifier = named_args!(modifier.padding_hv(horizontal = FabMenuPaddingHorizontal)),
        content = move || {
            FloatingActionButtonMenuItemColumn(
                Modifier.focus_requester(&focus_requester),
                expanded,
                column_alignment.clone(),
                {
                    let height = column_height.clone();
                    Rc::new(move || height.get())
                },
                content.clone(),
            );

            let focus_requester = focus_requester.clone();
            let button = button.clone();
            Box(
                modifier = Modifier.on_key_event(move |it: &KeyEvent| {
                    // For keyboard a11y, the focus order should go from the fab menu button to the
                    // first item at the top.
                    if expanded
                        && it.r#type == KeyEventType::KeyDown
                        && ((it.key == Key::Tab && !it.is_shift_pressed()) || it.key == Key::DirectionDown)
                    {
                        focus_requester.request_focus();
                        return true;
                    }
                    false
                }),
                content = move || button(),
            );
        },
        measure_policy = measure_policy(move |scope, measureables, constraints| {
            let menu_items_placeable = measureables[0].measure(constraints);

            let button_padding_bottom = scope.round_to_px(FabMenuButtonPaddingBottom);
            let mut button_placeable = None;
            let suggested_width;
            let suggested_height;
            if measureables.len() > 1 {
                let placeable = measureables[1].measure(constraints);
                measured_height.set(placeable.height());

                suggested_width = placeable.width().max(menu_items_placeable.width());
                suggested_height = (placeable.height() + button_padding_bottom).max(menu_items_placeable.height());
                button_placeable = Some(placeable);
            } else {
                suggested_width = menu_items_placeable.width();
                suggested_height = menu_items_placeable.height();
            }

            let width = suggested_width.min(constraints.max_width);
            let height = suggested_height.min(constraints.max_height);

            let horizontal_alignment = placed_alignment.clone();
            let layout_direction = scope.layout_direction();
            scope.layout(width, height, move |p| {
                let menu_items_x = horizontal_alignment.align(menu_items_placeable.width(), width, layout_direction);
                p.place(&menu_items_placeable, menu_items_x, 0);

                if let Some(button_placeable) = &button_placeable {
                    let button_x = horizontal_alignment.align(button_placeable.width(), width, layout_direction);
                    let button_y = height - button_placeable.height() - button_padding_bottom;
                    p.place(button_placeable, button_x, button_y);
                }
            })
        }),
    );
}

/// The menu's items (Compose's private `FloatingActionButtonMenuItemColumn`): a column of them, as many shown from
/// the bottom as a stagger animation (an `Int` along the slow effects spec, a spring's settling at 1) counts towards
/// all of them while `expanded`, none otherwise — each told by [`MenuItemRuler`] how far up the shown ones reach.
/// Room is kept below for the button (`button_height`). Gone (0 tall) once no item is visible; scrolling where taller
/// than the room it is given.
#[kompozable]
fn FloatingActionButtonMenuItemColumn(
    modifier: Modifier,
    expanded: bool,
    horizontal_alignment: HorizontalAlignment,
    button_height: Rc<dyn Fn() -> i32>,
    content: Rc<dyn Fn(&FloatingActionButtonMenuScope)>,
) {
    let item_count = remember(|| mutable_int_state_of(0));
    let items_need_vertical_scroll = remember(|| mutable_state_of(false));
    // (Kotlin's `var originalConstraints: Constraints? = null`, a local of each composition both lambdas capture)
    let original_constraints: Rc<Cell<Option<Constraints>>> = Rc::new(Cell::new(None));
    let stagger_anim: AnimatableState<i32> = remember(animatable_state);
    let coroutine_scope = remember_coroutine_scope();
    // TODO Load the motionScheme tokens from the component tokens file
    let mut stagger_anim_spec: Arc<dyn FiniteAnimationSpec<i32>> = MotionSchemeKeyTokens::SlowEffects.value();
    if let Some((damping_ratio, stiffness)) = MaterialTheme::motion_scheme().specs().slow_effects().spring_parameters()
    {
        // Apply a small visibilityThreshold to the provided SpringSpec to avoid a delay in the
        // appearance of the last item when the list is populated.
        stagger_anim_spec = Arc::new(spring(damping_ratio, stiffness, Some(1)));
    }
    let constraints_seen = original_constraints.clone();
    let scrolled = if items_need_vertical_scroll.get() {
        named_args!(Modifier.vertical_scroll(&remember_scroll_state(), enabled = expanded))
    } else {
        Modifier
    };
    let scope_alignment = horizontal_alignment.clone();
    let need_scroll = items_need_vertical_scroll.clone();
    let counted = item_count.clone();
    Layout(
        modifier = modifier
            .clip_to_bounds()
            .semantics(|s: &mut SemanticsConfiguration| {
                s.set_is_traversal_group(true);
                s.set_traversal_index(-0.9);
            })
            .layout(move |scope, measurable, constraints| {
                // Use a layout modifier before the verticalScroll to get the original
                // constraints from the parent, since verticalScroll will cause the constraints
                // max height to be infinity.
                constraints_seen.set(Some(constraints));
                let placeable = measurable.measure(constraints);
                scope.layout(placeable.width(), placeable.height(), move |p| p.place(&placeable, 0, 0))
            })
            .then(scrolled),
        content = move || {
            let alignment = scope_alignment.clone();
            let scope = remember_keyed(alignment.clone(), move || FloatingActionButtonMenuScope {
                horizontal_alignment: alignment,
            });
            content(&scope);
        },
        measure_policy = measure_policy(move |scope, measurables, constraints| {
            counted.set(measurables.len() as i32);
            let item_count = counted.get();

            let target_item_count = if expanded { item_count } else { 0 };
            match stagger_anim.get() {
                Some(it) => {
                    if it.target_value() != target_item_count {
                        let spec = stagger_anim_spec.clone();
                        named_args!(coroutine_scope.launch(async move {
                            named_args!(it.animate_to(target_item_count, animation_spec = spec)).await;
                        }));
                    }
                }
                None => stagger_anim.set(Some(Animatable((target_item_count, i32::VectorConverter)))),
            }

            let placeables: Vec<_> = measurables.iter().map(|measurable| measurable.measure(constraints)).collect();
            let width = placeables.iter().map(|p| p.width()).max().unwrap_or(0);

            let vertical_spacing = scope.round_to_px(FabMenuItemSpacingVertical);
            let vertical_spacing_height =
                if !placeables.is_empty() { vertical_spacing * (placeables.len() as i32 - 1) } else { 0 };
            let current_button_height = button_height();
            let bottom_padding = if current_button_height > 0 {
                current_button_height
                    + scope.round_to_px(FabMenuButtonPaddingBottom)
                    + scope.round_to_px(FabMenuPaddingBottom)
            } else {
                0
            };
            let height = placeables.iter().map(|p| p.height()).sum::<i32>() + vertical_spacing_height + bottom_padding;
            let mut visible_height = bottom_padding as f32;
            let shown = stagger_anim.get().map_or(0, |a| a.value());
            for (index, placeable) in placeables.iter().enumerate() {
                let item_visible = index as i32 >= item_count - shown;
                if item_visible {
                    visible_height += placeable.height() as f32;
                    if index < placeables.len() - 1 {
                        visible_height += vertical_spacing as f32;
                    }
                }
            }

            let final_height = if placeables.iter().any(is_visible) { height } else { 0 };

            need_scroll.set(final_height > original_constraints.get().expect("measured through its layout").max_height);

            let horizontal_alignment = horizontal_alignment.clone();
            let ruler_value = height as f32 - visible_height;
            let layout_direction = scope.layout_direction();
            scope.layout_with_rulers(
                width,
                final_height,
                move |r| r.provides(menu_item_ruler(), ruler_value),
                move |p| {
                    let mut y = 0;
                    let count = placeables.len();
                    for (index, placeable) in placeables.iter().enumerate() {
                        let x = horizontal_alignment.align(placeable.width(), width, layout_direction);
                        p.place(placeable, x, y);
                        y += placeable.height();
                        if index < count - 1 {
                            y += vertical_spacing;
                        }
                    }
                },
            )
        }),
    );
}

/// The scope of a [`FloatingActionButtonMenu`]'s content (Compose's Expressive interface
/// `FloatingActionButtonMenuScope`): where its items are made, aligned as the menu says.
#[derive(Clone, Debug)]
pub struct FloatingActionButtonMenuScope {
    horizontal_alignment: HorizontalAlignment,
}

#[named]
impl FloatingActionButtonMenuScope {
    /// How the menu's items are aligned (Compose's `horizontalAlignment`).
    pub fn horizontal_alignment(&self) -> HorizontalAlignment {
        self.horizontal_alignment.clone()
    }

    /// A FAB menu item (Compose's Expressive `FloatingActionButtonMenuScope.FloatingActionButtonMenuItem`): a pill —
    /// a clickable [`Surface`](crate::Surface) of `container_color` (the scheme's primary container by default), its
    /// content in `content_color` (None is Kotlin's default, `contentColorFor(container_color)`) — 56 dp tall and at
    /// least as wide, 24 dp in at the sides, `icon` then `text` (title medium) 8 dp apart; `on_click` called when it is
    /// clicked. As the menu shows it (its place reached by the shown ones) it widens from nothing (from where the
    /// menu aligns it) along the fast spatial spec and fades in along the fast effects one, closing the other way;
    /// invisible, it isn't placed and tells accessibility nothing. Keeps no 48 dp of room (its pill is 56). Composable.
    #[track_caller]
    #[allow(clippy::too_many_arguments)]
    pub fn FloatingActionButtonMenuItem(
        &self,
        on_click: impl Fn() + 'static,
        text: impl Fn() + 'static,
        icon: impl Fn() + 'static,
        #[default(Modifier)] modifier: Modifier,
        #[default(MaterialTheme::color_scheme().primary_container)] container_color: Color,
        #[default(None)] content_color: Option<Color>,
    ) {
        kompozable_block!({
            let content_color = content_color.unwrap_or_else(|| content_color_for(container_color));
            FloatingActionButtonMenuItemImpl(
                self.horizontal_alignment.clone(),
                Rc::new(on_click),
                Rc::new(text),
                Rc::new(icon),
                modifier,
                container_color,
                content_color,
            );
        });
    }
}

/// The body of [`FloatingActionButtonMenuScope::FloatingActionButtonMenuItem`] (a composable of its own: restartable
/// as Compose's).
#[kompozable]
fn FloatingActionButtonMenuItemImpl(
    horizontal_alignment: HorizontalAlignment,
    on_click: Rc<dyn Fn()>,
    text: Rc<dyn Fn()>,
    icon: Rc<dyn Fn()>,
    modifier: Modifier,
    container_color: Color,
    content_color: Color,
) {
    let width_anim: AnimatableState<f32> = remember(animatable_state);
    let alpha_anim: AnimatableState<f32> = remember(animatable_state);
    // TODO Load the motionScheme tokens from the component tokens file
    let width_spring: Arc<dyn FiniteAnimationSpec<f32>> = MotionSchemeKeyTokens::FastSpatial.value();
    let alpha_spring: Arc<dyn FiniteAnimationSpec<f32>> = MotionSchemeKeyTokens::FastEffects.value();
    let coroutine_scope = remember_coroutine_scope();

    let is_visible: MutableState<bool> = remember(|| mutable_state_of(false));
    let visible_reader = is_visible.clone();
    let item_visible: Rc<dyn Fn() -> bool> =
        remember(move || Rc::new(move || visible_reader.get()) as Rc<dyn Fn() -> bool>);
    let placed_width_anim = width_anim.clone();
    // Disable min interactive component size because it interferes with the item expand
    // animation and we know we are meeting the size requirements below.
    KompozitionLocalProvider(&[LocalMinimumInteractiveComponentSize.provides(Dp(0.0))], move || {
        let (width_anim, alpha_anim) = (placed_width_anim.clone(), alpha_anim.clone());
        let (width_spring, alpha_spring) = (width_spring.clone(), alpha_spring.clone());
        let (coroutine_scope, is_visible) = (coroutine_scope.clone(), is_visible.clone());
        let (text, icon) = (text.clone(), icon.clone());
        let horizontal_alignment = horizontal_alignment.clone();
        let content_width_anim = width_anim.clone();
        let on_click = on_click.clone();
        ClickableSurface(
            move || on_click(),
            modifier = item_visible_modifier(modifier.clone(), item_visible.clone()).layout(
                move |scope, measurable, constraints| {
                    let placeable = measurable.measure(constraints);
                    let (width_anim, alpha_anim) = (width_anim.clone(), alpha_anim.clone());
                    let (width_spring, alpha_spring) = (width_spring.clone(), alpha_spring.clone());
                    let (coroutine_scope, is_visible) = (coroutine_scope.clone(), is_visible.clone());
                    scope.layout(placeable.width(), placeable.height(), move |p| {
                        let target = if p.current(menu_item_ruler(), f32::INFINITY) <= 0.0 { 1.0 } else { 0.0 };

                        match width_anim.get() {
                            Some(it) => {
                                if it.target_value() != target {
                                    let spec = width_spring.clone();
                                    named_args!(coroutine_scope.launch(async move {
                                        named_args!(it.animate_to(target, animation_spec = spec)).await;
                                    }));
                                }
                            }
                            None => width_anim.set(Some(Animatable((target, f32::VectorConverter)))),
                        }

                        let temp_alpha_anim = match alpha_anim.get() {
                            Some(it) => {
                                if it.target_value() != target {
                                    let (it, spec) = (it.clone(), alpha_spring.clone());
                                    named_args!(coroutine_scope.launch(async move {
                                        named_args!(it.animate_to(target, animation_spec = spec)).await;
                                    }));
                                }
                                it
                            }
                            None => Animatable((target, f32::VectorConverter)),
                        };
                        alpha_anim.set(Some(temp_alpha_anim.clone()));
                        is_visible.set(temp_alpha_anim.value() != 0.0);
                        if is_visible.peek() {
                            p.place_with_layer(&placeable, 0, 0, 0.0, move |layer| {
                                layer.alpha = temp_alpha_anim.value();
                            });
                        }
                    })
                },
            ),
            shape = FabMenuBaselineTokens::ListItemContainerShape.value(),
            color = container_color,
            content_color = Some(content_color),
            content = move || {
                let (width_anim, horizontal_alignment) = (content_width_anim.clone(), horizontal_alignment.clone());
                let (text, icon) = (text.clone(), icon.clone());
                Row(
                    modifier = Modifier
                        .layout(move |scope, measurable, constraints| {
                            let placeable = measurable.measure(constraints);
                            let progress = width_anim.get().map_or(0.0, |a| a.value());
                            let width = (placeable.width() as f32 * progress.max(0.0)).round() as i32;
                            let horizontal_alignment = horizontal_alignment.clone();
                            let layout_direction = scope.layout_direction();
                            scope.layout(width, placeable.height(), move |p| {
                                let x = horizontal_alignment.align(placeable.width(), width, layout_direction);
                                p.place_with_layer(&placeable, x, 0, 0.0, |_| {});
                            })
                        })
                        .size_in_with(FabMenuItemMinWidth, FabMenuItemHeight, Dp::Unspecified, Dp::Unspecified)
                        .padding_sides_with(
                            FabMenuItemContentPaddingStart,
                            Dp(0.0),
                            FabMenuItemContentPaddingEnd,
                            Dp(0.0),
                        ),
                    vertical_alignment = Alignment::CenterVertically,
                    horizontal_arrangement = Arrangement::spaced_by_aligned(
                        FabMenuItemContentSpacingHorizontal,
                        Alignment::CenterHorizontally,
                    ),
                    content = move || {
                        icon();
                        let text = text.clone();
                        KompozitionLocalProvider(
                            &[LocalTextStyle.provides(MaterialTheme::typography().title_medium.clone())],
                            move || text(),
                        );
                    },
                );
            },
        );
    });
}

/// Compose's `mutableStateOf<Animatable<T, AnimationVector1D>?>(null)`: an animatable held by the composable and set
/// as it is measured or placed, reading it a state read (so a layout reading it before it is made measures again
/// once it is). A state confined to the UI thread (an `Animatable` isn't `Send`).
type AnimatableState<T> = MutableState<Option<Animatable<T, AnimationVector1D>>>;

/// A state holding no animatable yet.
fn animatable_state<T: kompoz_animation::AnimationValue>() -> AnimatableState<T> {
    mutable_confined_state_of(None)
}

thread_local! {
    /// Compose's private `MenuItemRuler`: how far down from the column's top the shown items start (a ruler holds a
    /// reference counted value, so one per thread here; the UI is one thread).
    static MENU_ITEM_RULER: HorizontalRuler = HorizontalRuler::named("MenuItemRuler");
}

/// [`MENU_ITEM_RULER`].
fn menu_item_ruler() -> HorizontalRuler {
    MENU_ITEM_RULER.with(HorizontalRuler::clone)
}

// ---- The toggleable FAB ----------------------------------------------------------------------------------------

/// A toggleable FAB (Compose's Expressive `ToggleFloatingActionButton`), for a [`FloatingActionButtonMenu`]: as
/// `checked` changes, its checked progress goes from 0 to 1 (or back) along the fast spatial spec, and its container
/// — of `container_size` (56 dp, 56 checked, by default), drawn in `container_color` and rounded by
/// `container_corner_radius` (16 dp, round checked), each of the progress — follows, raised 6 dp, aligned by
/// `content_alignment` in the room of its unchecked size (shrinking to its top end by default). `content` (in a
/// [`ToggleFloatingActionButtonScope`], whose `checked_progress` it reads; typically an [`Icon`](crate::Icon) turning
/// from Add to Close at half way, with [`animate_icon`](ToggleFloatingActionButtonModifiers::animate_icon)) is
/// centered in it. A click calls `on_checked_change` with the other state; a checkbox to accessibility, rippling to
/// the corners of its unchecked size.
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn ToggleFloatingActionButton(
    checked: bool,
    on_checked_change: impl Fn(bool) + 'static,
    #[default(Modifier)] modifier: Modifier,
    #[default(ToggleFloatingActionButtonDefaults.container_color())] container_color: Rc<dyn Fn(f32) -> Color>,
    #[default(Alignment::TopEnd)] content_alignment: Alignment,
    #[default(ToggleFloatingActionButtonDefaults.container_size())] container_size: Rc<dyn Fn(f32) -> Dp>,
    #[default(ToggleFloatingActionButtonDefaults.container_corner_radius())] container_corner_radius: Rc<
        dyn Fn(f32) -> Dp,
    >,
    content: impl Fn(&ToggleFloatingActionButtonScope) + 'static,
) {
    let checked_progress = animate_float_as_state(
        if checked { 1.0 } else { 0.0 },
        // TODO Load the motionScheme tokens from the component tokens file
        animation_spec = MotionSchemeKeyTokens::FastSpatial.value(),
    );
    // (Compose's `{ checkedProgress.value }`, the same lambda each composition as Compose remembers it)
    let progress = remember(move || Rc::new(move || checked_progress.value()) as Rc<dyn Fn() -> f32>);
    ToggleFloatingActionButtonWithProgress(
        checked,
        Rc::new(on_checked_change),
        progress,
        modifier,
        container_color,
        content_alignment,
        container_size,
        container_corner_radius,
        Rc::new(content),
    );
}

/// [`ToggleFloatingActionButton`] driven by `checked_progress` (Compose's private overload taking `checkedProgress:
/// () -> Float`).
#[kompozable]
#[allow(clippy::too_many_arguments)]
fn ToggleFloatingActionButtonWithProgress(
    checked: bool,
    on_checked_change: Rc<dyn Fn(bool)>,
    checked_progress: Rc<dyn Fn() -> f32>,
    modifier: Modifier,
    container_color: Rc<dyn Fn(f32) -> Color>,
    content_alignment: Alignment,
    container_size: Rc<dyn Fn(f32) -> Dp>,
    container_corner_radius: Rc<dyn Fn(f32) -> Dp>,
    content: Rc<dyn Fn(&ToggleFloatingActionButtonScope)>,
) {
    let initial_size = {
        let container_size = container_size.clone();
        remember_keyed(ByAddress(container_size.clone()), move || container_size(0.0))
    };
    Box(
        modifier = Modifier.size(initial_size),
        content_alignment = content_alignment,
        content = move || {
            let density = LocalDensity.current();
            let fab_ripple_radius = remember_keyed(initial_size, move || {
                let fab_size_half = density.to_px(initial_size) / 2.0;
                density.to_dp(fab_size_half.hypot(fab_size_half))
            });
            let (layer_progress, layer_radius) = (checked_progress.clone(), container_corner_radius.clone());
            let (draw_progress, draw_radius, draw_color) =
                (checked_progress.clone(), container_corner_radius.clone(), container_color.clone());
            let (size_progress, size) = (checked_progress.clone(), container_size.clone());
            let on_checked_change = on_checked_change.clone();
            let scope_progress = checked_progress.clone();
            let content = content.clone();
            Box(
                modifier = modifier
                    .clone()
                    .graphics_layer_block(move |layer| {
                        layer.shadow_elevation = density.to_px(FabShadowElevation);
                        // (Compose's remembered `GenericShape { … }` reads the radius as its outline is made; KompozR's
                        // shapes are `Send + Sync` and the progress is the UI thread's, so the radius is read here —
                        // this block runs again as it changes — and the shape made of it, the same rounded rect)
                        let radius = density.to_px(layer_radius(layer_progress()));
                        layer.shape = Arc::new(GenericShape(move |path: &mut Path, size: Size, _| {
                            path.add_round_rect(
                                &RoundRect::from_rect_radius(size.to_rect(), CornerRadius::new(radius, radius)),
                                PathDirection::CounterClockwise,
                            );
                        }));
                        layer.clip = true;
                    })
                    .draw_behind(move |d| {
                        let radius = d.to_px(draw_radius(draw_progress()));
                        named_args!(d.draw_round_rect(
                            draw_color(draw_progress()),
                            corner_radius = CornerRadius::new(radius, radius),
                        ));
                    })
                    .toggleable_indication_with(
                        checked,
                        None,
                        Some(named_args!(ripple(radius = fab_ripple_radius))),
                        true,
                        None,
                        move |on| on_checked_change(on),
                    )
                    .layout(move |scope, measurable, constraints| {
                        let placeable = measurable.measure(constraints);
                        let size_px = scope.round_to_px(size(size_progress()));
                        scope.layout(size_px, size_px, move |p| {
                            p.place(&placeable, (size_px - placeable.width()) / 2, (size_px - placeable.height()) / 2);
                        })
                    }),
                content = move || {
                    let progress = scope_progress.clone();
                    let scope = remember_keyed(ByAddress(progress.clone()), move || ToggleFloatingActionButtonScope {
                        checked_progress: progress,
                    });
                    content(&scope);
                },
            );
        },
    );
}

/// A shared value compared by its address (Kotlin's `remember(lambda)`: the same lambda instance).
struct ByAddress<T: ?Sized>(Rc<T>);

impl<T: ?Sized> PartialEq for ByAddress<T> {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

/// The scope of a [`ToggleFloatingActionButton`]'s content (Compose's Expressive interface
/// `ToggleFloatingActionButtonScope`): its checked progress. Cloned, it reads the same progress (what a lambda kept
/// past the content captures).
#[derive(Clone)]
pub struct ToggleFloatingActionButtonScope {
    checked_progress: Rc<dyn Fn() -> f32>,
}

impl ToggleFloatingActionButtonScope {
    /// How far checked the button is, 0 to 1 as it animates (Compose's `checkedProgress`; a state read).
    pub fn checked_progress(&self) -> f32 {
        (self.checked_progress)()
    }
}

impl std::fmt::Debug for ToggleFloatingActionButtonScope {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.debug_struct("ToggleFloatingActionButtonScope").field("checked_progress", &self.checked_progress()).finish()
    }
}

// ---- Defaults --------------------------------------------------------------------------------------------------

/// The toggleable FAB's defaults (Compose's Expressive `ToggleFloatingActionButtonDefaults`): its colors and sizes
/// as functions of the checked progress (0 to 1), from the unchecked FAB's (of the three sizes) to the checked round
/// close button's. Methods of the value (`ToggleFloatingActionButtonDefaults.container_size()`); Compose's
/// `Modifier.animateIcon` is [`ToggleFloatingActionButtonModifiers::animate_icon`].
///
/// Kotlin's overloads `containerSize()` / `containerSize(initialSize, finalSize)` (and the corner radius's and the icon
/// size's) are [`container_size`](Self::container_size) / [`container_size_of`](Self::container_size_of) here.
#[derive(Clone, Copy, Debug)]
pub struct ToggleFloatingActionButtonDefaults;

#[named]
impl ToggleFloatingActionButtonDefaults {
    /// The container's color: `initial_color` (the scheme's primary container) to `final_color` (its primary)
    /// (Compose's `containerColor`).
    ///
    /// Kotlin: `containerColor(initialColor: Color = MaterialTheme.colorScheme.primaryContainer, finalColor: Color = MaterialTheme.colorScheme.primary): (Float) -> Color`
    pub fn container_color(
        &self,
        #[default(MaterialTheme::color_scheme().primary_container)] initial_color: Color,
        #[default(MaterialTheme::color_scheme().primary)] final_color: Color,
    ) -> Rc<dyn Fn(f32) -> Color> {
        Rc::new(move |progress| lerp_color(initial_color, final_color, progress))
    }

    /// The container's size: `initial_size` to `final_size` (the close button's 56 dp) (Compose's
    /// `containerSize(initialSize, finalSize)`).
    ///
    /// Kotlin: `containerSize(initialSize: Dp, finalSize: Dp = FabFinalSize): (Float) -> Dp`
    pub fn container_size_of(
        &self,
        initial_size: Dp,
        #[default(FabFinalSize)] final_size: Dp,
    ) -> Rc<dyn Fn(f32) -> Dp> {
        Rc::new(move |progress| lerp_dp(initial_size, final_size, progress))
    }

    /// The FAB's container size: 56 dp to 56 (Compose's `containerSize()`).
    ///
    /// Kotlin: `containerSize(): (Float) -> Dp`
    pub fn container_size(&self) -> Rc<dyn Fn(f32) -> Dp> {
        self.container_size_of(FabInitialSize)
    }

    /// The medium FAB's container size: 80 dp to 56 (Compose's `containerSizeMedium`).
    pub fn container_size_medium(&self) -> Rc<dyn Fn(f32) -> Dp> {
        self.container_size_of(FabMediumInitialSize)
    }

    /// The large FAB's container size: 96 dp to 56 (Compose's `containerSizeLarge`).
    pub fn container_size_large(&self) -> Rc<dyn Fn(f32) -> Dp> {
        self.container_size_of(FabLargeInitialSize)
    }

    /// The container's corner radius: `initial_size` to `final_size` (28 dp, round) (Compose's
    /// `containerCornerRadius(initialSize, finalSize)`).
    ///
    /// Kotlin: `containerCornerRadius(initialSize: Dp, finalSize: Dp = FabFinalCornerRadius): (Float) -> Dp`
    pub fn container_corner_radius_of(
        &self,
        initial_size: Dp,
        #[default(FabFinalCornerRadius)] final_size: Dp,
    ) -> Rc<dyn Fn(f32) -> Dp> {
        Rc::new(move |progress| lerp_dp(initial_size, final_size, progress))
    }

    /// The FAB's corner radius: 16 dp to 28 (Compose's `containerCornerRadius()`).
    ///
    /// Kotlin: `containerCornerRadius(): (Float) -> Dp`
    pub fn container_corner_radius(&self) -> Rc<dyn Fn(f32) -> Dp> {
        self.container_corner_radius_of(FabInitialCornerRadius)
    }

    /// The medium FAB's corner radius: 20 dp to 28 (Compose's `containerCornerRadiusMedium`).
    pub fn container_corner_radius_medium(&self) -> Rc<dyn Fn(f32) -> Dp> {
        self.container_corner_radius_of(FabMediumInitialCornerRadius)
    }

    /// The large FAB's corner radius: 28 dp to 28 (Compose's `containerCornerRadiusLarge`).
    pub fn container_corner_radius_large(&self) -> Rc<dyn Fn(f32) -> Dp> {
        self.container_corner_radius_of(FabLargeInitialCornerRadius)
    }

    /// The icon's color: `initial_color` (the scheme's on primary container) to `final_color` (its on primary)
    /// (Compose's `iconColor`).
    ///
    /// Kotlin: `iconColor(initialColor: Color = MaterialTheme.colorScheme.onPrimaryContainer, finalColor: Color = MaterialTheme.colorScheme.onPrimary): (Float) -> Color`
    pub fn icon_color(
        &self,
        #[default(MaterialTheme::color_scheme().on_primary_container)] initial_color: Color,
        #[default(MaterialTheme::color_scheme().on_primary)] final_color: Color,
    ) -> Rc<dyn Fn(f32) -> Color> {
        Rc::new(move |progress| lerp_color(initial_color, final_color, progress))
    }

    /// The icon's size: `initial_size` to `final_size` (20 dp) (Compose's `iconSize(initialSize, finalSize)`).
    ///
    /// Kotlin: `iconSize(initialSize: Dp, finalSize: Dp = FabFinalIconSize): (Float) -> Dp`
    pub fn icon_size_of(&self, initial_size: Dp, #[default(FabFinalIconSize)] final_size: Dp) -> Rc<dyn Fn(f32) -> Dp> {
        Rc::new(move |progress| lerp_dp(initial_size, final_size, progress))
    }

    /// The FAB's icon size: 24 dp to 20 (Compose's `iconSize()`).
    ///
    /// Kotlin: `iconSize(): (Float) -> Dp`
    pub fn icon_size(&self) -> Rc<dyn Fn(f32) -> Dp> {
        self.icon_size_of(FabInitialIconSize)
    }

    /// The medium FAB's icon size: 28 dp to 20 (Compose's `iconSizeMedium`).
    pub fn icon_size_medium(&self) -> Rc<dyn Fn(f32) -> Dp> {
        self.icon_size_of(FabMediumInitialIconSize)
    }

    /// The large FAB's icon size: 36 dp to 20 (Compose's `iconSizeLarge`).
    pub fn icon_size_large(&self) -> Rc<dyn Fn(f32) -> Dp> {
        self.icon_size_of(FabLargeInitialIconSize)
    }
}

/// The toggleable FAB's icon modifier (Compose's `ToggleFloatingActionButtonDefaults.Modifier.animateIcon`, a member
/// extension of the object, which Kotlin imports to call as `Modifier.animateIcon(…)`; an extension trait of
/// [`Modifier`] here).
#[named(trait ToggleFloatingActionButtonModifiers)]
impl Modifier {
    /// Sizes what it modifies to `size` (24 dp to 20 by default) and tints it `color` (on primary container to on
    /// primary), each of `checked_progress` (Compose's `animateIcon`): measured at exactly that size, drawn through a
    /// layer with a tint color filter. Reads the progress as it measures and draws (no composition as it moves).
    /// Composable (its defaults read the theme).
    #[track_caller]
    fn animate_icon(
        self,
        checked_progress: Rc<dyn Fn() -> f32>,
        #[default(ToggleFloatingActionButtonDefaults.icon_color())] color: Rc<dyn Fn(f32) -> Color>,
        #[default(ToggleFloatingActionButtonDefaults.icon_size())] size: Rc<dyn Fn(f32) -> Dp>,
    ) -> Modifier {
        let measured_progress = checked_progress.clone();
        self.layout(move |scope, measurable, _| {
            let size_px = scope.round_to_px(size(measured_progress()));
            let placeable = measurable.measure(Constraints::fixed(size_px, size_px));
            scope.layout(size_px, size_px, move |p| p.place(&placeable, 0, 0))
        })
        .draw_with_cache(move |cache| {
            let layer = cache.obtain_graphics_layer();
            cache.record(&layer, |c| c.draw_content());
            layer.set_color_filter(Some(ColorFilter::tint(color(checked_progress()), BlendMode::SrcIn)));

            cache.on_draw_with_content(move |c| c.draw_layer(&layer))
        })
    }
}

// ---- Item visibility -------------------------------------------------------------------------------------------

/// `modifier` telling the menu's column (through its parent data) and accessibility whether the item shows
/// (Compose's private `Modifier.itemVisible`): an invisible item's semantics are cleared.
fn item_visible_modifier(modifier: Modifier, is_visible: Rc<dyn Fn() -> bool>) -> Modifier {
    modifier.then_element(MenuItemVisibleElement { is_visible })
}

/// Compose's private `MenuItemVisibleElement`: equal while it has the same function.
struct MenuItemVisibleElement {
    is_visible: Rc<dyn Fn() -> bool>,
}

impl PartialEq for MenuItemVisibleElement {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.is_visible, &other.is_visible)
    }
}

impl ModifierNodeElement for MenuItemVisibleElement {
    type Node = MenuItemVisibilityModifier;
    fn create(&self) -> MenuItemVisibilityModifier {
        MenuItemVisibilityModifier { visible: Rc::new(RefCell::new(self.is_visible.clone())) }
    }
    fn update(&self, node: &mut MenuItemVisibilityModifier) {
        *node.visible.borrow_mut() = self.is_visible.clone();
    }
    fn name(&self) -> &'static str {
        "itemVisible"
    }
}

/// Compose's private `MenuItemVisibilityModifier`: its parent data is itself (here a handle to its function, which
/// an update replaces for both), its descendants' semantics cleared while it isn't visible.
struct MenuItemVisibilityModifier {
    visible: Rc<RefCell<Rc<dyn Fn() -> bool>>>,
}

/// What the column reads of an item's parent data (Compose's parent data is the node itself).
struct MenuItemVisibility(Rc<RefCell<Rc<dyn Fn() -> bool>>>);

impl ModifierNode for MenuItemVisibilityModifier {
    node_capabilities!(parent_data, semantics);
}

impl ParentDataModifierNode for MenuItemVisibilityModifier {
    fn modify_parent_data(&mut self, _: &Density, _: Option<Box<dyn Any>>) -> Option<Box<dyn Any>> {
        Some(Box::new(MenuItemVisibility(self.visible.clone())))
    }
}

impl SemanticsModifierNode for MenuItemVisibilityModifier {
    fn should_clear_descendant_semantics(&self) -> bool {
        let visible = self.visible.borrow().clone();
        !visible()
    }

    fn apply_semantics(&mut self, _: &mut SemanticsConfiguration) {}
}

/// Whether a measured item is visible (Compose's private `Placeable.isVisible`): not unless its parent data says so,
/// visible when it has none.
fn is_visible(placeable: &kompoz_ui::layout::Placeable) -> bool {
    let Some(data) = placeable.parent_data() else { return true };
    match data.downcast_ref::<MenuItemVisibility>() {
        Some(MenuItemVisibility(visible)) => {
            let visible = visible.borrow().clone();
            visible()
        }
        None => true,
    }
}

// ---- Values ----------------------------------------------------------------------------------------------------

const FabInitialSize: Dp = FabBaselineTokens::ContainerHeight;
const FabInitialCornerRadius: Dp = Dp(16.0);
const FabInitialIconSize: Dp = FabBaselineTokens::IconSize;
const FabMediumInitialSize: Dp = FabMediumTokens::ContainerHeight;
const FabMediumInitialCornerRadius: Dp = Dp(20.0);
const FabMediumInitialIconSize: Dp = FabMediumTokens::IconSize;
const FabLargeInitialSize: Dp = FabLargeTokens::ContainerHeight;
const FabLargeInitialCornerRadius: Dp = Dp(28.0);
const FabLargeInitialIconSize: Dp = Dp(36.0); // TODO: FabLargeTokens.IconSize is incorrect
const FabFinalSize: Dp = FabMenuBaselineTokens::CloseButtonContainerHeight;
const FabFinalCornerRadius: Dp = Dp(FabFinalSize.0 / 2.0);
const FabFinalIconSize: Dp = FabMenuBaselineTokens::CloseButtonIconSize;
const FabShadowElevation: Dp = FabPrimaryContainerTokens::ContainerElevation;
const FabMenuPaddingHorizontal: Dp = Dp(16.0);
const FabMenuPaddingBottom: Dp = FabMenuBaselineTokens::CloseButtonBetweenSpace;
const FabMenuButtonPaddingBottom: Dp = Dp(16.0);
const FabMenuItemMinWidth: Dp = FabMenuBaselineTokens::ListItemContainerHeight;
const FabMenuItemHeight: Dp = FabMenuBaselineTokens::ListItemContainerHeight;
const FabMenuItemSpacingVertical: Dp = FabMenuBaselineTokens::ListItemBetweenSpace;
const FabMenuItemContentPaddingStart: Dp = FabMenuBaselineTokens::ListItemLeadingSpace;
const FabMenuItemContentPaddingEnd: Dp = FabMenuBaselineTokens::ListItemTrailingSpace;
const FabMenuItemContentSpacingHorizontal: Dp = FabMenuBaselineTokens::ListItemIconLabelSpace;
