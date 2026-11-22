//! Material's top app bars, as Compose's AppBar.kt has them: the small one ([`TopAppBar`], its title at the start,
//! with a subtitle: [`TopAppBarWithSubtitle`]), the center aligned one ([`CenterAlignedTopAppBar`]), and the two-rows
//! ones — medium and large ([`MediumTopAppBar`], [`LargeTopAppBar`]), their flexible Expressive forms
//! ([`MediumFlexibleTopAppBar`], [`LargeFlexibleTopAppBar`]) and one of the app's own titles
//! ([`TwoRowsTopAppBar`]) — that collapse to a small bar as the content scrolls under them. Each takes a
//! [`TopAppBarScrollBehavior`] moving it with a scrolled content, and is made by a component override
//! ([`LocalSingleRowTopAppBarOverride`], [`LocalTwoRowsTopAppBarOverride`]: Compose's experimental component override
//! API). Their state, colors, defaults and behaviors are in top_app_bar_defaults.rs.

use crate::content_color::LocalContentColor;
use crate::internal::provide_content_color_text_style::ProvideContentColorTextStyle;
use crate::tokens::{
    AppBarLargeFlexibleTokens, AppBarLargeTokens, AppBarMediumFlexibleTokens, AppBarMediumTokens, AppBarSmallTokens,
    MotionSchemeKeyTokens,
};
use crate::top_app_bar_defaults::{TopAppBarColors, TopAppBarDefaults, TopAppBarScrollBehavior, settle_app_bar};
use kompoz_animation::{CubicBezierEasing, Easing, animate_color_as_state};
use kompoz_foundation::gestures::{DraggableModifiers, OnDragStopped, Orientation, remember_draggable_state};
use kompoz_foundation::layout::{
    Arrangement, PaddingValues, VerticalArrangement, WindowInsets, WindowInsetsPaddingModifiers, WindowInsetsSides,
};
use kompoz_foundation::prelude::*;
use kompoz_graphics::LayoutDirection;
use kompoz_graphics::{Color, Dp};
use kompoz_runtime::{
    KompozitionLocal, KompozitionLocalProvider, derived_state_of, kompozable, kompozable_block, named_args,
    remember_keyed,
};
use kompoz_text::TextStyle;
use kompoz_ui::layout::{
    IntrinsicMeasurable, IntrinsicMeasureScope, LastBaseline, Measurable, MeasurePolicy, MeasureResult, MeasureScope,
    Placeable, layout_into,
};
use kompoz_ui::platform::LocalDensity;
use kompoz_ui::{Alignment, Constraints, HorizontalAlignment, Modifier};
use std::rc::Rc;

// ---- the top app bars ----------------------------------------------------------------------------------------------

/// A small top app bar (Compose's `TopAppBar`): `navigation_icon` at the start (in the navigation icon's color),
/// `title` after it (in the title's color and the title large style), `actions` in a row at the end (in the
/// actions' color), on a container in `colors`' container color — turning to its scrolled color once content
/// scrolls under it (`scroll_behavior`'s state says) —, `expanded_height` tall (an unspecified or infinite one:
/// [`TopAppBarDefaults::TopAppBarExpandedHeight`], 64 dp) inside `window_insets` and `content_padding`.
/// `scroll_behavior` (none: it stays) collapses and expands it with a scrolled content; a bar that isn't pinned
/// takes vertical drags too. Made by [`LocalSingleRowTopAppBarOverride`]'s override.
///
/// Kotlin: `TopAppBar(title: @Composable () -> Unit, modifier: Modifier = Modifier, navigationIcon: @Composable () -> Unit = {}, actions: @Composable RowScope.() -> Unit = {}, expandedHeight: Dp = TopAppBarDefaults.TopAppBarExpandedHeight, windowInsets: WindowInsets = TopAppBarDefaults.windowInsets, colors: TopAppBarColors = TopAppBarDefaults.topAppBarColors(), scrollBehavior: TopAppBarScrollBehavior? = null, contentPadding: PaddingValues = TopAppBarDefaults.ContentPadding)`
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn TopAppBar(
    title: impl Fn() + 'static,
    #[default(Modifier)] modifier: Modifier,
    #[default(|| {})] navigation_icon: impl Fn() + 'static,
    #[default(|| {})] actions: impl Fn() + 'static,
    #[default(TopAppBarDefaults::TopAppBarExpandedHeight)] expanded_height: Dp,
    #[default(TopAppBarDefaults.window_insets())] window_insets: WindowInsets,
    #[default(TopAppBarDefaults.top_app_bar_colors())] colors: TopAppBarColors,
    #[default(None)] scroll_behavior: Option<Rc<dyn TopAppBarScrollBehavior>>,
    #[default(TopAppBarDefaults::ContentPadding())] content_padding: PaddingValues,
) {
    SingleRowTopAppBar(
        modifier,
        Rc::new(title),
        AppBarSmallTokens::TitleFont.value(),
        None,
        TextStyle::Default,
        Alignment::Start,
        Rc::new(navigation_icon),
        Rc::new(actions),
        or_default(expanded_height, TopAppBarDefaults::TopAppBarExpandedHeight),
        content_padding,
        window_insets,
        colors,
        scroll_behavior,
    );
}

/// A small top app bar with a subtitle (Compose's experimental Expressive `TopAppBar(title, subtitle, …)`): as
/// [`TopAppBar`], `subtitle` under the title (in the subtitle's color and the label medium style), the two lined up
/// by `title_horizontal_alignment` (at the start; centered, they are in the middle of the whole bar).
///
/// Kotlin: `TopAppBar(title: @Composable () -> Unit, subtitle: @Composable () -> Unit, modifier: Modifier = Modifier, navigationIcon: @Composable () -> Unit = {}, actions: @Composable RowScope.() -> Unit = {}, titleHorizontalAlignment: Alignment.Horizontal = Alignment.Start, expandedHeight: Dp = TopAppBarDefaults.TopAppBarExpandedHeight, windowInsets: WindowInsets = TopAppBarDefaults.windowInsets, colors: TopAppBarColors = TopAppBarDefaults.topAppBarColors(), scrollBehavior: TopAppBarScrollBehavior? = null, contentPadding: PaddingValues = TopAppBarDefaults.ContentPadding)`
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn TopAppBarWithSubtitle(
    title: impl Fn() + 'static,
    subtitle: impl Fn() + 'static,
    #[default(Modifier)] modifier: Modifier,
    #[default(|| {})] navigation_icon: impl Fn() + 'static,
    #[default(|| {})] actions: impl Fn() + 'static,
    #[default(Alignment::Start)] title_horizontal_alignment: HorizontalAlignment,
    #[default(TopAppBarDefaults::TopAppBarExpandedHeight)] expanded_height: Dp,
    #[default(TopAppBarDefaults.window_insets())] window_insets: WindowInsets,
    #[default(TopAppBarDefaults.top_app_bar_colors())] colors: TopAppBarColors,
    #[default(None)] scroll_behavior: Option<Rc<dyn TopAppBarScrollBehavior>>,
    #[default(TopAppBarDefaults::ContentPadding())] content_padding: PaddingValues,
) {
    SingleRowTopAppBar(
        modifier,
        Rc::new(title),
        AppBarSmallTokens::TitleFont.value(),
        Some(Rc::new(subtitle)),
        AppBarSmallTokens::SubtitleFont.value(),
        title_horizontal_alignment,
        Rc::new(navigation_icon),
        Rc::new(actions),
        or_default(expanded_height, TopAppBarDefaults::TopAppBarExpandedHeight),
        content_padding,
        window_insets,
        colors,
        scroll_behavior,
    );
}

