//! KompozR — Jetpack Compose's UI toolkit in Rust — under one name: its crates as modules (`kompoz::runtime`,
//! `kompoz::ui`, …), a [`prelude`] for what an app writes with, and Kotlin's syntax: [`kompoz!`] (Kotlin
//! declarations in a Rust source) and [`kompoz_file!`] (a `.kt` file's).
//!
//! ```ignore
//! use kompoz::prelude::*;
//!
//! kompoz! {
//!     @Kompozable
//!     fun Greeting() {}
//! }
//!
//! fn main() -> std::process::ExitCode {
//!     App::new("Hello").run(Greeting)
//! }
//! ```
//!
//! What `#[kompozable]` and the Kotlin syntax generate names the crates through this one when a crate depends on it
//! (`::kompoz::runtime::…`), so it is the only dependency an app needs. `kompoz::test` (the headless test rule) is
//! behind the `test` feature, for a crate's tests.

// (the macros name this crate `::kompoz` in it too)
extern crate self as kompoz;

pub use kompoz_animation as animation;
pub use kompoz_animation_graphics as animation_graphics;
pub use kompoz_coroutines as coroutines;
pub use kompoz_foundation as foundation;
pub use kompoz_gpu as gpu;
pub use kompoz_graphics as graphics;
pub use kompoz_material_icons_core as material_icons_core;
// (WP4b: icons)
pub use kompoz_material_icons_extended as material_icons_extended;
pub use kompoz_material_ripple as material_ripple;
pub use kompoz_material3 as material3;
// (WP8: adaptive)
pub use kompoz_material3_adaptive as material3_adaptive;
pub use kompoz_material3_adaptive_navigation_suite as material3_adaptive_navigation_suite;
pub use kompoz_platform as platform;
pub use kompoz_runtime as runtime;
/// Kotlin's standard library as the Kotlin syntax needs it (`kompoz_runtime::kt`).
pub use kompoz_runtime::kt;
#[cfg(feature = "test")]
pub use kompoz_test as test;
pub use kompoz_text as text;
pub use kompoz_ui as ui;

pub use kompoz_kotlin_macro::{kompoz, kompoz_file};
pub use kompoz_runtime::{kompozable, kompozable_block, kompozable_lambda, named, named_args};

/// Material's icons (Compose's `Icons`), a module a theme, a function an icon: `Icons::Filled::Add()`, Kotlin's
/// `Icons.Filled.Add`. The core icons' (kompoz-material-icons-core) and the extended set's
/// (kompoz-material-icons-extended), each theme's module both sets' functions (as Compose's extended icons are
/// extension properties of core's `Icons.Filled` …: `Icons.Filled.Add` and `Icons.Filled.Tune` alike).
#[allow(non_snake_case)]
pub mod Icons {
    /// Compose's `Icons.Filled`.
    pub mod Filled {
        pub use kompoz_material_icons_core::Icons::Filled::*;
        pub use kompoz_material_icons_extended::Icons::Filled::*;
    }
    /// Compose's `Icons.Outlined`.
    pub mod Outlined {
        pub use kompoz_material_icons_core::Icons::Outlined::*;
        pub use kompoz_material_icons_extended::Icons::Outlined::*;
    }
    /// Compose's `Icons.Rounded`.
    pub mod Rounded {
        pub use kompoz_material_icons_core::Icons::Rounded::*;
        pub use kompoz_material_icons_extended::Icons::Rounded::*;
    }
    /// Compose's `Icons.Sharp`.
    pub mod Sharp {
        pub use kompoz_material_icons_core::Icons::Sharp::*;
        pub use kompoz_material_icons_extended::Icons::Sharp::*;
    }
    /// Compose's `Icons.TwoTone`.
    pub mod TwoTone {
        pub use kompoz_material_icons_core::Icons::TwoTone::*;
        pub use kompoz_material_icons_extended::Icons::TwoTone::*;
    }
    /// Compose's `Icons.Default`: the filled ones.
    pub use Filled as Default;

