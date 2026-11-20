//! The insets material3's components keep clear of by default, as Compose's internal SystemBarsDefaultInsets.kt
//! with Compose for desktop's actual: the system bars (status bars, navigation bars, the caption bar).

use kompoz_foundation::layout::WindowInsets;
use kompoz_runtime::kompozable;

/// The insets of the system bars the components draw around (Compose's internal
/// `WindowInsets.systemBarsForVisualComponents`, Compose for desktop's actual: `WindowInsets.systemBars`).
#[kompozable]
pub(crate) fn system_bars_for_visual_components() -> WindowInsets {
    WindowInsets::system_bars()
}