/// A center aligned top app bar (Compose's `CenterAlignedTopAppBar`): as [`TopAppBar`], its title in the middle of
/// the whole bar (moved aside where the navigation icon or the actions would cover it).
///
/// Kotlin: `CenterAlignedTopAppBar(title: @Composable () -> Unit, modifier: Modifier = Modifier, navigationIcon: @Composable () -> Unit = {}, actions: @Composable RowScope.() -> Unit = {}, expandedHeight: Dp = TopAppBarDefaults.TopAppBarExpandedHeight, windowInsets: WindowInsets = TopAppBarDefaults.windowInsets, colors: TopAppBarColors = TopAppBarDefaults.topAppBarColors(), scrollBehavior: TopAppBarScrollBehavior? = null, contentPadding: PaddingValues = TopAppBarDefaults.ContentPadding)`
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn CenterAlignedTopAppBar(
    title: impl Fn() + 'static,
    #[default(Modifier)] modifier: Modifier,
    #[default(|| {})] navigation_icon: impl Fn() + 'static,
    #[default(|| {})] actions: impl Fn() + 'static,
    #[default(TopAppBarDefaults::TopAppBarExpandedHeight)] expanded_height: Dp,
    #[default(TopAppBarDefaults.window_insets())] window_insets: WindowInsets,
    #[default(TopAppBarDefaults.top_app_bar_colors())] colors: TopAppBarColors,
    #[default(None)] scroll_behavior: Option<Rc<dyn TopAppBarScrollBehavior>>,
    #[default(TopAppBarDefaults::ContentPadding())] content_padding: PaddingValues,
) {
    SingleRowTopAppBar(
        modifier,
        Rc::new(title),
        AppBarSmallTokens::TitleFont.value(),
        None,
        TextStyle::Default,
        Alignment::CenterHorizontally,
        Rc::new(navigation_icon),
        Rc::new(actions),
        or_default(expanded_height, TopAppBarDefaults::TopAppBarExpandedHeight),
        content_padding,
        window_insets,
        colors,
        scroll_behavior,
    );
}

/// A medium top app bar (Compose's `MediumTopAppBar`): two rows — a small bar of `navigation_icon`, `title` (faded
/// in as it collapses) and `actions`, `collapsed_height` tall, over `title` again in the headline small style at the
/// start 24 dp above the bottom (faded out as it collapses) — `expanded_height` tall in all (unspecified or infinite
/// heights: [`TopAppBarDefaults::MediumAppBarCollapsedHeight`], 64 dp, and
/// [`TopAppBarDefaults::MediumAppBarExpandedHeight`], 112 dp). `scroll_behavior` collapses it to the small bar; its
/// container turns to the scrolled color as it collapses. Made by [`LocalTwoRowsTopAppBarOverride`]'s override.
///
/// Kotlin: `MediumTopAppBar(title: @Composable () -> Unit, modifier: Modifier = Modifier, navigationIcon: @Composable () -> Unit = {}, actions: @Composable RowScope.() -> Unit = {}, collapsedHeight: Dp = TopAppBarDefaults.MediumAppBarCollapsedHeight, expandedHeight: Dp = TopAppBarDefaults.MediumAppBarExpandedHeight, windowInsets: WindowInsets = TopAppBarDefaults.windowInsets, colors: TopAppBarColors = TopAppBarDefaults.topAppBarColors(), scrollBehavior: TopAppBarScrollBehavior? = null)`
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn MediumTopAppBar(
    title: impl Fn() + 'static,
    #[default(Modifier)] modifier: Modifier,
    #[default(|| {})] navigation_icon: impl Fn() + 'static,
    #[default(|| {})] actions: impl Fn() + 'static,
    #[default(TopAppBarDefaults::MediumAppBarCollapsedHeight)] collapsed_height: Dp,
    #[default(TopAppBarDefaults::MediumAppBarExpandedHeight)] expanded_height: Dp,
    #[default(TopAppBarDefaults.window_insets())] window_insets: WindowInsets,
    #[default(TopAppBarDefaults.top_app_bar_colors())] colors: TopAppBarColors,
    #[default(None)] scroll_behavior: Option<Rc<dyn TopAppBarScrollBehavior>>,
) {
    let title: Rc<dyn Fn()> = Rc::new(title);
    TwoRowsTopAppBarImpl(
        modifier,
        title.clone(),
        AppBarMediumTokens::TitleFont.value(),
        MediumTitleBottomPadding,
        title,
        AppBarSmallTokens::TitleFont.value(),
        None,
        TextStyle::Default,
        None,
        TextStyle::Default,
        Alignment::Start,
        Rc::new(navigation_icon),
        Rc::new(actions),
        or_default(collapsed_height, TopAppBarDefaults::MediumAppBarCollapsedHeight),
        or_default(expanded_height, TopAppBarDefaults::MediumAppBarExpandedHeight),
        window_insets,
        colors,
        scroll_behavior,
    );
}

/// A medium flexible top app bar (Compose's experimental Expressive `MediumFlexibleTopAppBar`): as
/// [`MediumTopAppBar`], its large title in the headline medium style with `subtitle` under it (label large; the
/// small bar's in label medium), both lined up by `title_horizontal_alignment`. `expanded_height` None: Compose's
/// default, [`TopAppBarDefaults::MediumFlexibleAppBarWithSubtitleExpandedHeight`] (136 dp) with a subtitle, else
/// [`TopAppBarDefaults::MediumFlexibleAppBarWithoutSubtitleExpandedHeight`] (112 dp), which an unspecified or
/// infinite height falls back to too.
///
/// Kotlin: `MediumFlexibleTopAppBar(title: @Composable () -> Unit, modifier: Modifier = Modifier, subtitle: (@Composable () -> Unit)? = null, navigationIcon: @Composable () -> Unit = {}, actions: @Composable RowScope.() -> Unit = {}, titleHorizontalAlignment: Alignment.Horizontal = Alignment.Start, collapsedHeight: Dp = TopAppBarDefaults.MediumAppBarCollapsedHeight, expandedHeight: Dp = if (subtitle != null) TopAppBarDefaults.MediumFlexibleAppBarWithSubtitleExpandedHeight else TopAppBarDefaults.MediumFlexibleAppBarWithoutSubtitleExpandedHeight, windowInsets: WindowInsets = TopAppBarDefaults.windowInsets, colors: TopAppBarColors = TopAppBarDefaults.topAppBarColors(), scrollBehavior: TopAppBarScrollBehavior? = null)`
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn MediumFlexibleTopAppBar(
    title: impl Fn() + 'static,
    #[default(Modifier)] modifier: Modifier,
    #[default(None)] subtitle: Option<Rc<dyn Fn()>>,
    #[default(|| {})] navigation_icon: impl Fn() + 'static,
    #[default(|| {})] actions: impl Fn() + 'static,
    #[default(Alignment::Start)] title_horizontal_alignment: HorizontalAlignment,
    #[default(TopAppBarDefaults::MediumAppBarCollapsedHeight)] collapsed_height: Dp,
    #[default(None)] expanded_height: Option<Dp>,
    #[default(TopAppBarDefaults.window_insets())] window_insets: WindowInsets,
    #[default(TopAppBarDefaults.top_app_bar_colors())] colors: TopAppBarColors,
    #[default(None)] scroll_behavior: Option<Rc<dyn TopAppBarScrollBehavior>>,
) {
    let flexible_default = if subtitle.is_some() {
        TopAppBarDefaults::MediumFlexibleAppBarWithSubtitleExpandedHeight
    } else {
        TopAppBarDefaults::MediumFlexibleAppBarWithoutSubtitleExpandedHeight
    };
    let expanded_height = expanded_height.unwrap_or(flexible_default);
    let title: Rc<dyn Fn()> = Rc::new(title);
    let subtitle = subtitle.unwrap_or_else(|| Rc::new(|| {}));
    TwoRowsTopAppBarImpl(
        modifier,
        title.clone(),
        AppBarMediumFlexibleTokens::TitleFont.value(),
        MediumTitleBottomPadding,
        title,
        AppBarSmallTokens::TitleFont.value(),
        Some(subtitle.clone()),
        AppBarMediumFlexibleTokens::SubtitleFont.value(),
        Some(subtitle),
        AppBarSmallTokens::SubtitleFont.value(),
        title_horizontal_alignment,
        Rc::new(navigation_icon),
        Rc::new(actions),
        or_default(collapsed_height, TopAppBarDefaults::MediumAppBarCollapsedHeight),
        or_default(expanded_height, flexible_default),
        window_insets,
        colors,
        scroll_behavior,
    );
}

