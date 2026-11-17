//! KompozR's Counter: a count and a button that adds to it, from foundation's pieces (Box, Column, BasicText,
//! `clickable`, `background`), the count kept across runs with `KOMPOZ_STATE_FILE` (`remember_saveable`).
//!
//! ```text
//! cargo run -p counter
//! KOMPOZ_HEADLESS=1 KOMPOZ_SCRIPT="wait 30;click 70 70;wait 30;shot shot.png;quit" cargo run -p counter
//! ```

#![allow(non_snake_case)]

mod counter;

use counter::Counter;
use kompoz_graphics::UnitExt;
use kompoz_platform::App;
use std::process::ExitCode;

fn main() -> ExitCode {
    App::new("Sayaç").size(360.dp(), 480.dp()).run(Counter)
}
