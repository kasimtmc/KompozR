//! Material's scaffold, as Compose's Scaffold.kt: a screen's basic layout ([`Scaffold`]) — a top bar, a bottom
//! bar, a floating action button (where [`FabPosition`] says), a snackbar host and the content between the bars,
//! given the padding they leave (and the window insets they don't take) —, and its defaults
//! ([`ScaffoldDefaults`]).
//!
//! As Compose's, the content's padding is one value made once, whose sides are read from a state the layout sets
//! as it measures, before it composes the content: what lays out by it (a padding modifier, a lazy list's content
//! padding) follows the bars without the content composing again; what reads it while composing composes again.

use crate::internal::mutable_window_insets::MutableWindowInsets;
use crate::internal::system_bars_default_insets::system_bars_for_visual_components;
use crate::material_theme::MaterialTheme;
use crate::surface::Surface;
use kompoz_foundation::layout::{CustomPaddingValues, WindowInsets};
use kompoz_foundation::prelude::*;
use kompoz_graphics::{Color, Dp, LayoutDirection};
use kompoz_runtime::{
    Memoized, MutableState, kompozable, kompozable_block, mutable_state_of, remember, remember_keyed,
};
use kompoz_ui::layout::{Placeable, SubcomposeLayout};
use kompoz_ui::{Constraints, Measurable, Modifier};
use std::fmt;
use std::rc::Rc;

/// [Material Design layout](https://m3.material.io/foundations/layout/understanding-layout/): the screen's basic
/// layout (Compose's `Scaffold`), putting Material's components together so they work together — `top_bar`
/// (typically a `TopAppBar`) at the top, `bottom_bar` (typically a `NavigationBar`) at the bottom,
/// `floating_action_button` at `floating_action_button_position` above the bottom bar (or over it:
/// [`FabPosition::EndOverlay`]), 16 dp from the edges, `snackbar_host` (typically a [`SnackbarHost`]: show a
/// snackbar with [`SnackbarHostState::show_snackbar`]) centered above them, and `content` filling the scaffold
/// under the bars, given the padding to apply to its root (with `consume_window_insets`) so the bars don't cover it.
/// (With a vertical scroll, apply the padding to the scrolled child, not the scroll.)
///
/// The scaffold is a [`Surface`] of `container_color` (the scheme's background; `Color::Transparent` for none) and
/// `content_color` (None: the container's content color, Compose's `contentColorFor(containerColor)`).
/// `content_window_insets` (Compose's default [`ScaffoldDefaults::content_window_insets`]) are what the content's
/// padding keeps clear of: at the top and the bottom only where there is no top / bottom bar (the bars take care of
/// the insets), and without what layouts around the scaffold already consumed. The snackbar host and the floating
/// action button keep clear of the insets at the bottom and the sides.
///
/// [`SnackbarHost`]: crate::SnackbarHost
/// [`SnackbarHostState::show_snackbar`]: crate::SnackbarHostState::show_snackbar
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn Scaffold(
    #[default(Modifier)] modifier: Modifier,
    #[default(|| {})] top_bar: impl Fn() + 'static,
    #[default(|| {})] bottom_bar: impl Fn() + 'static,
    #[default(|| {})] snackbar_host: impl Fn() + 'static,
    #[default(|| {})] floating_action_button: impl Fn() + 'static,
    #[default(FabPosition::End)] floating_action_button_position: FabPosition,
    #[default(MaterialTheme::color_scheme().background)] container_color: Color,
    #[default(None)] content_color: Option<Color>,
    #[default(ScaffoldDefaults.content_window_insets())] content_window_insets: WindowInsets,
    content: impl Fn(PaddingValues) + 'static,
) {
    let safe_insets = {
        let insets = content_window_insets.clone();
        remember_keyed(content_window_insets.clone(), move || MutableWindowInsets::new(insets))
    };
    let consumed_insets = safe_insets.clone();
    Surface(
        modifier = modifier.on_consumed_window_insets_changed(move |consumed_window_insets| {
            // Exclude currently consumed window insets from user provided contentWindowInsets
            consumed_insets.set_insets(content_window_insets.exclude(consumed_window_insets));
        }),
        color = container_color,
        content_color = content_color,
        content = || {
            ScaffoldLayout(
                fab_position = floating_action_button_position,
                top_bar = top_bar,
                bottom_bar = bottom_bar,
                content = content,
                snackbar = snackbar_host,
                content_window_insets = safe_insets.as_window_insets(),
                fab = floating_action_button,
            );
        },
    );
}

