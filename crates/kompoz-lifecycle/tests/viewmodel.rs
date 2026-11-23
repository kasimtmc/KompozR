//! savedstate's and the ViewModels' tests: the C++ Kompoz's tests by name — `saved_state_handles_outlive_the_process`
//! and the host's half of `saved_state_outlives_the_host` (their navigation halves are the NavHost's) —, and
//! Compose's behaviour of savedstate (SavedState's reader and writer, the registry's consume-once and unconsumed
//! states saved again, the controller's checks, the serialization over `#[derive(Saveable)]`, the `saved` delegates),
//! lifecycle-viewmodel (the store and its clearing order, providers by class and key, factories and initializers,
//! creation extras), lifecycle-viewmodel-savedstate (SavedStateHandle's values, flows, providers, `saveable`) and
//! lifecycle-viewmodel-compose (`viewModel`, `rememberViewModelStoreOwner` / `…Provider`), and the host as the app's
//! owners (CMP's DefaultArchitectureComponentsOwner).

#![allow(non_snake_case)]

use kompoz_coroutines::{MutableStateFlow, StateFlow, delay_ms};
use kompoz_foundation::text::input::remember_text_field_state;
use kompoz_lifecycle::*;
use kompoz_runtime::kt::{AutoCloseable, KtDelegate, KtMutableDelegate};
use kompoz_runtime::*;
use kompoz_test::{KompozTestRule, Text};
use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

type Log = Rc<RefCell<Vec<String>>>;

fn log() -> Log {
    Rc::new(RefCell::new(Vec::new()))
}

fn taken(log: &Log) -> Vec<String> {
    std::mem::take(&mut *log.borrow_mut())
}

/// What `f` panics with.
fn panics(f: impl FnOnce()) -> String {
    let e = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)).expect_err("it panics");
    e.downcast_ref::<String>().cloned().or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string())).unwrap_or_default()
}

// ---- the C++ Kompoz's ViewModels ----------------------------------------------------------------------------------

/// kompozable_tests.cpp's `class QueryViewModel(private val state: SavedStateHandle) : ViewModel() { val query:
/// StateFlow<String> = state.getStateFlow("query", ""); fun setQuery(q: String) { state["query"] = q } }`.
struct QueryViewModel {
    base: ViewModelImpl,
    state: SavedStateHandle,
    query: StateFlow<String>,
}

impl QueryViewModel {
    fn new(state: SavedStateHandle) -> QueryViewModel {
        let query = state.get_state_flow("query", String::new());
        QueryViewModel { base: ViewModelImpl::new(), state, query }
    }

    fn set_query(&self, q: &str) {
        self.state.set("query", q.to_string());
    }
}

impl ViewModel for QueryViewModel {
    fn view_model_impl(&self) -> &ViewModelImpl {
        &self.base
    }
    // (its constructor takes the handle: the default factory gives it one)
    fn new_instance(extras: &CreationExtras) -> Option<Self> {
        Some(QueryViewModel::new(extras.create_saved_state_handle()))
    }
}

