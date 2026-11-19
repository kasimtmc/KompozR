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
pub use kompoz_material_ripple as material_ripple;
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

/// What an app writes with: the runtime (states, `remember`, effects, locals, the macros), coroutines (effects,
/// flows), animation (Animatable, specs, easings, decays), graphics (units — `16.dp()` —, colors, geometry, shapes,
/// brushes, paths, images), text (styles, fonts, annotated strings), the UI core (`Modifier`, `Alignment`, `Layout`,
/// the host's types, the composition locals, `Role`), foundation (Box, Row, Column, BasicText, the modifiers' traits,
/// gestures, interactions), the ripple node (material-ripple's), the app entry (`App`), and the Kotlin syntax's
/// macros.
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
    pub use kompoz_runtime::*;
    pub use kompoz_text::*;
    pub use kompoz_ui::*;

    pub use kompoz_graphics::{shape, unit};
    pub use kompoz_ui::{draw, input};

    // (Compose's painters and vector content: `Image(rememberVectorPainter(icon))`, `Path(pathData = …)`)
    pub use kompoz_ui::draw::vector::{Group, Path};
    pub use kompoz_ui::draw::{
        BitmapPainter, BrushPainter, ColorPainter, Painter, VectorPainter, remember_vector_painter,
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
    // (the clipboard's entries, Compose's androidx.compose.ui.platform.ClipEntry & co.)
    pub use kompoz_ui::clipboard::{ClipEntry, ClipItem, ClipMetadata};
    pub use kompoz_ui::platform::{
        LocalClipboard, LocalClipboardManager, LocalCursorBlinkEnabled, LocalDeepLinks, LocalDensity,
        LocalDynamicTonalPalette, LocalFocusManager, LocalFontFamilyResolver, LocalHapticFeedback, LocalImeController,
        LocalInputModeManager, LocalLayoutDirection, LocalPlatform, LocalSoftwareKeyboardController, LocalSystemInsets,
        LocalSystemTheme, LocalTextToolbar, LocalUriHandler, LocalViewConfiguration, LocalWallpaperTonalPalette,
        LocalWindowInfo,
    };
    pub use kompoz_ui::semantics::Role;
    // (the state-based text fields, Compose's androidx.compose.foundation.text; the keyboard's options, its
    // androidx.compose.ui.text.input)
    pub use kompoz_foundation::text::{BasicSecureTextField, BasicTextField, TextFieldDecorator};
    pub use kompoz_ui::platform::{ImeAction, KeyboardCapitalization, KeyboardType};
}
