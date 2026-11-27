//! The app's entry as Compose Desktop writes it (Application.desktop.kt's `application`, Window.desktop.kt's
//! `singleWindowApplication`): the program's `main` is one call, the app's composition in it, until the app ends.
//! What Kotlin's syntax calls too (the only part of this crate in the Kotlin index: `cargo xtask kotlin-index`).
//!
//! ```text
//! fn main() -> ExitCode {
//!     single_window_application(title = "KompozR".to_string(), content = |_: &SingleWindowApplicationScope| App())
//! }
//! ```
//!
//! Kotlin: `fun main() = singleWindowApplication(title = "KompozR") { App() }`.

use super::{App, single_window_content};
use kompoz_ui::draw::Painter;
use kompoz_ui::input::KeyEvent;
use kompoz_ui::window::{ApplicationScope, SingleWindowApplicationScope, WindowState};
use std::process::ExitCode;
use std::rc::Rc;

/// The app's name where the program gives none: the program's file's (its caches', its window class's).
fn program_name() -> String {
    std::env::args()
        .next()
        .and_then(|a| std::path::Path::new(&a).file_stem().map(|s| s.to_string_lossy().into_owned()))
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| "KompozR".into())
}

/// What the app's end does: the process ends with its code (Compose Desktop's `exitProcessOnExit`: nothing after the
/// call runs, as `exitProcess` does), or it is returned to the program.
fn ended(code: ExitCode, exit_process_on_exit: bool) -> ExitCode {
    if exit_process_on_exit {
        std::process::exit(if code == ExitCode::SUCCESS { 0 } else { 1 });
    }
    code
}

/// Compose Desktop's `application(exitProcessOnExit = true) { }`: `content` composed without a window of its own,
/// given the app's scope ([`ApplicationScope::exit_application`]), its `Window`s and `Tray`s shown, until it exits or
/// nothing is left in it (no window, no tray icon). Then, with `exit_process_on_exit`, the process ends (what follows
/// the call doesn't run, Compose's); else what the program returns. The app's title (its caches', its window
/// class's) is the program's name; `App::new(title)….application(content)` gives another and the rest of its
/// configuration.
///
/// ```text
/// fn main() -> ExitCode {
///     application(|app: &ApplicationScope| {
///         let app = app.clone();
///         Window(on_close_request = move || app.exit_application(), content = |_: &FrameWindowScope| App());
///     })
/// }
/// ```
#[kompoz_runtime::named]
pub fn application(
    #[default(true)] exit_process_on_exit: bool,
    content: impl Fn(&ApplicationScope) + 'static,
) -> ExitCode {
    ended(App::new(program_name()).application(content), exit_process_on_exit)
}

/// Compose Desktop's `singleWindowApplication`: an app of one window (`application { Window(…) }`), its close request
/// `exitApplication`, `content` given the window's and the app's scopes together ([`SingleWindowApplicationScope`]:
/// its `window`, its `MenuBar`, `exit_application`). The window's parameters are [`Window`]'s; `state` isn't
/// remembered (made once by the caller: Compose's `WindowState()`). With `exit_process_on_exit` the process ends as
/// the app does.
#[allow(clippy::too_many_arguments)]
#[kompoz_runtime::named]
pub fn single_window_application(
    #[default(WindowState::default())] state: WindowState,
    #[default(true)] visible: bool,
    #[default("Untitled".to_string())] title: String,
    #[default(None)] icon: Option<Rc<dyn Painter>>,
    #[default(false)] undecorated: bool,
    #[default(false)] transparent: bool,
    #[default(true)] resizable: bool,
    #[default(true)] enabled: bool,
    #[default(true)] focusable: bool,
    #[default(false)] always_on_top: bool,
    #[default(|_: &KeyEvent| false)] on_preview_key_event: impl Fn(&KeyEvent) -> bool + 'static,
    #[default(|_: &KeyEvent| false)] on_key_event: impl Fn(&KeyEvent) -> bool + 'static,
    #[default(true)] exit_process_on_exit: bool,
    content: impl Fn(&SingleWindowApplicationScope) + 'static,
) -> ExitCode {
    let app = App::new(program_name());
    let content = single_window_content(
        state,
        visible,
        title,
        icon,
        undecorated,
        transparent,
        resizable,
        enabled,
        focusable,
        always_on_top,
        Rc::new(on_preview_key_event),
        Rc::new(on_key_event),
        Rc::new(content),
    );
    ended(app.application(content), exit_process_on_exit)
}
