//! The harness of the Kotlin-syntax tests: each test file's `main` runs its tests one after the other on one thread,
//! made the UI thread (`harness = false`: composition is single-threaded).

#![allow(dead_code)]

use kompoz::runtime::set_ui_thread;

/// Runs the tests given one after the other on this thread, made the UI thread; the arguments name the ones to
/// run (all if none).
pub fn run(tests: &[(&str, fn())]) {
    set_ui_thread();
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

/// What `f` panics with (its message: a Kotlin exception's text), the panic hook quiet meanwhile; none if it doesn't.
pub fn thrown<R>(f: impl FnOnce() -> R + std::panic::UnwindSafe) -> Option<String> {
    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let result = std::panic::catch_unwind(f);
    std::panic::set_hook(hook);
    let e = result.err()?;
    Some(
        e.downcast_ref::<String>()
            .cloned()
            .or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string()))
            .unwrap_or_default(),
    )
}

/// `fn main` running the tests named.
#[macro_export]
macro_rules! tests {
    ($($name:ident),* $(,)?) => {
        fn main() {
            $crate::common::run(&[$((stringify!($name), $name as fn())),*]);
        }
    };
}

/// Waits for idle with the clock advancing on its own, as it did before (Compose's `mainClock.autoAdvance = true;
/// waitForIdle()`): what a test stepping the clock by hand does to let every animation run its course. The clock's
/// `auto_advance` is put back as it was after.
pub fn settle(rule: &kompoz::test::KompozTestRule) {
    let clock = rule.main_clock();
    let was = clock.auto_advance();
    clock.set_auto_advance(true);
    rule.wait_for_idle();
    clock.set_auto_advance(was);
}
