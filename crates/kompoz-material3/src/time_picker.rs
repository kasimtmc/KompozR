//! Material's time pickers, as Compose's TimePicker.kt (material3 1.5.0-alpha17) with Compose for desktop's
//! TimePicker.skiko.kt: the clock dial picker ([`TimePicker`]: the hour and the minute above or beside a dial whose
//! numbers are tapped or dragged round, AM and PM beside them on a 12-hour clock), the text input ([`TimeInput`]:
//! two fields the time is typed in), the state an app hoists to set and read the time ([`TimePickerState`],
//! [`remember_time_picker_state`]), their colors ([`TimePickerColors`], [`TimePickerDefaults`]), the layouts
//! ([`TimePickerLayoutType`]) and which of the hour and the minute is being picked ([`TimePickerSelectionMode`]).
//!
//! Compose's interface `TimePickerState` is a trait here, its states shared as `Rc<dyn TimePickerState>`; its
//! extension properties (`isPm`, `isHourInputValid`, …) are [`TimePickerStateExt`]'s. Compose's value classes are
//! newtypes with associated constants. Compose caches the scheme's default colors in the scheme
//! (`defaultTimePickerColorsCached`); KompozR's [`ColorScheme`] has no caches (see `color_scheme.rs`): they are made
//! where they are asked for.

use crate::button::{ButtonDefaults, TextButton, corner_based_shape_of};
use crate::calendar_locale::{default_locale, to_local_string_min};
use crate::color_scheme::ColorScheme;
use crate::content_color::LocalContentColor;
use crate::interactive_component_size::InteractiveComponentSizeModifiers;
use crate::internal::accessibility_service_state_provider::remember_accessibility_service_state;
use crate::internal::strings::{Strings, get_string, get_string_formatted};
use crate::material_theme::MaterialTheme;
use crate::ripples::ripple;
use crate::shapes;
use crate::surface::SelectableSurface;
use crate::text::{LocalTextStyle, Text};
use crate::text_field_defaults::OutlinedTextFieldDefaults;
use crate::time_format::is_24_hour_format;
use crate::tokens::{MotionSchemeKeyTokens, TimeInputTokens, TimePickerTokens};
use kompoz_animation::{
    Animatable, AnimationSpec, AnimationVector1D, Crossfade, MutatePriority, MutatorMutex, SnapSpec,
};
use kompoz_coroutines::{LaunchedEffect, delay_ms, remember_coroutine_scope};
use kompoz_foundation::interaction::{MutableInteractionSource, PressInteraction};
use kompoz_foundation::prelude::*;
use kompoz_foundation::text::{BasicTextFieldValue, KeyboardActions, KeyboardOptions};
use kompoz_graphics::{
    BlendMode, BorderStroke, Brush, CircleShape, Color, CornerBasedShape, CornerSize, Dp, DpOffset, IntOffset, IntSize,
    LayoutDirection, Offset, Rect, Shape, TileMode,
};
use kompoz_runtime::{
    KompozitionLocalProvider, KotlinProperties, MutableState, RememberObserver, SavedValue, Saver, derived_state_of,
    kompozable, mutable_state_of, named, named_args, remember, remember_keyed, remember_observer_keyed,
    remember_saveable, remember_saveable_with,
};
use kompoz_text::input::VisualTransformation;
use kompoz_text::style::line_height_style::{Alignment as LineHeightAlignment, Trim};
use kompoz_text::{LineHeightStyle, TextAlign, TextFieldValue, TextRange};
use kompoz_ui::focus::{FocusDirection, FocusManager, FocusRequester};
use kompoz_ui::input::{Key, KeyEvent, KeyEventType};
use kompoz_ui::input::{PointerInputScope, SuspendingPointerInputModifierNode};
use kompoz_ui::layout::{
    Layout, LayoutAwareModifierNode, LayoutModifierNode, Measurable, MeasureResult, MeasureScope, measure_policy,
};
use kompoz_ui::modifier::DelegatedNode;
use kompoz_ui::platform::{
    HapticFeedback, HapticFeedbackType, ImeAction, InputMode, KeyboardType, LocalDensity, LocalFocusManager,
    LocalHapticFeedback, LocalInputModeManager, LocalLayoutDirection, LocalWindowInfo,
};
use kompoz_ui::semantics::{LiveRegionMode, Role};
use kompoz_ui::{
    Alignment, Constraints, Modifier, ModifierNode, ModifierNodeElement, NodeCx, NodeHandle, node_capabilities,
};
use std::cell::{Cell, RefCell};
use std::fmt;
use std::future::Future;
use std::rc::Rc;
use std::sync::Arc;

// ---------------------------------------------------------------------------------------------------------------
// TimePicker, TimeInput

/// A Material time picker (Compose's experimental `TimePicker`): the time `state` holds, its hour and minute in two
/// boxes (the one being picked selected; a tap on the other picks it), on a 12-hour clock AM and PM beside them, and
/// a clock dial whose numbers are tapped or dragged round to pick the hour, then (unless an accessibility service is
/// on) the minute. `layout_type` puts the display above the dial ([`TimePickerLayoutType::Vertical`]) or beside it
/// ([`TimePickerLayoutType::Horizontal`]; by default as the window is wider than tall); `colors` color its parts.
///
/// Kotlin: `TimePicker(state: TimePickerState, modifier: Modifier = Modifier, colors: TimePickerColors = TimePickerDefaults.colors(), layoutType: TimePickerLayoutType = TimePickerDefaults.layoutType())`
#[kompozable]
pub fn TimePicker(
    state: Rc<dyn TimePickerState>,
    #[default(Modifier)] modifier: Modifier,
    #[default(TimePickerDefaults.colors())] colors: TimePickerColors,
    #[default(TimePickerDefaults.layout_type())] layout_type: TimePickerLayoutType,
) {
    let a11y_services_enabled = remember_accessibility_service_state().value();
    let user_override: UserOverride = remember(|| Rc::new(Cell::new(None)));

    let analog_state = {
        let (state, user_override) = (state.clone(), user_override.clone());
        remember_observer_keyed(state.clone(), move || AnalogTimePickerState::new(state, user_override))
    };

    {
        let (analog_state, state) = (analog_state.clone(), state.clone());
        LaunchedEffect((state.hour(), state.minute()), async move {
            if user_override.get() == Some(true) {
                analog_state.set_hour(state.hour());
                analog_state.set_minute(state.minute());
            }
            user_override.set(Some(true));
        });
    }

    if layout_type == TimePickerLayoutType::Vertical {
        VerticalTimePicker(analog_state, modifier, colors, !a11y_services_enabled);
    } else {
        HorizontalTimePicker(analog_state, modifier, colors, !a11y_services_enabled);
    }
}

/// A Material time input (Compose's experimental `TimeInput`): the time `state` holds in two text fields, the hour
/// and the minute, typed in (each field says what it takes, or what is wrong with what was typed), on a 12-hour clock
/// AM and PM beside them; `colors` color its parts.
///
/// Kotlin: `TimeInput(state: TimePickerState, modifier: Modifier = Modifier, colors: TimePickerColors = TimePickerDefaults.colors())`
#[kompozable]
pub fn TimeInput(
    state: Rc<dyn TimePickerState>,
    #[default(Modifier)] modifier: Modifier,
    #[default(TimePickerDefaults.colors())] colors: TimePickerColors,
) {
    TimeInputImpl(modifier, colors, state);
}

// ---------------------------------------------------------------------------------------------------------------
// TimePickerDefaults, TimePickerColors

/// The time pickers' defaults (Compose's experimental `TimePickerDefaults`): their colors and the layout for the
/// window. A unit value: `TimePickerDefaults.colors()`.
#[derive(Clone, Copy, Debug)]
pub struct TimePickerDefaults;

#[named]
impl TimePickerDefaults {
    /// A time picker's colors (Compose's composable `colors()` and `colors(clockDialColor, …)`: one method, what
    /// isn't given — `Color::Unspecified` — the theme's): the dial and its numbers, the selector, the container, the
    /// AM/PM toggle's border, containers and contents, and the hour and minute boxes' containers and contents,
    /// selected and not.
    #[track_caller]
    #[allow(clippy::too_many_arguments)]
    pub fn colors(
        &self,
        #[default(Color::Unspecified)] clock_dial_color: Color,
        #[default(Color::Unspecified)] clock_dial_selected_content_color: Color,
        #[default(Color::Unspecified)] clock_dial_unselected_content_color: Color,
        #[default(Color::Unspecified)] selector_color: Color,
        #[default(Color::Unspecified)] container_color: Color,
        #[default(Color::Unspecified)] period_selector_border_color: Color,
        #[default(Color::Unspecified)] period_selector_selected_container_color: Color,
        #[default(Color::Unspecified)] period_selector_unselected_container_color: Color,
        #[default(Color::Unspecified)] period_selector_selected_content_color: Color,
        #[default(Color::Unspecified)] period_selector_unselected_content_color: Color,
        #[default(Color::Unspecified)] time_selector_selected_container_color: Color,
        #[default(Color::Unspecified)] time_selector_unselected_container_color: Color,
        #[default(Color::Unspecified)] time_selector_selected_content_color: Color,
        #[default(Color::Unspecified)] time_selector_unselected_content_color: Color,
    ) -> TimePickerColors {
        named_args!(TimePickerDefaults::default_time_picker_colors(&MaterialTheme::color_scheme()).copy(
            clock_dial_color = clock_dial_color,
            clock_dial_selected_content_color = clock_dial_selected_content_color,
            clock_dial_unselected_content_color = clock_dial_unselected_content_color,
            selector_color = selector_color,
            container_color = container_color,
            period_selector_border_color = period_selector_border_color,
            period_selector_selected_container_color = period_selector_selected_container_color,
            period_selector_unselected_container_color = period_selector_unselected_container_color,
            period_selector_selected_content_color = period_selector_selected_content_color,
            period_selector_unselected_content_color = period_selector_unselected_content_color,
            time_selector_selected_container_color = time_selector_selected_container_color,
            time_selector_unselected_container_color = time_selector_unselected_container_color,
            time_selector_selected_content_color = time_selector_selected_content_color,
            time_selector_unselected_content_color = time_selector_unselected_content_color,
        ))
    }

    /// The layout for the window (Compose's composable `layoutType()`; Compose for desktop's actual): side by side
    /// ([`TimePickerLayoutType::Horizontal`]) when the window is wider than tall, else one above the other.
    #[track_caller]
    pub fn layout_type(&self) -> TimePickerLayoutType {
        default_time_picker_layout_type()
    }
}

impl TimePickerDefaults {
    /// `scheme`'s default time picker colors (Compose's internal `ColorScheme.defaultTimePickerColors`, without its
    /// cache): the tokens', the unselected period's container transparent.
    pub(crate) fn default_time_picker_colors(scheme: &ColorScheme) -> TimePickerColors {
        TimePickerColors::new(
            scheme.from_token(TimePickerTokens::ClockDialColor),
            scheme.from_token(TimePickerTokens::ClockDialSelectorHandleContainerColor),
            scheme.from_token(TimePickerTokens::ContainerColor),
            scheme.from_token(TimePickerTokens::PeriodSelectorOutlineColor),
            scheme.from_token(TimePickerTokens::ClockDialSelectedLabelTextColor),
            scheme.from_token(TimePickerTokens::ClockDialUnselectedLabelTextColor),
            scheme.from_token(TimePickerTokens::PeriodSelectorSelectedContainerColor),
            Color::Transparent,
            scheme.from_token(TimePickerTokens::PeriodSelectorSelectedLabelTextColor),
            scheme.from_token(TimePickerTokens::PeriodSelectorUnselectedLabelTextColor),
            scheme.from_token(TimePickerTokens::TimeSelectorSelectedContainerColor),
            scheme.from_token(TimePickerTokens::TimeSelectorUnselectedContainerColor),
            scheme.from_token(TimePickerTokens::TimeSelectorSelectedLabelTextColor),
            scheme.from_token(TimePickerTokens::TimeSelectorUnselectedLabelTextColor),
        )
    }
}

/// The colors of a time picker's parts (Compose's experimental `TimePickerColors`): the dial
/// (`clock_dial_color`), its numbers (`clock_dial_selected_content_color` where the selector is over them,
/// `clock_dial_unselected_content_color` elsewhere), the selector, the container, the AM/PM toggle's border,
/// containers and contents, the hour and minute boxes' containers and contents, selected and not. Made by
/// [`TimePickerDefaults`]' `colors`. Equal as Compose's `equals`: by its colors but the dial's two content colors
/// (Compose's leaves them out).
#[derive(Clone, Copy, Debug, KotlinProperties)]
pub struct TimePickerColors {
    pub clock_dial_color: Color,
    pub selector_color: Color,
    pub container_color: Color,
    pub period_selector_border_color: Color,
    pub clock_dial_selected_content_color: Color,
    pub clock_dial_unselected_content_color: Color,
    pub period_selector_selected_container_color: Color,
    pub period_selector_unselected_container_color: Color,
    pub period_selector_selected_content_color: Color,
    pub period_selector_unselected_content_color: Color,
    pub time_selector_selected_container_color: Color,
    pub time_selector_unselected_container_color: Color,
    pub time_selector_selected_content_color: Color,
    pub time_selector_unselected_content_color: Color,
}

