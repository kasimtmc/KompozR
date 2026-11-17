//! What the animation tests share: a runner that runs the tests one after the other on one thread (made the UI
//! thread), as the UI does, and closeness checks.

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

#[macro_export]
macro_rules! tests {
    ($($name:ident),* $(,)?) => {
        fn main() {
            common::run(&[$((stringify!($name), $name as fn())),*]);
        }
    };
}

/// Whether `a` is within `tolerance` of `b`.
pub fn near(a: f32, b: f32, tolerance: f32) -> bool {
    (a - b).abs() <= tolerance
}

/// Asserts `a` is within `tolerance` of `b`, saying what they were.
#[track_caller]
pub fn assert_near(a: f32, b: f32, tolerance: f32) {
    assert!(near(a, b, tolerance), "{a} is not within {tolerance} of {b}");
}