/// C++ `saved_state_handles_outlive_the_process`, the app's half (its navigation entry's ViewModel is the NavHost's):
/// what a ViewModel keeps in its SavedStateHandle is written out with the host's state and given back to the same
/// ViewModel (by owner and key) made again after the process ended; saved before it is made again, it is still
/// there.
fn saved_state_handles_outlive_the_process() {
    struct Run {
        shown: MutableState<bool>,
        app: Rc<RefCell<Option<Rc<QueryViewModel>>>>,
    }
    let content = |r: &Run| {
        let (shown, app) = (r.shown.clone(), r.app.clone());
        kompozable_lambda!(move || {
            if !shown.get() {
                return;
            }
            // (its constructor takes the handle: it gets it)
            let vm: Rc<QueryViewModel> = view_model();
            *app.borrow_mut() = Some(vm.clone());
            Text(format!("uygulama {}", vm.query.collect_as_state().get()));
        })
    };
    let bytes = {
        let r = Run { shown: mutable_state_of(false), app: Rc::default() };
        let rule = KompozTestRule::new(content(&r));
        r.shown.set(true);
        rule.wait_for_idle();
        let app = r.app.borrow().clone().unwrap();
        app.set_query("kedi");
        app.state.set("sayfa", 3);
        rule.wait_for_idle();
        rule.on_node_with_text("uygulama kedi").assert_exists();
        assert!(
            app.state.get::<i32>("sayfa") == Some(3)
                && app.state.get::<String>("query").as_deref() == Some("kedi")
                && app.state.get::<i32>("query").is_none()
        );
        assert_eq!(app.state.remove::<i32>("sayfa"), Some(3));
        assert!(!app.state.contains("sayfa") && app.state.keys() == ["query"]);
        rule.host().save_state()
    };
    // The process ended; the app comes back with the saved state: the same ViewModel gets what it had.
    let r = Run { shown: mutable_state_of(false), app: Rc::default() };
    let rule = KompozTestRule::new(content(&r));
    rule.host().restore_state(&bytes);
    // (saved before any came back: still theirs)
    assert_eq!(SavedValue::decode(&rule.host().save_state()), SavedValue::decode(&bytes));
    r.shown.set(true);
    rule.wait_for_idle();
    rule.on_node_with_text("uygulama kedi").assert_exists();
    // (saved again: the same)
    assert_eq!(SavedValue::decode(&rule.host().save_state()), SavedValue::decode(&bytes));
    // The ViewModel's handle as Compose keeps it: the host's saved state registry's, under the handles' provider.
    let saved = SavedValue::decode(&bytes).unwrap();
    let handles = saved
        .get("savedState")
        .get("androidx.lifecycle.BundlableSavedStateRegistry.key")
        .get("androidx.lifecycle.internal.SavedStateHandlesProvider");
    let key = format!("androidx.lifecycle.ViewModelProvider.DefaultKey:{}", std::any::type_name::<QueryViewModel>());
    assert_eq!(handles.get(&key).get("query").as_str(), Some("kedi"), "{handles:?}");
}

/// C++ `saved_state_outlives_the_host`, the host's half (the navigation entries' are the NavHost's): the
/// composition's saveable values written out with the host's state and given back to a host made again from it
/// before its first frame; a plain `remember` starts over, a value whose composable left isn't saved; saved again
/// before what keeps them came back, they are still there.
fn saved_state_outlives_the_host() {
    #[derive(Default)]
    struct Run {
        count: RefCell<Option<MutableState<i32>>>,
        plain: RefCell<Option<MutableState<i32>>>,
        field: RefCell<Option<kompoz_foundation::text::input::TextFieldState>>,
    }
    let app = |r: &Rc<Run>, shown: &MutableState<bool>, extra: &MutableState<bool>| {
        let (r, shown, extra) = (r.clone(), shown.clone(), extra.clone());
        kompozable_lambda!(move || {
            if !shown.get() {
                return;
            }
            *r.count.borrow_mut() = Some(remember_saveable(|| mutable_state_of(0)));
            *r.field.borrow_mut() = Some(remember_text_field_state(initial_text = "a".to_string()));
            // (not saveable: starts over)
            *r.plain.borrow_mut() = Some(remember(|| mutable_state_of(0)));
            if extra.get() {
                remember_saveable(|| mutable_state_of("gitti".to_string()));
            }
        })
    };
    let bytes = {
        let (r, shown, extra) = (Rc::new(Run::default()), mutable_state_of(false), mutable_state_of(true));
        let rule = KompozTestRule::new(app(&r, &shown, &extra));
        shown.set(true);
        rule.wait_for_idle();
        r.count.borrow().as_ref().unwrap().set(5);
        r.plain.borrow().as_ref().unwrap().set(9);
        r.field.borrow().as_ref().unwrap().set_text_and_place_cursor_at_end("kaydet");
        extra.set(false); // (left the composition: not saved)
        rule.wait_for_idle();
        let bytes = rule.host().save_state();
        let saved = SavedValue::decode(&bytes).unwrap();
        let composition = saved.get("composition").as_map().cloned().unwrap_or_default();
        assert_eq!(composition.len(), 2, "{composition:?}"); // count and field; not the one that left
        assert!(composition.values().all(|v| v.as_str() != Some("gitti")));
        assert!(saved.get("savedState").as_map().is_some());
        bytes
    };
    let (r, shown, extra) = (Rc::new(Run::default()), mutable_state_of(false), mutable_state_of(false));
    let rule = KompozTestRule::new(app(&r, &shown, &extra));
    rule.host().restore_state(&bytes);
    shown.set(true);
    rule.wait_for_idle();
    assert!(
        r.count.borrow().as_ref().unwrap().peek() == 5
            && r.field.borrow().as_ref().unwrap().text() == "kaydet"
            && r.plain.borrow().as_ref().unwrap().peek() == 0
    );
    assert_eq!(SavedValue::decode(&rule.host().save_state()), SavedValue::decode(&bytes));
}