impl TimePickerColors {
    /// Kotlin: `TimePickerColors(clockDialColor: Color, selectorColor: Color, containerColor: Color, periodSelectorBorderColor: Color, clockDialSelectedContentColor: Color, clockDialUnselectedContentColor: Color, periodSelectorSelectedContainerColor: Color, periodSelectorUnselectedContainerColor: Color, periodSelectorSelectedContentColor: Color, periodSelectorUnselectedContentColor: Color, timeSelectorSelectedContainerColor: Color, timeSelectorUnselectedContainerColor: Color, timeSelectorSelectedContentColor: Color, timeSelectorUnselectedContentColor: Color)`
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        clock_dial_color: Color,
        selector_color: Color,
        container_color: Color,
        period_selector_border_color: Color,
        clock_dial_selected_content_color: Color,
        clock_dial_unselected_content_color: Color,
        period_selector_selected_container_color: Color,
        period_selector_unselected_container_color: Color,
        period_selector_selected_content_color: Color,
        period_selector_unselected_content_color: Color,
        time_selector_selected_container_color: Color,
        time_selector_unselected_container_color: Color,
        time_selector_selected_content_color: Color,
        time_selector_unselected_content_color: Color,
    ) -> TimePickerColors {
        TimePickerColors {
            clock_dial_color,
            selector_color,
            container_color,
            period_selector_border_color,
            clock_dial_selected_content_color,
            clock_dial_unselected_content_color,
            period_selector_selected_container_color,
            period_selector_unselected_container_color,
            period_selector_selected_content_color,
            period_selector_unselected_content_color,
            time_selector_selected_container_color,
            time_selector_unselected_container_color,
            time_selector_selected_content_color,
            time_selector_unselected_content_color,
        }
    }

    /// The period toggle's container, `selected` or not (Compose's internal `periodSelectorContainerColor`).
    pub(crate) fn period_selector_container_color(&self, selected: bool) -> Color {
        if selected {
            self.period_selector_selected_container_color
        } else {
            self.period_selector_unselected_container_color
        }
    }

    /// The period toggle's content, `selected` or not (Compose's internal `periodSelectorContentColor`).
    pub(crate) fn period_selector_content_color(&self, selected: bool) -> Color {
        if selected {
            self.period_selector_selected_content_color
        } else {
            self.period_selector_unselected_content_color
        }
    }

    /// An hour or minute box's container, `selected` or not (Compose's internal `timeSelectorContainerColor`).
    pub(crate) fn time_selector_container_color(&self, selected: bool) -> Color {
        if selected {
            self.time_selector_selected_container_color
        } else {
            self.time_selector_unselected_container_color
        }
    }

    /// An hour or minute box's content, `selected` or not (Compose's internal `timeSelectorContentColor`).
    pub(crate) fn time_selector_content_color(&self, selected: bool) -> Color {
        if selected { self.time_selector_selected_content_color } else { self.time_selector_unselected_content_color }
    }

    /// A dial number's color, under the selector or not (Compose's internal `clockDialContentColor`).
    pub(crate) fn clock_dial_content_color(&self, selected: bool) -> Color {
        if selected { self.clock_dial_selected_content_color } else { self.clock_dial_unselected_content_color }
    }
}

#[named]
impl TimePickerColors {
    /// The same colors with those given (specified ones) replaced (Compose's `copy`). As Compose's, the dial's color
    /// is by default the CONTAINER's (`clockDialColor: Color = this.containerColor`): `copy()` gives the dial the
    /// container's color.
    #[allow(clippy::too_many_arguments)]
    pub fn copy(
        &self,
        #[default(self.container_color)] clock_dial_color: Color,
        #[default(self.selector_color)] selector_color: Color,
        #[default(self.container_color)] container_color: Color,
        #[default(self.period_selector_border_color)] period_selector_border_color: Color,
        #[default(self.clock_dial_selected_content_color)] clock_dial_selected_content_color: Color,
        #[default(self.clock_dial_unselected_content_color)] clock_dial_unselected_content_color: Color,
        #[default(self.period_selector_selected_container_color)] period_selector_selected_container_color: Color,
        #[default(self.period_selector_unselected_container_color)] period_selector_unselected_container_color: Color,
        #[default(self.period_selector_selected_content_color)] period_selector_selected_content_color: Color,
        #[default(self.period_selector_unselected_content_color)] period_selector_unselected_content_color: Color,
        #[default(self.time_selector_selected_container_color)] time_selector_selected_container_color: Color,
        #[default(self.time_selector_unselected_container_color)] time_selector_unselected_container_color: Color,
        #[default(self.time_selector_selected_content_color)] time_selector_selected_content_color: Color,
        #[default(self.time_selector_unselected_content_color)] time_selector_unselected_content_color: Color,
    ) -> TimePickerColors {
        TimePickerColors::new(
            clock_dial_color.take_or_else(|| self.clock_dial_color),
            selector_color.take_or_else(|| self.selector_color),
            container_color.take_or_else(|| self.container_color),
            period_selector_border_color.take_or_else(|| self.period_selector_border_color),
            clock_dial_selected_content_color.take_or_else(|| self.clock_dial_selected_content_color),
            clock_dial_unselected_content_color.take_or_else(|| self.clock_dial_unselected_content_color),
            period_selector_selected_container_color.take_or_else(|| self.period_selector_selected_container_color),
            period_selector_unselected_container_color.take_or_else(|| self.period_selector_unselected_container_color),
            period_selector_selected_content_color.take_or_else(|| self.period_selector_selected_content_color),
            period_selector_unselected_content_color.take_or_else(|| self.period_selector_unselected_content_color),
            time_selector_selected_container_color.take_or_else(|| self.time_selector_selected_container_color),
            time_selector_unselected_container_color.take_or_else(|| self.time_selector_unselected_container_color),
            time_selector_selected_content_color.take_or_else(|| self.time_selector_selected_content_color),
            time_selector_unselected_content_color.take_or_else(|| self.time_selector_unselected_content_color),
        )
    }
}

/// Compose's `equals`: every color but `clock_dial_selected_content_color` and `clock_dial_unselected_content_color`,
/// which Compose's leaves out.
impl PartialEq for TimePickerColors {
    fn eq(&self, other: &Self) -> bool {
        self.clock_dial_color == other.clock_dial_color
            && self.selector_color == other.selector_color
            && self.container_color == other.container_color
            && self.period_selector_border_color == other.period_selector_border_color
            && self.period_selector_selected_container_color == other.period_selector_selected_container_color
            && self.period_selector_unselected_container_color == other.period_selector_unselected_container_color
            && self.period_selector_selected_content_color == other.period_selector_selected_content_color
            && self.period_selector_unselected_content_color == other.period_selector_unselected_content_color
            && self.time_selector_selected_container_color == other.time_selector_selected_container_color
            && self.time_selector_unselected_container_color == other.time_selector_unselected_container_color
            && self.time_selector_selected_content_color == other.time_selector_selected_content_color
            && self.time_selector_unselected_content_color == other.time_selector_unselected_content_color
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The state

/// The time picker state, `initial_hour`:`initial_minute` at first on a 24-hour clock or not (by default as the
/// platform's locale writes times), saved and restored (Compose's experimental `rememberTimePickerState`). Panics
/// (Compose's `require`) unless the hour is 0..=23 and the minute 0..=59.
///
/// Kotlin: `rememberTimePickerState(initialHour: Int = 0, initialMinute: Int = 0, is24Hour: Boolean = is24HourFormat): TimePickerState`
#[kompozable]
pub fn remember_time_picker_state(
    #[default(0)] initial_hour: i32,
    #[default(0)] initial_minute: i32,
    #[default(is_24_hour_format())] is_24_hour: bool,
) -> Rc<dyn TimePickerState> {
    let saver = TimePickerStateImpl::Saver();
    remember_saveable_with(&saver, move || Rc::new(TimePickerStateImpl::new(initial_hour, initial_minute, is_24_hour)))
        as Rc<dyn TimePickerState>
}

/// How a time picker is laid out (Compose's experimental value class `TimePickerLayoutType`): the hour and minute
/// beside the dial ([`Horizontal`](Self::Horizontal)) or above it ([`Vertical`](Self::Vertical)).
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct TimePickerLayoutType(i32);

impl TimePickerLayoutType {
    /// The display beside the dial.
    pub const Horizontal: TimePickerLayoutType = TimePickerLayoutType(0);
    /// The display above the dial.
    pub const Vertical: TimePickerLayoutType = TimePickerLayoutType(1);
}

/// Compose's toString: the layout's name.
impl fmt::Display for TimePickerLayoutType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(match *self {
            TimePickerLayoutType::Horizontal => "Horizontal",
            TimePickerLayoutType::Vertical => "Vertical",
            _ => "Unknown",
        })
    }
}

impl fmt::Debug for TimePickerLayoutType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

/// The largest hour (Compose's `MaxHourValue`).
const MaxHourValue: i32 = 23;
/// The largest minute (Compose's `MaxMinuteValue`).
const MaxMinuteValue: i32 = 59;

/// The state of a [`TimePicker`] or [`TimeInput`] (Compose's experimental interface `TimePickerState`): the time
/// picked (`hour` 0..=23, `minute` 0..=59), what is typed for them (`hour_input`, `minute_input`: what a time input's
/// fields hold, which may be out of range), whether it is a 24-hour clock, and which of the two is being picked.
/// [`remember_time_picker_state`] and [`TimePickerState()`](fn@TimePickerState) make the default one; shared as
/// `Rc<dyn TimePickerState>`, equal by identity.
pub trait TimePickerState {
    /// The minute picked, 0..=59 (Compose's `minute`).
    fn minute(&self) -> i32;
    /// Sets the minute (Compose's `minute =`).
    fn set_minute(&self, value: i32);

    /// The hour picked, 0..=23 (Compose's `hour`).
    fn hour(&self) -> i32;
    /// Sets the hour (Compose's `hour =`).
    fn set_hour(&self, value: i32);

    /// The hour typed (Compose's `hourInput`): by default the hour.
    fn hour_input(&self) -> i32 {
        self.hour()
    }
    /// Sets the hour typed (Compose's `hourInput =`): by default the hour, if it is one (0..=23).
    fn set_hour_input(&self, value: i32) {
        if (0..=MaxHourValue).contains(&value) {
            self.set_hour(value);
        }
    }

    /// The minute typed (Compose's `minuteInput`): by default the minute.
    fn minute_input(&self) -> i32 {
        self.minute()
    }
    /// Sets the minute typed (Compose's `minuteInput =`): by default the minute, if it is one (0..=59).
    fn set_minute_input(&self, value: i32) {
        if (0..=MaxMinuteValue).contains(&value) {
            self.set_minute(value);
        }
    }

    /// Whether it is a 24-hour clock (Compose's `is24hour`).
    fn is24hour(&self) -> bool;
    /// Sets whether it is a 24-hour clock (Compose's `is24hour =`).
    fn set_is24hour(&self, value: bool);

    /// Which of the hour and the minute is being picked (Compose's `selection`).
    fn selection(&self) -> TimePickerSelectionMode;
    /// Picks the hour or the minute (Compose's `selection =`).
    fn set_selection(&self, value: TimePickerSelectionMode);
}

impl PartialEq for dyn TimePickerState {
    /// The same state (Kotlin's `==` of an interface's instances without `equals`: their identity).
    fn eq(&self, other: &Self) -> bool {
        std::ptr::addr_eq(self, other)
    }
}

/// Compose's extension properties of `TimePickerState`: [`is_pm`](Self::is_pm), [`is_hour_input_valid`]
/// (Self::is_hour_input_valid), [`is_minute_input_valid`](Self::is_minute_input_valid), [`is_input_valid`]
/// (Self::is_input_valid).
pub trait TimePickerStateExt: TimePickerState {
    /// Whether the hour is past noon, 12..=23 (Compose's `isPm`).
    fn is_pm(&self) -> bool {
        self.hour() >= 12
    }

    /// Whether the hour typed is an hour, 0..=23 (Compose's `isHourInputValid`).
    fn is_hour_input_valid(&self) -> bool {
        (0..=MaxHourValue).contains(&self.hour_input())
    }

    /// Whether the minute typed is a minute, 0..=59 (Compose's `isMinuteInputValid`).
    fn is_minute_input_valid(&self) -> bool {
        (0..=MaxMinuteValue).contains(&self.minute_input())
    }

    /// Whether both are (Compose's `isInputValid`).
    fn is_input_valid(&self) -> bool {
        self.is_minute_input_valid() && self.is_hour_input_valid()
    }
}

impl<S: TimePickerState + ?Sized> TimePickerStateExt for S {}

/// A time picker state, `initial_hour`:`initial_minute` on a 24-hour clock or not, not remembered (Compose's
/// experimental factory `TimePickerState(initialHour, initialMinute, is24Hour)`). Panics (Compose's `require`)
/// unless the hour is 0..=23 and the minute 0..=59.
///
/// Kotlin: `TimePickerState(initialHour: Int, initialMinute: Int, is24Hour: Boolean): TimePickerState`
pub fn TimePickerState(initial_hour: i32, initial_minute: i32, is_24_hour: bool) -> Rc<dyn TimePickerState> {
    Rc::new(TimePickerStateImpl::new(initial_hour, initial_minute, is_24_hour))
}

/// Which of the hour and the minute a time picker is picking (Compose's experimental value class
/// `TimePickerSelectionMode`).
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct TimePickerSelectionMode(i32);

impl TimePickerSelectionMode {
    /// The hour.
    pub const Hour: TimePickerSelectionMode = TimePickerSelectionMode(0);
    /// The minute.
    pub const Minute: TimePickerSelectionMode = TimePickerSelectionMode(1);

    /// Its value (Compose's `value`): 0 for the hour, 1 for the minute.
    pub fn value(&self) -> i32 {
        self.0
    }
}

