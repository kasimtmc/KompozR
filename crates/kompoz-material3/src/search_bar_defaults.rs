//! The search bars' defaults, colors and input fields, as Compose's SearchBar.kt has them: [`SearchBarDefaults`]
//! (elevations, shapes, colors, insets, the scroll behavior, the input fields), [`SearchBarColors`] and
//! [`AppBarWithSearchColors`].
//!
//! Compose's three `SearchBarDefaults.InputField` overloads are methods of their own here, named by what tells them
//! apart: [`InputField`](SearchBarDefaults::InputField) (Kotlin's `InputField(textFieldState, searchBarState, …)`),
//! [`InputFieldWithExpanded`](SearchBarDefaults::InputFieldWithExpanded) (`InputField(state, onSearch, expanded,
//! onExpandedChange, …)`) and [`StringInputField`](SearchBarDefaults::StringInputField) (`InputField(query:
//! String, onQueryChange, …)`).

use crate::internal::strings::{Strings, get_string};
use crate::internal::system_bars_default_insets::system_bars_for_visual_components;
use crate::internal::text_field_impl::TextFieldImplModifiers;
use crate::search_bar::{
    AnimationDelayMillis, DetectClickFromInteractionSource, EnterAlwaysSearchBarScrollBehavior, SearchBarIconOffsetX,
    SearchBarMaxWidth, SearchBarMinWidth, SearchBarScrollBehavior, SearchBarState,
};
use crate::text::LocalTextStyle;
use crate::text_field_defaults::{TextFieldColors, TextFieldDefaults, TextFieldSlot};
use crate::tokens::{
    AppBarTokens, ColorSchemeKeyTokens, ElevationTokens, FilledTextFieldTokens, MotionSchemeKeyTokens, ScrimTokens,
    SearchBarTokens, SearchViewTokens,
};
use kompoz_animation::{
    AnimationSpec, DecayAnimationSpec, Easing, FastOutLinearInEasing, animate_color_as_state,
    remember_spline_based_decay,
};
use kompoz_coroutines::{LaunchedEffect, delay, remember_coroutine_scope, snapshot_flow};
use kompoz_foundation::prelude::*;
use kompoz_foundation::text::{KeyboardAction, KeyboardActions};
use kompoz_graphics::{Color, ColorProducer, Dp, RoundedCornerShape, Shape, SolidColor, lerp_color};
use kompoz_runtime::{KotlinProperties, kompozable_block, named, named_args, remember, remember_saveable_keyed_with};
use kompoz_text::TextStyle;
use kompoz_text::input::VisualTransformation;
use kompoz_ui::Modifier;
use kompoz_ui::focus::{FocusDirection, FocusRequester};
use kompoz_ui::input::{Key, KeyEvent};
use kompoz_ui::platform::{ImeAction, InputMode, LocalFocusManager, LocalInputModeManager};
use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;

/// The height of a search bar's input field, or a collapsed search bar (Compose's `SearchBarDefaults.InputFieldHeight`).
pub(crate) const InputFieldHeight: Dp = SearchBarTokens::ContainerHeight;

/// The search bars' defaults (Compose's `object SearchBarDefaults`).
#[derive(Clone, Copy, Debug)]
pub struct SearchBarDefaults;

impl SearchBarDefaults {
    /// The default tonal elevation of a search bar: none.
    pub const TonalElevation: Dp = ElevationTokens::Level0;

    /// The default shadow elevation of a search bar: none.
    pub const ShadowElevation: Dp = ElevationTokens::Level0;

    /// Compose's deprecated `Elevation`, renamed [`TonalElevation`](SearchBarDefaults::TonalElevation).
    #[deprecated(note = "Renamed to TonalElevation. Not to be confused with ShadowElevation.")]
    pub const Elevation: Dp = SearchBarDefaults::TonalElevation;

    /// The default height of a search bar's input field, or of a collapsed search bar: 56 dp.
    pub const InputFieldHeight: Dp = InputFieldHeight;

    /// The default gap between a docked search bar and its drop-down: 2 dp.
    ///
    /// Kotlin: `dockedDropdownGapSize`
    pub const DockedDropdownGapSize: Dp = Dp(2.0); // TODO: replace with token.

    /// The default shape of a search bar's input field, or of a collapsed search bar: a pill (Compose's
    /// `inputFieldShape`).
    pub fn input_field_shape() -> Arc<dyn Shape> {
        SearchBarTokens::ContainerShape.value()
    }

    /// The default shape of a search bar expanded over the window: a rectangle (Compose's `fullScreenShape`).
    pub fn full_screen_shape() -> Arc<dyn Shape> {
        SearchViewTokens::FullScreenContainerShape.value()
    }

    /// A collapsed contained search bar's color: the surface container high (Compose's
    /// `collapsedContainedSearchBarColor`).
    pub fn collapsed_contained_search_bar_color() -> Color {
        SearchBarTokens::ContainerColor.value()
    }

    /// An expanded contained search bar's color: the surface container low (Compose's
    /// `fullScreenContainedSearchBarColor`).
    pub fn full_screen_contained_search_bar_color() -> Color {
        ColorSchemeKeyTokens::SurfaceContainerLow.value() // TODO: replace with token.
    }

    /// The default shape of a docked search bar: the theme's extra large corners (Compose's `dockedShape`).
    pub fn docked_shape() -> Arc<dyn Shape> {
        SearchViewTokens::DockedContainerShape.value()
    }

    /// The default shape of a docked search bar's drop-down of results: 12 dp corners (Compose's
    /// `dockedDropdownShape`).
    pub fn docked_dropdown_shape() -> Arc<dyn Shape> {
        Arc::new(RoundedCornerShape::new(Dp(12.0), Dp(12.0), Dp(12.0), Dp(12.0))) // TODO: replace with token.
    }

    /// The default scrim color behind a docked search bar's drop-down: the scrim's at its opacity (Compose's
    /// `dockedDropdownScrimColor`).
    pub fn docked_dropdown_scrim_color() -> Color {
        with_alpha(ScrimTokens::ContainerColor.value(), ScrimTokens::ContainerOpacity)
    }

    /// The default padding of an [`AppBarWithSearch`](crate::AppBarWithSearch)'s content: none (Compose's
    /// `AppBarContentPadding`).
    pub fn AppBarContentPadding() -> PaddingValues {
        PaddingValues::all(Dp(0.0))
    }