// ---- savedstate --------------------------------------------------------------------------------------------------

/// SavedState's reader and writer: typed values, nulls, lists, nested states (copied in), `putAll`, `remove`,
/// `contentDeepEquals`/`ToString`; a missing or mistyped value panics as Kotlin's IllegalArgumentException.
fn saved_states_are_read_and_written_by_type() {
    let inner = saved_state(|w| w.put_string("ad", "kedi"));
    let s = saved_state(|w| {
        w.put_int("i", 7);
        w.put_long("l", 1 << 40);
        w.put_float("f", 1.5);
        w.put_boolean("b", true);
        w.put_char("c", 'ç');
        w.put_null("n");
        w.put_int_list("il", [1, 2]);
        w.put_string_array("sa", ["a", "b"]);
        w.put_saved_state("in", &inner);
    });
    inner.write(|w| w.put_string("ad", "köpek")); // (copied in: the one put isn't changed)
    s.read(|r| {
        assert!(r.get_int("i") == 7 && r.get_long("i") == 7 && r.get_int_or_null("l").is_none());
        assert!(r.get_long("l") == 1 << 40 && r.get_float("f") == 1.5 && r.get_double("f") == 1.5);
        assert!(r.get_boolean("b") && r.get_char("c") == 'ç' && r.is_null("n") && !r.is_null("i"));
        assert!(r.get_int_list("il") == [1, 2] && r.get_string_list("sa") == ["a", "b"]);
        assert_eq!(r.get_saved_state("in").read(|r| r.get_string("ad")), "kedi");
        assert!(r.contains("n") && !r.contains("x") && r.size() == 9);
    });
    let missing = panics(|| {
        s.read(|r| r.get_string("i"));
    });
    assert!(missing.contains("No valid saved state was found for the key 'i'"));
    let copy = saved_state_from(&s, |w| w.remove("n"));
    assert!(!copy.read(|r| r.content_deep_equals(&s)) && copy.read(|r| r.size()) == 8);
    copy.write(|w| w.put_null("n"));
    assert!(copy == s && copy.read(|r| r.content_deep_hash_code()) == s.read(|r| r.content_deep_hash_code()));
    assert_eq!(saved_state(|w| w.put_int("id", 1)).read(|r| r.content_deep_to_string()), "[id=1]");
    s.write(|w| w.clear());
    assert!(s.read(|r| r.is_empty()));
}

#[derive(Saveable, Clone, PartialEq, Debug)]
struct User {
    id: i32,
    name: String,
    #[saveable(name = "etiket")]
    tags: Vec<String>,
    #[saveable(default)]
    nick: Option<String>,
    #[saveable(skip)]
    cache: Vec<u8>,
}

#[derive(Saveable, Clone, Copy, PartialEq, Debug)]
enum Theme {
    Light,
    Dark,
}

#[derive(Saveable, Clone, PartialEq, Debug)]
enum Shape {
    Circle { radius: f64 },
    Square(f32),
    Dot,
}

#[derive(Saveable, Clone, PartialEq, Debug)]
struct Pair<T>(T, Theme);