/// A large top app bar (Compose's `LargeTopAppBar`): as [`MediumTopAppBar`], its large title in the headline medium
/// style 28 dp above the bottom, [`TopAppBarDefaults::LargeAppBarExpandedHeight`] (152 dp) tall expanded.
///
/// Kotlin: `LargeTopAppBar(title: @Composable () -> Unit, modifier: Modifier = Modifier, navigationIcon: @Composable () -> Unit = {}, actions: @Composable RowScope.() -> Unit = {}, collapsedHeight: Dp = TopAppBarDefaults.LargeAppBarCollapsedHeight, expandedHeight: Dp = TopAppBarDefaults.LargeAppBarExpandedHeight, windowInsets: WindowInsets = TopAppBarDefaults.windowInsets, colors: TopAppBarColors = TopAppBarDefaults.topAppBarColors(), scrollBehavior: TopAppBarScrollBehavior? = null)`
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn LargeTopAppBar(
    title: impl Fn() + 'static,
    #[default(Modifier)] modifier: Modifier,
    #[default(|| {})] navigation_icon: impl Fn() + 'static,
    #[default(|| {})] actions: impl Fn() + 'static,
    #[default(TopAppBarDefaults::LargeAppBarCollapsedHeight)] collapsed_height: Dp,
    #[default(TopAppBarDefaults::LargeAppBarExpandedHeight)] expanded_height: Dp,
    #[default(TopAppBarDefaults.window_insets())] window_insets: WindowInsets,
    #[default(TopAppBarDefaults.top_app_bar_colors())] colors: TopAppBarColors,
    #[default(None)] scroll_behavior: Option<Rc<dyn TopAppBarScrollBehavior>>,
) {
    let title: Rc<dyn Fn()> = Rc::new(title);
    TwoRowsTopAppBarImpl(
        modifier,
        title.clone(),
        AppBarLargeTokens::TitleFont.value(),
        LargeTitleBottomPadding,
        title,
        AppBarSmallTokens::TitleFont.value(),
        None,
        TextStyle::Default,
        None,
        TextStyle::Default,
        Alignment::Start,
        Rc::new(navigation_icon),
        Rc::new(actions),
        or_default(collapsed_height, TopAppBarDefaults::LargeAppBarCollapsedHeight),
        or_default(expanded_height, TopAppBarDefaults::LargeAppBarExpandedHeight),
        window_insets,
        colors,
        scroll_behavior,
    );
}

/// A large flexible top app bar (Compose's experimental Expressive `LargeFlexibleTopAppBar`): as
/// [`LargeTopAppBar`], its large title in the display small style with `subtitle` under it (title medium; the small
/// bar's in label medium), both lined up by `title_horizontal_alignment`. `expanded_height` None: Compose's
/// default, [`TopAppBarDefaults::LargeFlexibleAppBarWithSubtitleExpandedHeight`] (152 dp) with a subtitle, else
/// [`TopAppBarDefaults::LargeFlexibleAppBarWithoutSubtitleExpandedHeight`] (120 dp), which an unspecified or infinite
/// height falls back to too.
///
/// Kotlin: `LargeFlexibleTopAppBar(title: @Composable () -> Unit, modifier: Modifier = Modifier, subtitle: (@Composable () -> Unit)? = null, navigationIcon: @Composable () -> Unit = {}, actions: @Composable RowScope.() -> Unit = {}, titleHorizontalAlignment: Alignment.Horizontal = Alignment.Start, collapsedHeight: Dp = TopAppBarDefaults.LargeAppBarCollapsedHeight, expandedHeight: Dp = if (subtitle != null) TopAppBarDefaults.LargeFlexibleAppBarWithSubtitleExpandedHeight else TopAppBarDefaults.LargeFlexibleAppBarWithoutSubtitleExpandedHeight, windowInsets: WindowInsets = TopAppBarDefaults.windowInsets, colors: TopAppBarColors = TopAppBarDefaults.topAppBarColors(), scrollBehavior: TopAppBarScrollBehavior? = null)`
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn LargeFlexibleTopAppBar(
    title: impl Fn() + 'static,
    #[default(Modifier)] modifier: Modifier,
    #[default(None)] subtitle: Option<Rc<dyn Fn()>>,
    #[default(|| {})] navigation_icon: impl Fn() + 'static,
    #[default(|| {})] actions: impl Fn() + 'static,
    #[default(Alignment::Start)] title_horizontal_alignment: HorizontalAlignment,
    #[default(TopAppBarDefaults::LargeAppBarCollapsedHeight)] collapsed_height: Dp,
    #[default(None)] expanded_height: Option<Dp>,
    #[default(TopAppBarDefaults.window_insets())] window_insets: WindowInsets,
    #[default(TopAppBarDefaults.top_app_bar_colors())] colors: TopAppBarColors,
    #[default(None)] scroll_behavior: Option<Rc<dyn TopAppBarScrollBehavior>>,
) {
    let flexible_default = if subtitle.is_some() {
        TopAppBarDefaults::LargeFlexibleAppBarWithSubtitleExpandedHeight
    } else {
        TopAppBarDefaults::LargeFlexibleAppBarWithoutSubtitleExpandedHeight
    };
    let expanded_height = expanded_height.unwrap_or(flexible_default);
    let title: Rc<dyn Fn()> = Rc::new(title);
    let subtitle = subtitle.unwrap_or_else(|| Rc::new(|| {}));
    TwoRowsTopAppBarImpl(
        modifier,
        title.clone(),
        AppBarLargeFlexibleTokens::TitleFont.value(),
        LargeTitleBottomPadding,
        title,
        AppBarSmallTokens::TitleFont.value(),
        Some(subtitle.clone()),
        AppBarLargeFlexibleTokens::SubtitleFont.value(),
        Some(subtitle),
        AppBarSmallTokens::SubtitleFont.value(),
        title_horizontal_alignment,
        Rc::new(navigation_icon),
        Rc::new(actions),
        or_default(collapsed_height, TopAppBarDefaults::LargeAppBarCollapsedHeight),
        or_default(expanded_height, flexible_default),
        window_insets,
        colors,
        scroll_behavior,
    );
}

/// A two-rows top app bar of the app's own titles (Compose's experimental Expressive `TwoRowsTopAppBar(title: …
/// (expanded: Boolean) …)`): as [`MediumFlexibleTopAppBar`], `title` and `subtitle` asked for each row (`true` the
/// large, expanded one; `false` the small bar's), the large row's title right at its bottom; unspecified heights
/// the medium bar's (64 dp collapsed; 136 dp expanded with a subtitle, else 112 dp).
///
/// Kotlin: `TwoRowsTopAppBar(title: @Composable (expanded: Boolean) -> Unit, modifier: Modifier = Modifier, subtitle: (@Composable (expanded: Boolean) -> Unit)? = null, navigationIcon: @Composable () -> Unit = {}, actions: @Composable RowScope.() -> Unit = {}, titleHorizontalAlignment: Alignment.Horizontal = Alignment.Start, collapsedHeight: Dp = Dp.Unspecified, expandedHeight: Dp = Dp.Unspecified, windowInsets: WindowInsets = TopAppBarDefaults.windowInsets, colors: TopAppBarColors = TopAppBarDefaults.topAppBarColors(), scrollBehavior: TopAppBarScrollBehavior? = null)`
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn TwoRowsTopAppBar(
    title: impl Fn(bool) + 'static,
    #[default(Modifier)] modifier: Modifier,
    #[default(None)] subtitle: Option<Rc<dyn Fn(bool)>>,
    #[default(|| {})] navigation_icon: impl Fn() + 'static,
    #[default(|| {})] actions: impl Fn() + 'static,
    #[default(Alignment::Start)] title_horizontal_alignment: HorizontalAlignment,
    #[default(Dp::Unspecified)] collapsed_height: Dp,
    #[default(Dp::Unspecified)] expanded_height: Dp,
    #[default(TopAppBarDefaults.window_insets())] window_insets: WindowInsets,
    #[default(TopAppBarDefaults.top_app_bar_colors())] colors: TopAppBarColors,
    #[default(None)] scroll_behavior: Option<Rc<dyn TopAppBarScrollBehavior>>,
) {
    let title = Rc::new(title);
    let (expanded_title, small_title) = (title.clone(), title);
    let has_subtitle = subtitle.is_some();
    let (expanded_subtitle, small_subtitle) = (subtitle.clone(), subtitle);
    TwoRowsTopAppBarImpl(
        modifier,
        Rc::new(move || expanded_title(true)),
        AppBarMediumFlexibleTokens::TitleFont.value(),
        Dp(0.0),
        Rc::new(move || small_title(false)),
        AppBarSmallTokens::TitleFont.value(),
        Some(Rc::new(move || {
            if let Some(subtitle) = &expanded_subtitle {
                subtitle(true)
            }
        })),
        AppBarMediumFlexibleTokens::SubtitleFont.value(),
        Some(Rc::new(move || {
            if let Some(subtitle) = &small_subtitle {
                subtitle(false)
            }
        })),
        AppBarSmallTokens::SubtitleFont.value(),
        title_horizontal_alignment,
        Rc::new(navigation_icon),
        Rc::new(actions),
        // Default to the medium app-bar collapsed height
        or_default(collapsed_height, TopAppBarDefaults::MediumAppBarCollapsedHeight),
        // Default to the medium app-bar expanded height
        or_default(
            expanded_height,
            if has_subtitle {
                TopAppBarDefaults::MediumFlexibleAppBarWithSubtitleExpandedHeight
            } else {
                TopAppBarDefaults::MediumFlexibleAppBarWithoutSubtitleExpandedHeight
            },
        ),
        window_insets,
        colors,
        scroll_behavior,
    );
}

