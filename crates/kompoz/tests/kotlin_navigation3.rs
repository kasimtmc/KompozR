//! (stage 13 WP4) Navigation 3 in Kotlin's syntax, as a Compose app writes it: navigation3-ui's and navigation3-runtime's
//! samples from kt/navigation3.kt (kompoz_file!) — NavDisplay with a remembered back stack and the entry provider DSL,
//! the dialog scene strategy, a hoisted scene state with its navigation back handler, scenes giving transitions,
//! back stacks switched and concatenated, entries with their ViewModels. The same behaviour
//! crates/kompoz-navigation3/tests/navigation3.rs checks in Rust.

#![allow(non_snake_case, non_upper_case_globals)]

mod common;

use kompoz::prelude::*;
use kompoz::test::KompozTestRule;
use kompoz::ui::input::KeyModifiers;
use kompoz::ui::layout::SlotKey;
use kompoz::ui::window::DialogProperties;
use std::rc::Rc;

// ---- Rust twins of the Kotlin classes (stage 14) ------------------------------------------------------------------

/// OverlaySamples.kt's `@Serializable private object OverlaidNavKey : NavKey`.
#[derive(Saveable, Clone, PartialEq, Debug)]
pub struct OverlaidNavKey;
impl NavKey for OverlaidNavKey {}

/// OverlaySamples.kt's `@Serializable private object DialogNavKey : NavKey`.
#[derive(Saveable, Clone, PartialEq, Debug)]
pub struct DialogNavKey;
impl NavKey for DialogNavKey {}

/// CustomSceneSamples.kt's `@Serializable private object A : NavKey`.
#[derive(Saveable, Clone, PartialEq, Debug)]
pub struct KeyA;
impl NavKey for KeyA {}

/// CustomSceneSamples.kt's `@Serializable private object B : NavKey`.
#[derive(Saveable, Clone, PartialEq, Debug)]
pub struct KeyB;
impl NavKey for KeyB {}

/// CustomSceneSamples.kt's `private val slideHorizontal` (5 s, LinearOutSlowInEasing).
fn slide_horizontal() -> ContentTransform {
    let spec = || -> std::sync::Arc<dyn FiniteAnimationSpec<IntOffset>> {
        std::sync::Arc::new(named_args!(tween(5000, easing = std::sync::Arc::new(LinearOutSlowInEasing))))
    };
    named_args!(slide_in_horizontally(spec(), initial_offset_x = |it: i32| it / 2))
        .together_with(named_args!(slide_out_horizontally(spec(), target_offset_x = |it: i32| -it / 2)))
}

/// CustomSceneSamples.kt's `private data class DefaultSceneTransitionsScene<T : Any>(val currentEntry: NavEntry<T>,
/// override val previousEntries: List<NavEntry<T>>) : Scene<T>`: its metadata the scene's transitions.
#[derive(PartialEq, Debug)]
pub struct DefaultSceneTransitionsScene<T> {
    current_entry: NavEntry<T>,
    previous_entries: Vec<NavEntry<T>>,
}

impl<T: NavEntryKey> Scene<T> for DefaultSceneTransitionsScene<T> {
    fn key(&self) -> SlotKey {
        self.current_entry.content_key()
    }
    fn entries(&self) -> Vec<NavEntry<T>> {
        vec![self.current_entry.clone()]
    }
    fn previous_entries(&self) -> Vec<NavEntry<T>> {
        self.previous_entries.clone()
    }
    fn content(&self) {
        self.current_entry.Content();
    }
    // (overrides the default animations passed to NavDisplay: `sceneTransitions`)
    fn metadata(&self) -> Metadata {
        metadata(|m| {
            m.put(NavDisplay::TransitionKey, |_: &AnimatedContentTransitionScope<AnyScene>| slide_horizontal());
            m.put(NavDisplay::PopTransitionKey, |_: &AnimatedContentTransitionScope<AnyScene>| slide_horizontal());
            m.put(NavDisplay::PredictivePopTransitionKey, |_: &AnimatedContentTransitionScope<AnyScene>, _: i32| {
                slide_horizontal()
            });
        })
    }
}

/// CustomSceneSamples.kt's `private class DefaultSceneTransitionsSceneStrategy<T : Any>() : SceneStrategy<T>`.
#[derive(Default)]
pub struct DefaultSceneTransitionsSceneStrategy {
    _p: (),
}

pub fn DefaultSceneTransitionsSceneStrategy() -> DefaultSceneTransitionsSceneStrategy {
    DefaultSceneTransitionsSceneStrategy::default()
}

impl<T: NavEntryKey> SceneStrategy<T> for DefaultSceneTransitionsSceneStrategy {
    fn calculate_scene(&self, _: &SceneStrategyScope<T>, entries: Vec<NavEntry<T>>) -> Option<Rc<dyn Scene<T>>> {
        let last = entries.last()?.clone();
        Some(Rc::new(DefaultSceneTransitionsScene {
            current_entry: last,
            previous_entries: entries[..entries.len() - 1].to_vec(),
        }))
    }
}