/// `#[derive(Saveable)]` as kotlinx.serialization writes a class into a SavedState (savedstate's encode/decode
/// samples): a struct as its fields by name, an enum of plain variants as its index, a sealed one with its "type";
/// a value that isn't a structure under `""`; an explicit serializer (a `Saver`).
fn saveable_types_are_encoded_into_saved_states() {
    let user = User { id: 123, name: "foo".into(), tags: vec!["a".into()], nick: None, cache: vec![1] };
    let saved = encode_to_saved_state(&user);
    saved.read(|r| {
        assert!(r.get_int("id") == 123 && r.get_string("name") == "foo" && r.get_string_list("etiket") == ["a"]);
        assert!(!r.contains("cache"));
    });
    assert_eq!(decode_from_saved_state::<User>(&saved), User { cache: vec![], ..user.clone() });
    // (a property with a default may be missing: savedstate's `userSavedState`, written by hand)
    let by_hand = saved_state(|w| {
        w.put_int("id", 123);
        w.put_string("name", "foo");
        w.put_string_list("etiket", Vec::<String>::new());
    });
    assert_eq!(decode_from_saved_state::<User>(&by_hand).nick, None);
    assert_eq!(Theme::Dark.save(), SavedValue::Int(1));
    assert_eq!(Theme::restore(&SavedValue::Int(0)), Some(Theme::Light));
    for shape in [Shape::Circle { radius: 2.0 }, Shape::Square(3.0), Shape::Dot] {
        let s = encode_to_saved_state(&shape);
        assert!(s.read(|r| r.contains("type")));
        assert_eq!(decode_from_saved_state::<Shape>(&s), shape);
    }
    assert_eq!(
        Pair::<String>::restore(&Pair("x".to_string(), Theme::Dark).save()),
        Some(Pair("x".into(), Theme::Dark))
    );
    // (a value that isn't a structure: under "")
    let text = encode_to_saved_state(&"çay".to_string());
    assert_eq!(text.read(|r| r.get_string("")), "çay");
    assert_eq!(decode_from_saved_state::<String>(&text), "çay");
    // (an explicit serializer: savedstate's UUIDSerializer sample, a Saver)
    let upper = Saver::new(|s: &String| SavedValue::from(s.to_uppercase()), |v| v.as_str().map(str::to_lowercase));
    let encoded = encode_to_saved_state_with(&upper, &"abc".to_string());
    assert_eq!(encoded.read(|r| r.get_string("")), "ABC");
    assert_eq!(decode_from_saved_state_with(&upper, &encoded), "abc");
    let wrong = panics(|| {
        decode_from_saved_state::<User>(&text);
    });
    assert!(wrong.starts_with("SerializationException"));
}

/// An owner of a registry of its own (Compose's ComponentActivity's part): a lifecycle and a controller.
struct Screen {
    lifecycle: LifecycleRegistry,
    controller: SavedStateRegistryController,
}

impl LifecycleOwner for Screen {
    fn lifecycle(&self) -> Lifecycle {
        self.lifecycle.lifecycle()
    }
}

impl SavedStateRegistryOwner for Screen {
    fn saved_state_registry(&self) -> SavedStateRegistry {
        self.controller.saved_state_registry()
    }
}

fn screen(saved: Option<&SavedState>) -> Rc<Screen> {
    let s = Rc::new_cyclic(|me: &Weak<Screen>| Screen {
        lifecycle: LifecycleRegistry::new(me.clone()),
        controller: SavedStateRegistryController::create(me.clone()),
    });
    s.controller.perform_attach();
    s.controller.perform_restore(saved);
    s.lifecycle.set_current_state(LifecycleState::Resumed);
    s
}

/// Compose's SavedStateRegistry: providers by key (one each), asked as the owner saves; what was restored is given
/// once and, not consumed, saved again; `isRestored`; saving allowed between ON_START and ON_STOP; the controller's
/// checks (attached in INITIALIZED, restored before STARTED, once each); the `saved { }` delegate.
fn saved_state_registries_give_back_what_their_providers_saved() {
    let first = screen(None);
    let registry = first.saved_state_registry();
    assert!(registry.is_restored() && registry.consume_restored_state_for_key("a").is_none());
    let n = Rc::new(Cell::new(1));
    let m = n.clone();
    registry.register_saved_state_provider("a", Rc::new(move || saved_state(|w| w.put_int("n", m.get()))));
    registry.register_saved_state_provider("b", Rc::new(|| saved_state(|w| w.put_string("s", "x"))));
    assert!(registry.get_saved_state_provider("b").is_some());
    let twice = panics(|| registry.register_saved_state_provider("a", Rc::new(SavedState::new)));
    assert!(twice.contains("SavedStateProvider with the given key is already registered"));
    // (the `saved { }` delegate: registered as it is first read)
    let theme = first.saved("tema", || Theme::Light);
    assert_eq!(theme.get_value(), Theme::Light);
    theme.set_value(Theme::Dark);
    n.set(2);
    let out = SavedState::new();
    first.controller.perform_save(&out);
    assert!(registry.is_allowing_saving_state());
    first.lifecycle.set_current_state(LifecycleState::Created);
    assert!(!registry.is_allowing_saving_state());
    // A screen made again from it: each provider's state given once.
    let second = screen(Some(&out));
    let again = second.saved_state_registry();
    assert_eq!(again.consume_restored_state_for_key("a").unwrap().read(|r| r.get_int("n")), 2);
    assert!(again.consume_restored_state_for_key("a").is_none());
    assert_eq!(second.saved("tema", || Theme::Light).get_value(), Theme::Dark);
    // (not consumed: saved again as it was)
    let out2 = SavedState::new();
    second.controller.perform_save(&out2);
    let components =
        |s: &SavedState| s.read(|r| r.get_saved_state("androidx.lifecycle.BundlableSavedStateRegistry.key"));
    assert_eq!(components(&out2).read(|r| r.get_saved_state("b")), components(&out).read(|r| r.get_saved_state("b")));
    assert!(!components(&out2).read(|r| r.contains("a")));
    // The controller's checks.
    let started = panics(|| {
        second.controller.perform_restore(None);
    });
    assert!(started.contains("performRestore cannot be called when owner is RESUMED"));
    let late = panics(|| {
        second.controller.perform_attach();
    });
    assert!(late.contains("Restarter must be created only during owner's initialization stage"));
}

