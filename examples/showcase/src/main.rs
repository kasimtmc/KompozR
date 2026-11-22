//! KompozR's showcase (the C++ Kompoz showcase's shell and Material screen, in Kotlin's syntax: see lib.rs), run on
//! the platform's window (KOMPOZ_HEADLESS=1: without one), 440 × 900 dp as the C++ one.
//!
//! ```text
//! cargo run -p showcase
//! KOMPOZ_HEADLESS=1 KOMPOZ_SCRIPT="wait 10;click 385 830;wait 60;shot shot.png;quit" cargo run -p showcase
//! ```

use kompoz::prelude::*;
use std::process::ExitCode;

fn main() -> ExitCode {
    App::new("Kompoz Showcase").size(440.dp(), 900.dp()).run(showcase::ShowcaseApp)
}