    /// The default insets of an [`AppBarWithSearch`](crate::AppBarWithSearch): the system bars' at the sides and
    /// top (Compose's `windowInsets`).
    pub fn window_insets() -> WindowInsets {
        system_bars_for_visual_components().only(WindowInsetsSides::Horizontal + WindowInsetsSides::Top)
    }

    /// The default insets an [`ExpandedFullScreenSearchBar`](crate::ExpandedFullScreenSearchBar) pads by and
    /// consumes: the safe drawing ones (Compose's `fullScreenWindowInsets`).
    pub fn full_screen_window_insets() -> WindowInsets {
        WindowInsets::safe_drawing()
    }
}

#[named]
impl SearchBarDefaults {
    /// The scroll behavior of an [`AppBarWithSearch`](crate::AppBarWithSearch) that slides it away as content
    /// scrolls up and brings it back as soon as content scrolls down (Compose's
    /// `enterAlwaysSearchBarScrollBehavior`), saved and remade when the specs, `can_scroll` (by identity) or
    /// `reverse_layout` change: its offset from `initial_offset` within `initial_offset_limit` (until the bar is
    /// measured) and 0, the content's from `initial_content_offset`; it moves only while `can_scroll` says; a
    /// released drag settles it shown or gone along `snap_animation_spec`, a fling's velocity carried on along
    /// `fling_animation_spec`; `reverse_layout` for content laid out from the bottom (its pull down reveals it).
    ///
    /// Kotlin: `enterAlwaysSearchBarScrollBehavior(initialOffset: Float = 0f, initialOffsetLimit: Float = -Float.MAX_VALUE, initialContentOffset: Float = 0f, canScroll: () -> Boolean = { true }, snapAnimationSpec: AnimationSpec<Float> = MotionSchemeKeyTokens.DefaultEffects.value(), flingAnimationSpec: DecayAnimationSpec<Float> = rememberSplineBasedDecay(), reverseLayout: Boolean = false): SearchBarScrollBehavior`
    #[track_caller]
    #[allow(clippy::too_many_arguments)]
    pub fn enter_always_search_bar_scroll_behavior(
        &self,
        #[default(0.0)] initial_offset: f32,
        #[default(-f32::MAX)] initial_offset_limit: f32,
        #[default(0.0)] initial_content_offset: f32,
        #[default(|| true)]
        #[memoized]
        can_scroll: impl Fn() -> bool + 'static,
        // TODO Load the motionScheme tokens from the component tokens file
        #[default(MotionSchemeKeyTokens::DefaultEffects.value::<f32>())] snap_animation_spec: Arc<
            dyn AnimationSpec<f32>,
        >,
        #[default(None)] fling_animation_spec: Option<Arc<dyn DecayAnimationSpec<f32>>>,
        #[default(false)] reverse_layout: bool,
    ) -> Rc<dyn SearchBarScrollBehavior> {
        kompozable_block!({
            // (Kotlin's default `rememberSplineBasedDecay()`: None is it)
            let fling_animation_spec = fling_animation_spec
                .unwrap_or_else(|| Arc::new(remember_spline_based_decay::<f32>()) as Arc<dyn DecayAnimationSpec<f32>>);
            let can_scroll_id = can_scroll.id();
            let can_scroll: Rc<dyn Fn() -> bool> = Rc::new(can_scroll.into_value());
            let saver = EnterAlwaysSearchBarScrollBehavior::saver(
                can_scroll.clone(),
                snap_animation_spec.clone(),
                fling_animation_spec.clone(),
            );
            let fling_key = Arc::as_ptr(&fling_animation_spec) as *const () as usize;
            let behavior = remember_saveable_keyed_with(
                (snap_animation_spec.clone(), fling_key, can_scroll_id, reverse_layout),
                &saver,
                move || {
                    EnterAlwaysSearchBarScrollBehavior::new(
                        initial_offset,
                        initial_offset_limit,
                        initial_content_offset,
                        can_scroll,
                        reverse_layout,
                        snap_animation_spec,
                        fling_animation_spec,
                    )
                },
            );
            Rc::new(behavior) as Rc<dyn SearchBarScrollBehavior>
        })
    }

    /// A search bar's colors (Compose's composable `colors(containerColor, dividerColor, inputFieldColors)`): its
    /// container's (the surface container high), the divider's between the field and the results (the outline),
    /// the input field's ([`input_field_colors`](Self::input_field_colors)).
    ///
    /// Kotlin: `colors(containerColor: Color = SearchBarTokens.ContainerColor.value, dividerColor: Color = SearchViewTokens.DividerColor.value, inputFieldColors: TextFieldColors = inputFieldColors()): SearchBarColors`
    #[track_caller]
    pub fn colors(
        &self,
        #[default(SearchBarTokens::ContainerColor.value())] container_color: Color,
        #[default(SearchViewTokens::DividerColor.value())] divider_color: Color,
        #[default(self.input_field_colors())] input_field_colors: TextFieldColors,
    ) -> SearchBarColors {
        SearchBarColors::new(container_color, divider_color, input_field_colors)
    }

    /// A contained search bar's colors of `state` (Compose's Expressive composable `containedColors(state)`): its
    /// container the full-screen contained color while the state is expanded, else the collapsed one; its input
    /// field on the collapsed color.
    ///
    /// Kotlin: `containedColors(state: SearchBarState): SearchBarColors`
    #[track_caller]
    pub fn contained_colors(&self, state: SearchBarState) -> SearchBarColors {
        kompozable_block!({
            let container_color = if state.is_expanded() {
                SearchBarDefaults::full_screen_contained_search_bar_color()
            } else {
                SearchBarDefaults::collapsed_contained_search_bar_color()
            };
            let collapsed = SearchBarDefaults::collapsed_contained_search_bar_color();
            named_args!(self.colors(
                container_color = container_color,
                input_field_colors = named_args!(self.input_field_colors(
                    focused_container_color = collapsed,
                    unfocused_container_color = collapsed,
                    disabled_container_color = collapsed,
                )),
            ))
        })
    }