// ---- ViewModels --------------------------------------------------------------------------------------------------

/// A ViewModel logging what is done to it.
struct Logged {
    base: ViewModelImpl,
    log: Log,
    name: String,
}

impl ViewModel for Logged {
    fn view_model_impl(&self) -> &ViewModelImpl {
        &self.base
    }
    fn on_cleared(&self) {
        self.log.borrow_mut().push(format!("{} onCleared", self.name));
    }
}

fn logged(log: &Log, name: &str, closeables: Vec<Rc<dyn AutoCloseable>>) -> Logged {
    Logged { base: ViewModelImpl::with_closeables(closeables), log: log.clone(), name: name.into() }
}

fn closing(log: &Log, what: &str) -> Rc<dyn AutoCloseable> {
    let (log, what) = (log.clone(), what.to_string());
    Rc::new(move || log.borrow_mut().push(format!("close {what}")))
}

/// Compose's ViewModel and ViewModelStore: closeables closed as it is cleared — keyed ones, the constructor's, the
/// others — then onCleared; one added after is closed at once, one replacing another under its key closes that;
/// `viewModelScope` cancelled; the store clears what a `put` replaces and everything as it is cleared.
fn view_models_close_their_resources_as_they_are_cleared() {
    let log = log();
    let vm = logged(&log, "a", vec![closing(&log, "constructor")]);
    vm.add_closeable(closing(&log, "plain"));
    vm.add_closeable_with_key("k", closing(&log, "keyed 1"));
    vm.add_closeable_with_key("k", closing(&log, "keyed 2"));
    assert_eq!(taken(&log), ["close keyed 1"]);
    let scope = vm.view_model_scope();
    assert!(scope.is_active() && vm.view_model_scope() == scope);
    assert!(
        vm.get_closeable::<CloseableCoroutineScope>(
            "androidx.lifecycle.viewmodel.internal.ViewModelCoroutineScope.JOB_KEY"
        )
        .is_some()
    );
    let store = ViewModelStore::new();
    let vm: Rc<dyn ViewModel> = Rc::new(vm);
    store.put("a", vm.clone());
    store.put("b", Rc::new(logged(&log, "b", vec![])));
    store.put("b", Rc::new(logged(&log, "b2", vec![])));
    assert_eq!(taken(&log), ["b onCleared"]);
    assert_eq!(store.keys(), [AnyKey::from("a"), AnyKey::from("b")]);
    store.clear();
    assert!(!scope.is_active() && store.keys().is_empty());
    assert_eq!(taken(&log), ["close keyed 2", "close constructor", "close plain", "a onCleared", "b2 onCleared"]);
    vm.view_model_impl().add_closeable(closing(&log, "late"));
    assert_eq!(taken(&log), ["close late"]);
}

/// A ViewModel a factory makes by itself (Kotlin's public constructor taking nothing).
struct Counter {
    base: ViewModelImpl,
    count: MutableStateFlow<i32>,
}

impl ViewModel for Counter {
    fn view_model_impl(&self) -> &ViewModelImpl {
        &self.base
    }
    fn new_instance(_extras: &CreationExtras) -> Option<Self> {
        Some(Counter { base: ViewModelImpl::new(), count: MutableStateFlow::new(0) })
    }
}

