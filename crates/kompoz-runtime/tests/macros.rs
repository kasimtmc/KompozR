//! `#[kompozable]`, `#[named]` and `kompozable_lambda!`: named and defaulted arguments, trailing closures, methods,
//! generics, skipping, and how closures capture (the stage 0 samples, now on the real runtime).

#![allow(non_snake_case, non_upper_case_globals)]

mod common;

use common::*;
use kompoz_runtime::*;
use std::cell::{Cell, RefCell};
use std::fmt;
use std::rc::Rc;
use std::sync::atomic::{AtomicI32, Ordering};

// ---- Values with named, defaulted methods --------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq)]
struct Color {
    red: f32,
    green: f32,
    blue: f32,
    alpha: f32,
}

impl Color {
    const fn rgb(r: u8, g: u8, b: u8) -> Color {
        Color { red: r as f32 / 255.0, green: g as f32 / 255.0, blue: b as f32 / 255.0, alpha: 1.0 }
    }
}

#[named]
impl Color {
    fn copy(
        &self,
        #[default(self.alpha)] alpha: f32,
        #[default(self.red)] red: f32,
        #[default(self.green)] green: f32,
        #[default(self.blue)] blue: f32,
    ) -> Color {
        Color { red, green, blue, alpha }
    }
}

impl fmt::Debug for Color {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let c = |v: f32| (v * 255.0).round() as u8;
        write!(f, "#{:02X}{:02X}{:02X}{:02X}", c(self.alpha), c(self.red), c(self.green), c(self.blue))
    }
}

#[derive(Clone, PartialEq)]
struct Modifier {
    elements: Vec<String>,
}

/// Compose's `Modifier`: the type, and (as a constant of the same name) the empty chain to start one from.
const Modifier: Modifier = Modifier { elements: Vec::new() };

#[derive(Clone, Copy, Debug, PartialEq)]
enum Shape {
    Rectangle,
    Rounded(i32),
}

#[named]
impl Modifier {
    fn then(mut self, element: String) -> Modifier {
        self.elements.push(element);
        self
    }
    fn padding(self, all: i32) -> Modifier {
        self.then(format!("padding({all})"))
    }
    fn fill_max_height(self, #[default(1.0)] fraction: f32) -> Modifier {
        self.then(format!("fillMaxHeight({fraction})"))
    }
    fn background(self, color: Color, #[default(Shape::Rectangle)] shape: Shape) -> Modifier {
        self.then(format!("background({color:?}, {shape:?})"))
    }
}

impl fmt::Display for Modifier {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Modifier")?;
        for e in &self.elements {
            write!(f, ".{e}")?;
        }
        Ok(())
    }
}