    /// Compose's `Icons.AutoMirrored`: mirrored right to left.
    pub mod AutoMirrored {
        /// Compose's `Icons.AutoMirrored.Filled`.
        pub mod Filled {
            pub use kompoz_material_icons_core::Icons::AutoMirrored::Filled::*;
            pub use kompoz_material_icons_extended::Icons::AutoMirrored::Filled::*;
        }
        /// Compose's `Icons.AutoMirrored.Outlined`.
        pub mod Outlined {
            pub use kompoz_material_icons_core::Icons::AutoMirrored::Outlined::*;
            pub use kompoz_material_icons_extended::Icons::AutoMirrored::Outlined::*;
        }
        /// Compose's `Icons.AutoMirrored.Rounded`.
        pub mod Rounded {
            pub use kompoz_material_icons_core::Icons::AutoMirrored::Rounded::*;
            pub use kompoz_material_icons_extended::Icons::AutoMirrored::Rounded::*;
        }
        /// Compose's `Icons.AutoMirrored.Sharp`.
        pub mod Sharp {
            pub use kompoz_material_icons_core::Icons::AutoMirrored::Sharp::*;
            pub use kompoz_material_icons_extended::Icons::AutoMirrored::Sharp::*;
        }
        /// Compose's `Icons.AutoMirrored.TwoTone`.
        pub mod TwoTone {
            pub use kompoz_material_icons_core::Icons::AutoMirrored::TwoTone::*;
            pub use kompoz_material_icons_extended::Icons::AutoMirrored::TwoTone::*;
        }
        /// Compose's `Icons.AutoMirrored.Default`: the filled ones.
        pub use Filled as Default;
    }
}

/// What an app writes with: the runtime (states, `remember`, effects, locals, the macros), coroutines (effects,
/// flows), animation (Animatable, specs, easings, decays), graphics (units — `16.dp()` —, colors, geometry, shapes,
/// brushes, paths, images), text (styles, fonts, annotated strings), the UI core (`Modifier`, `Alignment`, `Layout`,
/// the host's types, the composition locals, `Role`), foundation (Box, Row, Column, BasicText, the modifiers' traits,
/// gestures, interactions), the ripple node (material-ripple's), Material 3 (`MaterialTheme`, `Text`, `Surface`,
/// `Icon`, …) and its icons (`Icons`), the app entry (`App`), and the Kotlin syntax's macros.
///
/// The crates' roots are taken whole (their modules with them); where two of them use a name, the one an app means
/// is named here: the modules `draw` and `input` are the UI's (`DrawScope`, pointer input), `shape` and `unit`
/// graphics' (shapes, `Dp`); text's are `kompoz::text::draw`, `kompoz::text::input`, `kompoz::text::shape`.
pub mod prelude {
    pub use kompoz_animation::*;
    pub use kompoz_coroutines::*;
    pub use kompoz_foundation::prelude::*;
    pub use kompoz_graphics::*;
    pub use kompoz_material_ripple::prelude::*;
    pub use kompoz_material3::prelude::*;
    // (WP8: adaptive — androidx.compose.material3.adaptive{,.layout,.navigation}, androidx.window.core.layout,
    // androidx.compose.material3.adaptive.navigationsuite)
    pub use kompoz_material3_adaptive::prelude::*;
    pub use kompoz_material3_adaptive_navigation_suite::prelude::*;
    pub use kompoz_runtime::*;
    pub use kompoz_text::*;
    pub use kompoz_ui::*;

    // (Material's icons, Compose's `Icons`, and building one's own; Material's names win over the lower crates' where
    // both have one, as in a Compose app's imports)
    pub use crate::Icons;
    pub use kompoz_material_icons_core::{MaterialPathBuilder, material_icon};
    pub use kompoz_material3::{Icon, Surface, Text};
    // (WP6: pickers — java.time's dates, as a Kotlin/JVM file imports them for the date pickers' JVM forms)
    pub use kompoz_runtime::kt::{DayOfWeek, LocalDate, YearMonth};