/// Compose's ViewModelProvider: one ViewModel per class (its default key) or per key, made by the factory the first
/// time with the key in its extras; another class under a key replaces it; initializers by class; the
/// NewInstanceFactory of a class without a constructor of its own panics as Kotlin's RuntimeException; extras
/// keys, `+`, `CreationExtras { }`.
fn view_model_providers_make_each_view_model_once_by_key() {
    let store = ViewModelStore::new();
    let provider = ViewModelProvider::create_from_store(store.clone(), Some(Rc::new(NewInstanceFactory)), None);
    let a = provider.get::<Counter>();
    assert!(Rc::ptr_eq(&a, &provider.get::<Counter>()));
    let keyed = provider.get_keyed::<Counter>("ikinci");
    assert!(!Rc::ptr_eq(&a, &keyed) && store.keys().len() == 2);
    assert_eq!(
        store.keys()[0],
        AnyKey::from(format!(
            "androidx.lifecycle.ViewModelProvider.DefaultKey:{}",
            KClass::of::<Counter>().canonical_name()
        ))
    );
    let log = log();
    let keys = Rc::new(RefCell::new(Vec::<String>::new()));
    let (l, k) = (log.clone(), keys.clone());
    let factory = view_model_factory(move |this| {
        let (l, k) = (l.clone(), k.clone());
        this.initializer(move |extras| {
            k.borrow_mut().push(extras.get(&VIEW_MODEL_KEY).unwrap());
            logged(&l, "logged", vec![])
        });
    });
    let by_initializer = ViewModelProvider::create_from_store(store.clone(), Some(factory), None);
    by_initializer.get_keyed::<Logged>("ikinci"); // (another class under the key: the counter there cleared)
    assert!(keys.borrow().as_slice() == ["ikinci"] && store.keys().len() == 2);
    let none = panics(|| {
        by_initializer.get_keyed::<Counter>("yeni");
    });
    assert!(none.contains("No initializer set for given class"));
    let no_constructor = panics(|| {
        provider.get::<Logged>();
    });
    assert!(no_constructor.starts_with("RuntimeException: Cannot create an instance of class"));
    let twice = panics(|| {
        view_model_factory(|this| {
            this.initializer(|_| Counter::new_instance(&CreationExtras::default()).unwrap());
            this.initializer(|_| Counter::new_instance(&CreationExtras::default()).unwrap());
        });
    });
    assert!(twice.contains("has already been added"));
    // Creation extras.
    let key = CreationExtrasKey::<i32>::new();
    let other = CreationExtrasKey::<i32>::new();
    let extras = CreationExtras(&CreationExtras::default(), |e| e.set(&key, 4));
    assert!(extras.contains(&key) && !extras.contains(&other) && extras.get(&key) == Some(4));
    let more = &extras + &CreationExtras(&CreationExtras::default(), |e| e.set(&other, 5));
    assert!(more.get(&key) == Some(4) && more.get(&other) == Some(5));
    // (a lazy ViewModel, `by viewModels()`'s)
    let lazy: ViewModelLazy<Counter> =
        ViewModelLazy::new(move || store.clone(), || Rc::new(NewInstanceFactory), || CreationExtras::Empty);
    assert!(!lazy.is_initialized() && Rc::ptr_eq(&lazy.get_value(), &a) && lazy.is_initialized());
}

