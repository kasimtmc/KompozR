//! Precision pointer sizing, as Compose's PrecisionPointer.kt: whether components may take their smaller sizes for
//! a mouse and keyboard (Material's dense desktop sizing), behind [`ComposeMaterial3Flags`]'
//! `is_precision_pointer_component_sizing_enabled`. On the desktop (Compose for desktop's actual) the theme registers
//! nothing and it stays off; Android's input device listening is the Android platform's (stage 15).

use crate::flags::ComposeMaterial3Flags;
use kompoz_runtime::{MutableState, kompozable, mutable_state_of};
use std::sync::OnceLock;

/// Whether components should take their precision pointer sizes now (Compose's
/// `shouldUsePrecisionPointerComponentSizing()`): a keyboard and a mouse attached, and the flag on.
pub fn should_use_precision_pointer_component_sizing() -> bool {
    should_use_precision_pointer_component_sizing_state().value()
}

/// The state behind it (Compose's internal `shouldUsePrecisionPointerComponentSizing`): what the platform's
/// listeners set.
pub(crate) fn should_use_precision_pointer_component_sizing_state() -> MutableState<bool> {
    static STATE: OnceLock<MutableState<bool>> = OnceLock::new();
    STATE.get_or_init(|| mutable_state_of(false)).clone()
}

/// `content`, the platform's input device listeners registered once around it when the flag asks for them
/// (Compose's internal `EnsurePrecisionPointerListenersRegistered`; the desktop's: `content` as it is).
#[kompozable(inline)]
pub(crate) fn EnsurePrecisionPointerListenersRegistered(content: impl FnOnce()) {
    // (Compose for desktop's actual registers nothing: the flag has no effect there)
    let _ = ComposeMaterial3Flags::is_precision_pointer_component_sizing_enabled();
    content();
}