    /// An [`AppBarWithSearch`](crate::AppBarWithSearch)'s colors (Compose's composable `appBarWithSearchColors`):
    /// the search bar's ([`colors`](Self::colors)), its container's while content is scrolled under it (the surface
    /// container highest), the app bar's container (the surface) and scrolled container (the surface container),
    /// its navigation and action icons' (the on-surface and on-surface variant).
    ///
    /// Kotlin: `appBarWithSearchColors(searchBarColors: SearchBarColors = colors(), scrolledSearchBarContainerColor: Color = ColorSchemeKeyTokens.SurfaceContainerHighest.value, appBarContainerColor: Color = AppBarTokens.ContainerColor.value, scrolledAppBarContainerColor: Color = AppBarTokens.OnScrollContainerColor.value, appBarNavigationIconColor: Color = AppBarTokens.LeadingIconColor.value, appBarActionIconColor: Color = AppBarTokens.TrailingIconColor.value): AppBarWithSearchColors`
    #[track_caller]
    pub fn app_bar_with_search_colors(
        &self,
        #[default(self.colors())] search_bar_colors: SearchBarColors,
        // TODO Load the color tokens from the component tokens file
        #[default(ColorSchemeKeyTokens::SurfaceContainerHighest.value())] scrolled_search_bar_container_color: Color,
        #[default(AppBarTokens::ContainerColor.value())] app_bar_container_color: Color,
        #[default(AppBarTokens::OnScrollContainerColor.value())] scrolled_app_bar_container_color: Color,
        #[default(AppBarTokens::LeadingIconColor.value())] app_bar_navigation_icon_color: Color,
        #[default(AppBarTokens::TrailingIconColor.value())] app_bar_action_icon_color: Color,
    ) -> AppBarWithSearchColors {
        AppBarWithSearchColors::new(
            search_bar_colors,
            scrolled_search_bar_container_color,
            app_bar_container_color,
            scrolled_app_bar_container_color,
            app_bar_navigation_icon_color,
            app_bar_action_icon_color,
        )
    }

    /// A search bar's input field's colors (Compose's composable `inputFieldColors(…)`): the search bar's text,
    /// icons and placeholder colors, the filled field's disabled, cursor, prefix and suffix ones, a transparent
    /// container; through [`TextFieldDefaults`]`.colors(…)` (what isn't given here — label, indicator, supporting
    /// text, errors — the filled field's).
    ///
    /// Kotlin: `inputFieldColors(focusedTextColor: Color = SearchBarTokens.InputTextColor.value): TextFieldColors`
    #[track_caller]
    #[allow(clippy::too_many_arguments)]
    pub fn input_field_colors(
        &self,
        #[default(SearchBarTokens::InputTextColor.value())] focused_text_color: Color,
        #[default(SearchBarTokens::InputTextColor.value())] unfocused_text_color: Color,
        #[default(with_alpha(FilledTextFieldTokens::DisabledInputColor.value(), FilledTextFieldTokens::DisabledInputOpacity))]
        disabled_text_color: Color,
        #[default(FilledTextFieldTokens::CaretColor.value())] cursor_color: Color,
        #[default(LocalTextSelectionColors.current())] selection_colors: TextSelectionColors,
        #[default(SearchBarTokens::LeadingIconColor.value())] focused_leading_icon_color: Color,
        #[default(SearchBarTokens::LeadingIconColor.value())] unfocused_leading_icon_color: Color,
        #[default(with_alpha(FilledTextFieldTokens::DisabledLeadingIconColor.value(), FilledTextFieldTokens::DisabledLeadingIconOpacity))]
        disabled_leading_icon_color: Color,
        #[default(SearchBarTokens::TrailingIconColor.value())] focused_trailing_icon_color: Color,
        #[default(SearchBarTokens::TrailingIconColor.value())] unfocused_trailing_icon_color: Color,
        #[default(with_alpha(FilledTextFieldTokens::DisabledTrailingIconColor.value(), FilledTextFieldTokens::DisabledTrailingIconOpacity))]
        disabled_trailing_icon_color: Color,
        #[default(SearchBarTokens::SupportingTextColor.value())] focused_placeholder_color: Color,
        #[default(SearchBarTokens::SupportingTextColor.value())] unfocused_placeholder_color: Color,
        #[default(with_alpha(FilledTextFieldTokens::DisabledInputColor.value(), FilledTextFieldTokens::DisabledInputOpacity))]
        disabled_placeholder_color: Color,
        #[default(FilledTextFieldTokens::InputPrefixColor.value())] focused_prefix_color: Color,
        #[default(FilledTextFieldTokens::InputPrefixColor.value())] unfocused_prefix_color: Color,
        #[default(with_alpha(FilledTextFieldTokens::InputPrefixColor.value(), FilledTextFieldTokens::DisabledInputOpacity))]
        disabled_prefix_color: Color,
        #[default(FilledTextFieldTokens::InputSuffixColor.value())] focused_suffix_color: Color,
        #[default(FilledTextFieldTokens::InputSuffixColor.value())] unfocused_suffix_color: Color,
        #[default(with_alpha(FilledTextFieldTokens::InputSuffixColor.value(), FilledTextFieldTokens::DisabledInputOpacity))]
        disabled_suffix_color: Color,
        #[default(Color::Transparent)] focused_container_color: Color,
        #[default(Color::Transparent)] unfocused_container_color: Color,
        #[default(Color::Transparent)] disabled_container_color: Color,
    ) -> TextFieldColors {
        named_args!(TextFieldDefaults.colors(
            focused_text_color = focused_text_color,
            unfocused_text_color = unfocused_text_color,
            disabled_text_color = disabled_text_color,
            cursor_color = cursor_color,
            selection_colors = Some(selection_colors),
            focused_leading_icon_color = focused_leading_icon_color,
            unfocused_leading_icon_color = unfocused_leading_icon_color,
            disabled_leading_icon_color = disabled_leading_icon_color,
            focused_trailing_icon_color = focused_trailing_icon_color,
            unfocused_trailing_icon_color = unfocused_trailing_icon_color,
            disabled_trailing_icon_color = disabled_trailing_icon_color,
            focused_placeholder_color = focused_placeholder_color,
            unfocused_placeholder_color = unfocused_placeholder_color,
            disabled_placeholder_color = disabled_placeholder_color,
            focused_prefix_color = focused_prefix_color,
            unfocused_prefix_color = unfocused_prefix_color,
            disabled_prefix_color = disabled_prefix_color,
            focused_suffix_color = focused_suffix_color,
            unfocused_suffix_color = unfocused_suffix_color,
            disabled_suffix_color = disabled_suffix_color,
            focused_container_color = focused_container_color,
            unfocused_container_color = unfocused_container_color,
            disabled_container_color = disabled_container_color,
        ))
    }