/// SavedStateHandle: values by key of any saveable type, its state flows (read-only ones told of each set, mutable
/// ones whose values it saves), providers saved with it, `remove`, `keys`; `saveable` (viewmodel-compose) and
/// `saved` (serialization) give back what was saved; `createHandle` from a restored state, else the defaults.
fn saved_state_handles_keep_values_flows_and_providers() {
    let handle = SavedStateHandle::new_with([("sayı", SavedValue::from(1))]);
    let flow = handle.get_state_flow("sayı", 0);
    assert_eq!(flow.value(), 1);
    handle.set("sayı", 2);
    assert_eq!(flow.value(), 2);
    let mutable = handle.get_mutable_state_flow("metin", "a".to_string());
    mutable.set_value("b".into());
    assert_eq!(handle.get::<String>("metin").as_deref(), Some("b"));
    handle.set_saved_state_provider("ek", Rc::new(|| saved_state(|w| w.put_boolean("var", true))));
    let items = handle.saveable("liste", || mutable_state_list_of(vec![1]));
    items.add(2);
    let theme = handle.saved("tema", || Theme::Light);
    theme.set_value(Theme::Dark);
    assert_eq!(handle.keys(), ["sayı", "metin", "ek", "liste", "tema"]);
    let saved = handle.saved_state_provider().save_state();
    saved.read(|r| {
        assert!(r.get_int("sayı") == 2 && r.get_string("metin") == "b");
        assert!(r.get_saved_state("ek").read(|r| r.get_boolean("var")));
    });
    let again = SavedStateHandle::create_handle(Some(&saved), None);
    assert_eq!(again.get_state_flow("sayı", 0).value(), 2);
    assert_eq!(again.saveable("liste", || mutable_state_list_of(Vec::<i32>::new())).to_vec(), [1, 2]);
    assert_eq!(again.saved("tema", || Theme::Light).get_value(), Theme::Dark);
    assert_eq!(again.remove::<i32>("sayı"), Some(2));
    assert!(!again.contains("sayı") && again.get::<i32>("sayı").is_none());
    let defaults = SavedStateHandle::create_handle(None, Some(&saved_state(|w| w.put_int("id", 7))));
    assert_eq!(defaults.get::<i32>("id"), Some(7));
}

/// The host is the app's owners (CMP's DefaultArchitectureComponentsOwner): `viewModel()` gives the host's
/// ViewModel (the same one while the host lives, under its default key or a key of its own; made by an
/// initializer with a handle of its own); its ViewModels are cleared as the host is destroyed, their scopes
/// cancelled; `LocalSavedStateRegistryOwner` is the host's.
fn the_host_is_the_apps_view_model_store_and_saved_state_owner() {
    let log = log();
    let counters = Rc::new(RefCell::new(Vec::<Rc<Counter>>::new()));
    let (l, c) = (log.clone(), counters.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let a: Rc<Counter> = view_model();
        let b: Rc<Counter> = view_model(key = "öteki".to_string());
        c.borrow_mut().push(a.clone());
        c.borrow_mut().push(b);
        let l = l.clone();
        let _logged: Rc<Logged> = view_model_with(move |extras| {
            let handle = extras.create_saved_state_handle();
            l.borrow_mut().push(format!("made with {:?}", handle.keys()));
            logged(&l, "logged", vec![])
        });
        let owner = LocalSavedStateRegistryOwner.current();
        Text(format!("sayı {} {}", a.count.collect_as_state().get(), owner.lifecycle().current_state()));
    }));
    rule.on_node_with_text("sayı 0 RESUMED").assert_exists();
    counters.borrow()[0].count.set_value(3);
    rule.wait_for_idle();
    rule.on_node_with_text("sayı 3 RESUMED").assert_exists();
    let all = counters.borrow().clone();
    let n = all.len();
    assert!(Rc::ptr_eq(&all[0], &all[n - 2]) && !Rc::ptr_eq(&all[0], &all[1]));
    assert_eq!(taken(&log), ["made with []"]);
    let store = rule.host().view_model_store_owner().view_model_store();
    assert_eq!(store.keys().len(), 3 + 1); // (the two counters, the logged one, and the handles' own)
    let scope = all[0].view_model_scope();
    drop(rule);
    assert!(store.keys().is_empty() && !scope.is_active());
    assert_eq!(taken(&log), ["logged onCleared"]);
}

/// rememberViewModelStoreOwner / rememberViewModelStoreProvider: a store of the caller's own, its ViewModels
/// cleared as the caller leaves for good; a provider's stores by key (a pager's pages), each kept while a token
/// references it.
fn remembered_view_model_store_owners_scope_view_models_to_the_caller() {
    let log = log();
    let shown = mutable_state_of(true);
    let pages = mutable_state_of(vec![0, 1]);
    let (l, s, p) = (log.clone(), shown.clone(), pages.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        if s.get() {
            let owner = remember_view_model_store_owner();
            let l = l.clone();
            KompozitionLocalProvider(&[LocalViewModelStoreOwner.provides(owner)], move || {
                let l = l.clone();
                let vm: Rc<Logged> = view_model_with(move |_| logged(&l, "ayrıntı", vec![]));
                Text(vm.name.clone());
            });
        }
        let provider = remember_view_model_store_provider();
        for page in p.get() {
            let (l, provider) = (l.clone(), provider.clone());
            key(&page, move || {
                let owner = remember_view_model_store_owner_keyed(page, provider.clone());
                let l = l.clone();
                KompozitionLocalProvider(&[LocalViewModelStoreOwner.provides(owner)], move || {
                    let l = l.clone();
                    let vm: Rc<Logged> = view_model_with(move |_| logged(&l, &format!("sayfa {page}"), vec![]));
                    Text(vm.name.clone());
                });
            });
        }
    }));
    rule.on_node_with_text("ayrıntı").assert_exists();
    rule.on_node_with_text("sayfa 1").assert_exists();
    shown.set(false);
    rule.wait_for_idle();
    assert_eq!(taken(&log), ["ayrıntı onCleared"]);
    // (a page gone: its store stays until its key is cleared — the provider's own composable still there)
    pages.set(vec![0]);
    rule.wait_for_idle();
    assert!(taken(&log).is_empty());
    drop(rule);
    let mut left = taken(&log);
    left.sort();
    assert_eq!(left, ["sayfa 0 onCleared", "sayfa 1 onCleared"]);
}

