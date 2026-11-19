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

/// The C++ test's animated vector with its `alpha` target (Compose can't play it) as a second target of "bar"
/// animating its fillAlpha the same way (merged with the first, as Compose merges targets of one name): the
/// fixture of the data and painter halves here.
pub const ANIMATED: &str = r##"<?xml version="1.0" encoding="utf-8"?>
<animated-vector xmlns:android="http://schemas.android.com/apk/res/android" xmlns:aapt="http://schemas.android.com/aapt">
    <aapt:attr name="android:drawable">
        <vector android:name="root" android:width="24dp" android:height="24dp" android:viewportWidth="24" android:viewportHeight="24">
            <group android:name="spin" android:pivotX="12" android:pivotY="12">
                <path android:name="dot" android:strokeColor="#000000" android:strokeWidth="1" android:pathData="M12,2L12,6"/>
            </group>
            <path android:name="bar" android:fillColor="#FF0000" android:pathData="M0,0h4v4h-4z"/>
            <path android:name="line" android:strokeColor="#000000" android:strokeWidth="1" android:pathData="M0,20h24"/>
        </vector>
    </aapt:attr>
    <target android:name="spin">
        <aapt:attr name="android:animation">
            <set>
                <objectAnimator android:propertyName="rotation" android:valueFrom="90" android:valueTo="180" android:startOffset="100"
                    android:duration="200" android:interpolator="@android:interpolator/linear"/>
                <objectAnimator android:propertyName="rotation" android:valueFrom="0" android:valueTo="90" android:duration="100"
                    android:interpolator="@android:interpolator/linear"/>
            </set>
        </aapt:attr>
    </target>
    <target android:name="bar">
        <aapt:attr name="android:animation">
            <set android:ordering="sequentially">
                <objectAnimator android:propertyName="fillColor" android:valueFrom="#FF0000" android:valueTo="#0000FF" android:duration="100"
                    android:interpolator="@android:interpolator/linear"/>
                <objectAnimator android:propertyName="pathData" android:valueType="pathType" android:valueFrom="M0,0h4v4h-4z"
                    android:valueTo="M0,0h8v8h-8z" android:duration="100" android:interpolator="@android:anim/linear_interpolator"/>
            </set>
        </aapt:attr>
    </target>
    <target android:name="line">
        <aapt:attr name="android:animation">
            <objectAnimator android:duration="60" android:startOffset="100" android:repeatCount="1" android:repeatMode="reverse"
                android:interpolator="@android:interpolator/linear">
                <propertyValuesHolder android:propertyName="trimPathEnd" android:valueFrom="0.8" android:valueTo="0"/>
                <propertyValuesHolder android:propertyName="strokeWidth" android:valueTo="3"/>
            </objectAnimator>
        </aapt:attr>
    </target>
    <target android:name="bar">
        <aapt:attr name="android:animation">
            <objectAnimator android:propertyName="fillAlpha" android:valueFrom="1" android:valueTo="0.5" android:duration="200"
                android:interpolator="@android:interpolator/accelerate_quad"/>
        </aapt:attr>
    </target>
</animated-vector>"##;