/// Compose's toString: the mode's name.
impl fmt::Display for TimePickerSelectionMode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(match *self {
            TimePickerSelectionMode::Hour => "Hour",
            TimePickerSelectionMode::Minute => "Minute",
            _ => "",
        })
    }
}

impl fmt::Debug for TimePickerSelectionMode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

/// The default state (Compose's private `TimePickerStateImpl`): snapshot states of the hour and minute and of what is
/// typed for them (setting the time sets those too), of the selection; whether it is a 24-hour clock is a plain
/// value (Compose's `var`).
struct TimePickerStateImpl {
    is24hour: Cell<bool>,
    selection: MutableState<TimePickerSelectionMode>,
    hour_state: MutableState<i32>,
    minute_state: MutableState<i32>,
    hour_input_state: MutableState<i32>,
    minute_input_state: MutableState<i32>,
}

impl TimePickerStateImpl {
    fn new(initial_hour: i32, initial_minute: i32, is_24_hour: bool) -> TimePickerStateImpl {
        assert!((0..=MaxHourValue).contains(&initial_hour), "initialHour should in [0..23] range");
        assert!((0..=MaxMinuteValue).contains(&initial_minute), "initialMinute should be in [0..59] range");
        TimePickerStateImpl {
            is24hour: Cell::new(is_24_hour),
            selection: mutable_state_of(TimePickerSelectionMode::Hour),
            hour_state: mutable_state_of(initial_hour),
            minute_state: mutable_state_of(initial_minute),
            hour_input_state: mutable_state_of(initial_hour),
            minute_input_state: mutable_state_of(initial_minute),
        }
    }

    /// Saves the hour, the minute and whether it is a 24-hour clock (Compose's companion's `Saver()`).
    fn Saver() -> Saver<Rc<TimePickerStateImpl>> {
        Saver::new(
            |it: &Rc<TimePickerStateImpl>| {
                SavedValue::list([
                    SavedValue::Int(it.hour() as i64),
                    SavedValue::Int(it.minute() as i64),
                    SavedValue::Bool(it.is24hour()),
                ])
            },
            |value| {
                let SavedValue::List(values) = value else { return None };
                let int = |v: &SavedValue| match v {
                    SavedValue::Int(i) => i32::try_from(*i).ok(),
                    _ => None,
                };
                let (hour, minute) = (int(values.first()?)?, int(values.get(1)?)?);
                let SavedValue::Bool(is_24_hour) = values.get(2)? else { return None };
                Some(Rc::new(TimePickerStateImpl::new(hour, minute, *is_24_hour)))
            },
        )
    }
}

impl TimePickerState for TimePickerStateImpl {
    fn minute(&self) -> i32 {
        self.minute_state.get()
    }
    fn set_minute(&self, value: i32) {
        self.minute_state.set(value);
        self.minute_input_state.set(value);
    }

    fn hour(&self) -> i32 {
        self.hour_state.get()
    }
    fn set_hour(&self, value: i32) {
        self.hour_state.set(value);
        self.hour_input_state.set(value);
    }

    fn hour_input(&self) -> i32 {
        self.hour_input_state.get()
    }
    fn set_hour_input(&self, value: i32) {
        // (Compose's `super.hourInput = value`: the interface's setter)
        if (0..=MaxHourValue).contains(&value) {
            self.set_hour(value);
        }
        self.hour_input_state.set(value);
    }

    fn minute_input(&self) -> i32 {
        self.minute_input_state.get()
    }
    fn set_minute_input(&self, value: i32) {
        // (Compose's `super.minuteInput = value`: the interface's setter)
        if (0..=MaxMinuteValue).contains(&value) {
            self.set_minute(value);
        }
        self.minute_input_state.set(value);
    }

    fn is24hour(&self) -> bool {
        self.is24hour.get()
    }
    fn set_is24hour(&self, value: bool) {
        self.is24hour.set(value);
    }

    fn selection(&self) -> TimePickerSelectionMode {
        self.selection.get()
    }
    fn set_selection(&self, value: TimePickerSelectionMode) {
        self.selection.set(value);
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The dial's state

/// Compose's `Ref<Boolean>` of whether the time was changed by the user (the picker's own changes set it false).
type UserOverride = Rc<Cell<Option<bool>>>;

/// The state of the dial (Compose's internal `AnalogTimePickerState`): `state` itself (Kotlin's `TimePickerState by
/// state`) but for what is typed, which turns the dial too; the dial's diameter; the hour's and the minute's angles
/// and the selector's animated angle (radians, 0 at 3 o'clock, clockwise).
pub(crate) struct AnalogTimePickerState {
    pub(crate) state: Rc<dyn TimePickerState>,
    pub(crate) user_override: UserOverride,
    current_diameter: MutableState<Dp>,
    hour_angle: Cell<f32>,
    minute_angle: Cell<f32>,
    /// (Compose's `private var anim`: a plain variable, replaced as the time is typed)
    anim: RefCell<Animatable<f32, AnimationVector1D>>,
    mutex: MutatorMutex,
}

impl AnalogTimePickerState {
    pub(crate) fn new(state: Rc<dyn TimePickerState>, user_override: UserOverride) -> AnalogTimePickerState {
        let hour_angle = RadiansPerHour * (state.hour() % 12) as f32 - FullCircle / 4.0;
        let minute_angle = RadiansPerMinute * state.minute() as f32 - FullCircle / 4.0;
        AnalogTimePickerState {
            state,
            user_override,
            current_diameter: mutable_state_of(Dp(0.0)),
            hour_angle: Cell::new(hour_angle),
            minute_angle: Cell::new(minute_angle),
            anim: RefCell::new(Animatable(hour_angle)),
            mutex: MutatorMutex::new(),
        }
    }

    /// The dial's diameter as last measured (Compose's `currentDiameter`).
    pub(crate) fn current_diameter(&self) -> Dp {
        self.current_diameter.get()
    }

    pub(crate) fn set_current_diameter(&self, value: Dp) {
        self.current_diameter.set(value);
    }

    /// The selector's angle (Compose's `currentAngle`).
    pub(crate) fn current_angle(&self) -> f32 {
        self.anim.borrow().value()
    }

    fn anim(&self) -> Animatable<f32, AnimationVector1D> {
        self.anim.borrow().clone()
    }

    /// Turns the selector to the picked hour or minute along `animation_spec`, if it isn't there (Compose's
    /// `animateToCurrent`).
    pub(crate) async fn animate_to_current(self: Rc<Self>, animation_spec: Arc<dyn AnimationSpec<f32>>) {
        if !self.is_updated() {
            return;
        }
        let end = if self.selection() == TimePickerSelectionMode::Hour {
            self.end_value_for_animation(self.hour_angle.get())
        } else {
            self.end_value_for_animation(self.minute_angle.get())
        };
        let anim = self.anim();
        let mutation = async move {
            named_args!(anim.animate_to(end, animation_spec = animation_spec)).await;
        };
        named_args!(self.mutex.mutate(MutatePriority::PreventUserInput, mutation)).await;
    }

    /// Whether the selector isn't headed for what is picked (Compose's private `isUpdated`).
    fn is_updated(&self) -> bool {
        let target = normalize(self.anim().target_value());
        if self.selection() == TimePickerSelectionMode::Hour && target == normalize(self.hour_angle.get()) {
            return false;
        }
        if self.selection() == TimePickerSelectionMode::Minute && target == normalize(self.minute_angle.get()) {
            return false;
        }
        true
    }

    /// The numbers on the dial (Compose's `clockFaceValues`): the minutes in fives or the hours.
    pub(crate) fn clock_face_values(&self) -> &'static [i32] {
        if self.selection() == TimePickerSelectionMode::Minute { &Minutes } else { &Hours }
    }

    /// The angle to animate to for `new`, the shorter way round from where the selector is (Compose's private
    /// `endValueForAnimation`).
    fn end_value_for_animation(&self, new: f32) -> f32 {
        let value = self.anim().value();
        // The angular difference, between -π and π.
        let mut diff = value - new;
        while diff > HalfCircle {
            diff -= FullCircle;
        }
        while diff <= -HalfCircle {
            diff += FullCircle;
        }
        value - diff
    }

    /// Takes the selector to what is picked along `animation_spec` as a gesture ends (Compose's `onGestureEnd`).
    pub(crate) async fn on_gesture_end(self: Rc<Self>, animation_spec: Arc<dyn AnimationSpec<f32>>) {
        let end = self.end_value_for_animation(if self.selection() == TimePickerSelectionMode::Hour {
            self.hour_angle.get()
        } else {
            self.minute_angle.get()
        });
        let anim = self.anim();
        let mutation = async move {
            named_args!(anim.animate_to(end, animation_spec = animation_spec)).await;
        };
        named_args!(self.mutex.mutate(MutatePriority::PreventUserInput, mutation)).await;
    }

    /// Picks the hour or the minute at `angle` (radians from 12 o'clock, clockwise) and turns the selector there, at
    /// once or (`animate`) along `animation_spec` (Compose's `rotateTo`); the user's doing.
    pub(crate) async fn rotate_to(
        self: Rc<Self>,
        angle: f32,
        animation_spec: Arc<dyn AnimationSpec<f32>>,
        animate: bool,
    ) {
        self.user_override.set(Some(false));
        let this = self.clone();
        let mutation = async move {
            if this.selection() == TimePickerSelectionMode::Hour {
                this.hour_angle.set((to_hour(angle) % 12) as f32 * RadiansPerHour);
                this.state.set_hour_input(to_hour(this.hour_angle.get()) % 12 + if this.is_pm() { 12 } else { 0 });
            } else {
                this.minute_angle.set(to_minute(angle) as f32 * RadiansPerMinute);
                this.state.set_minute_input(to_minute(this.minute_angle.get()));
            }

            let anim = this.anim();
            if !animate {
                anim.snap_to(offset_angle(angle)).await;
            } else {
                let end_angle = this.end_value_for_animation(offset_angle(angle));
                named_args!(anim.animate_to(end_angle, animation_spec = animation_spec)).await;
            }
        };
        named_args!(self.mutex.mutate(MutatePriority::UserInput, mutation)).await;
    }
}

/// `TimePickerState by state`, but for what is typed: typing a time turns the dial to it.
impl TimePickerState for AnalogTimePickerState {
    fn minute(&self) -> i32 {
        self.state.minute()
    }
    fn set_minute(&self, value: i32) {
        self.state.set_minute(value);
    }
    fn hour(&self) -> i32 {
        self.state.hour()
    }
    fn set_hour(&self, value: i32) {
        self.state.set_hour(value);
    }

    /// The minute (Compose's override: the state's minute, not what it holds typed).
    fn minute_input(&self) -> i32 {
        self.state.minute()
    }
    /// Sets the minute and its angle; the selector there at once if the minute is being picked.
    fn set_minute_input(&self, value: i32) {
        self.minute_angle.set(RadiansPerMinute * value as f32 - FullCircle / 4.0);
        self.state.set_minute(value);
        if self.selection() == TimePickerSelectionMode::Minute {
            *self.anim.borrow_mut() = Animatable(self.minute_angle.get());
        }
    }

    /// The hour (Compose's override: the state's hour, not what it holds typed).
    fn hour_input(&self) -> i32 {
        self.state.hour()
    }
    /// Sets the hour and its angle; the selector there at once if the hour is being picked.
    fn set_hour_input(&self, value: i32) {
        self.hour_angle.set(RadiansPerHour * (value % 12) as f32 - FullCircle / 4.0);
        self.state.set_hour(value);
        if self.selection() == TimePickerSelectionMode::Hour {
            *self.anim.borrow_mut() = Animatable(self.hour_angle.get());
        }
    }