/// A height given, or `default` for an unspecified or infinite one (what each bar does with its heights; Kotlin's
/// `==` of `Dp.Unspecified`, a NaN, is true for a NaN: `is_specified` here).
fn or_default(height: Dp, default: Dp) -> Dp {
    if !height.is_specified() || height == Dp::Infinity { default } else { height }
}

// ---- the single-row bar and its override ---------------------------------------------------------------------------

/// A single-row bar of these arguments (Compose's private `SingleRowTopAppBar`): made by
/// [`LocalSingleRowTopAppBarOverride`]'s override.
#[kompozable]
#[allow(clippy::too_many_arguments)]
fn SingleRowTopAppBar(
    modifier: Modifier,
    title: Rc<dyn Fn()>,
    title_text_style: TextStyle,
    subtitle: Option<Rc<dyn Fn()>>,
    subtitle_text_style: TextStyle,
    title_horizontal_alignment: HorizontalAlignment,
    navigation_icon: Rc<dyn Fn()>,
    actions: Rc<dyn Fn()>,
    expanded_height: Dp,
    content_padding: PaddingValues,
    window_insets: WindowInsets,
    colors: TopAppBarColors,
    scroll_behavior: Option<Rc<dyn TopAppBarScrollBehavior>>,
) {
    let scope = SingleRowTopAppBarOverrideScope {
        modifier,
        title,
        title_text_style,
        subtitle,
        subtitle_text_style,
        title_horizontal_alignment,
        navigation_icon,
        actions,
        expanded_height,
        content_padding,
        window_insets,
        colors,
        scroll_behavior,
    };
    LocalSingleRowTopAppBarOverride.current().SingleRowTopAppBar(&scope);
}

/// The single-row top app bar of an override (Compose's experimental `SingleRowTopAppBarOverride`): what makes every
/// [`TopAppBar`], [`TopAppBarWithSubtitle`] and [`CenterAlignedTopAppBar`] below a
/// [`LocalSingleRowTopAppBarOverride`] providing it, from the scope's arguments.
pub trait SingleRowTopAppBarOverride {
    /// Composes the bar of `scope`'s arguments (Compose's `SingleRowTopAppBarOverrideScope.SingleRowTopAppBar()`).
    fn SingleRowTopAppBar(&self, scope: &SingleRowTopAppBarOverrideScope);
}

impl PartialEq for dyn SingleRowTopAppBarOverride {
    /// The same override (Kotlin's `==` of objects: their identity).
    fn eq(&self, other: &Self) -> bool {
        std::ptr::addr_eq(self, other)
    }
}

/// The default way a single-row top app bar is made (Compose's experimental `DefaultSingleRowTopAppBarOverride`):
/// the container color animated to the scrolled one once content is under the bar (more than 1% of it), the bar
/// taking vertical drags unless pinned, its layout clipped inside the insets, collapsing by the scroll behavior's
/// height offset.
#[derive(Clone, Copy, Debug)]
pub struct DefaultSingleRowTopAppBarOverride;

impl SingleRowTopAppBarOverride for DefaultSingleRowTopAppBarOverride {
    #[track_caller]
    fn SingleRowTopAppBar(&self, scope: &SingleRowTopAppBarOverrideScope) {
        let SingleRowTopAppBarOverrideScope {
            modifier,
            title,
            title_text_style,
            subtitle,
            subtitle_text_style,
            title_horizontal_alignment,
            navigation_icon,
            actions,
            expanded_height,
            content_padding,
            window_insets,
            colors,
            scroll_behavior,
        } = scope.clone();
        kompozable_block!({
            assert!(
                expanded_height.is_specified() && expanded_height.value().is_finite(),
                "The expandedHeight is expected to be specified and finite"
            );

            // Obtain the container color from the TopAppBarColors using the `overlapFraction`. This ensures that the
            // colors will adjust whether the app bar behavior is pinned or scrolled. This may potentially animate or
            // interpolate a transition between the container-color and the container's scrolled-color according to
            // the app bar's scroll state.
            let target_color = {
                let behavior = scroll_behavior.clone();
                remember_keyed((colors, scroll_behavior.clone()), move || {
                    derived_state_of(move || {
                        let overlapping_fraction = behavior.as_ref().map_or(0.0, |b| b.state().overlapped_fraction());
                        colors.container_color_at(if overlapping_fraction > 0.01 { 1.0 } else { 0.0 })
                    })
                })
            };
            let app_bar_container_color = animate_color_as_state(
                target_color.value(),
                animation_spec = MotionSchemeKeyTokens::DefaultEffects.value::<Color>(),
            );

            // Wrap the given actions in a Row.
            let actions_row: Rc<dyn Fn()> = Rc::new(move || {
                let actions = actions.clone();
                Row(
                    horizontal_arrangement = Arrangement::End,
                    vertical_alignment = Alignment::CenterVertically,
                    content = move || actions(),
                );
            });

            // Set up support for resizing the top app bar when vertically dragging the bar itself.
            let app_bar_drag_modifier = app_bar_drag_modifier(&scroll_behavior);

            // Compose a Surface with a TopAppBarLayout content. The surface's background color is animated as
            // specified above. The height of the app bar is determined by subtracting the bar's height offset from
            // the app bar's defined constant height value (i.e. the ContainerHeight token).
            let behavior = scroll_behavior.clone();
            Box(
                modifier = modifier
                    .then(app_bar_drag_modifier)
                    .draw_behind(move |d| {
                        let color = app_bar_container_color.value();
                        if color != Color::Unspecified {
                            d.draw_rect(color);
                        }
                    })
                    .semantics(|c| c.set_is_traversal_group(true))
                    .pointer_input((), |_| async {}),
                content = move || {
                    let behavior = behavior.clone();
                    TopAppBarLayout(
                        Modifier
                            .window_insets_padding(window_insets.clone())
                            // clip after padding so we don't show the title over the inset area
                            .clip_to_bounds()
                            .then(adjust_height_offset_limit(&behavior)),
                        move || behavior.as_ref().map_or(0.0, |b| b.state().height_offset()),
                        colors.navigation_icon_content_color,
                        colors.title_content_color,
                        colors.subtitle_content_color,
                        colors.action_icon_content_color,
                        title.clone(),
                        title_text_style.clone(),
                        subtitle.clone(),
                        subtitle_text_style.clone(),
                        Rc::new(|| 1.0),
                        VerticalArrangement::Center,
                        title_horizontal_alignment.clone(),
                        0,
                        false,
                        navigation_icon.clone(),
                        actions_row.clone(),
                        expanded_height,
                        content_padding.clone(),
                    );
                },
            );
        })
    }
}

/// The arguments of a single-row top app bar, given to its override (Compose's experimental
/// `SingleRowTopAppBarOverrideScope`). Made by the bars.
#[derive(Clone)]
pub struct SingleRowTopAppBarOverrideScope {
    modifier: Modifier,
    title: Rc<dyn Fn()>,
    title_text_style: TextStyle,
    subtitle: Option<Rc<dyn Fn()>>,
    subtitle_text_style: TextStyle,
    title_horizontal_alignment: HorizontalAlignment,
    navigation_icon: Rc<dyn Fn()>,
    actions: Rc<dyn Fn()>,
    expanded_height: Dp,
    content_padding: PaddingValues,
    window_insets: WindowInsets,
    colors: TopAppBarColors,
    scroll_behavior: Option<Rc<dyn TopAppBarScrollBehavior>>,
}