#[named]
fn RoundedCornerShape(radius: i32) -> Shape {
    Shape::Rounded(radius)
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct BarColors {
    container: Color,
    title: Color,
}

#[named]
fn top_app_bar_colors(
    #[default(Color::rgb(0xE9, 0xDD, 0xFF))] container: Color,
    #[default(Color::rgb(0x1D, 0x1B, 0x20))] title: Color,
) -> BarColors {
    BarColors { container, title }
}

/// Something built by chained calls on it (as an ImageVector's builder is).
#[derive(Default)]
struct Steps {
    done: Vec<String>,
}

#[named]
impl Steps {
    fn add(&mut self, name: &str, #[default(1)] times: i32) -> &mut Steps {
        self.done.push(format!("{name}x{times}"));
        self
    }
}

/// What comes before `separator` (an output borrowing the only reference given).
#[named]
fn first_word(text: &str, #[default(' ')] separator: char) -> &str {
    text.split(separator).next().unwrap_or(text)
}

// ---- Composables ---------------------------------------------------------------------------------------------

/// A node named by what it was given.
#[kompozable]
fn Label(text: impl Into<String>, #[default(Modifier)] modifier: Modifier, #[default(16)] size: i32) {
    let name = if modifier.elements.is_empty() && size == 16 { text } else { format!("{text} [{modifier}, {size}]") };
    Text(name);
}

#[kompozable]
fn Boxed(#[default(Modifier)] modifier: Modifier, content: impl Fn()) {
    Column(if modifier.elements.is_empty() { "box" } else { "box+" }, content);
}

#[kompozable]
fn Button(on_click: impl Fn(), #[default(Modifier)] modifier: Modifier, content: impl Fn()) {
    let _ = modifier;
    CLICKS.with(|c| c.borrow_mut().push(Rc::new(on_click)));
    Column("button", content);
}

thread_local! {
    static CLICKS: RefCell<Vec<Rc<dyn Fn()>>> = const { RefCell::new(Vec::new()) };
}

fn click(i: usize) {
    let f = CLICKS.with(|c| c.borrow()[i].clone());
    f();
}

fn clear_clicks() {
    CLICKS.with(|c| c.borrow_mut().clear());
}

#[kompozable]
fn Scaffold(
    #[default(|| {})] top_bar: impl Fn(),
    #[default(|| {})] bottom_bar: impl Fn(),
    #[default(|| {})] floating_action_button: impl Fn(),
    content: impl Fn(i32),
) {
    Column("scaffold", move || {
        top_bar();
        bottom_bar();
        floating_action_button();
        content(64);
    });
}

#[kompozable]
fn TopAppBar(title: impl Fn(), #[default(top_app_bar_colors())] colors: BarColors) {
    Column(if colors == top_app_bar_colors() { "topBar" } else { "topBar (colored)" }, title);
}

#[kompozable]
fn Counter() {
    let count = remember(|| mutable_state_of(0));
    Boxed(modifier = Modifier.padding(16), || {
        Label(format!("Count: {count}"), size = 22);
        Button(on_click = || count.set(count.get() + 1), || {
            Label("Increment");
        });
    });
}

fn counters_take_named_arguments_and_trailing_closures() {
    clear_clicks();
    let rule = Rule::new(Counter);
    assert_eq!(rule.names(), ["box+", "Count: 0 [Modifier, 22]", "button", "Increment"]);
    click(0);
    rule.wait_for_idle();
    assert_eq!(rule.names(), ["box+", "Count: 1 [Modifier, 22]", "button", "Increment"]);
}

#[kompozable]
fn ScaffoldExample() {
    let presses = remember(|| mutable_int_state_of(0));
    Scaffold(
        top_bar = || {
            TopAppBar(colors = top_app_bar_colors(container = Color::rgb(0xFF, 0, 0)), title = || Label("Top app bar"));
        },
        bottom_bar = || Label(modifier = Modifier.fill_max_height(fraction = 0.3), text = "Bottom app bar"),
        floating_action_button = || {
            Button(on_click = || presses.update(|p| *p += 1), || Label("Add"));
        },
        |inner_padding| {
            Label(format!("pressed {presses} times, padding {inner_padding}"));
        },
    );
}

fn the_users_scaffold_sample_reads_as_in_kotlin() {
    clear_clicks();
    let rule = Rule::new(ScaffoldExample);
    assert_eq!(
        rule.names(),
        [
            "scaffold",
            "topBar (colored)",
            "Top app bar",
            "Bottom app bar [Modifier.fillMaxHeight(0.3), 16]",
            "button",
            "Add",
            "pressed 0 times, padding 64"
        ]
    );
    click(0);
    rule.wait_for_idle();
    assert!(rule.has("pressed 1 times, padding 64"), "{:?}", rule.names());
}

#[kompozable]
fn GradientCard(primary: Color) {
    Boxed(
        modifier = Modifier
            .background(color = primary.copy(alpha = 0.6), shape = RoundedCornerShape(28))
            .fill_max_height()
            .padding(4),
        || Label(format!("{:?}", primary.copy(red = 0.0))),
    );
}

fn methods_and_factories_take_named_arguments() {
    let rule = Rule::new(kompozable_lambda!(|| GradientCard(Color::rgb(0xE9, 0xDD, 0xFF))));
    assert_eq!(rule.names(), ["box+", "#FF00DDFF"]);
    // The plain method: the parameters without a default.
    let m = Modifier.background(Color::rgb(0, 0, 0)).fill_max_height();
    assert_eq!(m.to_string(), "Modifier.background(#FF000000, Rectangle).fillMaxHeight(1)");
    assert_eq!(top_app_bar_colors().title, Color::rgb(0x1D, 0x1B, 0x20));
    // Methods returning the receiver they borrow, and functions an argument they borrow, chain on.
    let mut steps = Steps::default();
    steps.add("plain").add("again");
    let rule = Rule::new(kompozable_lambda!(|| {
        let mut steps = Steps::default();
        steps.add("a", times = 2).add(name = "b").add("c");
        Label(steps.done.join(" "));
        Label(first_word("x-y", separator = '-').to_string());
    }));
    assert_eq!(steps.done, ["plainx1", "againx1"]);
    assert_eq!(rule.names(), ["ax2 bx1 cx1", "x"]);
    assert_eq!(first_word("hello world"), "hello");
}

// ---- Methods of a type from elsewhere -------------------------------------------------------------------------

/// What another crate adds to `Modifier` (Compose's extension functions, foundation's `Modifier.padding`): a trait
/// it implements, with the same named and defaulted parameters, used where the trait is in scope.
mod layout_modifiers {
    use super::Modifier;
    use kompoz_runtime::named;

    #[named(trait LayoutModifiers)]
    impl Modifier {
        /// Each side its own, none by default.
        fn padding_sides(
            self,
            #[default(0)] start: i32,
            #[default(0)] top: i32,
            #[default(0)] end: i32,
            #[default(0)] bottom: i32,
        ) -> Modifier {
            self.then(format!("padding({start}, {top}, {end}, {bottom})"))
        }
        fn size(self, width: i32, #[default(1)] height: i32) -> Modifier {
            self.then(format!("size({width}, {height})"))
        }
        /// Without a default: a plain trait method.
        fn wrap_content(mut self) -> Modifier {
            self = self.then("wrapContent".into());
            self
        }
    }

    /// Of the same name in another trait: the builders don't clash.
    #[named(pub(crate) trait OtherModifiers)]
    impl Modifier {
        fn size(self, #[default(1)] all: i32) -> Modifier {
            self.then(format!("other size({all})"))
        }
    }
}

use layout_modifiers::LayoutModifiers;

fn methods_of_a_type_from_elsewhere_take_named_arguments_through_a_trait() {
    // Outside a body: the plain methods.
    let m = Modifier.padding_sides().size(3).wrap_content();
    assert_eq!(m.to_string(), "Modifier.padding(0, 0, 0, 0).size(3, 1).wrapContent");
    let rule = Rule::new(kompozable_lambda!(|| {
        Label("a", modifier = Modifier.padding_sides(top = 4, end = 2).size(10, height = 20));
        Label("b", modifier = Modifier.size(width = 5).wrap_content());
        let other = {
            use layout_modifiers::OtherModifiers;
            OtherModifiers::size(Modifier)
        };
        Label(other.to_string());
    }));
    assert_eq!(
        rule.names(),
        [
            "a [Modifier.padding(0, 4, 2, 0).size(10, 20), 16]",
            "b [Modifier.size(5, 1).wrapContent, 16]",
            "Modifier.other size(1)"
        ]
    );
}

/// Methods of every value of a trait, trait objects of its subtraits too (Compose's members of an interface a scope
/// implements, material3's `AppBarScope.clickableItem`): a blanket `#[named(trait …)]` impl of `S: ?Sized`.
mod scope_items {
    use kompoz_runtime::named;
    use std::cell::RefCell;

    pub trait ItemScope {
        fn added(&self) -> &RefCell<Vec<String>>;
    }

    pub trait RowItemScope: ItemScope {}

    #[derive(Default)]
    pub struct RowItems(pub RefCell<Vec<String>>);

    impl ItemScope for RowItems {
        fn added(&self) -> &RefCell<Vec<String>> {
            &self.0
        }
    }

    impl RowItemScope for RowItems {}

    #[named(trait ItemScopeItems)]
    impl<S: ItemScope + ?Sized> S {
        fn item(&self, label: String, #[default(1)] times: i32) {
            self.added().borrow_mut().push(format!("{label}x{times}"));
        }
    }
}

use scope_items::{ItemScopeItems, RowItemScope, RowItems};

#[kompozable]
fn ScopeItems(content: impl Fn(&dyn RowItemScope) + 'static) {
    let items = RowItems::default();
    content(&items);
    Label(items.0.borrow().join(" "));
}

fn methods_of_every_value_of_a_trait_reach_its_subtraits_objects() {
    let plain = RowItems::default();
    (&plain as &dyn RowItemScope).item("plain".to_string());
    assert_eq!(*plain.0.borrow(), ["plainx1"]);
    let rule = Rule::new(kompozable_lambda!(|| {
        ScopeItems(|scope: &dyn RowItemScope| {
            scope.item("a".to_string());
            scope.item(label = "b".to_string(), times = 2);
        });
    }));
    assert_eq!(rule.names(), ["ax1 bx2"]);
}

/// A scope's method without defaults named as another type's indexed `#[named]` method with some (foundation's
/// `Modifier.weight(weight, fill = true)`): the macros rewrite a call of that name to a builder whatever the
/// receiver, so this one has a builder too (Material's `ButtonGroupScope.weight(modifier, weight)`).
struct GroupScope;

#[named]
impl GroupScope {
    fn weight(&self, base: i32, weight: i32) -> String {
        format!("weight({base}, {weight})")
    }

    /// (WP1b: fab_segmented) An associated function without `self` of an indexed method's name stays plain (no
    /// by-name method call is its: `MenuDefaults::container_color()` beside another type's `container_color` method).
    fn colors(all: i32) -> String {
        format!("colors({all})")
    }
}

fn a_method_without_defaults_named_as_an_indexed_one_takes_its_calls() {
    assert_eq!(GroupScope.weight(1, 2), "weight(1, 2)");
    let rule = Rule::new(kompozable_lambda!(|| {
        let scope = GroupScope;
        Label(scope.weight(3, 4));
        Label(scope.weight(weight = 6, base = 5));
        Label(GroupScope::colors(7));
    }));
    assert_eq!(rule.names(), ["weight(3, 4)", "weight(5, 6)", "colors(7)"]);
}

// ---- Generics ------------------------------------------------------------------------------------------------

#[kompozable]
fn Items<T: Clone + PartialEq + 'static>(items: Vec<T>, #[default(0)] skip: usize, row: impl Fn(&T)) {
    for item in items.iter().skip(skip) {
        row(item);
    }
}