    fn is24hour(&self) -> bool {
        self.state.is24hour()
    }
    fn set_is24hour(&self, value: bool) {
        self.state.set_is24hour(value);
    }
    fn selection(&self) -> TimePickerSelectionMode {
        self.state.selection()
    }
    fn set_selection(&self, value: TimePickerSelectionMode) {
        self.state.set_selection(value);
    }
}

/// As it enters the composition, the angles of the state's time and the selector at the one being picked.
impl RememberObserver for AnalogTimePickerState {
    fn on_remembered(&self) {
        self.hour_angle.set(RadiansPerHour * (self.state.hour() % 12) as f32 - FullCircle / 4.0);
        self.minute_angle.set(RadiansPerMinute * self.state.minute() as f32 - FullCircle / 4.0);
        *self.anim.borrow_mut() = Animatable(if self.state.selection() == TimePickerSelectionMode::Hour {
            self.hour_angle.get()
        } else {
            self.minute_angle.get()
        });
    }
}

/// `angle` in 0..2π (Compose's private `Float.normalize`, in doubles as Kotlin's `% (2 * PI)` is).
fn normalize(angle: f32) -> f32 {
    let mut normalized_angle = angle as f64 % (2.0 * std::f64::consts::PI);
    if normalized_angle < 0.0 {
        normalized_angle += 2.0 * std::f64::consts::PI;
    }
    normalized_angle as f32
}

/// The hour (0..=11) an angle from 3 o'clock points at (Compose's private `Float.toHour`).
fn to_hour(angle: f32) -> i32 {
    let hour_offset: f32 = RadiansPerHour / 2.0;
    let total_offset = hour_offset as f64 + QuarterCircle;
    ((angle as f64 + total_offset) / RadiansPerHour as f64) as i32 % 12
}

/// The minute (0..=59) an angle from 3 o'clock points at (Compose's private `Float.toMinute`).
fn to_minute(angle: f32) -> i32 {
    let minute_offset: f32 = RadiansPerMinute / 2.0;
    let total_offset = minute_offset as f64 + QuarterCircle;
    ((angle as f64 + total_offset) / RadiansPerMinute as f64) as i32 % 60
}

/// An angle from 12 o'clock as one from 3 o'clock, in 0..2π (Compose's private `offsetAngle`).
fn offset_angle(angle: f32) -> f32 {
    let ret = angle + QuarterCircle as f32;
    if ret < 0.0 { ret + FullCircle } else { ret }
}

/// The hour shown (Compose's internal `TimePickerState.hourForDisplay`): 0..=23 on a 24-hour clock, else 1..=12.
pub(crate) fn hour_for_display(state: &dyn TimePickerState) -> i32 {
    if state.is24hour() {
        state.hour() % 24
    } else if state.hour() % 12 == 0 {
        12
    } else if state.is_pm() {
        state.hour() - 12
    } else {
        state.hour()
    }
}

/// On a 24-hour clock's hours, the ring at (`x`, `y`) picks the hour's half of the day: the inner ring (nearer
/// `center` than `max_dist`) the afternoon's (Compose's private `TimePickerState.moveSelector`).
fn move_selector(state: &dyn TimePickerState, x: f32, y: f32, max_dist: f32, center: IntOffset) {
    if state.selection() == TimePickerSelectionMode::Hour && state.is24hour() {
        let current_dist = dist(x, y, center.x, center.y);
        if state.is_pm() {
            state.set_hour(state.hour() - if current_dist >= max_dist { 12 } else { 0 });
        } else {
            state.set_hour(state.hour() + if current_dist < max_dist { 12 } else { 0 });
        }
    }
}

/// A tap on the dial at (`x`, `y`) (Compose's private `AnalogTimePickerState.onTap`): picks the nearest hour or
/// five minutes, turns the selector there along `animation_spec`, and with `auto_switch_to_minute` moves on to the
/// minutes (from the hours a tenth of a second later).
async fn on_tap(
    state: Rc<AnalogTimePickerState>,
    x: f32,
    y: f32,
    max_dist: f32,
    auto_switch_to_minute: bool,
    center: IntOffset,
    animation_spec: Arc<dyn AnimationSpec<f32>>,
) {
    let mut angle = atan(y - center.y as f32, x - center.x as f32);
    if state.selection() == TimePickerSelectionMode::Minute {
        angle = (angle / RadiansPerMinute / 5.0).round_ties_even() * 5.0 * RadiansPerMinute;
    } else {
        angle = (angle / RadiansPerHour).round_ties_even() * RadiansPerHour;
    }

    move_selector(&*state, x, y, max_dist, center);
    state.clone().rotate_to(angle, animation_spec, true).await;

    if state.selection() == TimePickerSelectionMode::Hour && auto_switch_to_minute {
        delay_ms(100).await;
    }

    if auto_switch_to_minute {
        state.set_selection(TimePickerSelectionMode::Minute);
    }
}

/// Where the selector's middle is on the dial (Compose's internal `AnalogTimePickerState.selectorPos`): on the
/// outer ring, or the inner one for a 24-hour clock's afternoon hours.
pub(crate) fn selector_pos(state: &AnalogTimePickerState) -> DpOffset {
    let current_diameter = state.current_diameter();
    let scale: f32 = current_diameter / TimePickerTokens::ClockDialContainerSize;
    let handle_radius_dp = (TimePickerTokens::ClockDialSelectorHandleContainerSize / 2.0) * scale;
    let ring = if state.is24hour() && state.is_pm() && state.selection() == TimePickerSelectionMode::Hour {
        current_diameter * InnerCircleToSizeRatio
    } else {
        current_diameter * OuterCircleToSizeRatio
    };
    let selector_length = (ring - handle_radius_dp).coerce_at_least(Dp(0.0));

    let length = selector_length + handle_radius_dp;
    let current_angle = state.current_angle();
    let offset_x = length * cosf(current_angle) + current_diameter / 2.0;
    let offset_y = length * sinf(current_angle) + current_diameter / 2.0;

    DpOffset::new(offset_x, offset_y)
}

// ---------------------------------------------------------------------------------------------------------------
// The layouts

/// The display above the dial (Compose's internal `VerticalTimePicker`), a traversal group.
#[kompozable]
pub(crate) fn VerticalTimePicker(
    state: Rc<AnalogTimePickerState>,
    modifier: Modifier,
    colors: TimePickerColors,
    auto_switch_to_minute: bool,
) {
    let display_state: Rc<dyn TimePickerState> = state.clone();
    Column(
        modifier = modifier.semantics(|s| s.set_is_traversal_group(true)),
        horizontal_alignment = Alignment::CenterHorizontally,
        content = move || {
            VerticalClockDisplay(display_state, colors);
            Spacer(Modifier.height(ClockDisplayBottomMargin));
            ClockFace(Modifier.size(TimePickerTokens::ClockDialContainerSize), state, colors, auto_switch_to_minute);
            Spacer(Modifier.height(ClockFaceBottomMargin));
        },
    );
}

/// The display beside the dial (Compose's internal `HorizontalTimePicker`), a traversal group; the dial as large as
/// the height lets it be ([`ClockFaceSizeModifier`]).
#[kompozable]
pub(crate) fn HorizontalTimePicker(
    state: Rc<AnalogTimePickerState>,
    modifier: Modifier,
    colors: TimePickerColors,
    auto_switch_to_minute: bool,
) {
    let display_state: Rc<dyn TimePickerState> = state.clone();
    Row(
        modifier = modifier.semantics(|s| s.set_is_traversal_group(true)),
        vertical_alignment = Alignment::CenterVertically,
        content = move || {
            HorizontalClockDisplay(display_state, colors);
            Spacer(Modifier.width(ClockDisplayBottomMargin));
            ClockFace(Modifier.then_element(ClockFaceSizeModifier), state, colors, auto_switch_to_minute);
        },
    );
}

/// The time input (Compose's private `TimeInputImpl`): the hour and minute fields left to right whatever the layout
/// direction, a colon between them, on a 12-hour clock the AM/PM toggle beside them. What the fields hold is
/// remembered (saved) apart from the state: set again from it when the state's time changes not by typing.
#[kompozable]
fn TimeInputImpl(modifier: Modifier, colors: TimePickerColors, state: Rc<dyn TimePickerState>) {
    let locale = default_locale();
    let hour_text_value: Rc<dyn Fn() -> TextFieldValue> = {
        let (state, locale) = (state.clone(), locale.clone());
        Rc::new(move || {
            let hour = if state.is_hour_input_valid() { hour_for_display(&*state) } else { state.hour_input() };
            TextFieldValue::from_text(&to_local_string_min(hour, 2, &locale), TextRange::Zero, None)
        })
    };
    let minute_text_value: Rc<dyn Fn() -> TextFieldValue> = {
        let state = state.clone();
        Rc::new(move || {
            let minute = if state.is_minute_input_valid() { state.minute() } else { state.minute_input() };
            TextFieldValue::from_text(&to_local_string_min(minute, 2, &locale), TextRange::Zero, None)
        })
    };

    let hour_value: MutableState<TextFieldValue> = {
        let hour_text_value = hour_text_value.clone();
        remember_saveable(move || mutable_state_of(hour_text_value()))
    };
    let minute_value: MutableState<TextFieldValue> = {
        let minute_text_value = minute_text_value.clone();
        remember_saveable(move || mutable_state_of(minute_text_value()))
    };

    let user_override: UserOverride = remember(|| Rc::new(Cell::new(None)));
    // This is for manual overrides of the hour field
    {
        let (user_override, hour_value) = (user_override.clone(), hour_value.clone());
        LaunchedEffect(state.hour(), async move {
            if user_override.get() == Some(true) {
                hour_value.set(hour_text_value());
            }
            user_override.set(Some(true));
        });
    }
    // This is for manual overrides of the minute field
    {
        let (user_override, minute_value) = (user_override.clone(), minute_value.clone());
        LaunchedEffect(state.minute(), async move {
            if user_override.get() == Some(true) {
                minute_value.set(minute_text_value());
            }
            user_override.set(Some(true));
        });
    }

    Row(
        modifier = modifier,
        vertical_alignment = Alignment::Top,
        content = move || {
            let text_style = named_args!(
                TimeInputTokens::TimeFieldLabelTextFont
                    .value()
                    .copy(text_align = TextAlign::Center, color = colors.time_selector_content_color(true))
            );

            let a11y_services_enabled = remember_accessibility_service_state().value();
            let error_handler = remember_time_input_error_handler(a11y_services_enabled);

            {
                let state = state.clone();
                KompozitionLocalProvider(
                    &[
                        LocalTextStyle.provides(text_style),
                        // Always display the time input text field from left to right.
                        LocalLayoutDirection.provides(LayoutDirection::Ltr),
                    ],
                    move || {
                        Row(content = move || {
                            let switch_state = state.clone();
                            let switch_value = hour_value.clone();
                            let (hour_state, hour_field, hour_override, hour_errors) =
                                (state.clone(), hour_value.clone(), user_override.clone(), error_handler.clone());
                            let next_state = state.clone();
                            TimePickerTextField(
                                Modifier.on_key_event(move |event: &KeyEvent| {
                                    // Zero == 48, Nine == 57
                                    let value = switch_value.get();
                                    let switch_focus = (48..=57).contains(&event.utf16_code_point)
                                        && utf16_index(value.text(), value.selection().start) == 2
                                        && utf16_len(value.text()) == 2;

                                    if switch_focus {
                                        switch_state.set_selection(TimePickerSelectionMode::Minute);
                                    }

                                    false
                                }),
                                hour_value.get(),
                                Rc::new(move |new_value: TextFieldValue| {
                                    let field = hour_field.clone();
                                    time_input_on_change(
                                        TimePickerSelectionMode::Hour,
                                        &*hour_state,
                                        new_value,
                                        hour_field.get(),
                                        &hour_override,
                                        a11y_services_enabled,
                                        &*hour_errors,
                                        move |it| field.set(it),
                                    );
                                }),
                                state.clone(),
                                TimePickerSelectionMode::Hour,
                                KeyboardOptions(ime_action = ImeAction::Next, keyboard_type = KeyboardType::Number),
                                KeyboardActions(
                                    on_next =
                                        Some(Rc::new(move |_: &dyn kompoz_foundation::text::KeyboardActionScope| {
                                            next_state.set_selection(TimePickerSelectionMode::Minute)
                                        })),
                                ),
                                colors,
                            );
                            DisplaySeparator(
                                Modifier.size_wh(DisplaySeparatorWidth, TimeInputTokens::PeriodSelectorContainerHeight),
                            );
                            let (minute_state, minute_field, minute_override, minute_errors) =
                                (state.clone(), minute_value.clone(), user_override.clone(), error_handler.clone());
                            let next_state = state.clone();
                            TimePickerTextField(
                                Modifier,
                                minute_value.get(),
                                Rc::new(move |new_value: TextFieldValue| {
                                    let field = minute_field.clone();
                                    time_input_on_change(
                                        TimePickerSelectionMode::Minute,
                                        &*minute_state,
                                        new_value,
                                        minute_field.get(),
                                        &minute_override,
                                        a11y_services_enabled,
                                        &*minute_errors,
                                        move |it| field.set(it),
                                    );
                                }),
                                state.clone(),
                                TimePickerSelectionMode::Minute,
                                KeyboardOptions(ime_action = ImeAction::Done, keyboard_type = KeyboardType::Number),
                                KeyboardActions(
                                    on_next =
                                        Some(Rc::new(move |_: &dyn kompoz_foundation::text::KeyboardActionScope| {
                                            next_state.set_selection(TimePickerSelectionMode::Minute)
                                        })),
                                ),
                                colors,
                            );
                        });
                    },
                );
            }

            if !state.is24hour() {
                let state = state.clone();
                Box(
                    modifier = Modifier.padding_sides(start = PeriodToggleMargin),
                    content = move || {
                        VerticalPeriodToggle(
                            Modifier.size_wh(
                                TimeInputTokens::PeriodSelectorContainerWidth,
                                TimeInputTokens::PeriodSelectorContainerHeight,
                            ),
                            state,
                            colors,
                        );
                    },
                );
            }
        },
    );
}

/// The display beside the dial (Compose's private `HorizontalClockDisplay`): the hour and minute, on a 12-hour clock
/// the AM/PM toggle under them.
#[kompozable]
fn HorizontalClockDisplay(state: Rc<dyn TimePickerState>, colors: TimePickerColors) {
    Column(
        vertical_arrangement = Arrangement::Center,
        content = move || {
            ClockDisplayNumbers(state.clone(), colors);
            if !state.is24hour() {
                Box(
                    modifier = Modifier.padding_sides(top = PeriodToggleMargin),
                    content = move || {
                        HorizontalPeriodToggle(
                            Modifier.size_wh(
                                TimePickerTokens::PeriodSelectorHorizontalContainerWidth,
                                TimePickerTokens::PeriodSelectorHorizontalContainerHeight,
                            ),
                            state,
                            colors,
                        );
                    },
                );
            }
        },
    );
}

/// The display above the dial (Compose's private `VerticalClockDisplay`): the hour and minute, on a 12-hour clock the
/// AM/PM toggle beside them.
#[kompozable]
fn VerticalClockDisplay(state: Rc<dyn TimePickerState>, colors: TimePickerColors) {
    Row(
        horizontal_arrangement = Arrangement::Center,
        content = move || {
            ClockDisplayNumbers(state.clone(), colors);
            if !state.is24hour() {
                Box(
                    modifier = Modifier.padding_sides(start = PeriodToggleMargin),
                    content = move || {
                        VerticalPeriodToggle(
                            Modifier.size_wh(
                                TimePickerTokens::PeriodSelectorVerticalContainerWidth,
                                TimePickerTokens::PeriodSelectorVerticalContainerHeight,
                            ),
                            state,
                            colors,
                        );
                    },
                );
            }
        },
    );
}

/// The hour and minute boxes, a colon between them, left to right whatever the layout direction (Compose's private
/// `ClockDisplayNumbers`).
#[kompozable]
fn ClockDisplayNumbers(state: Rc<dyn TimePickerState>, colors: TimePickerColors) {
    KompozitionLocalProvider(
        &[
            LocalTextStyle.provides(TimePickerTokens::TimeSelectorLabelTextFont.value()),
            // Always display the TimeSelectors from left to right.
            LocalLayoutDirection.provides(LayoutDirection::Ltr),
        ],
        move || {
            Row(content = move || {
                TimeSelector(
                    Modifier.size_wh(
                        TimePickerTokens::TimeSelectorContainerWidth,
                        TimePickerTokens::TimeSelectorContainerHeight,
                    ),
                    hour_for_display(&*state),
                    state.clone(),
                    TimePickerSelectionMode::Hour,
                    colors,
                    true,
                );
                DisplaySeparator(
                    Modifier.size_wh(DisplaySeparatorWidth, TimePickerTokens::PeriodSelectorVerticalContainerHeight),
                );
                TimeSelector(
                    Modifier.size_wh(
                        TimePickerTokens::TimeSelectorContainerWidth,
                        TimePickerTokens::TimeSelectorContainerHeight,
                    ),
                    state.minute(),
                    state.clone(),
                    TimePickerSelectionMode::Minute,
                    colors,
                    true,
                );
            });
        },
    );
}

/// Whether a period toggle's child is the line between its halves (Compose's `layoutId == "Spacer"`).
fn is_spacer(measurable: &dyn Measurable) -> bool {
    measurable.layout_id().is_some_and(|id| id.is(&"Spacer"))
}

/// What lays a period toggle out (Compose's `remember { MeasurePolicy { … } }` of the toggles: the same each time).
type PeriodToggleMeasurePolicy = fn(&MeasureScope, &[&dyn Measurable], Constraints) -> MeasureResult;

/// AM beside PM, each half the width, the line between them over their edge (the horizontal toggle's policy).
fn horizontal_period_toggle_measure(
    scope: &MeasureScope,
    measurables: &[&dyn Measurable],
    constraints: Constraints,
) -> MeasureResult {
    let spacer = measurables.iter().copied().find(|m| is_spacer(*m)).expect("the period toggle's spacer");
    let spacer_placeable = spacer.measure(named_args!(
        constraints.copy(min_width = 0, max_width = scope.round_to_px(TimePickerTokens::PeriodSelectorOutlineWidth))
    ));

    let items: Vec<_> = measurables
        .iter()
        .copied()
        .filter(|m| !is_spacer(*m))
        .map(|item| item.measure(named_args!(constraints.copy(min_width = 0, max_width = constraints.max_width / 2))))
        .collect();

    scope.layout(constraints.max_width, constraints.max_height, move |p| {
        p.place(&items[0], 0, 0);
        p.place(&items[1], items[0].width(), 0);
        p.place(&spacer_placeable, items[0].width() - spacer_placeable.width() / 2, 0);
    })
}

/// AM above PM, each half the height, the line between them over their edge (the vertical toggle's policy).
fn vertical_period_toggle_measure(
    scope: &MeasureScope,
    measurables: &[&dyn Measurable],
    constraints: Constraints,
) -> MeasureResult {
    let spacer = measurables.iter().copied().find(|m| is_spacer(*m)).expect("the period toggle's spacer");
    let spacer_placeable = spacer.measure(named_args!(
        constraints.copy(min_height = 0, max_height = scope.round_to_px(TimePickerTokens::PeriodSelectorOutlineWidth))
    ));

    let items: Vec<_> = measurables
        .iter()
        .copied()
        .filter(|m| !is_spacer(*m))
        .map(|item| {
            item.measure(named_args!(constraints.copy(min_height = 0, max_height = constraints.max_height / 2)))
        })
        .collect();

    scope.layout(constraints.max_width, constraints.max_height, move |p| {
        p.place(&items[0], 0, 0);
        p.place(&items[1], 0, items[0].height());
        p.place(&spacer_placeable, 0, items[0].height() - spacer_placeable.height() / 2);
    })
}

/// AM beside PM (Compose's private `HorizontalPeriodToggle`): each half the width, the line between them.
#[kompozable]
fn HorizontalPeriodToggle(modifier: Modifier, state: Rc<dyn TimePickerState>, colors: TimePickerColors) {
    let shape = period_selector_container_shape();
    PeriodToggleImpl(
        modifier,
        state,
        colors,
        horizontal_period_toggle_measure,
        Arc::new(shapes::start(&shape, CornerSize::dp(Dp(0.0)))),
        Arc::new(shapes::end(&shape, CornerSize::dp(Dp(0.0)))),
    );
}

/// AM above PM (Compose's private `VerticalPeriodToggle`): each half the height, the line between them.
#[kompozable]
fn VerticalPeriodToggle(modifier: Modifier, state: Rc<dyn TimePickerState>, colors: TimePickerColors) {
    let shape = period_selector_container_shape();
    PeriodToggleImpl(
        modifier,
        state,
        colors,
        vertical_period_toggle_measure,
        Arc::new(shapes::top(&shape, CornerSize::dp(Dp(0.0)))),
        Arc::new(shapes::bottom(&shape, CornerSize::dp(Dp(0.0)))),
    );
}

/// The period toggle's shape (Compose's `PeriodSelectorContainerShape.value as CornerBasedShape`): the theme's small
/// corners.
#[track_caller]
fn period_selector_container_shape() -> CornerBasedShape {
    corner_based_shape_of(&*TimePickerTokens::PeriodSelectorContainerShape.value())
        .expect("the period selector's shape is a CornerBasedShape")
}

/// The AM/PM toggle (Compose's private `PeriodToggleImpl`): a selectable group, a traversal group described "Select
/// AM or PM", outlined in `shape`, its halves AM (in `start_shape`) and PM (in `end_shape`) laid out by
/// `measure_policy`; a tap on the other half takes the hour to its half of the day (unless the hour typed is wrong).
#[kompozable]
fn PeriodToggleImpl(
    modifier: Modifier,
    state: Rc<dyn TimePickerState>,
    colors: TimePickerColors,
    measure_policy: PeriodToggleMeasurePolicy,
    start_shape: Arc<dyn Shape>,
    end_shape: Arc<dyn Shape>,
) {
    let border_stroke = BorderStroke(TimePickerTokens::PeriodSelectorOutlineWidth, colors.period_selector_border_color);
    let shape: Arc<dyn Shape> = Arc::new(period_selector_container_shape());
    let style = TimePickerTokens::PeriodSelectorLabelTextFont.value();
    let content_description = get_string(Strings::TimePickerPeriodToggle);
    Layout(
        modifier = modifier
            .semantics(move |s| {
                s.set_is_traversal_group(true);
                s.set_content_description(content_description.clone());
            })
            .selectable_group()
            .border_stroke(border_stroke, shape),
        measure_policy = measure_policy,
        content = move || {
            let (am_state, am_style) = (state.clone(), style.clone());
            ToggleItem(
                !state.is_pm(),
                start_shape,
                Rc::new(move || {
                    if am_state.is_pm() && am_state.is_hour_input_valid() {
                        am_state.set_hour(am_state.hour() - 12);
                    }
                }),
                colors,
                move || Text(get_string(Strings::TimePickerAM), style = am_style.clone()),
            );
            Spacer(
                Modifier
                    .layout_id("Spacer")
                    .z_index(SeparatorZIndex)
                    .fill_max_size()
                    .background(colors.period_selector_border_color),
            );
            let pm_state = state.clone();
            ToggleItem(
                state.is_pm(),
                end_shape,
                Rc::new(move || {
                    if !pm_state.is_pm() && pm_state.is_hour_input_valid() {
                        pm_state.set_hour(pm_state.hour() + 12);
                    }
                }),
                colors,
                move || Text(get_string(Strings::TimePickerPM), style = style.clone()),
            );
        },
    );
}

/// A half of the period toggle (Compose's private `ToggleItem`): a text button filling it in `shape`, in the
/// period's colors as `checked` or not, the unchecked one drawn over the checked one, selected to accessibility as
/// `checked`.
#[kompozable]
fn ToggleItem(
    checked: bool,
    shape: Arc<dyn Shape>,
    on_click: Rc<dyn Fn()>,
    colors: TimePickerColors,
    content: impl Fn() + 'static,
) {
    let content_color = colors.period_selector_content_color(checked);
    let container_color = colors.period_selector_container_color(checked);

    TextButton(
        modifier = Modifier
            .z_index(if checked { 0.0 } else { 1.0 })
            .fill_max_size()
            .semantics(move |s| s.set_selected(checked)),
        content_padding = PaddingValues::all(Dp(0.0)),
        shape = shape,
        on_click = move || on_click(),
        content = content,
        colors = ButtonDefaults.text_button_colors(content_color = content_color, container_color = container_color),
    );
}

/// The colon between the hour and the minute (Compose's private `DisplaySeparator`): centered in its box, no
/// semantics.
#[kompozable]
fn DisplaySeparator(modifier: Modifier) {
    let style = named_args!(LocalTextStyle.current().copy(
        text_align = TextAlign::Center,
        line_height_style = Some(named_args!(LineHeightStyle(LineHeightAlignment::Center, Trim::Both))),
    ));

    Box(
        modifier = modifier.clear_and_set_semantics(|_| {}),
        content_alignment = Alignment::Center,
        content = move || {
            Text(":", color = TimeInputTokens::TimeFieldSeparatorColor.value(), style = style);
        },
    );
}

/// The hour's or the minute's box (Compose's private `TimeSelector`): a selectable surface, a radio button to
/// accessibility described "Select hour" or "Select minutes", showing `value` in two digits (described as a time:
/// "9 o'clock", "30 minutes"); selected while `selection` is being picked, a tap picks it. Not `is_valid`: in the
/// error colors.
#[kompozable]
fn TimeSelector(
    modifier: Modifier,
    value: i32,
    state: Rc<dyn TimePickerState>,
    selection: TimePickerSelectionMode,
    colors: TimePickerColors,
    is_valid: bool,
) {
    // (Compose's `LaunchedEffect(isValid) { if (!isValid) {} }`, which does nothing)
    LaunchedEffect(is_valid, async move {});

    let selected = state.selection() == selection;
    let selector_content_description = get_string(if selection == TimePickerSelectionMode::Hour {
        Strings::TimePickerHourSelection
    } else {
        Strings::TimePickerMinuteSelection
    });

    let container_color = if is_valid {
        colors.time_selector_container_color(selected)
    } else {
        MaterialTheme::color_scheme().error_container
    };
    let content_color = if is_valid {
        colors.time_selector_content_color(selected)
    } else {
        MaterialTheme::color_scheme().on_error_container
    };

    let click_state = state.clone();
    SelectableSurface(
        selected,
        move || {
            if selection != click_state.selection() {
                click_state.set_selection(selection);
            }
        },
        modifier = modifier.semantics(
            merge_descendants = true,
            properties = move |s| {
                s.set_role(Role::RadioButton);
                s.set_content_description(selector_content_description.clone());
            },
        ),
        shape = TimePickerTokens::TimeSelectorContainerShape.value(),
        color = container_color,
        content = move || {
            let value_content_description = number_content_description(selection, state.is24hour(), value);
            let text = to_local_string_min(value, 2, &default_locale());

            Box(
                content_alignment = Alignment::Center,
                content = move || {
                    Text(
                        text,
                        modifier =
                            Modifier.semantics(move |s| s.set_content_description(value_content_description.clone())),
                        color = content_color,
                    );
                },
            );
        },
    );
}

// ---------------------------------------------------------------------------------------------------------------
// The dial

/// The dial's gestures (Compose's internal data class `ClockDialModifier`): equal as its parts are (the state and
/// the spec by identity: KompozR's specs are shared objects).
#[derive(Clone)]
pub(crate) struct ClockDialModifier {
    state: Rc<AnalogTimePickerState>,
    auto_switch_to_minute: bool,
    selection: TimePickerSelectionMode,
    animation_spec: Arc<dyn AnimationSpec<f32>>,
}

impl PartialEq for ClockDialModifier {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.state, &other.state)
            && self.auto_switch_to_minute == other.auto_switch_to_minute
            && self.selection == other.selection
            && Arc::ptr_eq(&self.animation_spec, &other.animation_spec)
    }
}