impl SingleRowTopAppBarOverrideScope {
    /// The bar's modifier (Compose's `modifier`).
    pub fn modifier(&self) -> Modifier {
        self.modifier.clone()
    }

    /// The title (Compose's `title`).
    pub fn title(&self) -> Rc<dyn Fn()> {
        self.title.clone()
    }

    /// The title's style (Compose's `titleTextStyle`).
    pub fn title_text_style(&self) -> TextStyle {
        self.title_text_style.clone()
    }

    /// The subtitle, if any (Compose's `subtitle`).
    pub fn subtitle(&self) -> Option<Rc<dyn Fn()>> {
        self.subtitle.clone()
    }

    /// The subtitle's style (Compose's `subtitleTextStyle`).
    pub fn subtitle_text_style(&self) -> TextStyle {
        self.subtitle_text_style.clone()
    }

    /// How the title and subtitle line up (Compose's `titleHorizontalAlignment`).
    pub fn title_horizontal_alignment(&self) -> HorizontalAlignment {
        self.title_horizontal_alignment.clone()
    }

    /// The navigation icon (Compose's `navigationIcon`).
    pub fn navigation_icon(&self) -> Rc<dyn Fn()> {
        self.navigation_icon.clone()
    }

    /// The actions, a row's content (Compose's `actions`).
    pub fn actions(&self) -> Rc<dyn Fn()> {
        self.actions.clone()
    }

    /// The bar's height (Compose's `expandedHeight`).
    pub fn expanded_height(&self) -> Dp {
        self.expanded_height
    }

    /// The padding around the bar's content (Compose's `contentPadding`).
    pub fn content_padding(&self) -> PaddingValues {
        self.content_padding.clone()
    }

    /// The insets the bar keeps clear of (Compose's `windowInsets`).
    pub fn window_insets(&self) -> WindowInsets {
        self.window_insets.clone()
    }

    /// The bar's colors (Compose's `colors`).
    pub fn colors(&self) -> TopAppBarColors {
        self.colors
    }

    /// The bar's scroll behavior, if any (Compose's `scrollBehavior`).
    pub fn scroll_behavior(&self) -> Option<Rc<dyn TopAppBarScrollBehavior>> {
        self.scroll_behavior.clone()
    }
}

/// The override that makes the single-row top app bars below (Compose's experimental
/// `LocalSingleRowTopAppBarOverride`): [`DefaultSingleRowTopAppBarOverride`] unless provided.
pub static LocalSingleRowTopAppBarOverride: KompozitionLocal<Rc<dyn SingleRowTopAppBarOverride>> =
    KompozitionLocal::new(|| Rc::new(DefaultSingleRowTopAppBarOverride));

// ---- the two-rows bar and its override -----------------------------------------------------------------------------

/// A two-rows bar of these arguments (Compose's private `TwoRowsTopAppBar`, an overload of the public one's name):
/// made by [`LocalTwoRowsTopAppBarOverride`]'s override.
#[kompozable]
#[allow(clippy::too_many_arguments)]
fn TwoRowsTopAppBarImpl(
    modifier: Modifier,
    title: Rc<dyn Fn()>,
    title_text_style: TextStyle,
    title_bottom_padding: Dp,
    small_title: Rc<dyn Fn()>,
    small_title_text_style: TextStyle,
    subtitle: Option<Rc<dyn Fn()>>,
    subtitle_text_style: TextStyle,
    small_subtitle: Option<Rc<dyn Fn()>>,
    small_subtitle_text_style: TextStyle,
    title_horizontal_alignment: HorizontalAlignment,
    navigation_icon: Rc<dyn Fn()>,
    actions: Rc<dyn Fn()>,
    collapsed_height: Dp,
    expanded_height: Dp,
    window_insets: WindowInsets,
    colors: TopAppBarColors,
    scroll_behavior: Option<Rc<dyn TopAppBarScrollBehavior>>,
) {
    let scope = TwoRowsTopAppBarOverrideScope {
        modifier,
        title,
        title_text_style,
        title_bottom_padding,
        small_title,
        small_title_text_style,
        subtitle,
        subtitle_text_style,
        small_subtitle,
        small_subtitle_text_style,
        title_horizontal_alignment,
        navigation_icon,
        actions,
        collapsed_height,
        expanded_height,
        window_insets,
        colors,
        scroll_behavior,
    };
    LocalTwoRowsTopAppBarOverride.current().TwoRowsTopAppBar(&scope);
}

/// The two-rows top app bar of an override (Compose's experimental `TwoRowsTopAppBarOverride`): what makes every
/// [`MediumTopAppBar`], [`MediumFlexibleTopAppBar`], [`LargeTopAppBar`], [`LargeFlexibleTopAppBar`] and
/// [`TwoRowsTopAppBar`] below a [`LocalTwoRowsTopAppBarOverride`] providing it, from the scope's arguments.
pub trait TwoRowsTopAppBarOverride {
    /// Composes the bar of `scope`'s arguments (Compose's `TwoRowsTopAppBarOverrideScope.TwoRowsTopAppBar()`).
    fn TwoRowsTopAppBar(&self, scope: &TwoRowsTopAppBarOverrideScope);
}

impl PartialEq for dyn TwoRowsTopAppBarOverride {
    /// The same override (Kotlin's `==` of objects: their identity).
    fn eq(&self, other: &Self) -> bool {
        std::ptr::addr_eq(self, other)
    }
}

/// The default way a two-rows top app bar is made (Compose's experimental `DefaultTwoRowsTopAppBarOverride`): a
/// column of the small bar (its title faded in as the bar collapses, hidden from accessibility below half way) and
/// the large title's row (faded out, shrinking by the scroll behavior's height offset), the container color
/// following how far collapsed it is, the bar taking vertical drags unless pinned.
#[derive(Clone, Copy, Debug)]
pub struct DefaultTwoRowsTopAppBarOverride;

