//! KompozR's Counter with its UI in Kotlin's syntax (`src/counter.kt`, read by `kompoz_file!`): the Rust Counter's
//! count and button (examples/counter), drawn the same, run on the platform's window (KOMPOZ_HEADLESS=1: without
//! one), the count kept across runs with `KOMPOZ_STATE_FILE`.
//!
//! ```text
//! cargo run -p counter-kotlin
//! KOMPOZ_HEADLESS=1 KOMPOZ_SCRIPT="wait 30;click 70 70;wait 30;shot shot.png;quit" cargo run -p counter-kotlin
//! ```

use kompoz::prelude::*;
use std::process::ExitCode;

kompoz_file!("src/counter.kt");

fn main() -> ExitCode {
    App::new("Sayaç").size(360.dp(), 480.dp()).run(Counter)
}