impl ModifierNodeElement for ClockDialModifier {
    type Node = ClockDialNode;

    fn create(&self) -> ClockDialNode {
        ClockDialNode::new(self.state.clone(), self.auto_switch_to_minute, self.selection, self.animation_spec.clone())
    }

    fn update(&self, node: &mut ClockDialNode) {
        node.update_node(self.state.clone(), self.auto_switch_to_minute, self.selection, self.animation_spec.clone());
    }

    fn name(&self) -> &'static str {
        // (Compose shows nothing in the inspector)
        "ClockDialModifier"
    }
}

/// What the dial node's gesture coroutines share with it.
struct ClockDialShared {
    state: RefCell<Rc<AnalogTimePickerState>>,
    auto_switch_to_minute: Cell<bool>,
    animation_spec: RefCell<Arc<dyn AnimationSpec<f32>>>,
    offset_x: Cell<f32>,
    offset_y: Cell<f32>,
    center: MutableState<IntOffset>,
    handle: RefCell<Option<NodeHandle>>,
}

impl ClockDialShared {
    fn state(&self) -> Rc<AnalogTimePickerState> {
        self.state.borrow().clone()
    }

    fn animation_spec(&self) -> Arc<dyn AnimationSpec<f32>> {
        self.animation_spec.borrow().clone()
    }