impl TwoRowsTopAppBarOverride for DefaultTwoRowsTopAppBarOverride {
    #[track_caller]
    fn TwoRowsTopAppBar(&self, scope: &TwoRowsTopAppBarOverrideScope) {
        let TwoRowsTopAppBarOverrideScope {
            modifier,
            title,
            title_text_style,
            title_bottom_padding,
            small_title,
            small_title_text_style,
            subtitle,
            subtitle_text_style,
            small_subtitle,
            small_subtitle_text_style,
            title_horizontal_alignment,
            navigation_icon,
            actions,
            collapsed_height,
            expanded_height,
            window_insets,
            colors,
            scroll_behavior,
        } = scope.clone();
        kompozable_block!({
            assert!(
                collapsed_height.is_specified() && collapsed_height.value().is_finite(),
                "The collapsedHeight is expected to be specified and finite"
            );
            assert!(
                expanded_height.is_specified() && expanded_height.value().is_finite(),
                "The expandedHeight is expected to be specified and finite"
            );
            assert!(
                expanded_height >= collapsed_height,
                "The expandedHeight is expected to be greater or equal to the collapsedHeight"
            );
            let title_bottom_padding_px = LocalDensity.current().round_to_px(title_bottom_padding);

            // Obtain the container Color from the TopAppBarColors using the `collapsedFraction`, as the bottom part of
            // this TwoRowsTopAppBar changes color at the same rate the app bar expands or collapse.
            // This will potentially animate or interpolate a transition between the container color and the
            // container's scrolled color according to the app bar's scroll state.
            let color_transition_fraction: Rc<dyn Fn() -> f32> = {
                let behavior = scroll_behavior.clone();
                Rc::new(move || behavior.as_ref().map_or(0.0, |b| b.state().collapsed_fraction()))
            };
            let app_bar_container_color = {
                let fraction = color_transition_fraction.clone();
                move || colors.container_color_at(fraction())
            };

            // Wrap the given actions in a Row.
            let actions_row: Rc<dyn Fn()> = Rc::new(move || {
                let actions = actions.clone();
                Row(
                    horizontal_arrangement = Arrangement::End,
                    vertical_alignment = Alignment::CenterVertically,
                    content = move || actions(),
                );
            });
            let top_title_alpha: Rc<dyn Fn() -> f32> = {
                let fraction = color_transition_fraction.clone();
                Rc::new(move || TopTitleAlphaEasing.transform(fraction()))
            };
            let bottom_title_alpha: Rc<dyn Fn() -> f32> = {
                let fraction = color_transition_fraction.clone();
                Rc::new(move || 1.0 - fraction())
            };
            // Hide the top row title semantics when its alpha value goes below 0.5 threshold.
            // Hide the bottom row title semantics when the top title semantics are active.
            // (Compose keys this by the fraction's lambda, memoized by the scroll behavior it captures)
            let hide_top_row_semantics = {
                let fraction = color_transition_fraction.clone();
                remember_keyed(scroll_behavior.clone(), move || derived_state_of(move || fraction() < 0.5)).value()
            };
            let hide_bottom_row_semantics = !hide_top_row_semantics;

            // Set up support for resizing the top app bar when vertically dragging the bar itself.
            let app_bar_drag_modifier = app_bar_drag_modifier(&scroll_behavior);

            let behavior = scroll_behavior.clone();
            Box(
                modifier = modifier
                    .then(app_bar_drag_modifier)
                    .draw_behind(move |d| d.draw_rect(app_bar_container_color()))
                    .semantics(|c| c.set_is_traversal_group(true))
                    .pointer_input((), |_| async {}),
                content = move || {
                    let (title, title_text_style) = (title.clone(), title_text_style.clone());
                    let (small_title, small_title_text_style) = (small_title.clone(), small_title_text_style.clone());
                    let (subtitle, subtitle_text_style) = (subtitle.clone(), subtitle_text_style.clone());
                    let (small_subtitle, small_subtitle_text_style) =
                        (small_subtitle.clone(), small_subtitle_text_style.clone());
                    let (navigation_icon, actions_row) = (navigation_icon.clone(), actions_row.clone());
                    let (top_title_alpha, bottom_title_alpha) = (top_title_alpha.clone(), bottom_title_alpha.clone());
                    let title_horizontal_alignment = title_horizontal_alignment.clone();
                    let window_insets = window_insets.clone();
                    let behavior = behavior.clone();
                    Column(
                        content = move || {
                            TopAppBarLayout(
                                Modifier
                                    .window_insets_padding(window_insets.clone())
                                    // clip after padding so we don't show the title over the inset area
                                    .clip_to_bounds(),
                                || 0.0,
                                colors.navigation_icon_content_color,
                                colors.title_content_color,
                                colors.subtitle_content_color,
                                colors.action_icon_content_color,
                                small_title.clone(),
                                small_title_text_style.clone(),
                                small_subtitle.clone(),
                                small_subtitle_text_style.clone(),
                                top_title_alpha.clone(),
                                VerticalArrangement::Center,
                                title_horizontal_alignment.clone(),
                                0,
                                hide_top_row_semantics,
                                navigation_icon.clone(),
                                actions_row.clone(),
                                collapsed_height,
                                TopAppBarDefaults::ContentPadding(),
                            );
                            let offset_behavior = behavior.clone();
                            TopAppBarLayout(
                                Modifier
                                    // only apply the horizontal sides of the window insets padding, since the top
                                    // padding will always be applied by the layout above
                                    .window_insets_padding(window_insets.only(WindowInsetsSides::Horizontal))
                                    .clip_to_bounds()
                                    .then(adjust_height_offset_limit(&behavior)),
                                move || offset_behavior.as_ref().map_or(0.0, |b| b.state().height_offset()),
                                colors.navigation_icon_content_color,
                                colors.title_content_color,
                                colors.subtitle_content_color,
                                colors.action_icon_content_color,
                                title.clone(),
                                title_text_style.clone(),
                                subtitle.clone(),
                                subtitle_text_style.clone(),
                                bottom_title_alpha.clone(),
                                VerticalArrangement::Bottom,
                                title_horizontal_alignment.clone(),
                                title_bottom_padding_px,
                                hide_bottom_row_semantics,
                                Rc::new(|| {}),
                                Rc::new(|| {}),
                                expanded_height - collapsed_height,
                                TopAppBarDefaults::ContentPadding(),
                            );
                        },
                    );
                },
            );
        })
    }
}

/// The arguments of a two-rows top app bar, given to its override (Compose's experimental
/// `TwoRowsTopAppBarOverrideScope`). Made by the bars.
#[derive(Clone)]
pub struct TwoRowsTopAppBarOverrideScope {
    modifier: Modifier,
    title: Rc<dyn Fn()>,
    title_text_style: TextStyle,
    title_bottom_padding: Dp,
    small_title: Rc<dyn Fn()>,
    small_title_text_style: TextStyle,
    subtitle: Option<Rc<dyn Fn()>>,
    subtitle_text_style: TextStyle,
    small_subtitle: Option<Rc<dyn Fn()>>,
    small_subtitle_text_style: TextStyle,
    title_horizontal_alignment: HorizontalAlignment,
    navigation_icon: Rc<dyn Fn()>,
    actions: Rc<dyn Fn()>,
    collapsed_height: Dp,
    expanded_height: Dp,
    window_insets: WindowInsets,
    colors: TopAppBarColors,
    scroll_behavior: Option<Rc<dyn TopAppBarScrollBehavior>>,
}

impl TwoRowsTopAppBarOverrideScope {
    /// The bar's modifier (Compose's `modifier`).
    pub fn modifier(&self) -> Modifier {
        self.modifier.clone()
    }

    /// The large row's title (Compose's `title`).
    pub fn title(&self) -> Rc<dyn Fn()> {
        self.title.clone()
    }

    /// The large title's style (Compose's `titleTextStyle`).
    pub fn title_text_style(&self) -> TextStyle {
        self.title_text_style.clone()
    }

    /// The space under the large title's baseline (Compose's `titleBottomPadding`).
    pub fn title_bottom_padding(&self) -> Dp {
        self.title_bottom_padding
    }

    /// The small bar's title (Compose's `smallTitle`).
    pub fn small_title(&self) -> Rc<dyn Fn()> {
        self.small_title.clone()
    }

    /// The small title's style (Compose's `smallTitleTextStyle`).
    pub fn small_title_text_style(&self) -> TextStyle {
        self.small_title_text_style.clone()
    }

    /// The large row's subtitle, if any (Compose's `subtitle`).
    pub fn subtitle(&self) -> Option<Rc<dyn Fn()>> {
        self.subtitle.clone()
    }

    /// The large subtitle's style (Compose's `subtitleTextStyle`).
    pub fn subtitle_text_style(&self) -> TextStyle {
        self.subtitle_text_style.clone()
    }

    /// The small bar's subtitle, if any (Compose's `smallSubtitle`).
    pub fn small_subtitle(&self) -> Option<Rc<dyn Fn()>> {
        self.small_subtitle.clone()
    }

    /// The small subtitle's style (Compose's `smallSubtitleTextStyle`).
    pub fn small_subtitle_text_style(&self) -> TextStyle {
        self.small_subtitle_text_style.clone()
    }

    /// How the titles and subtitles line up (Compose's `titleHorizontalAlignment`).
    pub fn title_horizontal_alignment(&self) -> HorizontalAlignment {
        self.title_horizontal_alignment.clone()
    }

    /// The navigation icon (Compose's `navigationIcon`).
    pub fn navigation_icon(&self) -> Rc<dyn Fn()> {
        self.navigation_icon.clone()
    }

    /// The actions, a row's content (Compose's `actions`).
    pub fn actions(&self) -> Rc<dyn Fn()> {
        self.actions.clone()
    }

    /// The bar's height collapsed (Compose's `collapsedHeight`).
    pub fn collapsed_height(&self) -> Dp {
        self.collapsed_height
    }

    /// The bar's height expanded (Compose's `expandedHeight`).
    pub fn expanded_height(&self) -> Dp {
        self.expanded_height
    }

    /// The insets the bar keeps clear of (Compose's `windowInsets`).
    pub fn window_insets(&self) -> WindowInsets {
        self.window_insets.clone()
    }

    /// The bar's colors (Compose's `colors`).
    pub fn colors(&self) -> TopAppBarColors {
        self.colors
    }

