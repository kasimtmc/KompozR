//! Compose for desktop's actuals of SearchBar.kt (SearchBar.skiko.kt): the height of the window a docked search
//! bar's results take a share of.

use kompoz_graphics::Dp;
use kompoz_runtime::kompozable;
use kompoz_ui::platform::{LocalDensity, LocalWindowInfo};

/// The height of the window's container (Compose's internal `expect fun getWindowContainerHeight()`, Compose for
/// desktop's actual: `LocalWindowInfo`'s container height in dp).
#[kompozable]
pub(crate) fn get_window_container_height() -> Dp {
    let density = LocalDensity.current();
    let window_info = LocalWindowInfo.current();
    density.to_dp(window_info.container_size().height as f32)
}