    fn handle(&self) -> Option<NodeHandle> {
        self.handle.borrow().clone()
    }

    /// The distance from the middle within which a 24-hour clock's hour is the inner ring's, for the dial's size
    /// (Compose's private `maxDist`).
    fn max_dist(&self) -> f32 {
        let Some(handle) = self.handle() else { return 0.0 };
        let density = handle.require_density();
        density.to_px(MaxDistance) * density.round_to_px(self.state().current_diameter()) as f32
            / density.round_to_px(TimePickerTokens::ClockDialContainerSize) as f32
    }

    /// Clears the focus (Compose's `currentValueOf(LocalFocusManager).clearFocus()`).
    fn clear_focus(&self) {
        if let Some(handle) = self.handle() {
            handle.current_value_of(&LocalFocusManager).clear_focus(false);
        }
    }

    /// Runs `future` in the node's coroutine scope (Compose's `coroutineScope.launch`).
    fn launch(&self, future: impl Future<Output = ()> + 'static) {
        if let Some(scope) = self.handle().and_then(|h| h.coroutine_scope()) {
            scope.launch(future);
        }
    }
}

/// The dial's node (Compose's internal `ClockDialNode`): taps pick the nearest number, drags turn the selector with
/// the finger (and pick the 24-hour clock's ring under it), both delegated to pointer input nodes of their own; it
/// tells the state the dial's diameter, and turns the selector to the new selection's value when it changes.
/// (Compose's node is a pointer input node itself, forwarding its events to the two; KompozR's delegated nodes take
/// their events themselves: the same two detectors get the same events.)
pub(crate) struct ClockDialNode {
    shared: Rc<ClockDialShared>,
    selection: TimePickerSelectionMode,
    pointer_input_tap_node: Option<DelegatedNode<SuspendingPointerInputModifierNode>>,
    pointer_input_drag_node: Option<DelegatedNode<SuspendingPointerInputModifierNode>>,
}

impl ClockDialNode {
    fn new(
        state: Rc<AnalogTimePickerState>,
        auto_switch_to_minute: bool,
        selection: TimePickerSelectionMode,
        animation_spec: Arc<dyn AnimationSpec<f32>>,
    ) -> ClockDialNode {
        ClockDialNode {
            shared: Rc::new(ClockDialShared {
                state: RefCell::new(state),
                auto_switch_to_minute: Cell::new(auto_switch_to_minute),
                animation_spec: RefCell::new(animation_spec),
                offset_x: Cell::new(0.0),
                offset_y: Cell::new(0.0),
                center: mutable_state_of(IntOffset::Zero),
                handle: RefCell::new(None),
            }),
            selection,
            pointer_input_tap_node: None,
            pointer_input_drag_node: None,
        }
    }

    /// The tap detector (Compose's `pointerInputTapNode`).
    fn tap_node(shared: Rc<ClockDialShared>) -> SuspendingPointerInputModifierNode {
        SuspendingPointerInputModifierNode::new(move |scope: PointerInputScope| {
            let (press, tap) = (shared.clone(), shared.clone());
            async move {
                scope
                    .detect_tap_gestures()
                    .on_press(move |_, offset: Offset| {
                        press.clear_focus();
                        press.offset_x.set(offset.x);
                        press.offset_y.set(offset.y);
                        async {}
                    })
                    .on_tap(move |offset: Offset| {
                        tap.clear_focus();
                        let shared = tap.clone();
                        tap.launch(async move {
                            let state = shared.state();
                            on_tap(
                                state,
                                offset.x,
                                offset.y,
                                shared.max_dist(),
                                shared.auto_switch_to_minute.get(),
                                shared.center.get(),
                                shared.animation_spec(),
                            )
                            .await;
                        });
                    })
                    .await
            }
        })
    }

    /// The drag detector (Compose's `pointerInputDragNode`).
    fn drag_node(shared: Rc<ClockDialShared>) -> SuspendingPointerInputModifierNode {
        SuspendingPointerInputModifierNode::new(move |scope: PointerInputScope| {
            let (drag, end) = (shared.clone(), shared.clone());
            async move {
                scope
                    .detect_drag_gestures(move |_, drag_amount: Offset| {
                        let shared = drag.clone();
                        drag.launch(async move {
                            shared.offset_x.set(shared.offset_x.get() + drag_amount.x);
                            shared.offset_y.set(shared.offset_y.get() + drag_amount.y);
                            let (offset_x, offset_y, center) =
                                (shared.offset_x.get(), shared.offset_y.get(), shared.center.get());
                            let state = shared.state();
                            state
                                .clone()
                                .rotate_to(
                                    atan(offset_y - center.y as f32, offset_x - center.x as f32),
                                    shared.animation_spec(),
                                    false,
                                )
                                .await;
                            move_selector(&*state, offset_x, offset_y, shared.max_dist(), center);
                        });
                    })
                    .on_drag_end(move || {
                        let shared = end.clone();
                        end.launch(async move {
                            let state = shared.state();
                            if shared.auto_switch_to_minute.get() {
                                state.set_selection(TimePickerSelectionMode::Minute);
                            }
                            state.on_gesture_end(shared.animation_spec()).await;
                        });
                    })
                    .await
            }
        })
    }

    /// Compose's `updateNode`: the new parts; a new selection turns the selector to its value.
    fn update_node(
        &mut self,
        state: Rc<AnalogTimePickerState>,
        auto_switch_to_minute: bool,
        selection: TimePickerSelectionMode,
        animation_spec: Arc<dyn AnimationSpec<f32>>,
    ) {
        *self.shared.state.borrow_mut() = state.clone();
        self.shared.auto_switch_to_minute.set(auto_switch_to_minute);
        *self.shared.animation_spec.borrow_mut() = animation_spec.clone();
        if self.selection != selection {
            self.selection = selection;
            self.shared.launch(state.animate_to_current(animation_spec));
        }
    }
}

impl ModifierNode for ClockDialNode {
    node_capabilities!(layout_aware);

    fn on_attach(&mut self, cx: &NodeCx) {
        *self.shared.handle.borrow_mut() = Some(cx.handle());
        // (Compose delegates as it is made; a node has its context once attached)
        if self.pointer_input_tap_node.is_none() {
            self.pointer_input_tap_node = Some(cx.delegate(ClockDialNode::tap_node(self.shared.clone())));
            self.pointer_input_drag_node = Some(cx.delegate(ClockDialNode::drag_node(self.shared.clone())));
        }
    }
}

impl LayoutAwareModifierNode for ClockDialNode {
    fn on_remeasured(&mut self, size: IntSize) {
        self.shared.center.set(size.center());
        if let Some(handle) = self.shared.handle() {
            let diameter = handle.require_density().to_dp(size.width as f32);
            self.shared.state().set_current_diameter(diameter);
        }
    }
}

/// The dial (Compose's internal `ClockFace`): a circle in the dial's color with the selector drawn over it, the
/// hours (on a 24-hour clock 1..=12 outside, 13..=23 and 00 inside) or the minutes in fives round it, crossfading
/// between the two; taps and drags on it pick (see [`ClockDialNode`]).
#[kompozable]
pub(crate) fn ClockFace(
    modifier: Modifier,
    state: Rc<AnalogTimePickerState>,
    colors: TimePickerColors,
    auto_switch_to_minute: bool,
) {
    let focus_manager = LocalFocusManager.current();
    // TODO Load the motionScheme tokens from the component tokens file
    let dial = ClockDialModifier {
        state: state.clone(),
        auto_switch_to_minute,
        selection: state.selection(),
        animation_spec: MotionSchemeKeyTokens::DefaultSpatial.value::<f32>(),
    };
    Crossfade(
        state.clock_face_values(),
        modifier = modifier
            .background(colors.clock_dial_color, Arc::new(CircleShape))
            .then_element(dial)
            .draw_selector(state.clone(), colors),
        animation_spec = MotionSchemeKeyTokens::DefaultEffects.value::<f32>(),
        content = move |screen: &'static [i32]| {
            let (state, focus_manager) = (state.clone(), focus_manager.clone());
            CircularLayout(
                Modifier.size(TimePickerTokens::ClockDialContainerSize).semantics(|s| s.selectable_group()),
                OuterCircleToSizeRatio,
                move || {
                    KompozitionLocalProvider(
                        &[LocalContentColor.provides(colors.clock_dial_content_color(false))],
                        move || {
                            for (index, value) in screen.iter().enumerate() {
                                let outer_value =
                                    if !state.is24hour() || state.selection() == TimePickerSelectionMode::Minute {
                                        *value
                                    } else {
                                        *value % 12
                                    };
                                ClockText(
                                    Modifier.semantics(move |s| s.set_traversal_index(index as f32 + 1.0)),
                                    state.clone(),
                                    outer_value,
                                    auto_switch_to_minute,
                                    focus_manager.clone(),
                                );
                            }

                            if state.selection() == TimePickerSelectionMode::Hour && state.is24hour() {
                                let (state, focus_manager) = (state.clone(), focus_manager.clone());
                                CircularLayout(
                                    Modifier
                                        .layout_id(LayoutId::InnerCircle)
                                        .size(TimePickerTokens::ClockDialContainerSize)
                                        .background(Color::Transparent, Arc::new(CircleShape)),
                                    InnerCircleToSizeRatio,
                                    move || {
                                        for (index, inner_value) in ExtraHours.iter().enumerate() {
                                            ClockText(
                                                Modifier.semantics(move |s| s.set_traversal_index(12.0 + index as f32)),
                                                state.clone(),
                                                *inner_value,
                                                auto_switch_to_minute,
                                                focus_manager.clone(),
                                            );
                                        }
                                    },
                                );
                            }
                        },
                    );
                },
            );
        },
    );
}

/// The dial's selector drawing (Compose's private `Modifier.drawSelector`).
trait DrawSelectorModifier {
    /// Draws the selector over the dial (Compose's private `Modifier.drawSelector`): the circle under it cleared, the
    /// numbers drawn, the selector's circle XOR-ed over them, its line from the middle and the middle's dot, then
    /// what the selector covers of the number in the selected content color under it all.
    fn draw_selector(self, state: Rc<AnalogTimePickerState>, colors: TimePickerColors) -> Modifier;
}

impl DrawSelectorModifier for Modifier {
    fn draw_selector(self, state: Rc<AnalogTimePickerState>, colors: TimePickerColors) -> Modifier {
        self.draw_with_content(move |scope| {
            let selector_pos = selector_pos(&state);
            let selector_offset_px = Offset::new(scope.to_px(selector_pos.x), scope.to_px(selector_pos.y));

            let selector_radius = scope.to_px(TimePickerTokens::ClockDialSelectorHandleContainerSize) / 2.0
                * scope.round_to_px(state.current_diameter()) as f32
                / scope.round_to_px(TimePickerTokens::ClockDialContainerSize) as f32;
            let selector_color = colors.selector_color;

            // clear out the selector section
            named_args!(scope.draw_circle(
                Color::Black,
                radius = selector_radius,
                center = selector_offset_px,
                blend_mode = BlendMode::Clear
            ));

            // draw the text composables
            scope.draw_content();

            // draw the selector and clear out the numbers overlapping
            named_args!(scope.draw_circle(
                selector_color,
                radius = selector_radius,
                center = selector_offset_px,
                blend_mode = BlendMode::Xor
            ));

            let stroke_width = scope.to_px(TimePickerTokens::ClockDialSelectorTrackContainerWidth);
            let current_angle = state.current_angle();
            let line_length = selector_offset_px
                - Offset::new(selector_radius * cosf(current_angle), selector_radius * sinf(current_angle));

            // draw the selector line
            let center = scope.center();
            named_args!(scope.draw_line(
                selector_color,
                center,
                line_length,
                stroke_width = stroke_width,
                blend_mode = BlendMode::SrcOver
            ));

            // draw the selector small dot
            let dot_radius = scope.to_px(TimePickerTokens::ClockDialSelectorCenterContainerSize) / 2.0;
            named_args!(scope.draw_circle(selector_color, radius = dot_radius, center = center));

            // draw the portion of the number that was overlapping
            named_args!(scope.draw_circle(
                colors.clock_dial_content_color(true),
                radius = selector_radius,
                center = selector_offset_px,
                blend_mode = BlendMode::DstOver
            ));
        })
    }
}