    /// The bar's scroll behavior, if any (Compose's `scrollBehavior`).
    pub fn scroll_behavior(&self) -> Option<Rc<dyn TopAppBarScrollBehavior>> {
        self.scroll_behavior.clone()
    }
}

/// The override that makes the two-rows top app bars below (Compose's experimental `LocalTwoRowsTopAppBarOverride`):
/// [`DefaultTwoRowsTopAppBarOverride`] unless provided.
pub static LocalTwoRowsTopAppBarOverride: KompozitionLocal<Rc<dyn TwoRowsTopAppBarOverride>> =
    KompozitionLocal::new(|| Rc::new(DefaultTwoRowsTopAppBarOverride));

// ---- what the bars share -------------------------------------------------------------------------------------------

/// The bar's vertical drags, unless it is pinned (or has no behavior): each delta moves the height offset, and the
/// bar settles as it is let go (Compose's `appBarDragModifier` of the default overrides).
#[track_caller]
fn app_bar_drag_modifier(scroll_behavior: &Option<Rc<dyn TopAppBarScrollBehavior>>) -> Modifier {
    match scroll_behavior {
        Some(behavior) if !behavior.is_pinned() => kompozable_block!({
            let on_delta_behavior = behavior.clone();
            let state = remember_draggable_state(move |delta| {
                let state = on_delta_behavior.state();
                state.set_height_offset(state.height_offset() + delta);
            });
            let stopped_behavior = behavior.clone();
            let on_drag_stopped: OnDragStopped = Rc::new(move |_, velocity| {
                let behavior = stopped_behavior.clone();
                Box::pin(async move {
                    settle_app_bar(
                        &behavior.state(),
                        velocity,
                        behavior.fling_animation_spec(),
                        behavior.snap_animation_spec(),
                    )
                    .await;
                })
            });
            named_args!(Modifier.draggable(state, Orientation::Vertical, on_drag_stopped = Some(on_drag_stopped)))
        }),
        _ => Modifier,
    }
}

/// Sets the scroll behavior's height offset limit to what the layout can collapse by, as its size changes (Compose's
/// private `Modifier.adjustHeightOffsetLimit`); nothing without a behavior.
fn adjust_height_offset_limit(scroll_behavior: &Option<Rc<dyn TopAppBarScrollBehavior>>) -> Modifier {
    match scroll_behavior {
        Some(behavior) => {
            let state = behavior.state();
            Modifier.on_size_changed(move |size| {
                let offset = size.height as f32 - state.height_offset();
                state.set_height_offset_limit(-offset);
            })
        }
        None => Modifier,
    }
}

/// A bar's row (Compose's private `TopAppBarLayout`): the navigation icon (4 dp from the start), the title (and
/// subtitle under it, a column lined up by `title_horizontal_alignment`; 4 dp at its sides, faded by `title_alpha`,
/// hidden from accessibility with `hide_title_semantics`) and the actions (4 dp from the end), each in its color;
/// measured by [`TopAppBarMeasurePolicy`], `height` tall less the scrolled offset.
#[kompozable(inline)]
#[allow(clippy::too_many_arguments)]
fn TopAppBarLayout(
    modifier: Modifier,
    #[memoized] scrolled_offset: impl Fn() -> f32 + 'static,
    navigation_icon_content_color: Color,
    title_content_color: Color,
    subtitle_content_color: Color,
    action_icon_content_color: Color,
    title: Rc<dyn Fn()>,
    title_text_style: TextStyle,
    subtitle: Option<Rc<dyn Fn()>>,
    subtitle_text_style: TextStyle,
    title_alpha: Rc<dyn Fn() -> f32>,
    title_vertical_arrangement: VerticalArrangement,
    title_horizontal_alignment: HorizontalAlignment,
    title_bottom_padding: i32,
    hide_title_semantics: bool,
    navigation_icon: Rc<dyn Fn()>,
    actions: Rc<dyn Fn()>,
    height: Dp,
    content_padding: PaddingValues,
) {
    let scrolled_offset_id = scrolled_offset.id();
    let scrolled_offset: Rc<dyn Fn() -> f32> = Rc::new(scrolled_offset.into_value());
    // (Compose's measure policy is remembered by these; its content padding isn't a key, as there)
    let measure_policy = {
        let keys = (
            scrolled_offset_id,
            title_vertical_arrangement.clone(),
            title_horizontal_alignment.clone(),
            title_bottom_padding,
            height,
        );
        let (arrangement, alignment) = (title_vertical_arrangement.clone(), title_horizontal_alignment.clone());
        remember_keyed(keys, move || {
            Rc::new(TopAppBarMeasurePolicy {
                scrolled_offset,
                title_vertical_arrangement: arrangement,
                title_horizontal_alignment: alignment,
                title_bottom_padding,
                height,
                content_padding,
            }) as Rc<dyn MeasurePolicy>
        })
    };
    let title_semantics =
        move || if hide_title_semantics { Modifier.clear_and_set_semantics(|_| {}) } else { Modifier };
    layout_into(modifier, measure_policy, false, move || {
        let navigation_icon = navigation_icon.clone();
        Box(
            modifier = Modifier.layout_id(NavigationIconId).padding_sides(start = TopAppBarHorizontalPadding),
            content = move || {
                let navigation_icon = navigation_icon.clone();
                KompozitionLocalProvider(&[LocalContentColor.provides(navigation_icon_content_color)], move || {
                    navigation_icon()
                });
            },
        );
        let title_alpha = title_alpha.clone();
        let title_modifier = Modifier
            .layout_id(TitleId)
            .padding_hv(horizontal = TopAppBarHorizontalPadding)
            .then(title_semantics())
            .graphics_layer_block(move |layer| layer.set_alpha(title_alpha()));
        if let Some(subtitle) = subtitle.clone() {
            let (title, title_text_style) = (title.clone(), title_text_style.clone());
            let subtitle_text_style = subtitle_text_style.clone();
            Column(
                modifier = title_modifier,
                horizontal_alignment = title_horizontal_alignment.clone(),
                content = move || {
                    let (title, subtitle) = (title.clone(), subtitle.clone());
                    ProvideContentColorTextStyle(title_content_color, title_text_style.clone(), move || title());
                    ProvideContentColorTextStyle(subtitle_content_color, subtitle_text_style.clone(), move || {
                        subtitle()
                    });
                },
            );
        } else {
            // TODO(b/352770398): Workaround to maintain compatibility (Compose's own note: the title alone in a Box)
            let (title, title_text_style) = (title.clone(), title_text_style.clone());
            Box(
                modifier = title_modifier,
                content = move || {
                    let title = title.clone();
                    ProvideContentColorTextStyle(title_content_color, title_text_style.clone(), move || title());
                },
            );
        }
        let actions = actions.clone();
        Box(
            modifier = Modifier.layout_id(ActionIconsId).padding_sides(end = TopAppBarHorizontalPadding),
            content = move || {
                let actions = actions.clone();
                KompozitionLocalProvider(&[LocalContentColor.provides(action_icon_content_color)], move || actions());
            },
        );
    });
}

/// The layout ids of a bar's row (Compose's strings "navigationIcon", "title", "actionIcons").
const NavigationIconId: &str = "navigationIcon";
const TitleId: &str = "title";
const ActionIconsId: &str = "actionIcons";

/// How a bar's row is measured and placed (Compose's private `TopAppBarMeasurePolicy`): the navigation icon at the
/// start and the actions at the end, centered down; the title between them (at least 12 dp in), lined up by
/// `title_horizontal_alignment` across the whole width and moved aside where the icon or the actions would cover it,
/// centered down or at the bottom (`title_bottom_padding` px under its last baseline where there is room); the row
/// `height` tall (or the title's, if taller) and the padding, less the scrolled offset when bounded.
struct TopAppBarMeasurePolicy {
    scrolled_offset: Rc<dyn Fn() -> f32>,
    title_vertical_arrangement: VerticalArrangement,
    title_horizontal_alignment: HorizontalAlignment,
    title_bottom_padding: i32,
    height: Dp,
    content_padding: PaddingValues,
}

/// The child of `id` (Kotlin's `measurables.fastFirst { it.layoutId == id }`).
fn find<'a>(measurables: &[&'a dyn Measurable], id: &'static str) -> &'a dyn Measurable {
    *measurables
        .iter()
        .find(|m| m.layout_id().is_some_and(|key| key.eq_key(&id)))
        .unwrap_or_else(|| panic!("no child of layout id {id}"))
}

