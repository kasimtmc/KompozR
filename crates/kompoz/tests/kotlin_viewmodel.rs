//! ViewModels and saved state in Kotlin's syntax, as a Compose user writes them: the C++ Kompoz's TodoScreen
//! (kompozable_tests.cpp `classes_as_kotlin_writes_them`: its ViewModel, handle and effect parts — `viewModel<T>()`,
//! a ViewModel's state flow and function, `LifecycleResumeEffect`; QueryViewModel's `state["query"] = q`), and
//! lifecycle-viewmodel-compose's and savedstate's samples from kt/viewmodel.kt (kompoz_file!). The same behaviour
//! crates/kompoz-lifecycle/tests/viewmodel.rs checks in Rust.

#![allow(non_snake_case, non_upper_case_globals)]

mod common;

use kompoz::prelude::*;
use kompoz::test::KompozTestRule;
use std::cell::Cell;
use std::rc::Rc;

// ---- Rust twins of the Kotlin classes (stage 14) -------------------------------------------------------------------

/// kompozable_tests.cpp's `data class Todo(val id: Int, val title: String, val done: Boolean = false)`.
#[derive(Clone, PartialEq, Debug, KotlinProperties)]
pub struct Todo {
    pub id: i32,
    pub title: String,
    pub done: bool,
}

/// kompozable_tests.cpp's `class TodoViewModel : ViewModel() { val items = MutableStateFlow(listOf(Todo(1, "Yaz"),
/// Todo(2, "Gönder"))); var toggles = 0; fun toggle(id: Int, done: Boolean) { toggles++; items.update { list ->
/// list.map { if (it.id == id) it.copy(done = done) else it } } }; override fun onCleared() {} }`.
pub type TodoViewModel = Rc<TodoViewModelClass>;

/// TodoViewModel's object.
pub struct TodoViewModelClass {
    base: ViewModelImpl,
    items: MutableStateFlow<Vec<Todo>>,
    toggles: Cell<i32>,
}

impl TodoViewModelClass {
    pub fn items(&self) -> MutableStateFlow<Vec<Todo>> {
        self.items.clone()
    }
    pub fn toggles(&self) -> i32 {
        self.toggles.get()
    }
    pub fn toggle(&self, id: i32, done: bool) {
        self.toggles.set(self.toggles.get() + 1);
        self.items.update(|list| {
            list.iter().map(|it| if it.id == id { Todo { done, ..it.clone() } } else { it.clone() }).collect()
        });
    }
}

impl ViewModel for TodoViewModelClass {
    fn view_model_impl(&self) -> &ViewModelImpl {
        &self.base
    }
    fn new_instance(_extras: &CreationExtras) -> Option<Self> {
        let todo = |id, title: &str| Todo { id, title: title.into(), done: false };
        Some(TodoViewModelClass {
            base: ViewModelImpl::new(),
            items: MutableStateFlow::new(vec![todo(1, "Yaz"), todo(2, "Gönder")]),
            toggles: Cell::new(0),
        })
    }
}

/// kompozable_tests.cpp's `class QueryViewModel(private val state: SavedStateHandle) : ViewModel() { val query:
/// StateFlow<String> = state.getStateFlow("query", ""); fun setQuery(q: String) { state["query"] = q } }`.
pub type QueryViewModel = Rc<QueryViewModelClass>;

/// QueryViewModel's object.
pub struct QueryViewModelClass {
    base: ViewModelImpl,
    state: SavedStateHandle,
    query: StateFlow<String>,
}

impl QueryViewModelClass {
    pub fn new(state: SavedStateHandle) -> QueryViewModelClass {
        let query = state.get_state_flow("query", String::new());
        QueryViewModelClass { base: ViewModelImpl::new(), state, query }
    }
    pub fn query(&self) -> StateFlow<String> {
        self.query.clone()
    }
    pub fn set_query(&self, q: impl Into<String>) {
        self.state.set("query", q.into());
    }
}

impl ViewModel for QueryViewModelClass {
    fn view_model_impl(&self) -> &ViewModelImpl {
        &self.base
    }
    fn new_instance(extras: &CreationExtras) -> Option<Self> {
        Some(QueryViewModelClass::new(extras.create_saved_state_handle()))
    }
}

/// kompozable_tests.cpp's `class Player { var playing = false; fun play() { playing = true } fun pause() { playing =
/// false } }` (a copy is the same object, as Kotlin's).
#[derive(Clone, Default)]
pub struct Player(Rc<Cell<bool>>);

impl Player {
    pub fn play(&self) {
        self.0.set(true);
    }
    pub fn pause(&self) {
        self.0.set(false);
    }
    pub fn playing(&self) -> bool {
        self.0.get()
    }
}