/// A number on the dial (Compose's private `ClockText`): 48 dp, described as a time ("9 o'clock", "13 hours", "30
/// minutes"), selected while the selector's middle is over it; a tap (or Enter or Space when focused: focusable
/// out of touch mode, its ripple 24 dp round) picks it as a tap on the dial there would, at once; the arrow keys move
/// the focus on (down, right) or back (up, left).
#[kompozable]
fn ClockText(
    modifier: Modifier,
    state: Rc<AnalogTimePickerState>,
    value: i32,
    auto_switch_to_minute: bool,
    focus_manager: FocusManager,
) {
    let style = TimePickerTokens::ClockDialLabelTextFont.value();
    let density = LocalDensity.current();
    let max_dist = density.to_px(MaxDistance);
    let center: MutableState<Offset> = remember(|| mutable_state_of(Offset::Zero));
    let parent_center: MutableState<IntOffset> = remember(|| mutable_state_of(IntOffset::Zero));
    let bounds_in_parent: MutableState<Rect> = remember(|| mutable_state_of(Rect::Zero));
    let interaction_source = remember(MutableInteractionSource::new);
    let scope = remember_coroutine_scope();
    let content_description = number_content_description(state.selection(), state.is24hour(), value);

    let text = to_local_string_min(value, 1, &default_locale());
    let selected = {
        let (state, bounds_in_parent) = (state.clone(), bounds_in_parent.clone());
        remember_keyed(StateKey(state.clone()), move || {
            derived_state_of(move || {
                let selector_pos = selector_pos(&state);
                let offset = Offset::new(density.to_px(selector_pos.x), density.to_px(selector_pos.y));
                bounds_in_parent.get().contains(offset)
            })
        })
    };

    let on_clock_text_click: Rc<dyn Fn()> = {
        let (state, center, parent_center, scope) =
            (state.clone(), center.clone(), parent_center.clone(), scope.clone());
        Rc::new(move || {
            let (state, center, parent_center) = (state.clone(), center.get(), parent_center.get());
            scope.launch(async move {
                on_tap(
                    state,
                    center.x,
                    center.y,
                    max_dist,
                    auto_switch_to_minute,
                    parent_center,
                    Arc::new(SnapSpec::<f32>::new(0)),
                )
                .await;
            });
        })
    };
    let focusable = LocalInputModeManager.current().input_mode() != InputMode::Touch;

    let positioned = (parent_center.clone(), bounds_in_parent.clone(), center.clone());
    let keys = (interaction_source.clone(), scope.clone(), center.clone(), on_clock_text_click.clone());
    let click = on_clock_text_click;
    // TODO Load the motionScheme tokens from the component tokens file
    Box(
        content_alignment = Alignment::Center,
        modifier = modifier
            .on_globally_positioned(move |it| {
                let (parent_center, bounds_in_parent, center) = &positioned;
                parent_center.set(it.parent_coordinates().map_or(IntOffset::Zero, |p| p.size().center()));
                bounds_in_parent.set(it.bounds_in_parent());
                center.set(bounds_in_parent.get().center());
            })
            .minimum_interactive_component_size()
            .size(MinimumInteractiveSize)
            .on_key_event(move |it: &KeyEvent| {
                let (interaction_source, scope, center, on_clock_text_click) = &keys;
                if it.r#type == KeyEventType::KeyDown && is_enter(it) {
                    // Emit ripple.
                    let (source, center) = (interaction_source.clone(), center.get());
                    scope.launch(async move { source.emit(PressInteraction::Press::new(center)).await });
                }
                if is_click(it) {
                    on_clock_text_click();
                    // Make sure indication is cleared.
                    let (source, center) = (interaction_source.clone(), center.get());
                    scope.launch(async move {
                        source.emit(PressInteraction::Release::new(PressInteraction::Press::new(center))).await
                    });
                    return true;
                }
                // The arrow keys navigation should follow the same flow as tabbing navigation.
                // Down/Right moves focus forward and Up/Left moves focus backwards.
                if it.r#type == KeyEventType::KeyDown {
                    if it.key == Key::DirectionDown || it.key == Key::DirectionRight {
                        focus_manager.move_focus(FocusDirection::Next);
                        return true;
                    } else if it.key == Key::DirectionUp || it.key == Key::DirectionLeft {
                        focus_manager.move_focus(FocusDirection::Previous);
                        return true;
                    }
                }

                false
            })
            .indication(interaction_source.clone(), Some(ripple(radius = MinimumInteractiveSize / 2.0)))
            .focusable(focusable, Some(interaction_source.clone()))
            .semantics(
                merge_descendants = true,
                properties = move |s| {
                    let click = click.clone();
                    s.on_click(None, move || {
                        click();
                        true
                    });
                    s.set_selected(selected.value());
                },
            ),
        content = move || {
            Text(
                text,
                modifier =
                    Modifier.clear_and_set_semantics(move |s| s.set_content_description(content_description.clone())),
                style = style,
            );
        },
    );
}

/// An analog state as a key: by identity (Compose's `remember(state)` of a class without `equals`).
#[derive(Clone)]
struct StateKey(Rc<AnalogTimePickerState>);