/// NavigationSamples.kt's `@Serializable object Profile : NavKey`.
#[derive(Saveable, Clone, PartialEq, Debug)]
pub struct Profile;
impl NavKey for Profile {}

/// NavigationSamples.kt's `@Serializable data class Dashboard(val userId: String? = "no value given") : NavKey`.
#[derive(Saveable, KotlinProperties, Clone, PartialEq, Debug)]
pub struct Dashboard {
    pub user_id: Option<String>,
}
impl NavKey for Dashboard {}

/// Kotlin's `Dashboard()` (its default `userId`).
pub fn Dashboard() -> Dashboard {
    Dashboard { user_id: Some("no value given".into()) }
}

/// NavigationSamples.kt's `class ProfileViewModel : ViewModel() { val name = "no user" }`.
pub type ProfileViewModel = Rc<ProfileViewModelClass>;

pub struct ProfileViewModelClass {
    base: ViewModelImpl,
}

impl ProfileViewModelClass {
    pub fn name(&self) -> String {
        "no user".into()
    }
}

impl ViewModel for ProfileViewModelClass {
    fn view_model_impl(&self) -> &ViewModelImpl {
        &self.base
    }
    fn new_instance(_: &CreationExtras) -> Option<Self> {
        Some(ProfileViewModelClass { base: ViewModelImpl::new() })
    }
}

kompoz_file!("tests/kt/navigation3.kt");

// ---- tests ----------------------------------------------------------------------------------------------------------

fn escape(rule: &KompozTestRule) -> bool {
    let taken = rule.host().key_down(Key::Escape, KeyModifiers::None);
    rule.host().key_up(Key::Escape, KeyModifiers::None);
    rule.wait_for_idle();
    taken
}

fn shown(rule: &KompozTestRule, text: &str) -> bool {
    rule.drawn_texts().iter().any(|t| t == text)
}

/// OverlaySamples.kt's DialogSample: the dialog entry over the overlaid one, back closing it.
fn dialog_sample_as_compose_writes_it() {
    let rule = KompozTestRule::new(DialogSample);
    assert!(shown(&rule, "Overlaid Entry") && !shown(&rule, "Dialog"));
    rule.on_node_with_tag("open").perform_click();
    rule.wait_for_idle();
    assert!(shown(&rule, "Overlaid Entry") && shown(&rule, "Dialog"));
    assert!(escape(&rule));
    assert!(!shown(&rule, "Dialog"));
}

/// SceneStateSamples.kt's SceneStateSample: back pops through the hoisted handler.
fn scene_state_sample_as_compose_writes_it() {
    let rule = KompozTestRule::new(SceneStateSample);
    assert!(shown(&rule, "Key = b"));
    assert!(escape(&rule));
    assert!(shown(&rule, "Key = a") && !shown(&rule, "Key = b"));
    assert!(!escape(&rule));
}

/// CustomSceneSamples.kt's SceneDefaultTransitionsSample: the scene's horizontal slides, not NavDisplay's vertical
/// ones (both 5 s: mid-way both contents shown, offset horizontally).
fn scene_default_transitions_sample_as_compose_writes_it() {
    let rule = KompozTestRule::new(SceneDefaultTransitionsSample);
    assert!(shown(&rule, "A"));
    rule.main_clock().set_auto_advance(false);
    rule.on_node_with_tag("navigate").perform_click();
    rule.advance_time_by(2500);
    assert!(shown(&rule, "A") && shown(&rule, "B"));
    rule.main_clock().set_auto_advance(true);
    rule.wait_for_idle();
    assert!(shown(&rule, "B") && !shown(&rule, "A"));
    assert!(escape(&rule));
    assert!(shown(&rule, "A") && !shown(&rule, "B"));
}

/// BasicSamples.kt's SceneNav: Profile's ViewModel scoped to its entry, Dashboard navigated to and back.
fn scene_nav_as_compose_writes_it() {
    let rule = KompozTestRule::new(SceneNav);
    assert!(shown(&rule, "no user Profile"));
    rule.on_node_with_tag("Dashboard").perform_click();
    rule.wait_for_idle();
    assert!(shown(&rule, "no value given"));
    rule.on_node_with_tag("back").perform_click();
    rule.wait_for_idle();
    assert!(shown(&rule, "no user Profile") && !shown(&rule, "no value given"));
}

fn main() {
    common::run(&[
        (
            "scene_default_transitions_sample_as_compose_writes_it",
            scene_default_transitions_sample_as_compose_writes_it,
        ),
        ("scene_nav_as_compose_writes_it", scene_nav_as_compose_writes_it),
        ("dialog_sample_as_compose_writes_it", dialog_sample_as_compose_writes_it),
        ("scene_state_sample_as_compose_writes_it", scene_state_sample_as_compose_writes_it),
    ]);
}