impl MeasurePolicy for TopAppBarMeasurePolicy {
    fn measure(
        &self,
        scope: &MeasureScope,
        measurables: &[&dyn Measurable],
        constraints: Constraints,
    ) -> MeasureResult {
        let navigation_icon_placeable =
            find(measurables, NavigationIconId).measure(named_args!(constraints.copy(min_width = 0)));
        let action_icons_placeable =
            find(measurables, ActionIconsId).measure(named_args!(constraints.copy(min_width = 0)));
        let layout_direction = scope.layout_direction();
        let start_padding_px = self.content_padding.calculate_start_padding(layout_direction);
        let end_padding_px = self.content_padding.calculate_end_padding(layout_direction);
        let title_start_space = scope.round_to_px(TopAppBarTitleInset).max(navigation_icon_placeable.width());
        let max_title_width = if constraints.max_width == Constraints::Infinity {
            constraints.max_width
        } else {
            (constraints.max_width
                - title_start_space
                - action_icons_placeable.width()
                - scope.round_to_px(start_padding_px)
                - scope.round_to_px(end_padding_px))
            .max(0)
        };
        let title_placeable = find(measurables, TitleId)
            .measure(named_args!(constraints.copy(min_width = 0, max_width = max_title_width)));

        // Locate the title's baseline.
        let title_baseline = if title_placeable.get(LastBaseline) != kompoz_ui::layout::AlignmentLine::Unspecified {
            title_placeable.get(LastBaseline)
        } else {
            0
        };

        // Subtract the scrolledOffset from the maxHeight. The scrolledOffset is expected to be equal or smaller than
        // zero.
        let scrolled_offset_value = (self.scrolled_offset)();
        let height_offset = if scrolled_offset_value.is_nan() { 0 } else { scrolled_offset_value.round() as i32 };
        let top_padding_px = scope.round_to_px(self.content_padding.calculate_top_padding());
        let bottom_padding_px = scope.round_to_px(self.content_padding.calculate_bottom_padding());
        let max_layout_height =
            scope.round_to_px(self.height).max(title_placeable.height()) + top_padding_px + bottom_padding_px;
        let layout_height = if constraints.max_height == Constraints::Infinity {
            max_layout_height
        } else {
            (max_layout_height + height_offset).max(0)
        };

        self.place_top_app_bar(
            scope,
            constraints,
            layout_height,
            max_layout_height,
            navigation_icon_placeable,
            title_placeable,
            action_icons_placeable,
            title_baseline,
        )
    }

    fn min_intrinsic_width(
        &self,
        _scope: &IntrinsicMeasureScope,
        measurables: &[&dyn IntrinsicMeasurable],
        height: i32,
    ) -> i32 {
        measurables.iter().map(|m| m.min_intrinsic_width(height)).sum()
    }

    fn min_intrinsic_height(
        &self,
        scope: &IntrinsicMeasureScope,
        measurables: &[&dyn IntrinsicMeasurable],
        width: i32,
    ) -> i32 {
        scope.round_to_px(self.height).max(measurables.iter().map(|m| m.min_intrinsic_height(width)).max().unwrap_or(0))
    }

    fn max_intrinsic_width(
        &self,
        _scope: &IntrinsicMeasureScope,
        measurables: &[&dyn IntrinsicMeasurable],
        height: i32,
    ) -> i32 {
        measurables.iter().map(|m| m.max_intrinsic_width(height)).sum()
    }

    fn max_intrinsic_height(
        &self,
        scope: &IntrinsicMeasureScope,
        measurables: &[&dyn IntrinsicMeasurable],
        width: i32,
    ) -> i32 {
        scope.round_to_px(self.height).max(measurables.iter().map(|m| m.max_intrinsic_height(width)).max().unwrap_or(0))
    }
}

impl TopAppBarMeasurePolicy {
    /// Places the row's parts (Compose's `placeTopAppBar`).
    #[allow(clippy::too_many_arguments)]
    fn place_top_app_bar(
        &self,
        scope: &MeasureScope,
        constraints: Constraints,
        layout_height: i32,
        max_layout_height: i32,
        navigation_icon_placeable: Placeable,
        title_placeable: Placeable,
        action_icons_placeable: Placeable,
        title_baseline: i32,
    ) -> MeasureResult {
        let layout_direction = scope.layout_direction();
        let top_padding = scope.round_to_px(self.content_padding.calculate_top_padding());
        let bottom_padding = scope.round_to_px(self.content_padding.calculate_bottom_padding());
        let start_padding = scope.round_to_px(self.content_padding.calculate_start_padding(layout_direction));
        let end_padding = scope.round_to_px(self.content_padding.calculate_end_padding(layout_direction));
        let content_height = layout_height + top_padding - bottom_padding;
        let title_inset = scope.round_to_px(TopAppBarTitleInset);
        let title_horizontal_alignment = self.title_horizontal_alignment.clone();
        let title_vertical_arrangement = self.title_vertical_arrangement.clone();
        let title_bottom_padding = self.title_bottom_padding;
        scope.layout(constraints.max_width, layout_height, move |p| {
            // Navigation icon
            p.place_relative(
                &navigation_icon_placeable,
                start_padding,
                (content_height - navigation_icon_placeable.height()) / 2,
            );

            let start = title_inset.max(navigation_icon_placeable.width());
            let end = action_icons_placeable.width();

            // Align using the maxWidth. We will adjust the position later according to the start and end. This is
            // done to ensure that a center alignment is still maintained when the start and end have different
            // widths. Note that the title is centered relative to the entire app bar width, and not just centered
            // between the navigation icon and the actions.
            let mut title_x = title_horizontal_alignment.align(
                title_placeable.width(),
                constraints.max_width,
                // Using Ltr as we call placeRelative later on.
                LayoutDirection::Ltr,
            );
            // Reposition the title based on the start and the end (i.e. the navigation and action widths).
            if title_x < start {
                title_x += start_padding + (start - title_x);
            } else if title_x + title_placeable.width() > constraints.max_width - end {
                title_x += start_padding + ((constraints.max_width - end) - (title_x + title_placeable.width()));
            }

            // The titleVerticalArrangement is always one of Center or Bottom.
            let title_y = if title_vertical_arrangement == VerticalArrangement::Center {
                (content_height - title_placeable.height()) / 2
            } else if title_vertical_arrangement == VerticalArrangement::Bottom {
                // Apply bottom padding from the title's baseline only when the Arrangement is "Bottom".
                if title_bottom_padding == 0 {
                    content_height - title_placeable.height()
                } else {
                    // Calculate the actual padding from the bottom of the title, taking into account its baseline.
                    let padding_from_bottom = title_bottom_padding - (title_placeable.height() - title_baseline);
                    // Adjust the bottom padding to a smaller number if there is no room to fit the title.
                    let height_with_padding = padding_from_bottom + title_placeable.height();
                    let adjusted_bottom_padding = if height_with_padding > max_layout_height {
                        padding_from_bottom - (height_with_padding - max_layout_height)
                    } else {
                        padding_from_bottom
                    };

                    content_height - title_placeable.height() - adjusted_bottom_padding.max(0)
                }
            } else {
                // Arrangement.Top
                0
            };

            p.place_relative(&title_placeable, title_x, title_y);

            // Action icons
            p.place_relative(
                &action_icons_placeable,
                constraints.max_width - action_icons_placeable.width() - end_padding,
                (content_height - action_icons_placeable.height()) / 2,
            );
        })
    }
}

// An easing function used to compute the alpha value that is applied to the top title part of a Medium or Large app
// bar.
/// The easing of a two-rows bar's small title's alpha as it collapses (Compose's internal `TopTitleAlphaEasing`).
pub(crate) const TopTitleAlphaEasing: CubicBezierEasing = CubicBezierEasing::constant(0.8, 0.0, 0.8, 0.15);

const MediumTitleBottomPadding: Dp = Dp(24.0);
const LargeTitleBottomPadding: Dp = Dp(28.0);
const TopAppBarHorizontalPadding: Dp = Dp(4.0);

// A title inset when the App-Bar is a Medium or Large one. Also used to size a spacer when the navigation icon is
// missing.
const TopAppBarTitleInset: Dp = Dp(16.0 - 4.0);
