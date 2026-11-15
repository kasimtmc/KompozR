//! UI state values (Compose's `androidx.compose.ui.state`).

/// The state of something that toggles (Compose's ToggleableState): a checkbox can also be indeterminate (some of
/// what it stands for is on).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum ToggleableState {
    On,
    Off,
    Indeterminate,
}

impl ToggleableState {
    /// On or off (Compose's `ToggleableState(value)`).
    pub fn of(value: bool) -> ToggleableState {
        if value { ToggleableState::On } else { ToggleableState::Off }
    }
}

impl From<bool> for ToggleableState {
    fn from(value: bool) -> Self {
        ToggleableState::of(value)
    }
}