/// The content's padding (Compose's `object : PaddingValues` of ScaffoldLayout): its sides read from `holder`,
/// which the layout sets as it measures. Compose's holder is a `mutableStateOf(PaddingValues(0.dp))` it sets to
/// `PaddingValues(top, bottom, start, end)`; KompozR's states hold `Send` values (a `PaddingValues` isn't), so it
/// holds those four sides (equal when the padding values are: the same structural equality).
struct ScaffoldContentPadding {
    holder: MutableState<PaddingSides>,
}

/// The sides of Compose's `PaddingValues(start, top, end, bottom)` the holder is set to.
#[derive(Clone, Copy, PartialEq, Debug)]
struct PaddingSides {
    start: Dp,
    top: Dp,
    end: Dp,
    bottom: Dp,
}

impl ScaffoldContentPadding {
    fn padding_holder(&self) -> PaddingValues {
        let PaddingSides { start, top, end, bottom } = self.holder.get();
        PaddingValues::new(start, top, end, bottom)
    }
}

impl CustomPaddingValues for ScaffoldContentPadding {
    fn calculate_left_padding(&self, layout_direction: LayoutDirection) -> Dp {
        self.padding_holder().calculate_left_padding(layout_direction)
    }

    fn calculate_top_padding(&self) -> Dp {
        self.padding_holder().calculate_top_padding()
    }

    fn calculate_right_padding(&self, layout_direction: LayoutDirection) -> Dp {
        self.padding_holder().calculate_right_padding(layout_direction)
    }

    fn calculate_bottom_padding(&self) -> Dp {
        self.padding_holder().calculate_bottom_padding()
    }
}