#[kompozable(inline)]
fn Provide<R>(value: i32, content: impl FnOnce(i32) -> R) -> R {
    content(value)
}

fn generic_composables_infer_their_types_from_the_arguments() {
    let rule = Rule::new(kompozable_lambda!(|| {
        Items(vec!["a", "b", "c"], skip = 1, |s| Label(s.to_uppercase()));
        Items(items = vec![1, 2], row = |n| Label(format!("{}", n * 10)));
        let doubled = Provide(21, |v| v * 2);
        Label(format!("{doubled}"));
    }));
    assert_eq!(rule.names(), ["B", "C", "10", "20", "42"]);
}

// ---- Skipping ------------------------------------------------------------------------------------------------

thread_local! {
    static RUNS: Cell<u32> = const { Cell::new(0) };
}

#[kompozable]
fn Equal(n: i32, label: String) {
    RUNS.with(|r| r.set(r.get() + 1));
    Label(format!("{label}{n}"));
}

#[kompozable]
fn WithContent(n: i32, content: impl Fn()) {
    RUNS.with(|r| r.set(r.get() + 1));
    let _ = n;
    content();
}

fn composables_whose_arguments_compare_are_skipped() {
    RUNS.with(|r| r.set(0));
    let tick = mutable_state_of(0);
    let t = tick.clone();
    let rule = Rule::new(kompozable_lambda!(move || {
        Label(format!("tick {t}"));
        Equal(1, "n".into()); // equal arguments: skipped when the parent runs again
        WithContent(1, || Label("in")); // a closure capturing nothing is the same closure: skipped too
    }));
    let runs = || RUNS.with(|r| r.get());
    assert_eq!(runs(), 2);
    tick.set(1);
    rule.wait_for_idle();
    assert!(rule.has("tick 1"));
    assert_eq!(runs(), 2);
}

// ---- Strong skipping: closures memoized by what they capture ---------------------------------------------------