impl PartialEq for StateKey {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The time input's parts

/// What a time input's field does with a new `value` (Compose's private `timeInputOnChange`): a change of the
/// selection only is kept; an emptied field sets the hour to 0 (12 in a 12-hour clock's afternoon) or the minute to
/// 0; a number up to 99 (the second digit typed into a full field at its start: that digit alone) is what is typed
/// for the hour (as a 12-hour clock's half of the day says) or the minute — on a 12-hour clock an hour over 1 moves
/// on to the minute unless an accessibility service is on —, kept to its first digit if longer; a larger number is an
/// error (`error_handler`); what isn't a number is dropped.
#[allow(clippy::too_many_arguments)]
fn time_input_on_change(
    selection: TimePickerSelectionMode,
    state: &dyn TimePickerState,
    value: TextFieldValue,
    prev_value: TextFieldValue,
    user_override: &UserOverride,
    a11y_services_enabled: bool,
    error_handler: &dyn TimeInputErrorHandler,
    on_new_value: impl Fn(TextFieldValue),
) {
    user_override.set(Some(false));
    if value.text() == prev_value.text() {
        // just selection change
        on_new_value(value);
        return;
    }

    if value.text().is_empty() {
        if selection == TimePickerSelectionMode::Hour {
            state.set_hour_input(if state.is_pm() && !state.is24hour() { 12 } else { 0 });
        } else {
            state.set_minute_input(0);
        }
        on_new_value(value.with_text(""));
        return;
    }

    let text = value.text();
    let new_value = if utf16_len(text) == 3 && utf16_index(text, value.selection().start) == 1 {
        // (Kotlin's `Char.digitToInt()`: an IllegalArgumentException for what isn't a digit — an error)
        match text.chars().next().and_then(|c| c.to_digit(10)) {
            Some(digit) => digit as i32,
            None => {
                error_handler.on_error();
                return;
            }
        }
    } else {
        // (Kotlin's `String.toInt()`: a NumberFormatException for what isn't a number — dropped)
        match text.parse::<i32>() {
            Ok(number) => number,
            Err(_) => return,
        }
    };

    if new_value <= MaxValueForTextField {
        if selection == TimePickerSelectionMode::Hour {
            state.set_hour_input(if new_value == 12 && state.is_pm() {
                12
            } else if new_value == 12 && !state.is_pm() && !state.is24hour() {
                0
            } else {
                new_value + if state.is_pm() && !state.is24hour() { 12 } else { 0 }
            });
            if new_value > 1 && !state.is24hour() && !a11y_services_enabled {
                state.set_selection(TimePickerSelectionMode::Minute);
            }
        } else {
            state.set_minute_input(new_value);
        }

        on_new_value(if utf16_len(value.text()) <= 2 {
            value
        } else {
            let first: String = value.text().chars().take(1).collect();
            value.with_text(&first)
        });
    } else {
        error_handler.on_error();
    }
}

/// `text`'s length in UTF-16 units (Kotlin's `String.length`).
fn utf16_len(text: &str) -> usize {
    text.encode_utf16().count()
}

/// A byte offset into `text` as an index in UTF-16 units (Kotlin's string indices).
fn utf16_index(text: &str, byte_offset: usize) -> usize {
    text.get(..byte_offset.min(text.len())).map_or(0, utf16_len)
}

/// What a field takes, or what is wrong with what was typed (Compose's private `SupportingText`): "Hour" or
/// "Minute", or the error ("Hour must be 1–12", "Hour must be 0–23", "Minute must be 0–59") in the error color; two
/// lines, no semantics.
#[kompozable]
fn SupportingText(
    modifier: Modifier,
    selection: TimePickerSelectionMode,
    state: Rc<dyn TimePickerState>,
    is_valid: bool,
) {
    let res_id = if is_valid && selection == TimePickerSelectionMode::Hour {
        Strings::TimePickerHour
    } else if is_valid {
        Strings::TimePickerMinute
    } else if selection == TimePickerSelectionMode::Hour && state.is24hour() {
        Strings::TimePicker24HourError
    } else if selection == TimePickerSelectionMode::Hour {
        Strings::TimePickerHourError
    } else {
        Strings::TimePickerMinuteError
    };

    let text = get_string(res_id);

    let color = if !is_valid {
        MaterialTheme::color_scheme().error
    } else {
        TimeInputTokens::TimeFieldSupportingTextColor.value()
    };

    Text(
        text,
        modifier = modifier.padding_sides(top = SupportLabelTop).clear_and_set_semantics(|_| {}),
        color = color,
        min_lines = 2,
        style = TimeInputTokens::TimeFieldSupportingTextFont.value(),
    );
}

/// A time input's field (Compose's private `TimePickerTextField`): while `selection` isn't being picked, its value's
/// box ([`TimeSelector`]: a tap picks it); while it is, an outlined text field of two digits taking the focus,
/// described "for hour" or "for minutes", in the error colors while what is typed is wrong; under it what it takes
/// ([`SupportingText`]), said politely as it changes. The text field stays composed (measured as nothing) while the
/// box shows, which keeps its focus handling right.
#[kompozable]
#[allow(clippy::too_many_arguments)]
fn TimePickerTextField(
    modifier: Modifier,
    value: TextFieldValue,
    on_value_change: Rc<dyn Fn(TextFieldValue)>,
    state: Rc<dyn TimePickerState>,
    selection: TimePickerSelectionMode,
    keyboard_options: KeyboardOptions,
    keyboard_actions: KeyboardActions,
    colors: TimePickerColors,
) {
    let focus_requester = remember(FocusRequester::new);
    let container_color = MaterialTheme::color_scheme().error_container;
    let label_color = MaterialTheme::color_scheme().on_error_container;
    let text_field_colors = OutlinedTextFieldDefaults.colors(
        focused_container_color = colors.time_selector_container_color(true),
        unfocused_container_color = colors.time_selector_container_color(true),
        focused_text_color = colors.time_selector_content_color(true),
        error_label_color = label_color,
        error_container_color = container_color,
    );
    let selected = selection == state.selection();
    let is_valid = if selection == TimePickerSelectionMode::Hour {
        state.is_hour_input_valid()
    } else {
        state.is_minute_input_valid()
    };
    let text_color =
        if is_valid { colors.time_selector_content_color(true) } else { MaterialTheme::color_scheme().error };

    {
        let (state, focus_requester) = (state.clone(), focus_requester.clone());
        Column(
            modifier = modifier.width_intrinsic(IntrinsicSize::Min),
            content = move || {
                if !selected {
                    let selector_value = if selection == TimePickerSelectionMode::Hour {
                        if state.is_hour_input_valid() { hour_for_display(&*state) } else { state.hour_input() }
                    } else {
                        state.minute_input()
                    };
                    TimeSelector(
                        Modifier.size_wh(
                            TimeInputTokens::TimeFieldContainerWidth,
                            TimeInputTokens::TimeFieldContainerHeight,
                        ),
                        selector_value,
                        state.clone(),
                        selection,
                        colors,
                        is_valid,
                    );
                }

                let content_description = get_string(if selection == TimePickerSelectionMode::Minute {
                    Strings::TimePickerMinuteTextField
                } else {
                    Strings::TimePickerHourTextField
                });
                let interaction_source = remember(MutableInteractionSource::new);

                let primary = MaterialTheme::color_scheme().primary;
                let text_style = named_args!(LocalTextStyle.current().copy(color = text_color));
                Box(
                    modifier = Modifier.visible(selected),
                    content = move || {
                        let text = value.text().to_string();
                        let source = interaction_source.as_interaction_source();
                        let decoration_box: kompoz_foundation::text::DecorationBox =
                            Rc::new(move |inner_text_field: &dyn Fn()| {
                                let container_source = source.clone();
                                let container: Rc<dyn Fn()> = Rc::new(move || {
                                    named_args!(OutlinedTextFieldDefaults.Container(
                                        true,
                                        !is_valid,
                                        container_source.clone(),
                                        shape = TimeInputTokens::TimeFieldContainerShape.value(),
                                        colors = text_field_colors,
                                    ));
                                });
                                named_args!(OutlinedTextFieldDefaults.DecorationBox(
                                    &text,
                                    inner_text_field,
                                    true,
                                    true,
                                    VisualTransformation::None,
                                    source.clone(),
                                    is_error = !is_valid,
                                    colors = text_field_colors,
                                    content_padding = PaddingValues::all(Dp(0.0)),
                                    container = Some(container),
                                ));
                            });
                        let on_value_change = on_value_change.clone();
                        BasicTextFieldValue(
                            value.clone(),
                            move |it: TextFieldValue| on_value_change(it),
                            modifier = Modifier
                                .focus_requester(&focus_requester)
                                .size_wh(
                                    TimeInputTokens::TimeFieldContainerWidth,
                                    TimeInputTokens::TimeFieldContainerHeight,
                                )
                                .semantics(move |s| {
                                    s.set_content_description(content_description.clone());
                                    s.set_max_text_length(2);
                                }),
                            interaction_source = Some(interaction_source.clone()),
                            keyboard_options = keyboard_options.clone(),
                            keyboard_actions = keyboard_actions.clone(),
                            text_style = text_style.clone(),
                            enabled = true,
                            single_line = true,
                            cursor_brush = Brush::vertical_gradient_stops(
                                &[
                                    (0.00, Color::Transparent),
                                    (0.10, Color::Transparent),
                                    (0.10, primary),
                                    (0.90, primary),
                                    (0.90, Color::Transparent),
                                    (1.00, Color::Transparent),
                                ],
                                0.0,
                                f32::INFINITY,
                                TileMode::Clamp,
                            ),
                            decoration_box = Some(decoration_box),
                        );
                    },
                );

                SupportingText(
                    Modifier.fill_max_width().semantics(|s| s.set_live_region(LiveRegionMode::Polite)),
                    selection,
                    state.clone(),
                    is_valid,
                );
            },
        );
    }

    let effect_state = state.clone();
    LaunchedEffect(state.selection(), async move {
        if effect_state.selection() == selection {
            focus_requester.request_focus_with(FocusDirection::Enter);
        }
    });
}

/// Children evenly round a circle of radius the layout's height times `radius_to_size_ratio`, the first at 12
/// o'clock (Compose's private `CircularLayout`); the inner circle's layout in the middle.
#[kompozable]
fn CircularLayout(modifier: Modifier, radius_to_size_ratio: f32, content: impl Fn() + 'static) {
    Layout(
        modifier = modifier,
        content = content,
        measure_policy = measure_policy(move |scope, measurables, constraints| {
            let radius_px = constraints.max_height as f32 * radius_to_size_ratio;
            let item_constraints = named_args!(constraints.copy(min_width = 0, min_height = 0));
            let is = |m: &dyn Measurable, id: LayoutId| m.layout_id().is_some_and(|i| i.is(&id));
            let placeables: Vec<_> = measurables
                .iter()
                .copied()
                .filter(|m| !is(*m, LayoutId::Selector) && !is(*m, LayoutId::InnerCircle))
                .map(|measurable| measurable.measure(item_constraints))
                .collect();
            let selector_measurable = measurables.iter().copied().find(|m| is(*m, LayoutId::Selector));
            let inner_measurable = measurables.iter().copied().find(|m| is(*m, LayoutId::InnerCircle));
            let theta = FullCircle / placeables.len() as f32;
            let selector_placeable = selector_measurable.map(|m| m.measure(item_constraints));
            let inner_circle_placeable = inner_measurable.map(|m| m.measure(item_constraints));

            scope.layout(constraints.min_width, constraints.min_height, move |p| {
                if let Some(selector_placeable) = &selector_placeable {
                    p.place(selector_placeable, 0, 0);
                }

                for (i, it) in placeables.iter().enumerate() {
                    let center_offset_x = constraints.max_width / 2 - it.width() / 2;
                    let center_offset_y = constraints.max_height / 2 - it.height() / 2;
                    // (Kotlin's `theta * i - QuarterCircle`: a Double, as QuarterCircle is)
                    let angle = (theta * i as f32) as f64 - QuarterCircle;
                    let offset_x = radius_px as f64 * angle.cos() + center_offset_x as f64;
                    let offset_y = radius_px as f64 * angle.sin() + center_offset_y as f64;
                    p.place(it, round_to_int(offset_x), round_to_int(offset_y));
                }

                if let Some(inner_circle_placeable) = &inner_circle_placeable {
                    p.place(
                        inner_circle_placeable,
                        (constraints.min_width - inner_circle_placeable.width()) / 2,
                        (constraints.min_height - inner_circle_placeable.height()) / 2,
                    );
                }
            })
        }),
    );
}

/// Kotlin's `Double.roundToInt()` (Java's `Math.round`: halves up).
fn round_to_int(value: f64) -> i32 {
    (value + 0.5).floor() as i32
}

/// A dial number's description (Compose's internal `numberContentDescription`): "30 minutes", "13 hours" on a
/// 24-hour clock, "9 o'clock" on a 12-hour one, in the platform's language.
#[track_caller]
pub(crate) fn number_content_description(selection: TimePickerSelectionMode, is_24_hour: bool, number: i32) -> String {
    let id = if selection == TimePickerSelectionMode::Minute {
        Strings::TimePickerMinuteSuffix
    } else if is_24_hour {
        Strings::TimePicker24HourSuffix
    } else {
        Strings::TimePickerHourSuffix
    };

    get_string_formatted(id, &[&number])
}

/// The distance between (`x1`, `y1`) and (`x2`, `y2`) (Compose's private `dist`).
fn dist(x1: f32, y1: f32, x2: i32, y2: i32) -> f32 {
    let x = x2 as f32 - x1;
    let y = y2 as f32 - y1;
    (x as f64).hypot(y as f64) as f32
}

/// The angle of (`x`, `y`) from 12 o'clock, clockwise, in 0..2π (Compose's private `atan`).
fn atan(y: f32, x: f32) -> f32 {
    let ret = atan2f(y, x) - QuarterCircle as f32;
    if ret < 0.0 { ret + FullCircle } else { ret }
}

/// Kotlin's `atan2(Float, Float)` (the JVM's, in doubles).
fn atan2f(y: f32, x: f32) -> f32 {
    (y as f64).atan2(x as f64) as f32
}

/// Kotlin's `cos(Float)` (the JVM's, in doubles).
fn cosf(x: f32) -> f32 {
    (x as f64).cos() as f32
}

/// Kotlin's `sin(Float)` (the JVM's, in doubles).
fn sinf(x: f32) -> f32 {
    (x as f64).sin() as f32
}

/// The dial's layout ids (Compose's private enum `LayoutId`).
#[derive(Clone, Copy, PartialEq, Debug)]
enum LayoutId {
    Selector,
    InnerCircle,
}

/// Whether a key event is a click: Enter's or Space's release (Compose's private `KeyEvent.isClick`).
fn is_click(event: &KeyEvent) -> bool {
    event.r#type == KeyEventType::KeyUp && is_enter(event)
}

/// Whether a key event is Enter's or Space's (Compose's private `KeyEvent.isEnter`).
fn is_enter(event: &KeyEvent) -> bool {
    matches!(event.key, Key::DirectionCenter | Key::Enter | Key::NumPadEnter | Key::Spacebar)
}

/// The layout for the window (Compose's internal `defaultTimePickerLayoutType`; Compose for desktop's actual): side
/// by side when the window ([`LocalWindowInfo`]'s container) is wider than tall.
#[track_caller]
pub(crate) fn default_time_picker_layout_type() -> TimePickerLayoutType {
    let container_size = LocalWindowInfo.current().container_size();
    if container_size.height < container_size.width {
        TimePickerLayoutType::Horizontal
    } else {
        TimePickerLayoutType::Vertical
    }
}

/// A full turn (Compose's `FullCircle`).
const FullCircle: f32 = (std::f64::consts::PI * 2.0) as f32;
/// Half a turn (Compose's `HalfCircle`).
const HalfCircle: f32 = FullCircle / 2.0;
/// A quarter turn, a Double as Compose's (`QuarterCircle = PI / 2`).
const QuarterCircle: f64 = std::f64::consts::PI / 2.0;
/// A minute's angle (Compose's `RadiansPerMinute`).
const RadiansPerMinute: f32 = FullCircle / 60.0;
/// An hour's angle (Compose's `RadiansPerHour`).
const RadiansPerHour: f32 = FullCircle / 12.0;
/// The z index of the period toggle's line (Compose's `SeparatorZIndex`).
const SeparatorZIndex: f32 = 2.0;
/// The largest number a field takes (Compose's `MaxValueForTextField`).
const MaxValueForTextField: i32 = 99;

/// The outer ring's radius for the dial's size (Compose's `OuterCircleToSizeRatio`: 101 dp of 256).
const OuterCircleToSizeRatio: f32 = 101.0 / TimePickerTokens::ClockDialContainerSize.0;
/// The inner ring's radius for the dial's size (Compose's `InnerCircleToSizeRatio`: 69 dp of 256).
const InnerCircleToSizeRatio: f32 = 69.0 / TimePickerTokens::ClockDialContainerSize.0;
const ClockDisplayBottomMargin: Dp = Dp(36.0);
const ClockFaceBottomMargin: Dp = Dp(24.0);
const DisplaySeparatorWidth: Dp = Dp(24.0);

const SupportLabelTop: Dp = Dp(7.0);
const MaxDistance: Dp = Dp(74.0);
const MinimumInteractiveSize: Dp = Dp(48.0);
/// The minutes on the dial (Compose's `Minutes`).
const Minutes: [i32; 12] = [0, 5, 10, 15, 20, 25, 30, 35, 40, 45, 50, 55];
/// The hours on the dial (Compose's `Hours`).
const Hours: [i32; 12] = [12, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11];
/// The 24-hour clock's inner hours (Compose's `ExtraHours`: each of `Hours` % 12 + 12).
const ExtraHours: [i32; 12] = [12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23];
const PeriodToggleMargin: Dp = Dp(12.0);

/// The heights at which the side-by-side dial is its full size or its middle one (Compose's `TimePickerMaxHeight`,
/// `TimePickerMidHeight`), the middle size (`ClockDialMidContainerSize`) and the smallest (`ClockDialMinContainerSize`,
/// which the dialog reads).
const TimePickerMaxHeight: Dp = Dp(384.0);
const TimePickerMidHeight: Dp = Dp(330.0);
const ClockDialMidContainerSize: Dp = Dp(238.0);
pub(crate) const ClockDialMinContainerSize: Dp = Dp(200.0);

/// The text field's visibility (Compose's private `Modifier.visible`).
trait VisibleModifierExt {
    /// Measured but laid out as nothing unless `visible` (Compose's private `Modifier.visible`): it stays composed,
    /// which keeps the focus handling right.
    fn visible(self, visible: bool) -> Modifier;
}

impl VisibleModifierExt for Modifier {
    fn visible(self, visible: bool) -> Modifier {
        self.then_element(VisibleModifier { visible })
    }
}

/// Compose's private `VisibleModifier`: equal by its visibility.
#[derive(Clone, PartialEq)]
struct VisibleModifier {
    visible: bool,
}

impl ModifierNodeElement for VisibleModifier {
    type Node = VisibleModifier;
    fn create(&self) -> VisibleModifier {
        self.clone()
    }
    fn update(&self, node: &mut VisibleModifier) {
        node.visible = self.visible;
    }
    fn name(&self) -> &'static str {
        "visible"
    }
}

impl ModifierNode for VisibleModifier {
    node_capabilities!(layout);
}

impl LayoutModifierNode for VisibleModifier {
    fn measure(
        &mut self,
        scope: &MeasureScope,
        measurable: &dyn Measurable,
        constraints: Constraints,
    ) -> MeasureResult {
        let placeable = measurable.measure(constraints);

        if !self.visible {
            return scope.layout(0, 0, |_| {});
        }
        scope.layout(placeable.width(), placeable.height(), move |p| p.place(&placeable, 0, 0))
    }
}

/// The side-by-side dial's size for the height there is (Compose's internal `ClockFaceSizeModifier`): its full size
/// from 384 dp, its middle one from 330 dp, else its smallest. (Compose's, a class without `equals`, is another
/// modifier each composition, its node updated with nothing to update; this one is equal to itself: the same layout.)
#[derive(Clone, PartialEq)]
pub(crate) struct ClockFaceSizeModifier;

impl ModifierNodeElement for ClockFaceSizeModifier {
    type Node = ClockFaceSizeModifier;
    fn create(&self) -> ClockFaceSizeModifier {
        ClockFaceSizeModifier
    }
    fn update(&self, _: &mut ClockFaceSizeModifier) {}
    fn name(&self) -> &'static str {
        "ClockFaceSizeModifier"
    }
}

impl ModifierNode for ClockFaceSizeModifier {
    node_capabilities!(layout);
}

impl LayoutModifierNode for ClockFaceSizeModifier {
    fn measure(
        &mut self,
        scope: &MeasureScope,
        measurable: &dyn Measurable,
        constraints: Constraints,
    ) -> MeasureResult {
        let max = scope.to_dp(constraints.max_height as f32);
        let size = scope.round_to_px(if max >= TimePickerMaxHeight {
            TimePickerTokens::ClockDialContainerSize
        } else if max >= TimePickerMidHeight {
            ClockDialMidContainerSize
        } else {
            ClockDialMinContainerSize
        });

        let placeable = measurable.measure(Constraints::fixed(size, size));
        scope.layout(placeable.width(), placeable.height(), move |p| p.place(&placeable, 0, 0))
    }
}

/// What a time input does when what is typed is wrong (Compose's internal interface `TimeInputErrorHandler`).
pub(crate) trait TimeInputErrorHandler {
    fn on_error(&self);
}

/// The time input's error handler (Compose's internal `rememberTimeInputErrorHandler`; Compose for desktop's actual):
/// a reject haptic feedback, whether touch exploration is on or not.
#[kompozable]
pub(crate) fn remember_time_input_error_handler(is_touch_exploration_enabled: bool) -> Rc<dyn TimeInputErrorHandler> {
    // (Compose for desktop's actual doesn't read it)
    let _ = is_touch_exploration_enabled;
    let haptics = LocalHapticFeedback.current();
    remember_keyed(haptics.clone(), move || {
        Rc::new(TimeInputErrorHandlerImpl { haptics }) as Rc<dyn TimeInputErrorHandler>
    })
}

/// Compose for desktop's private `TimeInputErrorHandlerImpl`: a reject haptic feedback.
struct TimeInputErrorHandlerImpl {
    haptics: HapticFeedback,
}

impl TimeInputErrorHandler for TimeInputErrorHandlerImpl {
    fn on_error(&self) {
        self.haptics.perform_haptic_feedback(HapticFeedbackType::Reject);
    }
}