    /// The input field of a search bar of `search_bar_state` (Compose's `InputField(textFieldState, searchBarState,
    /// onSearch, …)`): a field editing `text_field_state` (one line by default, `line_limits`), at least 360 dp wide
    /// (720 at most) and 56 dp tall, its placeholder, icons (4 dp further in than a text field's), prefix and suffix
    /// in `colors`, on a container of `shape` in their container color (animated); the keyboard's search action
    /// gives `on_search` the text. It expands the state when focused by touch, when a press is released on it,
    /// when typed into while focused, and on the down key out of touch mode (down again moves the focus to the
    /// results); it gives the focus up a moment after the state collapses (in touch mode). It tells accessibility
    /// it searches, and that suggestions are below while expanded. The other parameters as `BasicTextField`'s.
    ///
    /// Kotlin: `InputField(textFieldState: TextFieldState, searchBarState: SearchBarState, onSearch: (String) -> Unit, modifier: Modifier = Modifier, enabled: Boolean = true, readOnly: Boolean = false, textStyle: TextStyle = LocalTextStyle.current, placeholder: @Composable (() -> Unit)? = null, leadingIcon: @Composable (() -> Unit)? = null, trailingIcon: @Composable (() -> Unit)? = null, prefix: @Composable (() -> Unit)? = null, suffix: @Composable (() -> Unit)? = null, inputTransformation: InputTransformation? = null, outputTransformation: OutputTransformation? = null, scrollState: ScrollState = rememberScrollState(), shape: Shape = inputFieldShape, colors: TextFieldColors = inputFieldColors(), interactionSource: MutableInteractionSource? = null, keyboardOptions: KeyboardOptions = KeyboardOptions.Default, lineLimits: TextFieldLineLimits = TextFieldLineLimits.SingleLine)`
    #[track_caller]
    #[allow(clippy::too_many_arguments)]
    pub fn InputField(
        &self,
        text_field_state: TextFieldState,
        search_bar_state: SearchBarState,
        on_search: impl Fn(String) + 'static,
        #[default(Modifier)] modifier: Modifier,
        #[default(true)] enabled: bool,
        #[default(false)] read_only: bool,
        #[default(LocalTextStyle.current())] text_style: TextStyle,
        #[default(None)] placeholder: Option<TextFieldSlot>,
        #[default(None)] leading_icon: Option<TextFieldSlot>,
        #[default(None)] trailing_icon: Option<TextFieldSlot>,
        #[default(None)] prefix: Option<TextFieldSlot>,
        #[default(None)] suffix: Option<TextFieldSlot>,
        #[default(None)] input_transformation: Option<InputTransformation>,
        #[default(None)] output_transformation: Option<Rc<dyn OutputTransformation>>,
        #[default(None)] scroll_state: Option<ScrollState>,
        #[default(SearchBarDefaults::input_field_shape())] shape: Arc<dyn Shape>,
        #[default(self.input_field_colors())] colors: TextFieldColors,
        #[default(None)] interaction_source: Option<MutableInteractionSource>,
        #[default(KeyboardOptions::Default)] keyboard_options: KeyboardOptions,
        #[default(TextFieldLineLimits::SingleLine)] line_limits: TextFieldLineLimits,
    ) {
        let on_search = Rc::new(on_search);
        kompozable_block!({
            let interaction_source = interaction_source.unwrap_or_else(|| remember(MutableInteractionSource::new));
            let source = interaction_source.as_interaction_source();

            // Relationship between focus and expansion state:
            //  - In touch mode, the field is focused if and only if the search bar is expanded. Focus changes trigger
            //    expansion changes, and vice versa.
            //  - In non-touch mode, focus and expansion are independent: focus is changed by moving with keyboard
            //    arrows/tabs, and expansion is changed by pressing the down-key or by typing.
            let focused = source.collect_is_focused_as_state();
            let focus_manager = LocalFocusManager.current();
            let is_in_touch_mode = LocalInputModeManager.current().input_mode() == InputMode::Touch;

            let search_semantics = get_string(Strings::SearchBarSearch);
            let suggestions_available_semantics = get_string(Strings::SuggestionsAvailable);

            let text_color = text_style.color.take_or_else(|| colors.text_color(enabled, false, focused.get()));
            let merged_text_style = text_style.merge(&TextStyle { color: text_color, ..TextStyle::Default });

            let coroutine_scope = remember_coroutine_scope();
            let scroll_state = scroll_state.unwrap_or_else(|| remember_scroll_state());

            let (key_state, key_scope, key_focus_manager) =
                (search_bar_state.clone(), coroutine_scope.clone(), focus_manager.clone());
            let (focus_state, focus_scope) = (search_bar_state.clone(), coroutine_scope.clone());
            let semantics_state = search_bar_state.clone();
            let search_text = text_field_state.clone();
            let on_keyboard_action: Rc<dyn KeyboardActionHandler> =
                Rc::new(move |_: &dyn Fn()| on_search(search_text.text()));
            let container_source = source.clone();
            let container: TextFieldSlot = Rc::new(move || {
                let focused = container_source.collect_is_focused_as_state().get();
                let container_color = animate_color_as_state(
                    colors.container_color(enabled, false, focused),
                    animation_spec = MotionSchemeKeyTokens::FastEffects.value::<Color>(),
                );
                Box(modifier =
                    Modifier.text_field_background(ColorProducer::new(move || container_color.get()), shape.clone()));
            });
            BasicTextField(
                text_field_state.clone(),
                modifier = modifier
                    .on_preview_key_event(move |it: &KeyEvent| {
                        let expand_on_down_key = !is_in_touch_mode && !key_state.is_expanded();
                        if expand_on_down_key && it.key == Key::DirectionDown {
                            launch_animate_to_expanded(&key_scope, &key_state);
                            return true;
                        }
                        // Make sure arrow key down moves to list of suggestions.
                        if key_state.is_expanded() && it.key == Key::DirectionDown {
                            key_focus_manager.move_focus(FocusDirection::Down);
                            return true;
                        }
                        false
                    })
                    .size_in(
                        min_width = SearchBarMinWidth,
                        max_width = SearchBarMaxWidth,
                        min_height = InputFieldHeight,
                    )
                    .on_focus_changed(move |it| {
                        if it.is_focused() && is_in_touch_mode {
                            launch_animate_to_expanded(&focus_scope, &focus_state);
                        }
                    })
                    .semantics(move |c| {
                        c.set_content_description(search_semantics.clone());
                        if semantics_state.is_expanded() {
                            c.set_state_description(suggestions_available_semantics.clone());
                        }
                    }),
                enabled = enabled,
                read_only = read_only,
                line_limits = line_limits,
                text_style = merged_text_style,
                cursor_brush = SolidColor(colors.cursor_color_of(false)),
                keyboard_options = keyboard_options
                    .merge(Some(&KeyboardOptions { ime_action: ImeAction::Search, ..KeyboardOptions::Default })),
                on_keyboard_action = Some(on_keyboard_action),
                interaction_source = Some(interaction_source.clone()),
                input_transformation = input_transformation,
                output_transformation = output_transformation.clone(),
                scroll_state = Some(scroll_state),
                decorator = Some(TextFieldDefaults.decorator(
                    text_field_state.clone(),
                    enabled,
                    line_limits,
                    output_transformation,
                    source.clone(),
                    placeholder = placeholder,
                    leading_icon = leading_icon.map(offset_icon(SearchBarIconOffsetX)),
                    trailing_icon = trailing_icon.map(offset_icon(-SearchBarIconOffsetX)),
                    prefix = prefix,
                    suffix = suffix,
                    colors = colors,
                    content_padding = Some(TextFieldDefaults.content_padding_without_label()),
                    container = Some(container),
                )),
            );

            // Most expansions from touch happen via `onFocusChanged` above, but in a mixed keyboard-touch flow, the
            // user can focus via keyboard (with no expansion), and subsequent touches won't change focus state. So this
            // effect is needed as well.
            let (click_state, click_scope) = (search_bar_state.clone(), coroutine_scope.clone());
            DetectClickFromInteractionSource(source.clone(), move || {
                if !click_state.is_expanded() {
                    launch_animate_to_expanded(&click_scope, &click_state);
                }
            });

            // Expand search bar if the user starts typing
            let (typing_state, typing_text, typing_scope, typing_focused) =
                (search_bar_state.clone(), text_field_state.clone(), coroutine_scope.clone(), focused.clone());
            LaunchedEffect((search_bar_state.clone(), text_field_state.clone()), async move {
                if !typing_state.is_expanded() {
                    let prev_length = Cell::new(typing_text.text().chars().count());
                    let text = typing_text.clone();
                    snapshot_flow(move || text.text())
                        .collect(move |it: String| {
                            let curr_length = it.chars().count();
                            if curr_length > prev_length.get() && typing_focused.peek() && !typing_state.is_expanded() {
                                // Don't use LaunchedEffect's coroutine because cancelling the animation shouldn't cancel
                                // the Flow
                                launch_animate_to_expanded(&typing_scope, &typing_state);
                            }
                            prev_length.set(curr_length);
                        })
                        .await;
                }
            });

            let should_clear_focus_on_collapse = !search_bar_state.is_expanded() && focused.get() && is_in_touch_mode;
            LaunchedEffect(search_bar_state.is_expanded(), async move {
                if should_clear_focus_on_collapse {
                    // Not strictly needed according to the motion spec, but since the animation already has a delay, this
                    // works around b/261632544.
                    delay(AnimationDelayMillis as i64).await;
                    focus_manager.clear_focus(false);
                }
            });
        });
    }