/// The scaffold's layout (Compose's private `ScaffoldLayout`): `top_bar` at the top, `content` (the main body,
/// given the padding the bars and `content_window_insets` leave) under everything, `snackbar` above the floating
/// action button `fab` (where `fab_position` says), above `bottom_bar` at the bottom. Each slot is composed in a
/// `Box` as the layout measures, the body last, once its padding is set.
#[kompozable]
fn ScaffoldLayout(
    fab_position: FabPosition,
    top_bar: impl Fn() + 'static,
    content: impl Fn(PaddingValues) + 'static,
    snackbar: impl Fn() + 'static,
    fab: impl Fn() + 'static,
    content_window_insets: WindowInsets,
    bottom_bar: impl Fn() + 'static,
) {
    // Create the backing value for the content padding
    // These values will be updated during measurement, but before subcomposing the body content
    // Remembering and updating a single PaddingValues avoids needing to recompose when the values
    // change
    let (padding_holder, content_padding) = remember(|| {
        let zero = Dp(0.0);
        let holder = mutable_state_of(PaddingSides { start: zero, top: zero, end: zero, bottom: zero });
        (holder.clone(), PaddingValues::custom(ScaffoldContentPadding { holder }))
    });

    // (Compose's `remember(slot) { { Box { slot() } } }`: kept while the slot is the same lambda — each closure
    // parameter's identity is `__kz_id_<name>`, bound by `#[kompozable]` —; the content padding, remembered for
    // good, needs no key of its own)
    let top_bar_content = box_content(Memoized::new(top_bar, __kz_id_top_bar));
    let snackbar_content = box_content(Memoized::new(snackbar, __kz_id_snackbar));
    let fab_content = box_content(Memoized::new(fab, __kz_id_fab));
    let body_content: Rc<dyn Fn()> = Memoized::new(content, __kz_id_content).remember(move |content| {
        Rc::new(move || {
            kompozable_block!({
                let (content, content_padding) = (content.clone(), content_padding.clone());
                Box(content = move || content(content_padding));
            })
        }) as Rc<dyn Fn()>
    });
    let bottom_bar_content = box_content(Memoized::new(bottom_bar, __kz_id_bottom_bar));
    SubcomposeLayout(
        measure_policy = move |scope, constraints| {
            let layout_width = constraints.max_width;
            let layout_height = constraints.max_height;
            let layout_direction = scope.layout_direction();
            let density = scope.density();

            let loose_constraints = Constraints { min_width: 0, min_height: 0, ..constraints };

            // respect only bottom and horizontal for snackbar and fab
            let left_inset = content_window_insets.get_left(&density, layout_direction);
            let right_inset = content_window_insets.get_right(&density, layout_direction);
            let bottom_inset = content_window_insets.get_bottom(&density);

            let first = |measurables: Vec<Rc<dyn Measurable>>, constraints: Constraints| -> Placeable {
                measurables.first().expect("a slot composes a Box").measure(constraints)
            };

            let top_bar_placeable = first(
                scope.subcompose_shared(ScaffoldLayoutContent::TopBar, top_bar_content.clone()),
                loose_constraints,
            );

            let snackbar_placeable = first(
                scope.subcompose_shared(ScaffoldLayoutContent::Snackbar, snackbar_content.clone()),
                loose_constraints.r#offset(-left_inset - right_inset, -bottom_inset),
            );

            let fab_placeable = first(
                scope.subcompose_shared(ScaffoldLayoutContent::Fab, fab_content.clone()),
                loose_constraints.r#offset(-left_inset - right_inset, -bottom_inset),
            );

            let fab_spacing = density.round_to_px(FabSpacing);
            let is_fab_empty = fab_placeable.width() == 0 && fab_placeable.height() == 0;
            let fab_placement = if !is_fab_empty {
                let fab_width = fab_placeable.width();
                let fab_height = fab_placeable.height();
                // FAB distance from the left of the layout, taking into account LTR / RTL
                let fab_left_offset = match fab_position {
                    FabPosition::Start => {
                        if layout_direction == LayoutDirection::Ltr {
                            fab_spacing + left_inset
                        } else {
                            layout_width - fab_spacing - fab_width - right_inset
                        }
                    }
                    FabPosition::End | FabPosition::EndOverlay => {
                        if layout_direction == LayoutDirection::Ltr {
                            layout_width - fab_spacing - fab_width - right_inset
                        } else {
                            fab_spacing + left_inset
                        }
                    }
                    _ => (layout_width - fab_width + left_inset - right_inset) / 2,
                };

                Some(FabPlacement { left: fab_left_offset, width: fab_width, height: fab_height })
            } else {
                None
            };

            let bottom_bar_placeable = first(
                scope.subcompose_shared(ScaffoldLayoutContent::BottomBar, bottom_bar_content.clone()),
                loose_constraints,
            );

            let is_bottom_bar_empty = bottom_bar_placeable.width() == 0 && bottom_bar_placeable.height() == 0;

            let fab_offset_from_bottom = fab_placement.as_ref().map(|it| {
                if is_bottom_bar_empty || fab_position == FabPosition::EndOverlay {
                    it.height + fab_spacing + content_window_insets.get_bottom(&density)
                } else {
                    // Total height is the bottom bar height + the FAB height + the padding
                    // between the FAB and bottom bar
                    bottom_bar_placeable.height() + it.height + fab_spacing
                }
            });

            let snackbar_height = snackbar_placeable.height();
            let snackbar_offset_from_bottom = if snackbar_height != 0 {
                snackbar_height
                    + fab_offset_from_bottom
                        .or((!is_bottom_bar_empty).then(|| bottom_bar_placeable.height()))
                        .unwrap_or_else(|| content_window_insets.get_bottom(&density))
            } else {
                0
            };

            // Update the backing state for the content padding before subcomposing the body
            let insets = content_window_insets.as_padding_values_with(density);
            padding_holder.set(PaddingSides {
                top: if top_bar_placeable.width() == 0 && top_bar_placeable.height() == 0 {
                    insets.calculate_top_padding()
                } else {
                    density.to_dp(top_bar_placeable.height() as f32)
                },
                bottom: if is_bottom_bar_empty {
                    insets.calculate_bottom_padding()
                } else {
                    density.to_dp(bottom_bar_placeable.height() as f32)
                },
                start: insets.calculate_start_padding(layout_direction),
                end: insets.calculate_end_padding(layout_direction),
            });

            let body_content_placeable = first(
                scope.subcompose_shared(ScaffoldLayoutContent::MainContent, body_content.clone()),
                loose_constraints,
            );

            let content_window_insets = content_window_insets.clone();
            scope.layout(layout_width, layout_height, move |p| {
                // Placing to control drawing order to match default elevation of each placeable
                p.place(&body_content_placeable, 0, 0);
                p.place(&top_bar_placeable, 0, 0);
                p.place(
                    &snackbar_placeable,
                    (layout_width - snackbar_placeable.width()
                        + content_window_insets.get_left(&density, layout_direction)
                        - content_window_insets.get_right(&density, layout_direction))
                        / 2,
                    layout_height - snackbar_offset_from_bottom,
                );
                // The bottom bar is always at the bottom of the layout
                p.place(&bottom_bar_placeable, 0, layout_height - bottom_bar_placeable.height());
                // Explicitly not using placeRelative here as `leftOffset` already accounts for RTL
                if let (Some(placement), Some(fab_offset_from_bottom)) = (&fab_placement, fab_offset_from_bottom) {
                    p.place(&fab_placeable, placement.left, layout_height - fab_offset_from_bottom);
                }
            })
        },
    );
}