/// A ViewModel's flow in its scope (KitchenForms' TickViewModel's `stateIn(viewModelScope, WhileSubscribed(5000))`):
/// collected while the app collects it, its scope cancelled as the host goes.
fn view_model_scopes_run_on_the_main_dispatcher() {
    struct Ticks {
        base: ViewModelImpl,
        ticks: StateFlow<i32>,
    }
    impl ViewModel for Ticks {
        fn view_model_impl(&self) -> &ViewModelImpl {
            &self.base
        }
        fn new_instance(_: &CreationExtras) -> Option<Self> {
            let base = ViewModelImpl::new();
            let ticks = kompoz_coroutines::flow(|emit| async move {
                let mut i = 0;
                loop {
                    emit.emit(i).await;
                    i += 1;
                    delay_ms(100).await;
                }
            })
            .state_in(
                &base.view_model_scope(),
                kompoz_coroutines::SharingStarted::WhileSubscribed_millis(5000, i64::MAX),
                0,
            );
            Some(Ticks { base, ticks })
        }
    }
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        let vm: Rc<Ticks> = view_model();
        Text(format!("tik {}", vm.ticks.collect_as_state_with_lifecycle().get()));
    }));
    rule.advance_time_by(250);
    rule.on_node_with_text("tik 2").assert_exists();
}

fn main() {
    let tests: &[(&str, fn())] = &[
        ("saved_state_handles_outlive_the_process", saved_state_handles_outlive_the_process),
        ("saved_state_outlives_the_host", saved_state_outlives_the_host),
        ("saved_states_are_read_and_written_by_type", saved_states_are_read_and_written_by_type),
        ("saveable_types_are_encoded_into_saved_states", saveable_types_are_encoded_into_saved_states),
        (
            "saved_state_registries_give_back_what_their_providers_saved",
            saved_state_registries_give_back_what_their_providers_saved,
        ),
        (
            "view_models_close_their_resources_as_they_are_cleared",
            view_models_close_their_resources_as_they_are_cleared,
        ),
        (
            "view_model_providers_make_each_view_model_once_by_key",
            view_model_providers_make_each_view_model_once_by_key,
        ),
        ("saved_state_handles_keep_values_flows_and_providers", saved_state_handles_keep_values_flows_and_providers),
        (
            "the_host_is_the_apps_view_model_store_and_saved_state_owner",
            the_host_is_the_apps_view_model_store_and_saved_state_owner,
        ),
        (
            "remembered_view_model_store_owners_scope_view_models_to_the_caller",
            remembered_view_model_store_owners_scope_view_models_to_the_caller,
        ),
        ("view_model_scopes_run_on_the_main_dispatcher", view_model_scopes_run_on_the_main_dispatcher),
    ];
    let filters: Vec<String> = std::env::args().skip(1).filter(|a| !a.starts_with('-')).collect();
    let mut failed = Vec::new();
    for (name, test) in tests {
        if !filters.is_empty() && !filters.iter().any(|f| name.contains(f.as_str())) {
            continue;
        }
        if std::panic::catch_unwind(test).is_err() {
            failed.push(*name);
            println!("FAILED {name}");
        } else {
            println!("ok     {name}");
        }
    }
    println!("\n{} run, {} failed", tests.len(), failed.len());
    if !failed.is_empty() {
        std::process::exit(1);
    }
}