    /// The input field of a search bar that is `expanded` (Compose's `InputField(state: TextFieldState, onSearch,
    /// expanded, onExpandedChange, …)`): a one-line field editing `state`, at least 360 dp wide (720 at most) and
    /// 56 dp tall, its placeholder, icons (4 dp further in than a text field's), prefix and suffix in `colors`, on a
    /// container of `shape` in their container color (animated); the keyboard's search action gives `on_search` the
    /// text; focused, it gives `on_expanded_change` true; it gives the focus up a moment after `expanded` turns
    /// false. It tells accessibility it searches, and that suggestions are below while expanded.
    ///
    /// Kotlin: `InputField(state: TextFieldState, onSearch: (String) -> Unit, expanded: Boolean, onExpandedChange: (Boolean) -> Unit, modifier: Modifier = Modifier, enabled: Boolean = true, readOnly: Boolean = false, textStyle: TextStyle = LocalTextStyle.current, placeholder: @Composable (() -> Unit)? = null, leadingIcon: @Composable (() -> Unit)? = null, trailingIcon: @Composable (() -> Unit)? = null, prefix: @Composable (() -> Unit)? = null, suffix: @Composable (() -> Unit)? = null, inputTransformation: InputTransformation? = null, outputTransformation: OutputTransformation? = null, scrollState: ScrollState = rememberScrollState(), shape: Shape = inputFieldShape, colors: TextFieldColors = inputFieldColors(), interactionSource: MutableInteractionSource? = null)`
    #[track_caller]
    #[allow(clippy::too_many_arguments)]
    pub fn InputFieldWithExpanded(
        &self,
        state: TextFieldState,
        on_search: impl Fn(String) + 'static,
        expanded: bool,
        on_expanded_change: impl Fn(bool) + 'static,
        #[default(Modifier)] modifier: Modifier,
        #[default(true)] enabled: bool,
        #[default(false)] read_only: bool,
        #[default(LocalTextStyle.current())] text_style: TextStyle,
        #[default(None)] placeholder: Option<TextFieldSlot>,
        #[default(None)] leading_icon: Option<TextFieldSlot>,
        #[default(None)] trailing_icon: Option<TextFieldSlot>,
        #[default(None)] prefix: Option<TextFieldSlot>,
        #[default(None)] suffix: Option<TextFieldSlot>,
        #[default(None)] input_transformation: Option<InputTransformation>,
        #[default(None)] output_transformation: Option<Rc<dyn OutputTransformation>>,
        #[default(None)] scroll_state: Option<ScrollState>,
        #[default(SearchBarDefaults::input_field_shape())] shape: Arc<dyn Shape>,
        #[default(self.input_field_colors())] colors: TextFieldColors,
        #[default(None)] interaction_source: Option<MutableInteractionSource>,
    ) {
        let on_search = Rc::new(on_search);
        let on_expanded_change = Rc::new(on_expanded_change);
        kompozable_block!({
            let interaction_source = interaction_source.unwrap_or_else(|| remember(MutableInteractionSource::new));
            let source = interaction_source.as_interaction_source();

            let focused = source.collect_is_focused_as_state().get();
            let focus_requester = remember(FocusRequester::new);
            let focus_manager = LocalFocusManager.current();

            let search_semantics = get_string(Strings::SearchBarSearch);
            let suggestions_available_semantics = get_string(Strings::SuggestionsAvailable);

            let text_color = text_style.color.take_or_else(|| colors.text_color(enabled, false, focused));
            let merged_text_style = text_style.merge(&TextStyle { color: text_color, ..TextStyle::Default });
            let scroll_state = scroll_state.unwrap_or_else(|| remember_scroll_state());

            let search_text = state.clone();
            let on_keyboard_action: Rc<dyn KeyboardActionHandler> =
                Rc::new(move |_: &dyn Fn()| on_search(search_text.text()));
            BasicTextField(
                state.clone(),
                modifier = modifier
                    .size_in(
                        min_width = SearchBarMinWidth,
                        max_width = SearchBarMaxWidth,
                        min_height = InputFieldHeight,
                    )
                    .focus_requester(&focus_requester)
                    .on_focus_changed(move |it| {
                        if it.is_focused() {
                            on_expanded_change(true);
                        }
                    })
                    .semantics(move |c| {
                        c.set_content_description(search_semantics.clone());
                        if expanded {
                            c.set_state_description(suggestions_available_semantics.clone());
                        }
                    }),
                enabled = enabled,
                read_only = read_only,
                line_limits = TextFieldLineLimits::SingleLine,
                text_style = merged_text_style,
                cursor_brush = SolidColor(colors.cursor_color_of(false)),
                keyboard_options = KeyboardOptions { ime_action: ImeAction::Search, ..KeyboardOptions::Default },
                on_keyboard_action = Some(on_keyboard_action),
                interaction_source = Some(interaction_source.clone()),
                input_transformation = input_transformation,
                output_transformation = output_transformation.clone(),
                scroll_state = Some(scroll_state),
                decorator = Some(TextFieldDefaults.decorator(
                    state.clone(),
                    enabled,
                    TextFieldLineLimits::SingleLine,
                    output_transformation,
                    source.clone(),
                    placeholder = placeholder,
                    leading_icon = leading_icon.map(offset_icon(SearchBarIconOffsetX)),
                    trailing_icon = trailing_icon.map(offset_icon(-SearchBarIconOffsetX)),
                    prefix = prefix,
                    suffix = suffix,
                    colors = colors,
                    content_padding = Some(TextFieldDefaults.content_padding_without_label()),
                    container = Some(animated_container(source.clone(), enabled, colors, shape)),
                )),
            );

            let should_clear_focus = !expanded && focused;
            LaunchedEffect(expanded, async move {
                if should_clear_focus {
                    // Not strictly needed according to the motion spec, but since the animation already has a delay, this
                    // works around b/261632544.
                    delay(AnimationDelayMillis as i64).await;
                    focus_manager.clear_focus(false);
                }
            });
        });
    }