thread_local! {
    static CHILD_RUNS: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

fn child_runs() -> Vec<String> {
    CHILD_RUNS.with(|r| r.borrow().clone())
}

/// A child taking a click handler: noted each time it runs, its handler kept for `click`.
#[kompozable]
fn Clicker(name: &'static str, on_click: impl Fn()) {
    CHILD_RUNS.with(|r| r.borrow_mut().push(name.to_string()));
    CLICKS.with(|c| c.borrow_mut().push(Rc::new(on_click)));
}

/// What a parent passes its own handler on to (and runs on its own when `tick` changes).
#[kompozable]
fn Forwarding(tick: MutableState<i32>, on_click: impl Fn()) {
    Label(format!("forwarding {tick}"));
    Clicker("forwarded", on_click);
    Clicker("wrapped", || on_click());
}

/// No `==`: a closure capturing it is a new one each time.
#[derive(Clone)]
struct Opaque(i32);

fn closures_are_the_same_while_what_they_capture_is() {
    CHILD_RUNS.with(|r| r.borrow_mut().clear());
    clear_clicks();
    let tick = mutable_state_of(0);
    let step = mutable_state_of(1);
    let (t, s) = (tick.clone(), step.clone());
    let total = Rc::new(Cell::new(0));
    let sum = total.clone();
    let rule = Rule::new(kompozable_lambda!(move || {
        Label(format!("tick {t}"));
        let step = s.get();
        let sum = sum.clone();
        // Captures a value with `==` and an `Rc` (by `==` of what it holds): the same while they are.
        Clicker("by value", on_click = || sum.set(sum.get() + step));
        // Captures a value without `==`: never the same.
        let opaque = Opaque(step);
        Clicker("opaque", || {
            let _ = opaque.0;
        });
        // A handle without `==`, by instance: the same one, the same closure.
        let log: Rc<RefCell<Vec<i32>>> = remember(|| Rc::new(RefCell::new(Vec::new())));
        let shared: Rc<dyn Fn(i32)> = remember(|| {
            let log = log.clone();
            Rc::new(move |v| log.borrow_mut().push(v)) as Rc<dyn Fn(i32)>
        });
        Clicker("by instance", move || shared(step));
    }));
    let runs = |name: &str| child_runs().iter().filter(|n| *n == name).count();
    assert_eq!((runs("by value"), runs("opaque"), runs("by instance")), (1, 1, 1));
    // The parent runs again, nothing captured changed: only the closure capturing what can't be compared is new.
    tick.set(1);
    rule.wait_for_idle();
    assert!(rule.has("tick 1"));
    assert_eq!((runs("by value"), runs("opaque"), runs("by instance")), (1, 2, 1));
    // A captured value changed: the closures capturing it are new, their children run again with them.
    step.set(5);
    rule.wait_for_idle();
    assert_eq!((runs("by value"), runs("opaque"), runs("by instance")), (2, 3, 2));
    clear_clicks();
    tick.set(2);
    rule.wait_for_idle();
    // (only the opaque child ran, so it alone handed its handler over)
    assert_eq!(CLICKS.with(|c| c.borrow().len()), 1);
    // The kept handler is the newest: it adds the step it captured.
    CLICKS.with(|c| c.borrow_mut().clear());
    step.set(7);
    rule.wait_for_idle();
    click(0);
    assert_eq!(total.get(), 7);
}

fn a_closure_parameter_passed_on_keeps_its_identity() {
    CHILD_RUNS.with(|r| r.borrow_mut().clear());
    let tick = mutable_state_of(0);
    let t = tick.clone();
    let other = mutable_state_of(0);
    let o = other.clone();
    let rule = Rule::new(kompozable_lambda!(move || {
        Label(format!("other {o}"));
        Forwarding(t.clone(), on_click = || {});
    }));
    assert_eq!(child_runs(), ["forwarded", "wrapped"]);
    // Running on its own, it passes on the handler it was given, the same one: its children are skipped.
    tick.set(1);
    rule.wait_for_idle();
    assert!(rule.has("forwarding 1"));
    assert_eq!(child_runs(), ["forwarded", "wrapped"]);
    // Its parent running again gives it the same handler: it is skipped.
    other.set(1);
    rule.wait_for_idle();
    assert!(rule.has("other 1"));
    assert_eq!(child_runs(), ["forwarded", "wrapped"]);
}

/// Two slots with default closures (Kotlin's `slot: @Composable () -> Unit = {}`).
#[kompozable]
fn Slotted(name: &'static str, #[default(|| {})] top: impl Fn(), #[default(|| {})] bottom: impl Fn()) {
    CHILD_RUNS.with(|r| r.borrow_mut().push(name.to_string()));
    top();
    bottom();
}

fn a_closure_parameter_left_at_its_default_is_the_same_each_time() {
    // Kotlin's default lambda is one instance: a composable called again with its closure parameters at their
    // defaults (and the others equal) is skipped, as Compose skips it.
    CHILD_RUNS.with(|r| r.borrow_mut().clear());
    let tick = mutable_state_of(0);
    let given = mutable_state_of(false);
    let (t, g) = (tick.clone(), given.clone());
    let rule = Rule::new(kompozable_lambda!(move || {
        Label(format!("tick {t}"));
        Slotted("defaults");
        Slotted("one given", top = || Label("top"));
        if g.get() {
            Slotted("switched", bottom = || Label("bottom"));
        } else {
            Slotted("switched");
        }
    }));
    let runs = |name: &str| child_runs().iter().filter(|n| *n == name).count();
    assert_eq!((runs("defaults"), runs("one given"), runs("switched")), (1, 1, 1));
    tick.set(1);
    rule.wait_for_idle();
    assert!(rule.has("tick 1"));
    assert_eq!((runs("defaults"), runs("one given"), runs("switched")), (1, 1, 1));
    // A closure given where the default was (another call site here, so composed anew) and back: run again.
    given.set(true);
    rule.wait_for_idle();
    assert!(rule.has("bottom"));
    given.set(false);
    rule.wait_for_idle();
    assert_eq!(runs("switched"), 3);
}

/// An inline composable keeping what it makes of a `#[memoized]` closure while it is the same.
#[kompozable(inline)]
fn Keeping(#[memoized] make: impl Fn() -> String + 'static) {
    let made = make.remember(|f| Rc::new(f()));
    Label(made.to_string());
}

fn a_memoized_parameter_is_kept_by_its_identity() {
    let tick = mutable_state_of(0);
    let word = mutable_state_of("a".to_string());
    let (t, w) = (tick.clone(), word.clone());
    let made = Rc::new(Cell::new(0));
    let m = made.clone();
    let rule = Rule::new(kompozable_lambda!(move || {
        Label(format!("tick {t}"));
        let word = w.get();
        let m = m.clone();
        Keeping(|| {
            m.set(m.get() + 1);
            format!("made of {word}")
        });
    }));
    assert_eq!((rule.names(), made.get()), (vec!["tick 0".to_string(), "made of a".to_string()], 1));
    tick.set(1);
    rule.wait_for_idle();
    assert_eq!(made.get(), 1);
    word.set("b".into());
    rule.wait_for_idle();
    assert_eq!((rule.has("made of b"), made.get()), (true, 2));
    // Outside a composition, nothing to keep it by: made each time.
    let plain = Memoized::new(|| 3, None);
    assert_eq!((plain.id(), plain.remember(|f| f())), (None, 3));
}

/// A type with methods taking a memoized closure (by `&self`, and by `&'static self` as a composition local's
/// `provides_computed` does): they give its identity back.
struct Ids;

#[named]
impl Ids {
    fn of(&self, #[memoized] make: impl Fn() -> String + 'static) -> Option<u64> {
        make.id()
    }

    fn of_static(&'static self, #[memoized] make: impl Fn() -> String + 'static) -> (Option<u64>, String) {
        (make.id(), make.into_value()())
    }
}

static IDS: Ids = Ids;

fn a_memoized_method_parameter_has_its_identity_from_a_positional_call() {
    // (the method has no defaulted parameter: its positional call goes through its builder all the same, the API index
    // marking the memoized parameter, and the closure keeps its identity while what it captures stays the same)
    let tick = mutable_state_of(0);
    let word = mutable_state_of("a".to_string());
    let (t, w) = (tick.clone(), word.clone());
    let seen = Rc::new(RefCell::new(Vec::<(Option<u64>, Option<u64>, String)>::new()));
    let s = seen.clone();
    let rule = Rule::new(kompozable_lambda!(move || {
        Label(format!("tick {t}"));
        let word = w.get();
        let by_ref = Ids.of(move || word.clone());
        let word = w.get();
        let (by_static, made) = IDS.of_static(move || format!("made of {word}"));
        s.borrow_mut().push((by_ref, by_static, made));
    }));
    tick.set(1);
    rule.wait_for_idle();
    word.set("b".into());
    rule.wait_for_idle();
    let seen = seen.borrow();
    let [(a0, s0, m0), (a1, s1, _), (a2, s2, m2)] = seen.as_slice() else { panic!("{seen:?}") };
    assert!(a0.is_some() && s0.is_some() && m0 == "made of a" && m2 == "made of b");
    assert!(a1 == a0 && s1 == s0, "the same captures: the same identities");
    assert!(a2 != a0 && s2 != s0, "other captures: other identities");
    // A plain call (outside a composable body): no identity.
    assert_eq!(Ids.of(String::new), None);
}

// ---- Captures ------------------------------------------------------------------------------------------------

#[derive(Clone)]
struct Shared(Rc<Cell<i32>>);

fn closures_share_what_they_capture_and_move_when_told() {
    clear_clicks();
    let seen = Rc::new(RefCell::new(Vec::<String>::new()));
    let s = seen.clone();
    let rule = Rule::new(kompozable_lambda!(move || {
        let state = remember(|| mutable_state_of(0));
        let shared = Shared(Rc::new(Cell::new(0)));
        // Three closures use `state`, nested ones too: each has its own clone of the handle.
        Boxed(|| {
            Button(on_click = || state.set(state.get() + 1), || Label(format!("state {state}")));
            Button(on_click = || shared.0.set(shared.0.get() + 1), || Label("shared"));
        });
        // `move` at the top of the body: moved, as in plain Rust (a value that can't be cloned).
        let only = NotClone(format!("moved {}", state.get()));
        let s = s.clone();
        SideEffect(move || s.borrow_mut().push(only.0));
    }));
    assert!(rule.has("state 0") && *seen.borrow() == ["moved 0"]);
    click(0);
    rule.wait_for_idle();
    assert!(rule.has("state 1") && seen.borrow().last().unwrap() == "moved 1", "{:?}", rule.names());
}

struct NotClone(String);

fn a_name_bound_inside_a_closure_hides_the_outer_one_only_after_its_binding() {
    let width = mutable_state_of(1);
    let w = width.clone();
    // `let w = w.get();` reads the outer `w` (captured by clone: the content runs again), then hides it below; a
    // `w` bound in a nested closure's own scope doesn't hide it outside that scope either.
    let rule = Rule::new(kompozable_lambda!(move || {
        Boxed(|| {
            let w = w.get();
            Label(format!("w {w}"));
        });
        Boxed(|| {
            let shown = { |w: i32| w * 10 }(2);
            Label(format!("{shown} {}", w.get()));
            match Some(w.get()) {
                Some(w) if w > 0 => Label(format!("matched {w}")),
                _ => Label("none"),
            }
        });
    }));
    assert_eq!(rule.names(), ["box", "w 1", "box", "20 1", "matched 1"]);
    width.set(2);
    rule.wait_for_idle();
    assert_eq!(rule.names(), ["box", "w 2", "box", "20 2", "matched 2"]);
}

// ---- A composable inside a function ------------------------------------------------------------------------

fn composables_can_be_defined_inside_functions() {
    #[kompozable]
    fn Local(#[default("local")] name: &'static str) {
        Label(name);
    }
    let rule = Rule::new(kompozable_lambda!(|| {
        Local();
        Local(name = "named");
    }));
    assert_eq!(rule.names(), ["local", "named"]);
}

// ---- Calls inside macros ------------------------------------------------------------------------------------

#[named]
fn Twice(n: i32, #[default(2)] times: i32) -> i32 {
    n * times
}

fn calls_inside_macros_are_rewritten() {
    let rule = Rule::new(kompozable_lambda!(|| {
        assert_eq!(Twice(3), 6); // a statement macro
        Label(format!("{} {}", Twice(4), Twice(1, times = 5))); // an expression one
    }));
    assert_eq!(rule.names(), ["8 5"]);
}

// ---- Positional arguments past the ones without a default ------------------------------------------------------

/// A type of its own with a method named as an indexed one (`copy`), without defaults.
#[derive(Clone, Copy)]
struct Ruler(i32);

impl Ruler {
    fn copy(&self, length: i32) -> Ruler {
        Ruler(self.0 + length)
    }
}

fn positional_arguments_past_the_required_ones_go_through_the_builder() {
    let rule = Rule::new(kompozable_lambda!(|| {
        let c = Color::rgb(0, 0, 0);
        // Methods: `copy(alpha, red, …)` of the API index (this crate's), with what an extension trait adds.
        Label(format!("{:?} {:?}", c.copy(0.5), c.copy(1.0, 1.0)));
        Label(Modifier.background(c, Shape::Rounded(2)).padding_sides(1, 2).size(3, 4).to_string());
        // Functions: lowercase and PascalCase (`Twice` isn't a composable: through its type).
        Label(format!("{:?} {}", top_app_bar_colors(c).container, Twice(3, 5)));
        // Defined in the body: its own items are known.
        #[named]
        fn Scaled(n: i32, #[default(10)] by: i32) -> i32 {
            n * by
        }
        struct Meters(i32);
        #[named]
        impl Meters {
            fn plus(&self, #[default(1)] more: i32) -> i32 {
                self.0 + more
            }
        }
        Label(format!("{} {} {}", Scaled(2), Scaled(2, 3), Meters(4).plus(5)));
    }));
    assert_eq!(
        rule.names(),
        [
            "#80000000 #FFFF0000",
            "Modifier.background(#FF000000, Rounded(2)).padding(1, 2, 0, 0).size(3, 4)",
            "#FF000000 15",
            "20 6 9"
        ]
    );
}

/// `copy` here is `Ruler`'s, not the indexed one: left as written.
#[kompozable(plain(copy))]
fn RulerLabel(ruler: Ruler) {
    Label(format!("ruler {}", ruler.copy(2).0));
}

fn a_call_can_be_kept_as_written() {
    let rule = Rule::new(kompozable_lambda!(|| {
        // A raw name keeps a call as written: `Ruler::copy`, not the indexed `copy` with defaults.
        Label(format!("{}", Ruler(1).r#copy(3).0));
        RulerLabel(Ruler(5));
    }));
    assert_eq!(rule.names(), ["4", "ruler 7"]);
}

// ---- PascalCase functions anywhere --------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq)]
struct Insets {
    left: i32,
    top: i32,
}

/// Compose's `Insets(left = 0, top = 0)`: its builder an associated function of the type it makes.
#[named(type)]
fn Insets(#[default(0)] left: i32, #[default(0)] top: i32) -> Insets {
    Insets { left, top }
}

fn pascal_case_functions_are_functions_everywhere() {
    // Plain code: the parameters without a default, or named arguments through `named_args!`.
    assert_eq!(RoundedCornerShape(3), Shape::Rounded(3));
    assert_eq!(Insets(), Insets { left: 0, top: 0 });
    assert_eq!(named_args!(Insets(top = 2)), Insets { left: 0, top: 2 });
    assert_eq!(named_args!(Insets(1, 2)), Insets { left: 1, top: 2 });
    assert_eq!(named_args!(Twice(4, times = 3)), 12);
    assert_eq!(named_args!(Color::rgb(0, 0, 0).copy(0.5)), named_args!(Color::rgb(0, 0, 0).copy(alpha = 0.5)));
    // A function value as any other.
    let shapes: Vec<Shape> = [1, 2].into_iter().map(RoundedCornerShape).collect();
    assert_eq!(shapes, [Shape::Rounded(1), Shape::Rounded(2)]);
    let rule = Rule::new(kompozable_lambda!(|| {
        Label(format!("{:?} {:?} {:?}", Insets(left = 3), Insets(4), RoundedCornerShape(radius = 5)));
    }));
    assert_eq!(rule.names(), ["Insets { left: 3, top: 0 } Insets { left: 4, top: 0 } Rounded(5)"]);
}

// ---- Option parameters ---------------------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq)]
enum Role {
    Button,
}

#[kompozable]
fn Tagged(text: &'static str, #[default(None)] role: Option<Role>, #[default(Some(1))] weight: Option<i32>) {
    Label(format!("{text} {role:?} {weight:?}"));
}

fn an_option_parameter_takes_what_it_holds() {
    let rule = Rule::new(kompozable_lambda!(|| {
        Tagged("a", role = Role::Button);
        Tagged("b", role = None, weight = 2);
        Tagged("c", role = Some(Role::Button), weight = None);
        Tagged("d");
    }));
    assert_eq!(rule.names(), ["a Some(Button) Some(1)", "b None Some(2)", "c Some(Button) None", "d None Some(1)"]);
}

#[kompozable]
fn Iconed(
    text: &'static str,
    #[default(None)] icon: Option<Rc<dyn Fn() -> String>>,
    #[default(None)] border: Option<Option<i32>>,
) {
    let icon = icon.map_or("-".to_string(), |f| f());
    Label(format!("{text} {icon} {border:?}"));
}

/// A generic composable: its `T` is fixed by what the `Some` holds.
#[kompozable]
fn Held<T: Clone + fmt::Debug + 'static>(value: Option<T>) {
    Label(format!("{value:?}"));
}

struct Shapes {
    shape: Option<Rc<dyn fmt::Debug>>,
}

#[named]
impl Shapes {
    fn copy(&self, #[default(self.shape.clone())] shape: Option<Rc<dyn fmt::Debug>>) -> Shapes {
        Shapes { shape }
    }
}

/// An impl's generic parameter, known from the receiver: a `T` that is an `Option` still takes a `Some(x)` for
/// `Some(Some(x))`.
struct Slot<T>(Option<T>);

#[named]
impl<T: Clone + fmt::Debug> Slot<T> {
    fn refilled(&self, #[default(self.0.clone())] value: Option<T>) -> String {
        format!("{value:?}")
    }
}

#[named]
fn handled(#[default(None)] on: Option<Rc<dyn Fn() -> i32>>, #[default(0)] base: i32) -> i32 {
    base + on.map_or(0, |f| f())
}

#[named]
fn Handler(#[default(None)] on: Option<Rc<dyn Fn() -> i32>>, #[default(1)] times: i32) -> i32 {
    times * on.map_or(0, |f| f())
}

/// A function's own generic parameter, free until an argument names it: `Some(x)` makes it `x`'s type.
#[named]
fn shown<T>(#[default(None)] value: Option<T>) -> Option<T> {
    value
}

/// What the prelude's variants hold coerces to the type wanted where they go: an argument written `Some(…)` to an
/// `Option<Rc<dyn Fn()>>` (named, positional, a composable's, a function's, a PascalCase function's through its
/// type, a method's), a `Some`, `Ok` or `Err` elsewhere in a body; an `Option<Option<T>>` (and an `Option<T>` whose
/// `T` is an impl's generic parameter) still takes a `Some(x)` for `Some(Some(x))`, a generic `T` is fixed by what
/// is in the `Some`, and a closure in a `Some(…)` still captures clones.
fn what_a_some_holds_coerces_to_the_parameters_type() {
    let rule = Rule::new(kompozable_lambda!(|| {
        let name = "kalem".to_string();
        Iconed("a", icon = Some(Rc::new(|| "ikon".to_string())));
        Iconed("b", Some(Rc::new(move || name.clone())), border = Some(3));
        Iconed("c", Some(Rc::new(|| "son".to_string())), Some(None));
        let kept: Option<Rc<dyn Fn() -> String>> = Some(Rc::new(|| "yerel".to_string()));
        Iconed("d", icon = kept);
        let ok: Result<Box<dyn fmt::Debug>, Box<dyn fmt::Debug>> =
            if true { Ok(Box::new(1)) } else { Err(Box::new(2)) };
        Label(format!("{ok:?}"));
        Held(Some(3));
        Held(value = None::<u8>);
        Held(Some(Some("iç")));
        let count = Rc::new(Cell::new(5));
        let a = handled(on = Some(Rc::new(move || count.get())));
        let b = handled(Some(Rc::new(|| 2)), 10);
        let c = Handler(on = Some(Rc::new(|| 3)), times = 2);
        let d = Handler(Some(Rc::new(|| 4)), 3);
        Label(format!("{a} {b} {c} {d}"));
        let shapes = Shapes { shape: None };
        let named = shapes.copy(shape = Some(Rc::new("kare")));
        let positional = shapes.copy(Some(Rc::new(1)));
        Label(format!("{:?} {:?}", named.shape, positional.shape));
        Label(format!("{:?} {:?} {:?}", shown(value = Some(2)), shown(value = "düz"), shown(Some(Some(1)))));
        let slot = Slot(Some(Some(1)));
        Label(format!("{} {} {}", slot.refilled(value = Some(2)), slot.refilled(Some(None)), slot.refilled()));
    }));
    assert_eq!(
        rule.names(),
        [
            "a ikon None",
            "b kalem Some(Some(3))",
            "c son Some(None)",
            "d yerel None",
            "Ok(1)",
            "Some(3)",
            "None",
            "Some(Some(\"iç\"))",
            "5 12 6 12",
            "Some(\"kare\") Some(1)",
            "Some(2) Some(\"düz\") Some(Some(1))",
            "Some(Some(2)) Some(None) Some(Some(1))",
        ]
    );
    // Plain code, through `named_args!`.
    let shapes = Shapes { shape: None };
    let square = named_args!(shapes.copy(shape = Some(Rc::new("kare"))));
    assert_eq!(format!("{:?}", square.shape), "Some(\"kare\")");
    assert_eq!(named_args!(handled(on = Some(Rc::new(|| 7)))), 7);
    assert_eq!(named_args!(Handler(Some(Rc::new(|| 2)), times = 4)), 8);
}

// ---- Identity ------------------------------------------------------------------------------------------------

#[kompozable]
fn Remembering(id: i32) {
    let made = remember(|| mutable_state_of(id));
    Label(format!("{id}:{made}"));
}

/// (WP8) A parameter named by a keyword (Kotlin's `type`: Rust's `r#type`), captured by closures given to calls
/// and kept by a restartable composable, with a closure parameter of a keyword's name passed on.
#[kompozable]
fn KeywordNamed(r#type: i32, r#move: impl Fn(i32) + 'static) {
    WithContent(r#type, move || Label(format!("type {type}", type = r#type)));
    Button(move || r#move(r#type), content = || Label("move"));
}

fn a_keyword_parameter_is_captured_and_passed_on_raw() {
    clear_clicks();
    let n = mutable_state_of(3);
    let got = Rc::new(Cell::new(0));
    let (m, g) = (n.clone(), got.clone());
    let rule = Rule::new(kompozable_lambda!(move || {
        let g = g.clone();
        KeywordNamed(m.get(), move |t| g.set(t))
    }));
    assert_eq!(rule.names(), ["type 3", "button", "move"]);
    click(0);
    assert_eq!(got.get(), 3);
    n.set(4);
    rule.wait_for_idle();
    assert_eq!(rule.names(), ["type 4", "button", "move"]);
}

/// (WP8) Composables called from a plain function through `kompozable_block!`, given a closure that only calls a
/// local closure of the block: the closure is another one when that local is (its identity keyed by it), so the
/// restartable callee isn't skipped with the old one.
#[track_caller]
fn block_with_content(label: Rc<dyn Fn()>) {
    kompozable_block!({
        let label = label.clone();
        WithContent(1, move || label());
    })
}

fn a_closure_calling_a_local_of_a_block_has_its_identity() {
    let n = mutable_state_of(3);
    let m = n.clone();
    let rule = Rule::new(kompozable_lambda!(move || {
        let v = m.get();
        block_with_content(Rc::new(move || Label(format!("n {v}"))));
    }));
    assert_eq!(rule.names(), ["n 3"]);
    n.set(4);
    rule.wait_for_idle();
    assert_eq!(rule.names(), ["n 4"]);
}

fn each_call_site_is_its_own_identity() {
    let flip = mutable_state_of(false);
    let f = flip.clone();
    let rule = Rule::new(kompozable_lambda!(move || {
        if f.get() {
            Remembering(2);
        }
        Remembering(1); // its own call site: it keeps its state whether the one above is called or not
    }));
    assert_eq!(rule.names(), ["1:1"]);
    flip.set(true);
    rule.wait_for_idle();
    assert_eq!(rule.names(), ["2:2", "1:1"]);
}

// ---- Method calls by name and their receivers' types --------------------------------------------------------------

/// A type with a `#[named]` method of every parameter defaulted (`OutlinedTextFieldDefaults.content_padding()`'s
/// shape): a positional call of that name in a composable body goes to its builder.
#[derive(Clone, Copy)]
struct Paddings;

#[named]
impl Paddings {
    fn inset(&self, #[default(1)] start: i32, #[default(2)] end: i32) -> i32 {
        start * 10 + end
    }
}

/// What reaches a `Paddings` through a deref (Kotlin's supertype): its calls of that name are `Paddings`' too.
struct PaddingsHolder(Paddings);

impl std::ops::Deref for PaddingsHolder {
    type Target = Paddings;
    fn deref(&self) -> &Paddings {
        &self.0
    }
}

/// An unrelated trait with a method of the same name taking an argument (foundation's `StyleScope.content_padding`).
trait InsetScope {
    fn inset(&self, by: i32) -> i32;
}

struct InsetCounter(AtomicI32);

impl InsetScope for InsetCounter {
    fn inset(&self, by: i32) -> i32 {
        self.0.fetch_add(by, Ordering::Relaxed) + by
    }
}

/// The scope the test below hands its composable (a `'static` one, as a scope a layout gives; a static rather than a
/// leaked box, which Miri reports).
static INSET_COUNTER: InsetCounter = InsetCounter(AtomicI32::new(0));

/// The macros rewrite a method call by its name (they can't see a receiver's type in general); where they can — a
/// receiver bound with its type written: a parameter, a closure's parameter, a `let` — and that type is no owner of
/// a `#[named]` method of that name, nor has one as a supertype (what it derefs to, a trait's supertraits), the call
/// stays the plain method's.
fn method_calls_by_name_keep_the_plain_method_of_a_typed_receiver_of_another_type() {
    #[kompozable]
    fn Insetting(scope: &'static dyn InsetScope, out: Rc<RefCell<Vec<i32>>>) {
        // (the parameter's type, a closure parameter's, a `let`'s: plain calls)
        out.borrow_mut().push(scope.inset(5));
        let with = |s: &dyn InsetScope| s.inset(7);
        out.borrow_mut().push(with(scope));
        let typed: &dyn InsetScope = scope;
        out.borrow_mut().push(typed.inset(1));
        // (an owner, as it is or through a deref: its builder)
        out.borrow_mut().push(Paddings.inset(3));
        let holder: PaddingsHolder = PaddingsHolder(Paddings);
        out.borrow_mut().push(holder.inset(4, 5));
        let untyped = Paddings;
        out.borrow_mut().push(untyped.inset(6));
    }
    let scope: &'static InsetCounter = &INSET_COUNTER;
    let out = Rc::new(RefCell::new(Vec::new()));
    let o = out.clone();
    let _rule = Rule::new(kompozable_lambda!(move || Insetting(scope, o.clone())));
    assert_eq!(*out.borrow(), [5, 12, 13, 32, 45, 62]);
}

tests!(
    method_calls_by_name_keep_the_plain_method_of_a_typed_receiver_of_another_type,
    counters_take_named_arguments_and_trailing_closures,
    the_users_scaffold_sample_reads_as_in_kotlin,
    methods_and_factories_take_named_arguments,
    generic_composables_infer_their_types_from_the_arguments,
    composables_whose_arguments_compare_are_skipped,
    closures_share_what_they_capture_and_move_when_told,
    composables_can_be_defined_inside_functions,
    each_call_site_is_its_own_identity,
    calls_inside_macros_are_rewritten,
    methods_of_a_type_from_elsewhere_take_named_arguments_through_a_trait,
    methods_of_every_value_of_a_trait_reach_its_subtraits_objects,
    a_method_without_defaults_named_as_an_indexed_one_takes_its_calls,
    a_name_bound_inside_a_closure_hides_the_outer_one_only_after_its_binding,
    closures_are_the_same_while_what_they_capture_is,
    a_closure_parameter_passed_on_keeps_its_identity,
    a_closure_parameter_left_at_its_default_is_the_same_each_time,
    a_memoized_parameter_is_kept_by_its_identity,
    a_memoized_method_parameter_has_its_identity_from_a_positional_call,
    positional_arguments_past_the_required_ones_go_through_the_builder,
    a_call_can_be_kept_as_written,
    pascal_case_functions_are_functions_everywhere,
    an_option_parameter_takes_what_it_holds,
    what_a_some_holds_coerces_to_the_parameters_type,
    a_keyword_parameter_is_captured_and_passed_on_raw,
    a_closure_calling_a_local_of_a_block_has_its_identity,
);
