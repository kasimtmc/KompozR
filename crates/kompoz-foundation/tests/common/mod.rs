//! What the UI's tests share: a runner that runs the tests one after the other on one thread (made the UI thread),
//! as the UI does.

#![allow(dead_code)]

/// Runs the tests given one after the other on this thread, made the UI thread; the arguments name the ones to
/// run (all if none).
pub fn run(tests: &[(&str, fn())]) {
    kompoz_runtime::set_ui_thread();
    let filters: Vec<String> = std::env::args().skip(1).filter(|a| !a.starts_with('-')).collect();
    let mut failed = Vec::new();
    let mut ran = 0;
    for (name, test) in tests {
        if !filters.is_empty() && !filters.iter().any(|f| name.contains(f.as_str())) {
            continue;
        }
        ran += 1;
        if std::panic::catch_unwind(test).is_err() {
            failed.push(*name);
            println!("FAILED {name}");
        } else {
            println!("ok     {name}");
        }
    }
    println!("\n{ran} run, {} failed", failed.len());
    if !failed.is_empty() {
        std::process::exit(1);
    }
}

/// Gives this thread's fields Compose's default key mapping (Linux's) whatever the platform: the tests of keys
/// ported from C++ Kompoz's press its keys (Ctrl by words and for shortcuts, Ctrl+Shift+Z redoing, Home and End by
/// lines), which macOS's mapping (Cmd shortcuts, Option by words) gives other commands. The platforms' own mappings
/// are text_input's `key_mappings_follow_the_platform`'s.
pub fn use_the_default_key_mapping() {
    use kompoz_foundation::text::{DesktopPlatform, create_platform_default_key_mapping, set_key_mapping_override};
    set_key_mapping_override(Some(create_platform_default_key_mapping(DesktopPlatform::Linux)));
}

#[macro_export]
macro_rules! tests {
    // (the tests of keys: with Compose's default key mapping on every platform, `use_the_default_key_mapping`)
    (default_key_mapping; $($name:ident),* $(,)?) => {
        fn main() {
            use_the_default_key_mapping();
            run(&[$((stringify!($name), $name as fn())),*]);
        }
    };
    ($($name:ident),* $(,)?) => {
        fn main() {
            run(&[$((stringify!($name), $name as fn())),*]);
        }
    };
}

/// Waits for idle with the clock advancing on its own (Compose's `mainClock.autoAdvance = true; waitForIdle()`):
/// what a test stepping the clock by hand does to let every animation run its course. The clock's `auto_advance`
/// is put back as it was after.
pub fn settle(rule: &kompoz_test::KompozTestRule) {
    let clock = rule.main_clock();
    let was = clock.auto_advance();
    clock.set_auto_advance(true);
    rule.wait_for_idle();
    clock.set_auto_advance(was);
}