    /// The input field of a search bar of the text `query` that is `expanded` (Compose's `InputField(query: String,
    /// onQueryChange, onSearch, expanded, onExpandedChange, …)`): a one-line field of `query`, each text the user
    /// makes given to `on_query_change`, at least 360 dp wide (720 at most) and 56 dp tall, its placeholder and
    /// icons (4 dp further in than a text field's) in `colors`, on a container of the input field shape in their
    /// container color (animated); the keyboard's search action gives `on_search` the query; focused, it gives
    /// `on_expanded_change` true; it gives the focus up a moment after `expanded` turns false. It tells
    /// accessibility it searches, and that suggestions are below while expanded.
    ///
    /// Kotlin: `InputField(query: String, onQueryChange: (String) -> Unit, onSearch: (String) -> Unit, expanded: Boolean, onExpandedChange: (Boolean) -> Unit, modifier: Modifier = Modifier, enabled: Boolean = true, placeholder: @Composable (() -> Unit)? = null, leadingIcon: @Composable (() -> Unit)? = null, trailingIcon: @Composable (() -> Unit)? = null, colors: TextFieldColors = inputFieldColors(), interactionSource: MutableInteractionSource? = null)`
    #[track_caller]
    #[allow(clippy::too_many_arguments)]
    pub fn StringInputField(
        &self,
        query: impl Into<String>,
        on_query_change: impl Fn(String) + 'static,
        on_search: impl Fn(String) + 'static,
        expanded: bool,
        on_expanded_change: impl Fn(bool) + 'static,
        #[default(Modifier)] modifier: Modifier,
        #[default(true)] enabled: bool,
        #[default(None)] placeholder: Option<TextFieldSlot>,
        #[default(None)] leading_icon: Option<TextFieldSlot>,
        #[default(None)] trailing_icon: Option<TextFieldSlot>,
        #[default(self.input_field_colors())] colors: TextFieldColors,
        #[default(None)] interaction_source: Option<MutableInteractionSource>,
    ) {
        let query: String = query.into();
        let on_query_change = Rc::new(on_query_change);
        let on_search = Rc::new(on_search);
        let on_expanded_change = Rc::new(on_expanded_change);
        kompozable_block!({
            let interaction_source = interaction_source.unwrap_or_else(|| remember(MutableInteractionSource::new));
            let source = interaction_source.as_interaction_source();

            let focused = source.collect_is_focused_as_state().get();
            let focus_requester = remember(FocusRequester::new);
            let focus_manager = LocalFocusManager.current();

            let search_semantics = get_string(Strings::SearchBarSearch);
            let suggestions_available_semantics = get_string(Strings::SuggestionsAvailable);

            let text_style = LocalTextStyle.current();
            let text_color = text_style.color.take_or_else(|| colors.text_color(enabled, false, focused));

            let input_field_shape = SearchBarDefaults::input_field_shape();
            let decoration_query = query.clone();
            let decoration_source = source.clone();
            let decoration_box: DecorationBox = Rc::new(move |inner_text_field: &dyn Fn()| {
                named_args!(TextFieldDefaults.DecorationBox(
                    &decoration_query,
                    inner_text_field,
                    enabled,
                    true,
                    VisualTransformation::None,
                    decoration_source.clone(),
                    placeholder = placeholder.clone(),
                    leading_icon = leading_icon.clone().map(offset_icon(SearchBarIconOffsetX)),
                    trailing_icon = trailing_icon.clone().map(offset_icon(-SearchBarIconOffsetX)),
                    shape = input_field_shape.clone(),
                    colors = colors,
                    content_padding = Some(TextFieldDefaults.content_padding_without_label()),
                    container =
                        Some(
                            animated_container(decoration_source.clone(), enabled, colors, input_field_shape.clone(),)
                        ),
                ));
            });
            let search_query = query.clone();
            let on_search_action: KeyboardAction = Rc::new(move |_| on_search(search_query.clone()));
            BasicTextFieldString(
                query.clone(),
                move |it: String| on_query_change(it),
                modifier = modifier
                    .size_in(
                        min_width = SearchBarMinWidth,
                        max_width = SearchBarMaxWidth,
                        min_height = InputFieldHeight,
                    )
                    .focus_requester(&focus_requester)
                    .on_focus_changed(move |it| {
                        if it.is_focused() {
                            on_expanded_change(true);
                        }
                    })
                    .semantics(move |c| {
                        c.set_content_description(search_semantics.clone());
                        if expanded {
                            c.set_state_description(suggestions_available_semantics.clone());
                        }
                    }),
                enabled = enabled,
                single_line = true,
                max_lines = 1,
                text_style = text_style.merge(&TextStyle { color: text_color, ..TextStyle::Default }),
                cursor_brush = SolidColor(colors.cursor_color_of(false)),
                keyboard_options = KeyboardOptions { ime_action: ImeAction::Search, ..KeyboardOptions::Default },
                keyboard_actions = KeyboardActions { on_search: Some(on_search_action), ..KeyboardActions::Default },
                interaction_source = Some(interaction_source.clone()),
                decoration_box = Some(decoration_box),
            );

            let should_clear_focus = !expanded && focused;
            LaunchedEffect(expanded, async move {
                if should_clear_focus {
                    // Not strictly needed according to the motion spec, but since the animation already has a delay, this
                    // works around b/261632544.
                    delay(AnimationDelayMillis as i64).await;
                    focus_manager.clear_focus(false);
                }
            });
        });
    }
}