/// The samples' `class TestViewModel(val args: String?) : ViewModel()`.
pub struct TestViewModel {
    base: ViewModelImpl,
    args: Option<String>,
}

impl TestViewModel {
    pub fn args(&self) -> Option<String> {
        self.args.clone()
    }
}

impl ViewModel for TestViewModel {
    fn view_model_impl(&self) -> &ViewModelImpl {
        &self.base
    }
}

pub fn TestViewModel(args: impl Into<Option<String>>) -> TestViewModel {
    TestViewModel { base: ViewModelImpl::new(), args: args.into() }
}

/// The samples' `class SavedStateViewModel(val handle: SavedStateHandle, val value: String) : ViewModel()`.
pub struct SavedStateViewModel {
    base: ViewModelImpl,
    handle: SavedStateHandle,
    value: String,
}

impl SavedStateViewModel {
    pub fn handle(&self) -> SavedStateHandle {
        self.handle.clone()
    }
    pub fn value(&self) -> String {
        self.value.clone()
    }
}

impl ViewModel for SavedStateViewModel {
    fn view_model_impl(&self) -> &ViewModelImpl {
        &self.base
    }
}

pub fn SavedStateViewModel(handle: SavedStateHandle, value: impl Into<String>) -> SavedStateViewModel {
    SavedStateViewModel { base: ViewModelImpl::new(), handle, value: value.into() }
}

/// CreationExtrasViewModel's local `object : ViewModelProvider.Factory { override fun <T : ViewModel> create(modelClass:
/// Class<T>, extras: CreationExtras): T { val args = extras[DEFAULT_ARGS_KEY]?.getString("test"); return
/// TestViewModel(args) as T } }`: the factory, and its `create` as the sample calls it (typed by the class given).
#[derive(Clone)]
pub struct CustomFactory {}

pub fn CustomFactory() -> CustomFactory {
    CustomFactory {}
}

impl CustomFactory {
    pub fn create(&self, model_class: KClass, extras: MutableCreationExtras) -> Rc<TestViewModel> {
        assert_eq!(model_class, KClass::of::<TestViewModel>());
        let args = extras.get(&DEFAULT_ARGS_KEY).and_then(|s| s.read(|r| r.get_string_or_null("test")));
        Rc::new(TestViewModel(args))
    }
}

impl ViewModelProviderFactory for CustomFactory {
    fn create(&self, model_class: &KClass, extras: &CreationExtras) -> Rc<dyn ViewModel> {
        CustomFactory::create(self, *model_class, MutableCreationExtras::new(extras))
    }
}

/// The codec samples' local `@Serializable data class User(val id: Int, val name: String)`.
#[derive(Clone, PartialEq, Debug, Saveable, KotlinProperties)]
pub struct User {
    pub id: i32,
    pub name: String,
}

pub fn User(id: i32, name: String) -> User {
    User { id, name }
}

/// The codec samples' `class UUIDSerializer : KSerializer<UUID>` (a UUID's text, as a string): a Saver.
pub fn UUIDSerializer() -> Saver<String> {
    Saver::new(|uuid: &String| SavedValue::from(uuid.as_str()), |v| v.as_str().map(str::to_string))
}

kompoz_file!("tests/kt/viewmodel.kt");

// ---- tests ----------------------------------------------------------------------------------------------------------

/// C++ `classes_as_kotlin_writes_them` (kompozable_tests.cpp 2625), its ViewModel, handle and effect parts: the
/// screen's ViewModel (the host's) shows its items and toggles one; the resume effect plays while the app is
/// resumed; `QueryViewModel`'s `state["query"] = q` reaches its flow. (Greeter, a class with a primary constructor, is
/// stage 14's.)
fn classes_as_kotlin_writes_them() {
    let player = Player::default();
    let p = player.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || TodoScreen(p.clone())));
    rule.on_node_with_text("Yaz").assert_exists();
    assert!(player.playing()); // (the composable copied the handle: the same object)
    rule.on_node_with_text("Yaz").perform_click();
    rule.on_node_with_text("✓ Yaz").assert_exists();
    let vm: TodoViewModel =
        ViewModelProvider::create(&*rule.host().view_model_store_owner(), None, None).get::<TodoViewModelClass>();
    assert_eq!(vm.toggles(), 1);
    rule.set_lifecycle_state(LifecycleState::Started);
    assert!(!player.playing());
    let q = QueryViewModelClass::new(SavedStateHandle::new());
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

/// savedstate's codec samples: a `@Serializable` class (a `#[derive(Saveable)]` struct) encoded into a SavedState and
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