/// A slot in a `Box` (Compose's `remember(slot) { { Box { slot() } } }`): made again when the slot is another
/// lambda. `slot` comes with its identity as a memoized closure (a restartable composable's `__kz_id_<name>`, what
/// `#[kompozable]` binds beside a closure parameter).
#[track_caller]
fn box_content(slot: Memoized<impl Fn() + Clone + 'static>) -> Rc<dyn Fn()> {
    slot.remember(|slot| {
        Rc::new(move || {
            kompozable_block!({
                let slot = slot.clone();
                Box(content = slot);
            })
        }) as Rc<dyn Fn()>
    })
}

/// The scaffold's defaults (Compose's `object ScaffoldDefaults`): its composables are methods of the value, as
/// Kotlin reads them (`ScaffoldDefaults.content_window_insets()`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ScaffoldDefaults;

impl ScaffoldDefaults {
    /// The insets the scaffold's content slot is given and consumes by default (Compose's `contentWindowInsets`):
    /// the system bars' (Compose's internal `WindowInsets.systemBarsForVisualComponents`).
    #[track_caller]
    pub fn content_window_insets(&self) -> WindowInsets {
        system_bars_for_visual_components()
    }
}

/// Where a [`Scaffold`]'s floating action button goes (Compose's value class `FabPosition`).
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct FabPosition(i32);

impl FabPosition {
    /// At the bottom of the screen at the start, above the bottom bar (a `NavigationBar`), if there is one.
    pub const Start: FabPosition = FabPosition(0);
    /// At the bottom of the screen in the center, above the bottom bar, if there is one.
    pub const Center: FabPosition = FabPosition(1);
    /// At the bottom of the screen at the end, above the bottom bar, if there is one.
    pub const End: FabPosition = FabPosition(2);
    /// At the bottom of the screen at the end, over the bottom bar, if there is one.
    pub const EndOverlay: FabPosition = FabPosition(3);
}

/// Compose's toString (`FabPosition.End`).
impl fmt::Display for FabPosition {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(match *self {
            FabPosition::Start => "FabPosition.Start",
            FabPosition::Center => "FabPosition.Center",
            FabPosition::End => "FabPosition.End",
            _ => "FabPosition.EndOverlay",
        })
    }
}

impl fmt::Debug for FabPosition {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

/// Where a [`Scaffold`] places its floating action button (Compose's internal `FabPlacement`): its offset from the
/// left of the bottom bar (already right to left's), its width and its height.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct FabPlacement {
    pub(crate) left: i32,
    // (read by nothing in M15, as in Compose: what read it in older material3s, LocalFabPlacement, is gone)
    #[allow(dead_code)]
    pub(crate) width: i32,
    pub(crate) height: i32,
}

/// The FAB's spacing above the bottom bar / the bottom of the scaffold (Compose's `FabSpacing`).
const FabSpacing: Dp = Dp(16.0);

/// The scaffold's slots (Compose's `ScaffoldLayoutContent`).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum ScaffoldLayoutContent {
    TopBar,
    MainContent,
    Snackbar,
    Fab,
    BottomBar,
}