/// An icon slot moved `x` sideways (Kotlin's `leadingIcon?.let { leading -> { Box(Modifier.offset(x = …)) {
/// leading() } } }`).
fn offset_icon(x: Dp) -> impl Fn(TextFieldSlot) -> TextFieldSlot {
    move |icon: TextFieldSlot| -> TextFieldSlot {
        Rc::new(move || {
            let icon = icon.clone();
            named_args!(Box(modifier = named_args!(Modifier.offset(x = x)), content = move || icon()));
        })
    }
}

/// The input fields' container (their `container = { … }` lambda): `colors`' container color of the field's
/// state, animated by the theme's fast effects motion, in `shape` behind.
fn animated_container(
    source: InteractionSource,
    enabled: bool,
    colors: TextFieldColors,
    shape: Arc<dyn Shape>,
) -> TextFieldSlot {
    Rc::new(move || {
        let focused = source.collect_is_focused_as_state().get();
        let container_color = named_args!(animate_color_as_state(
            colors.container_color(enabled, false, focused),
            animation_spec = MotionSchemeKeyTokens::FastEffects.value::<Color>(),
        ));
        named_args!(Box(
            modifier = Modifier.text_field_background(ColorProducer::new(move || container_color.get()), shape.clone())
        ));
    })
}

/// `color` at `alpha` (Kotlin's `color.copy(alpha = alpha)`).
fn with_alpha(color: Color, alpha: f32) -> Color {
    named_args!(color.copy(alpha = alpha))
}

/// Kotlin's `scope.launch { searchBarState.animateToExpanded() }`.
fn launch_animate_to_expanded(scope: &kompoz_coroutines::CoroutineScope, state: &SearchBarState) {
    let state = state.clone();
    named_args!(scope.launch(async move { state.animate_to_expanded().await }));
}

// ---------------------------------------------------------------------------------------------------------------
// SearchBarColors, AppBarWithSearchColors

/// The colors of a search bar (Compose's `SearchBarColors`): its container's, the divider's between its input
/// field and its results, its input field's.
#[derive(Clone, Copy, PartialEq, Debug, KotlinProperties)]
pub struct SearchBarColors {
    pub container_color: Color,
    pub divider_color: Color,
    pub input_field_colors: TextFieldColors,
}

impl SearchBarColors {
    /// Kotlin: `SearchBarColors(containerColor: Color, dividerColor: Color, inputFieldColors: TextFieldColors)`
    pub fn new(container_color: Color, divider_color: Color, input_field_colors: TextFieldColors) -> SearchBarColors {
        SearchBarColors { container_color, divider_color, input_field_colors }
    }

    /// Compose's deprecated `SearchBarColors(containerColor, dividerColor)`: its input field's colors all unspecified.
    ///
    /// Kotlin: `SearchBarColors(containerColor: Color, dividerColor: Color)`
    #[deprecated(note = "Use overload that takes `inputFieldColors`")]
    pub fn without_input_field_colors(container_color: Color, divider_color: Color) -> SearchBarColors {
        SearchBarColors::new(container_color, divider_color, UnspecifiedTextFieldColors)
    }
}

#[named]
impl SearchBarColors {
    /// The same with what is given (Compose's `copy`).
    pub fn copy(
        &self,
        #[default(self.container_color)] container_color: Color,
        #[default(self.divider_color)] divider_color: Color,
        #[default(self.input_field_colors)] input_field_colors: TextFieldColors,
    ) -> SearchBarColors {
        SearchBarColors::new(container_color, divider_color, input_field_colors)
    }
}

