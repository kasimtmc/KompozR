//! `AnimatedImageVector.animatedVectorResource` on KompozR's resources: an `<animated-vector>` drawable of
//! tests/composeResources read through its `Res`, its `android:drawable="@drawable/spin_vector"` that `Res`'s
//! (picked for the same environment: the dark theme's in a dark one), played by rememberAnimatedVectorPainter; a
//! reference to nothing is Android's NotFoundException.

#![allow(non_snake_case, non_upper_case_globals)]

mod common;

use common::*;
use kompoz_animation_graphics::*;
use kompoz_foundation::prelude::*;
use kompoz_graphics::*;
use kompoz_resources::{
    ComposeEnvironment, DensityQualifier, DrawableResource, LanguageQualifier, LocalComposeEnvironment,
    RegionQualifier, ResourceEnvironment, ResourceItem, ThemeQualifier, compose_resources, default_resource_reader,
};
use kompoz_runtime::*;
use kompoz_test::*;
use kompoz_ui::Modifier;
use std::cell::RefCell;
use std::rc::Rc;

compose_resources!(dir = "tests/composeResources");

fn rotation(v: &ImageVector) -> f32 {
    v.root().iter().filter_map(VectorNode::group).find(|g| g.name == "spin").expect("the group").rotation
}

fn environment(dark: bool) -> ResourceEnvironment {
    ResourceEnvironment::new(
        LanguageQualifier::new("en"),
        RegionQualifier::new(""),
        ThemeQualifier::select_by_value(dark),
        DensityQualifier::Mdpi,
    )
}

/// Read in a composition and played: its drawable the referenced one, rotated to its end.
fn animated_vector_resources_are_read_and_played() {
    let at_end = mutable_state_of(false);
    let seen: Rc<RefCell<Option<(AnimatedImageVector, kompoz_ui::draw::VectorPainter)>>> = Rc::new(RefCell::new(None));
    let (a, s) = (at_end.clone(), seen.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let image = animated_vector_resource(Res::drawable::spin());
        let painter = remember_animated_vector_painter(image.clone(), a.get());
        *s.borrow_mut() = Some((image, painter.clone()));
        Image(std::rc::Rc::new(painter), Some("dönen".into()), modifier = Modifier.size(24.dp()));
    }));
    let image = seen.borrow().as_ref().unwrap().0.clone();
    assert_eq!(image.total_duration(), 160);
    assert_eq!(image.image_vector().default_width(), Dp(24.0));
    at_end.set(true);
    rule.wait_for_idle();
    let painted = seen.borrow().as_ref().unwrap().1.vector();
    assert_near(rotation(&painted), 180.0, 1e-3);
}

/// Its reference picked for the environment (the dark theme's drawable); one to nothing a NotFoundException.
fn references_follow_the_environment() {
    let reader = default_resource_reader();
    Res::register();
    let dark = load_animated_vector_resource(&Res::drawable::spin(), &*reader, &environment(true)).unwrap();
    assert_eq!(dark.image_vector().default_width(), Dp(48.0));
    let light = load_animated_vector_resource(&Res::drawable::spin(), &*reader, &environment(false)).unwrap();
    assert_eq!(light.image_vector().default_width(), Dp(24.0));
    // In a composition whose environment is dark.
    let seen: Rc<RefCell<Option<AnimatedImageVector>>> = Rc::new(RefCell::new(None));
    let s = seen.clone();
    let _rule = KompozTestRule::new(kompozable_lambda!(move || {
        let s = s.clone();
        KompozitionLocalProvider(
            &[LocalComposeEnvironment.provides(ComposeEnvironment::fixed(environment(true)))],
            || {
                *s.borrow_mut() = Some(animated_vector_resource(Res::drawable::spin()));
            },
        );
    }));
    assert_eq!(seen.borrow().as_ref().unwrap().image_vector().default_width(), Dp(48.0));
    // A reference to a drawable its Res doesn't have.
    let missing = DrawableResource::new(
        "drawable:missing",
        vec![ResourceItem::new(vec![], Res::drawable::spin().items()[0].path.replace("spin.xml", "none.xml"), -1, -1)],
    );
    let e = load_animated_vector_resource(&missing, &*reader, &environment(false)).unwrap_err();
    assert_eq!(e.exception, "NotFoundException");
}

tests!(animated_vector_resources_are_read_and_played, references_follow_the_environment);
