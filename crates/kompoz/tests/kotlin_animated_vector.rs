//! Animated vectors in Kotlin's syntax, as a Compose user writes them: `Image(rememberAnimatedVectorPainter(
//! animatedImageVector = …, atEnd = atEnd), …)` played to its end and back (each frame what the image's
//! `vector_at` says); and in a `.kt` file (kompoz_file!) Compose's AnimatedVectorSample (its image read from XML,
//! Kotlin's `Result.getOrThrow()`, toggled by a click) and a failed read as Kotlin reads a Result. The same behaviour
//! crates/kompoz-animation-graphics/tests checks in Rust.

#![allow(non_snake_case, non_upper_case_globals)]

mod common;

use kompoz::prelude::*;
use kompoz::test::KompozTestRule;
use std::cell::RefCell;
use std::rc::Rc;

/// A dot turning and a bar changing color, one after the other.
const SPINNER: &str = r##"<animated-vector xmlns:android="http://schemas.android.com/apk/res/android" xmlns:aapt="http://schemas.android.com/aapt">
    <aapt:attr name="android:drawable">
        <vector android:width="24dp" android:height="24dp" android:viewportWidth="24" android:viewportHeight="24">
            <group android:name="spin" android:pivotX="12" android:pivotY="12">
                <path android:name="dot" android:strokeColor="#000000" android:strokeWidth="1" android:pathData="M12,2L12,6"/>
            </group>
            <path android:name="bar" android:fillColor="#FF0000" android:pathData="M0,0h4v4h-4z"/>
        </vector>
    </aapt:attr>
    <target android:name="spin">
        <aapt:attr name="android:animation">
            <objectAnimator android:propertyName="rotation" android:valueFrom="0" android:valueTo="180" android:duration="160"
                android:interpolator="@android:interpolator/linear"/>
        </aapt:attr>
    </target>
    <target android:name="bar">
        <aapt:attr name="android:animation">
            <objectAnimator android:propertyName="fillColor" android:valueFrom="#FF0000" android:valueTo="#0000FF" android:duration="100"
                android:startOffset="160"/>
        </aapt:attr>
    </target>
</animated-vector>"##;

kompoz! {
    @Composable
    fun Spinner(image: AnimatedImageVector, atEnd: Boolean, seen: (VectorPainter) -> Unit) {
        val painter = rememberAnimatedVectorPainter(animatedImageVector = image, atEnd = atEnd)
        seen(painter)
        Image(painter, contentDescription = "dönen", modifier = Modifier.size(24.dp))
    }
}

fn rotation(v: &ImageVector) -> f32 {
    v.root().iter().filter_map(VectorNode::group).find(|g| g.name == "spin").expect("the group").rotation
}

fn animated_vector_painter_as_compose_writes_it() {
    let image = AnimatedImageVector::from_animated_vector_xml(SPINNER).expect("an animated vector");
    assert_eq!(image.total_duration(), 260);
    let at_end = mutable_state_of(false);
    let seen: Rc<RefCell<Option<VectorPainter>>> = Rc::new(RefCell::new(None));
    let (i, a, s) = (image.clone(), at_end.clone(), seen.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let s = s.clone();
        Spinner(i.clone(), a.get(), move |p| *s.borrow_mut() = Some(p));
    }));
    let painted = || seen.borrow().as_ref().expect("composed").vector.clone();
    assert_eq!(rotation(&painted()), 0.0);
    assert!(rule.draw().vectors.contains(&painted().id()));
    at_end.set(true);
    rule.advance_time_by(80);
    // Mid-way: a frame's time, what vector_at says of it.
    let now = painted();
    let t = (0..=10).map(|f| f as f32 * 16.0).find(|t| image.vector_at(*t).root() == now.root()).expect("a frame");
    assert!(t > 0.0 && (rotation(&now) - 180.0 * t / 160.0).abs() < 1e-3, "{t}");
    rule.wait_for_idle();
    assert_eq!(painted().root(), image.vector_at(260.0).root());
    at_end.set(false);
    rule.wait_for_idle();
    assert_eq!(rotation(&painted()), 0.0);
}

// ---- kompoz_file! ------------------------------------------------------------------------------------------------

kompoz_file!("tests/kt/animated_vector.kt");

fn the_animated_vector_sample_in_a_kt_file() {
    let seen = Rc::new(RefCell::new(Vec::<(bool, VectorPainter)>::new()));
    let s = seen.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let s = s.clone();
        AnimatedVectorSample(SPINNER.to_string(), move |at_end, painter| s.borrow_mut().push((at_end, painter)));
    }));
    let last = || seen.borrow().last().cloned().expect("composed");
    assert!(!last().0 && rotation(&last().1.vector) == 0.0);
    rule.on_node_with_content_description("Your content description").perform_click();
    rule.wait_for_idle();
    assert!(last().0 && rotation(&last().1.vector) == 180.0);
    rule.on_node_with_content_description("Your content description").perform_click();
    rule.wait_for_idle();
    assert!(!last().0 && rotation(&last().1.vector) == 0.0);
}

fn a_failed_read_as_kotlin_reads_a_result() {
    let seen = Rc::new(RefCell::new(Vec::<(bool, Option<String>, i32)>::new()));
    let s = seen.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let s = s.clone();
        ReadOrNot(SPINNER.to_string(), {
            let s = s.clone();
            move |ok, error, duration| s.borrow_mut().push((ok, error, duration))
        });
        ReadOrNot("<animated-vector/>".to_string(), move |ok, error, duration| {
            s.borrow_mut().push((ok, error, duration))
        });
    }));
    rule.wait_for_idle();
    let seen = seen.borrow();
    assert_eq!(seen[0], (true, None, 260));
    assert_eq!(seen[1], (false, Some("NotFoundException: Resource ID #0x0".to_string()), -1));
    // getOrThrow of a failure: Kotlin's exception thrown (a panic of its text).
    let thrown = common::thrown(|| {
        use kompoz::kt::KtResult;
        AnimatedImageVector::from_animated_vector_xml("not xml").get_or_throw()
    });
    assert_eq!(thrown.as_deref(), Some("XmlPullParserException: No start tag found"));
}

tests!(
    animated_vector_painter_as_compose_writes_it,
    the_animated_vector_sample_in_a_kt_file,
    a_failed_read_as_kotlin_reads_a_result,
);