/// The colors of an [`AppBarWithSearch`](crate::AppBarWithSearch) (Compose's `AppBarWithSearchColors`): its search
/// bar's, the search bar's container while content is scrolled under it (unspecified: the same), the app bar's
/// container at rest and scrolled (unspecified: the same), its navigation and action icons'.
#[derive(Clone, Copy, PartialEq, Debug, KotlinProperties)]
pub struct AppBarWithSearchColors {
    pub search_bar_colors: SearchBarColors,
    pub scrolled_search_bar_container_color: Color,
    pub app_bar_container_color: Color,
    pub scrolled_app_bar_container_color: Color,
    pub app_bar_navigation_icon_color: Color,
    pub app_bar_action_icon_color: Color,
}

impl AppBarWithSearchColors {
    /// Kotlin: `AppBarWithSearchColors(searchBarColors: SearchBarColors, scrolledSearchBarContainerColor: Color, appBarContainerColor: Color, scrolledAppBarContainerColor: Color, appBarNavigationIconColor: Color, appBarActionIconColor: Color)`
    pub fn new(
        search_bar_colors: SearchBarColors,
        scrolled_search_bar_container_color: Color,
        app_bar_container_color: Color,
        scrolled_app_bar_container_color: Color,
        app_bar_navigation_icon_color: Color,
        app_bar_action_icon_color: Color,
    ) -> AppBarWithSearchColors {
        AppBarWithSearchColors {
            search_bar_colors,
            scrolled_search_bar_container_color,
            app_bar_container_color,
            scrolled_app_bar_container_color,
            app_bar_navigation_icon_color,
            app_bar_action_icon_color,
        }
    }

    /// Colors of no scrolled forms (Compose's `AppBarWithSearchColors(searchBarColors, appBarContainerColor,
    /// appBarNavigationIconColor, appBarActionIconColor)`: both scrolled colors unspecified).
    ///
    /// Kotlin: `AppBarWithSearchColors(searchBarColors: SearchBarColors, appBarContainerColor: Color, appBarNavigationIconColor: Color, appBarActionIconColor: Color)`
    pub fn without_scrolled_colors(
        search_bar_colors: SearchBarColors,
        app_bar_container_color: Color,
        app_bar_navigation_icon_color: Color,
        app_bar_action_icon_color: Color,
    ) -> AppBarWithSearchColors {
        AppBarWithSearchColors::new(
            search_bar_colors,
            Color::Unspecified,
            app_bar_container_color,
            Color::Unspecified,
            app_bar_navigation_icon_color,
            app_bar_action_icon_color,
        )
    }

    /// The search bar's container color `color_transition_fraction` of the way scrolled (Compose's internal
    /// `searchBarContainerColor(colorTransitionFraction)`): through the fast-out-linear-in easing.
    pub(crate) fn search_bar_container_color_at(&self, color_transition_fraction: f32) -> Color {
        if self.scrolled_search_bar_container_color == Color::Unspecified {
            return self.search_bar_colors.container_color;
        }
        lerp_color(
            self.search_bar_colors.container_color,
            self.scrolled_search_bar_container_color,
            FastOutLinearInEasing.transform(color_transition_fraction),
        )
    }

    /// The app bar's container color `color_transition_fraction` of the way scrolled (Compose's internal
    /// `appBarContainerColor(colorTransitionFraction)`).
    pub(crate) fn app_bar_container_color_at(&self, color_transition_fraction: f32) -> Color {
        if self.scrolled_app_bar_container_color == Color::Unspecified {
            return self.app_bar_container_color;
        }
        lerp_color(
            self.app_bar_container_color,
            self.scrolled_app_bar_container_color,
            FastOutLinearInEasing.transform(color_transition_fraction),
        )
    }
}

/// Text field colors all unspecified (Compose's private `UnspecifiedTextFieldColors`).
const UnspecifiedTextFieldColors: TextFieldColors = TextFieldColors {
    focused_text_color: Color::Unspecified,
    unfocused_text_color: Color::Unspecified,
    disabled_text_color: Color::Unspecified,
    error_text_color: Color::Unspecified,
    focused_container_color: Color::Unspecified,
    unfocused_container_color: Color::Unspecified,
    disabled_container_color: Color::Unspecified,
    error_container_color: Color::Unspecified,
    cursor_color: Color::Unspecified,
    error_cursor_color: Color::Unspecified,
    text_selection_colors: TextSelectionColors {
        handle_color: Color::Unspecified,
        background_color: Color::Unspecified,
    },
    focused_indicator_color: Color::Unspecified,
    unfocused_indicator_color: Color::Unspecified,
    disabled_indicator_color: Color::Unspecified,
    error_indicator_color: Color::Unspecified,
    focused_leading_icon_color: Color::Unspecified,
    unfocused_leading_icon_color: Color::Unspecified,
    disabled_leading_icon_color: Color::Unspecified,
    error_leading_icon_color: Color::Unspecified,
    focused_trailing_icon_color: Color::Unspecified,
    unfocused_trailing_icon_color: Color::Unspecified,
    disabled_trailing_icon_color: Color::Unspecified,
    error_trailing_icon_color: Color::Unspecified,
    focused_label_color: Color::Unspecified,
    unfocused_label_color: Color::Unspecified,
    disabled_label_color: Color::Unspecified,
    error_label_color: Color::Unspecified,
    focused_placeholder_color: Color::Unspecified,
    unfocused_placeholder_color: Color::Unspecified,
    disabled_placeholder_color: Color::Unspecified,
    error_placeholder_color: Color::Unspecified,
    focused_supporting_text_color: Color::Unspecified,
    unfocused_supporting_text_color: Color::Unspecified,
    disabled_supporting_text_color: Color::Unspecified,
    error_supporting_text_color: Color::Unspecified,
    focused_prefix_color: Color::Unspecified,
    unfocused_prefix_color: Color::Unspecified,
    disabled_prefix_color: Color::Unspecified,
    error_prefix_color: Color::Unspecified,
    focused_suffix_color: Color::Unspecified,
    unfocused_suffix_color: Color::Unspecified,
    disabled_suffix_color: Color::Unspecified,
    error_suffix_color: Color::Unspecified,
};

pub(crate) use crate::search_bar_desktop::get_window_container_height;