    pub use kompoz_graphics::{shape, unit};
    pub use kompoz_ui::{draw, input};

    // (Compose's painters and vector content: `Image(rememberVectorPainter(icon))`, `Path(pathData = …)`; Compose's
    // `DefaultAlpha`, a painter's `draw(size)` default)
    pub use kompoz_ui::draw::vector::{Group, Path};
    pub use kompoz_ui::draw::{
        BitmapPainter, BrushPainter, ColorPainter, DefaultAlpha, Painter, VectorPainter, remember_vector_painter,
        remember_vector_painter_content,
    };
    // (animation-graphics': `Image(rememberAnimatedVectorPainter(image, atEnd))`; its modules `res` and `vector`
    // stay under `kompoz::animation_graphics`)
    pub use kompoz_animation_graphics::{
        AnimatedImageVector, NoXmlResources, ResourceReference, XmlResourceError, XmlResources,
        remember_animated_vector_painter,
    };

    pub use kompoz_kotlin_macro::{kompoz, kompoz_file};
    pub use kompoz_platform::App;
    // (WP1b: toggle_split — the trait a TooltipBox's `positionProvider` is: Kotlin code naming none of it still
    // makes a value of it, `TooltipDefaults.rememberTooltipPositionProvider(…)`, which the generated Rust types)
    pub use kompoz_ui::window::PopupPositionProvider;
    // (WP1b: fab_segmented — what M15's FloatingActionButtonMenuSample imports: the back handler (CMP's
    // androidx.compose.ui.backhandler.BackHandler), the keys of androidx.compose.ui.input.key, the semantics' custom
    // actions, the focus requester)
    pub use kompoz_ui::back::BackHandler;
    pub use kompoz_ui::focus::FocusRequester;
    pub use kompoz_ui::input::{Key, KeyEvent, KeyEventType};
    pub use kompoz_ui::semantics::CustomAccessibilityAction;
    // (the clipboard's entries, Compose's androidx.compose.ui.platform.ClipEntry & co.)
    pub use kompoz_ui::clipboard::{ClipEntry, ClipItem, ClipMetadata};
    pub use kompoz_ui::platform::{
        LocalClipboard, LocalClipboardManager, LocalCursorBlinkEnabled, LocalDeepLinks, LocalDensity,
        LocalDynamicTonalPalette, LocalFocusManager, LocalFontFamilyResolver, LocalHapticFeedback, LocalImeController,
        LocalInputModeManager, LocalLayoutDirection, LocalPlatform, LocalSoftwareKeyboardController, LocalSystemInsets,
        LocalSystemTheme, LocalTextToolbar, LocalUriHandler, LocalViewConfiguration, LocalWallpaperTonalPalette,
        LocalWindowInfo,
    };
    // (the system's theme, Compose's androidx.compose.foundation.isSystemInDarkTheme: what a theme picks its scheme
    // by)
    pub use kompoz_ui::platform::is_system_in_dark_theme;
    pub use kompoz_ui::semantics::Role;
    // (a layer's transform origin, Compose's androidx.compose.ui.graphics.TransformOrigin: `scaleIn(transformOrigin =
    // TransformOrigin(0f, 0f))`)
    pub use kompoz_ui::layout::TransformOrigin;
    // (the state-based text fields, Compose's androidx.compose.foundation.text; the keyboard's options, its
    // androidx.compose.ui.text.input)
    pub use kompoz_foundation::text::{BasicSecureTextField, BasicTextField, TextFieldDecorator};
    pub use kompoz_ui::platform::{ImeAction, KeyboardCapitalization, KeyboardType};
    // (WP4a: tooling preview) (Compose's androidx.compose.ui.tooling.preview: `LoremIpsum().values.first()` in the
    // samples)
    pub use kompoz_ui::tooling::preview::{CollectionPreviewParameterProvider, LoremIpsum, PreviewParameterProvider};
}
