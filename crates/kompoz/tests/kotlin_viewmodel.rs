//! ViewModels and saved state in Kotlin's syntax, as a Compose user writes them: the C++ Kompoz's TodoScreen
//! (kompozable_tests.cpp `classes_as_kotlin_writes_them`, whole: its classes, `viewModel<T>()`, a ViewModel's state
//! flow and function, `LifecycleResumeEffect`; QueryViewModel's `state["query"] = q`), and
//! lifecycle-viewmodel-compose's and savedstate's samples from kt/viewmodel.kt (kompoz_file!). The same behaviour
//! crates/kompoz-lifecycle/tests/viewmodel.rs checks in Rust.

#![allow(non_snake_case, non_upper_case_globals)]

mod common;

use kompoz::prelude::*;
use kompoz::test::KompozTestRule;

// ---- what stays Rust ------------------------------------------------------------------------------------------

/// The codec samples' `class UUIDSerializer : KSerializer<UUID>` (a UUID's text, as a string): a Saver, KompozR's
/// explicit serializer (kotlinx.serialization's KSerializer/Encoder/Decoder and java.util.UUID aren't the API's).
pub fn UUIDSerializer() -> Saver<String> {
    Saver::new(|uuid: &String| SavedValue::from(uuid.as_str()), |v| v.as_str().map(str::to_string))
}

kompoz_file!("tests/kt/viewmodel.kt");

// ---- tests ----------------------------------------------------------------------------------------------------------

/// C++ `classes_as_kotlin_writes_them` (kompozable_tests.cpp 2625): the screen's ViewModel (the host's) shows its
/// items and toggles one; the resume effect plays while the app is resumed; Greeter's primary constructor, its
/// init block, a copy the same object; `QueryViewModel`'s `state["query"] = q` reaches its flow.
fn classes_as_kotlin_writes_them() {
    let player = Player();
    let p = player.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || TodoScreen(p.clone())));
    rule.on_node_with_text("Yaz").assert_exists();
    assert!(player.playing()); // (the composable copied the handle: the same object)
    rule.on_node_with_text("Yaz").perform_click();
    rule.on_node_with_text("✓ Yaz").assert_exists();
    let vm: std::rc::Rc<TodoViewModel> =
        ViewModelProvider::create(&*rule.host().view_model_store_owner(), None, None).get::<TodoViewModel>();
    assert_eq!(vm.toggles(), 1);
    rule.set_lifecycle_state(LifecycleState::Started);
    assert!(!player.playing());
    let g = named_args!(Greeter("Ada", " Lovelace"));
    assert!(g.greeting() == "Merhaba, Ada Lovelace" && g.greeted() == -1);
    assert!(g.greet() == "Merhaba, Ada Lovelace!" && g.greeted() == 0);
    let same = g.clone(); // (a copy is the same object, as Kotlin's)
    same.greet();
    assert!(g.greeted() == 1 && same == g);
    assert_eq!(
        Greeter::__kz_builder().name("Bo").suffix("").punctuation("?".to_string()).__call().greet(),
        "Merhaba, Bo?"
    );
    let _ = ticks(); // (it compiles: a Flow<i32> from flow { emit(i) })
    let q = QueryViewModel(SavedStateHandle::new());
    q.set_query("kompoz");
    assert_eq!(q.query().value(), "kompoz");
    let handle = SavedStateHandle::new();
    assert_eq!(set_and_get(handle.clone(), "kedi"), "kedi");
    assert_eq!(handle.keys(), ["query"]);
    // QueryViewModel's screen: its query set from the screen, saved in its handle.
    let rule = KompozTestRule::new(kompozable_lambda!(|| SearchScreen()));
    rule.on_node_with_text("uygulama ").perform_click();
    rule.wait_for_idle();
    rule.on_node_with_text("uygulama !").assert_exists();
    let bytes = rule.host().save_state();
    drop(rule);
    let rule = KompozTestRule::new(kompozable_lambda!(|| SearchScreen()));
    rule.host().restore_state(&bytes);
    rule.wait_for_idle();
    rule.on_node_with_text("uygulama !").assert_exists();
}

/// lifecycle-viewmodel-compose's samples: a factory given custom extras, a ViewModel made by an initializer with a
/// handle of its own, owners scoped to a call site (its ViewModels cleared as it leaves), to a pager's pages and
/// to a feature's key (shared by its parts).
fn lifecycle_viewmodel_compose_samples() {
    let detail = mutable_state_of(true);
    let d = detail.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        Column(
            content = || {
                CreationExtrasViewModel();
                CreationExtrasViewModelInitializer();
                RememberViewModelStoreOwnerSample(d.get());
                RememberViewModelStoreOwnerWithKeySample();
                RememberViewModelStoreProviderSample();
                RememberViewModelStoreProviderWithKeySample();
            },
        );
    }));
    rule.on_node_with_text("args my_value").assert_exists();
    rule.on_node_with_text("custom_value 0").assert_exists();
    rule.on_node_with_text("detail_data").assert_exists();
    for page in ["Page 1", "Page 2", "Page 3"] {
        rule.on_node_with_text(page).assert_exists();
    }
    rule.on_node_with_text("isolated_data").assert_exists();
    // (both parts' owners are the provider's store of one key: one ViewModel, the first part's)
    rule.on_node_with_text("A shared").assert_exists();
    rule.on_node_with_text("B shared").assert_exists();
    detail.set(false);
    rule.wait_for_idle();
    rule.on_node_with_text("detail_data").assert_does_not_exist();
}

/// savedstate's codec samples: a `@Serializable` class encoded into a SavedState and
/// decoded from one written by hand, with an explicit serializer too; a saved state read back.
fn savedstate_samples() {
    let saved = encode();
    assert!(saved.read(|r| r.get_int("id") == 123 && r.get_string("name") == "foo"));
    assert_eq!(decode(), User { id: 123, name: "foo".into() });
    assert_eq!(encode_with_explicit_serializer().read(|r| r.get_string("")), "0f0e0d0c-0b0a-0908-0706-050403020100");
    assert_eq!(decode_with_explicit_serializer(), "0f0e0d0c-0b0a-0908-0706-050403020100");
    assert_eq!(user_name(user_saved_state()), "foo");
    assert_eq!(user_name(SavedState::new()), "?");
}

fn main() {
    common::run(&[
        ("classes_as_kotlin_writes_them", classes_as_kotlin_writes_them),
        ("lifecycle_viewmodel_compose_samples", lifecycle_viewmodel_compose_samples),
        ("savedstate_samples", savedstate_samples),
    ]);
}
