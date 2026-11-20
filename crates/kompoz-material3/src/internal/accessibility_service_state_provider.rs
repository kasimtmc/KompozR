//! Whether an accessibility service is on, as Compose's internal AccessibilityServiceStateProvider.kt with Compose
//! for desktop's actual: the desktop has no touch exploration, switch or voice access to listen to, so the state is
//! false (one state, shared by all who ask).

use kompoz_runtime::{MutableState, State, kompozable, mutable_state_of};
use std::sync::OnceLock;

/// Compose for desktop's `AccessibilityServiceEnabled`: never set.
fn accessibility_service_enabled() -> MutableState<bool> {
    static ENABLED: OnceLock<MutableState<bool>> = OnceLock::new();
    ENABLED.get_or_init(|| mutable_state_of(false)).clone()
}

/// Whether one of the services listened to is on (Compose's internal `rememberAccessibilityServiceState`): touch
/// exploration, switch access, voice access as asked; on the desktop none is, the state stays false.
#[kompozable]
pub(crate) fn remember_accessibility_service_state(
    #[default(true)] listen_to_touch_exploration_state: bool,
    #[default(true)] listen_to_switch_access_state: bool,
    #[default(true)] listen_to_voice_access_state: bool,
) -> State<bool> {
    // (Compose for desktop's actual listens to none of them)
    let _ = (listen_to_touch_exploration_state, listen_to_switch_access_state, listen_to_voice_access_state);
    State::from(accessibility_service_enabled())
}
