//! KompozR's showcase: the C++ Kompoz showcase (~/Projects/kompoz/examples/showcase) ported — its shell (main.cpp:
//! the drawer, the navigation suite, the app bar, the snackbars, the NavHost of its screens), its Material screen
//! (material/*: 27 sections, one under another) and its places screen (places/places.cpp), in Kotlin's syntax: `.kt`
//! files read by `kompoz_file!`, in the C++ showcase's folders and files (showcase.hpp + common.cpp → `common.kt`,
//! main.cpp → `main.kt`, material/*.cpp → `material/*.kt`, places/places.cpp → `places/places.kt`).
//!
//! Rust here: `onAndroid` (what the C++ checks with `__ANDROID__`). The two indications the ripple section provides
//! (the C++'s `Indication.Kind.StateLayer` and `LocalIndication provides false`) are Kotlin's, as Compose writes them:
//! `object … : IndicationNodeFactory` creating a `Modifier.Node(), DrawModifierNode` of its own (material/sections.kt). The other screens (home, list, components) come with stage 16: until then their routes say so.

#![allow(non_snake_case, non_upper_case_globals)]

use kompoz::foundation::interaction::{
    DragInteraction, FocusInteraction, HoverInteraction, InteractionSource, PressInteraction,
};
pub mod shots;

use kompoz::graphics::Color;
use kompoz::graphics::vector::ImageVector;

// Each `.kt` file in a module of its own, as Kotlin scopes a file's private declarations to it; what they declare
// otherwise is the crate's, seen from every file (Kotlin's package).
mod common {
    use super::*;
    use kompoz::prelude::*;
    kompoz::kompoz_file!("src/common.kt");
}
mod main {
    use super::*;
    use kompoz::prelude::*;
    use kompoz::ui::input::NestedScrollConnection;
    use kompoz::ui::strings::{LocalStrings, Strings};
    kompoz::kompoz_file!("src/main.kt");
}
mod material {
    use super::*;
    use kompoz::prelude::*;
    kompoz::kompoz_file!("src/material/material.kt");
}
mod sections {
    use super::*;
    use kompoz::foundation::indication::{Indication, IndicationNodeFactory};
    use kompoz::prelude::*;
    use kompoz::ui::draw::{ContentDrawScope, DrawModifierNode};
    use kompoz::ui::focus::FocusDirection;
    use kompoz::ui::focus::FocusRequester;
    use kompoz::ui::modifier::CompositionLocalConsumerModifierNode;
    use kompoz::ui::platform::FrameRateCategory;
    use kompoz::ui::semantics::CustomAccessibilityAction;
    use kompoz::ui::semantics::LiveRegionMode;
    use kompoz::ui::window::PopupPositionProvider;
    use kompoz::ui::{ModifierNode, ModifierNodeBase, ModifierNodeImpl};
    kompoz::kompoz_file!("src/material/sections.kt");
}
mod text_fields {
    use super::*;
    use kompoz::foundation::text::selection::TextSelectionColors;
    use kompoz::prelude::*;
    use kompoz::text::intl::Locale;
    use kompoz::ui::focus::FocusRequester;
    kompoz::kompoz_file!("src/material/text_fields.kt");
}
mod controls {
    use super::*;
    use kompoz::prelude::*;
    use kompoz::ui::semantics::ToggleableState;
    kompoz::kompoz_file!("src/material/controls.kt");
}
mod containers {
    // (the C++'s rememberModalBottomSheetState, deprecated by material3 1.5.0-alpha20: Kotlin warns of it as Rust does)
    #![allow(deprecated)]
    use super::*;
    use kompoz::prelude::*;
    kompoz::kompoz_file!("src/material/containers.kt");
}
mod navigation {
    use super::*;
    use kompoz::prelude::*;
    use kompoz::ui::back::BackHandler;
    use kompoz::ui::input::NestedScrollConnection;
    kompoz::kompoz_file!("src/material/navigation.kt");
}
mod places {
    use super::*;
    use kompoz::prelude::*;
    kompoz::kompoz_file!("src/places/places.kt");
}
pub use common::*;
pub use containers::*;
pub use controls::*;
pub use main::*;
pub use material::*;
pub use navigation::*;
pub use places::*;
pub use sections::*;
pub use text_fields::*;

/// Where the app runs, for what differs (a phone has no hardware keyboard): the C++ showcase's `kOnAndroid`.
pub const on_android: bool = cfg!(target_os = "android");

// The showcase on Android: this library built as the app's `libshowcase.so` (a cdylib, by cargo-ndk from the Gradle
// project in android/: `cargo xtask android`), its activity showing what the desktop program's window does.
kompoz::android_app!(kompoz::platform::App::new("Kompoz Showcase"), ShowcaseApp);
