//! Navigation 3's behaviour, as Compose's semantics say (navigation3-runtime 1.2's and navigation3-ui 1.2's KDoc and
//! code, lifecycle-viewmodel-navigation3 2.11's): the entry provider DSL, back stacks and their saving, decorated
//! entries and their decorators' pops, the saveable state holder's and the ViewModel store's decorators, results
//! between screens; scenes and scene strategies, the scene state, NavDisplay's navigations, pops and predictive back
//! (seeking, cancelled, completed), its transitions (its own, an entry's, a scene's), z-order, lifecycles, overlay
//! (dialog) scenes, shared entries; and the runtime and ui samples, in Rust (their Kotlin classes as Rust twins).

#![allow(non_snake_case)]

use kompoz_foundation::layout::{Box, Column, Row, SizeModifiers};
use kompoz_foundation::prelude::ClickableModifiers;
use kompoz_foundation::text::BasicText;
use kompoz_graphics::UnitExt;
use kompoz_lifecycle::{LocalLifecycleOwner, ViewModel, ViewModelImpl, view_model};
use kompoz_navigation3::runtime::*;
use kompoz_navigation3::scene::*;
use kompoz_navigation3::ui::*;
use kompoz_navigation3::viewmodel::*;
use kompoz_navigationevent::{NavigationBackHandler, remember_navigation_event_state};
use kompoz_runtime::*;
use kompoz_test::KompozTestRule;
use kompoz_ui::Modifier;
use kompoz_ui::back::BackEventCompat;
use kompoz_ui::input::{Key, KeyModifiers};
use kompoz_ui::layout::SlotKey;
use kompoz_ui::window::DialogProperties;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

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

fn escape(rule: &KompozTestRule) -> bool {
    let taken = rule.host().key_down(Key::Escape, KeyModifiers::None);
    rule.host().key_up(Key::Escape, KeyModifiers::None);
    rule.wait_for_idle();
    taken
}

fn step(progress: f32) -> BackEventCompat {
    BackEventCompat { progress, ..BackEventCompat::default() }
}

fn key<K: NavKey>(k: K) -> Rc<dyn NavKey> {
    Rc::new(k)
}

fn shown(rule: &KompozTestRule, text: &str) -> bool {
    rule.drawn_texts().iter().any(|t| t == text)
}

// ---- keys (Kotlin's `@Serializable object`s and data classes : NavKey) -------------------------------------------

#[derive(Saveable, Clone, PartialEq, Debug)]
struct Home;
impl NavKey for Home {}

#[derive(Saveable, Clone, PartialEq, Debug)]
struct Detail {
    id: i32,
}
impl NavKey for Detail {}

#[derive(Saveable, Clone, PartialEq, Debug)]
struct Settings;
impl NavKey for Settings {}

/// Kotlin's `sealed class SealedKey : NavKey` (closed polymorphism).
#[derive(Saveable, Clone, PartialEq, Debug)]
enum SealedKey {
    Inbox,
    Settings,
}
impl NavKey for SealedKey {}

// ---- runtime -----------------------------------------------------------------------------------------------------

/// entryProvider: an entry of a key, of a class; a key's entry wins over its class's; the fallback for others (by
/// default it throws "Unknown screen"); metadata computed once per key; a key or a class given twice throws.
fn entry_providers_find_entries_by_key_then_class() {
    let computed = Rc::new(Cell::new(0));
    let c = computed.clone();
    let provider = named_args!(entry_provider(
        fallback = |k: Rc<dyn NavKey>| NavEntry::new(k, SlotKey::new("fallback"), Metadata::new(), |_| {}),
        |scope: &mut EntryProviderScope<Rc<dyn NavKey>>| {
            named_args!(scope.entry::<Detail>(
                metadata = move |d: Detail| {
                    c.set(c.get() + 1);
                    metadata(|m| m.put(DialogKey, DialogProperties::default())).plus(&Metadata::of("id", d.id))
                },
                content = |_: Detail| {}
            ));
            named_args!(scope.entry_of_key(Home, content_key = Some(SlotKey::new("home")), content = |_: Home| {}));
            named_args!(scope.entry_of_key(Detail { id: 7 }, content = |_: Detail| {}));
        },
    ));
    let home = provider(key(Home));
    assert_eq!(home.content_key(), SlotKey::new("home"));
    let seven = provider(key(Detail { id: 7 }));
    assert_eq!(seven.content_key(), default_content_key(&Detail { id: 7 }));
    assert_eq!(
        default_content_key(&Detail { id: 7 }),
        SlotKey::new(format!("Detail {{ id: 7 }}:class {}", std::any::type_name::<Detail>()))
    );
    // (Detail(7) is a key's entry, but its class's metadata comes first)
    assert_eq!(seven.metadata().get_as::<i32>("id"), Some(7));
    let three = provider(key(Detail { id: 3 }));
    assert!(three.metadata().contains(DialogKey));
    provider(key(Detail { id: 3 }));
    assert_eq!(computed.get(), 2); // (7 and 3, each once)
    assert_eq!(provider(key(Settings)).content_key(), SlotKey::new("fallback"));
    let unknown = entry_provider(|_: &mut EntryProviderScope<String>| {});
    assert_eq!(panics(|| drop(unknown("x".to_string()))), "IllegalStateException: Unknown screen x");
    let twice = panics(|| {
        drop(entry_provider(|s: &mut EntryProviderScope<String>| {
            named_args!(s.entry_of_key("a".to_string(), content = |_: String| {}));
            named_args!(s.entry_of_key("a".to_string(), content = |_: String| {}));
        }))
    });
    assert_eq!(twice, "IllegalArgumentException: An `entry` with the key `key` has already been added: a.");
    let class_twice = panics(|| {
        drop(entry_provider(|s: &mut EntryProviderScope<Rc<dyn NavKey>>| {
            named_args!(s.entry::<Home>(content = |_: Home| {}));
            named_args!(s.entry::<Home>(content = |_: Home| {}));
        }))
    });
    assert_eq!(class_twice, "IllegalArgumentException: An `entry` with the same `clazz` has already been added: Home.");
}

/// Metadata: the DSL's keys, `+` (the right side wins), equality by values' identity.
fn metadata_is_a_map_of_typed_keys() {
    let a = metadata(|m| m.put(DialogKey, DialogProperties::default()));
    assert!(a.contains(DialogKey) && a.get(DialogKey) == Some(DialogProperties::default()));
    let b = metadata(|m| {
        m.put(DialogKey, named_args!(DialogProperties(dismiss_on_back_press = false)));
    });
    let sum = a.clone() + b.clone();
    assert_eq!(sum.get(DialogKey).map(|p| p.dismiss_on_back_press), Some(false));
    assert_eq!(sum.size(), 1);
    assert_eq!(a, a.clone());
    assert_ne!(a, b);
    assert!(Metadata::new().is_empty());
    let specs = NavDisplay::transition_spec(|_| None) + NavDisplay::pop_transition_spec(|_| None);
    assert!(specs.contains(NavDisplay::TransitionKey) && specs.contains(NavDisplay::PopTransitionKey));
    assert!(!specs.contains(NavDisplay::PredictivePopTransitionKey));
}

/// NavBackStack: a snapshot list of keys of any NavKey type; saved with its keys' classes (open polymorphism) or
/// as its keys (closed); the savers of Compose's serializers.
fn back_stacks_are_lists_saved_with_their_keys() {
    let stack = NavBackStack(vec![key(Home)]);
    stack.add(Detail { id: 1 });
    stack.add(Settings);
    assert_eq!(stack.size(), 3);
    assert!(stack.contains(Detail { id: 1 }));
    assert_eq!(stack.last_index(), 2);
    assert_eq!(stack.remove_last_or_null().map(|k| k.is::<Settings>()), Some(true));
    let saved = stack.save();
    let restored = NavBackStack::<Rc<dyn NavKey>>::restore(&saved).expect("restored");
    assert_eq!(restored.to_list(), stack.to_list());
    let saver = NavBackStackSerializer(NavKeySerializer());
    assert_eq!(saver.restore(&saver.save(&stack)).unwrap().to_list(), stack.to_list());
    // (a class never seen in this process can't be read back: the stack is made anew)
    let unknown =
        SavedValue::list([SavedValue::map([("type", SavedValue::from("nowhere::Key")), ("value", SavedValue::Null)])]);
    assert!(NavBackStack::<Rc<dyn NavKey>>::restore(&unknown).is_none());
    let sealed = NavBackStack(vec![SealedKey::Inbox, SealedKey::Settings]);
    let back = NavBackStack::<SealedKey>::restore(&sealed.save()).unwrap();
    assert_eq!(back.to_list(), [SealedKey::Inbox, SealedKey::Settings]);
    assert_eq!(
        panics(|| drop(NavBackStack(Vec::<String>::new()).remove_at(0))),
        "IndexOutOfBoundsException: Index 0 out of bounds for length 0"
    );
}

/// rememberNavBackStack: remembered, and back after the process ended (the host's saved state).
fn remembered_back_stacks_come_back_with_the_saved_state() {
    let held: Rc<RefCell<Option<NavBackStack<Rc<dyn NavKey>>>>> = Rc::default();
    let show = mutable_state_of(false);
    let content = {
        let (h, s) = (held.clone(), show.clone());
        kompozable_lambda!(move || {
            if s.get() {
                let stack = remember_nav_back_stack(vec![key(Home)]);
                *h.borrow_mut() = Some(stack.clone());
                BasicText(format!("{:?}", stack.to_list()));
            }
        })
    };
    let rule = KompozTestRule::new(content.clone());
    show.set(true);
    rule.wait_for_idle();
    held.borrow().as_ref().unwrap().add(Detail { id: 4 });
    rule.wait_for_idle();
    assert!(shown(&rule, "[Home, Detail { id: 4 }]"));
    let bytes = rule.host().save_state();
    drop(rule);
    show.set(false);
    let rule = KompozTestRule::new(content);
    rule.host().restore_state(&bytes);
    show.set(true);
    rule.wait_for_idle();
    assert!(shown(&rule, "[Home, Detail { id: 4 }]"));
}

/// rememberDecoratedNavEntries: decorators wrap in order (decorated again: the new ones outside); a decorator's onPop
/// runs, in reverse, once the popped entry has left the composition — at once for one never shown.
fn decorators_wrap_entries_and_clean_up_popped_keys() {
    let log = log();
    let stack = NavBackStack(vec!["a".to_string()]);
    let (l, s) = (log.clone(), stack.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (l1, l2, l3, l4, l5) = (l.clone(), l.clone(), l.clone(), l.clone(), l.clone());
        let original = remember(move || {
            NavEntryDecorator::new(
                move |k: SlotKey| l1.borrow_mut().push(format!("pop original {k:?}")),
                move |e: NavEntry<String>| {
                    l2.borrow_mut().push("original".into());
                    e.Content();
                },
            )
        });
        let second = remember(move || {
            NavEntryDecorator::new(
                move |k: SlotKey| l3.borrow_mut().push(format!("pop second {k:?}")),
                |e: NavEntry<String>| e.Content(),
            )
        });
        let entries = remember_decorated_nav_entries(s.clone(), vec![original, second], |k: String| {
            NavEntry::new(k.clone(), SlotKey::new(k.clone()), Metadata::new(), |k: String| BasicText(k))
        });
        let additional = remember(move || {
            NavEntryDecorator::new(
                |_| {},
                move |e: NavEntry<String>| {
                    l4.borrow_mut().push("additional".into());
                    e.Content();
                },
            )
        });
        let again = remember_decorated_nav_entries_of(entries, vec![additional]);
        let _ = &l5;
        again.last().unwrap().Content();
    }));
    assert_eq!(taken(&log), ["additional", "original"]);
    // A key pushed and popped without being shown: its pop at once.
    stack.add("b".to_string());
    stack.add("c".to_string());
    rule.wait_for_idle();
    assert!(shown(&rule, "c"));
    taken(&log);
    stack.remove_at(1);
    rule.wait_for_idle();
    let pops = |l: Vec<String>| l.into_iter().filter(|e| e.starts_with("pop")).collect::<Vec<_>>();
    assert_eq!(pops(taken(&log)), ["pop second b", "pop original b"]);
    // The shown one popped: once its content has gone.
    stack.remove_last_or_null();
    rule.wait_for_idle();
    assert_eq!(pops(taken(&log)), ["pop second c", "pop original c"]);
    assert!(shown(&rule, "a"));
}

/// SaveableStateHolderNavEntryDecorator: an entry's saveable state kept while it is on the back stack (away and back),
/// gone once popped.
fn saveable_state_is_kept_per_entry_until_popped() {
    let stack = NavBackStack(vec!["a".to_string()]);
    let counters: Rc<RefCell<Vec<(String, MutableState<i32>)>>> = Rc::default();
    let (s, c) = (stack.clone(), counters.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let c = c.clone();
        let decorator = remember_saveable_state_holder_nav_entry_decorator::<String>();
        let entries = remember_decorated_nav_entries(s.clone(), vec![decorator], move |k: String| {
            let c = c.clone();
            NavEntry::new(k.clone(), SlotKey::new(k.clone()), Metadata::new(), move |k: String| {
                let count = remember_saveable(|| mutable_state_of(0));
                c.borrow_mut().push((k.clone(), count.clone()));
                BasicText(format!("{k} {}", count.get()));
            })
        });
        entries.last().unwrap().Content();
    }));
    let counter = |k: &str| counters.borrow().iter().rev().find(|(n, _)| n == k).unwrap().1.clone();
    counter("a").set(5);
    rule.wait_for_idle();
    stack.add("b".to_string());
    rule.wait_for_idle();
    assert!(shown(&rule, "b 0"));
    counter("b").set(2);
    stack.remove_last_or_null();
    rule.wait_for_idle();
    assert!(shown(&rule, "a 5"));
    stack.add("b".to_string());
    rule.wait_for_idle();
    assert!(shown(&rule, "b 0"));
}

/// ResultEventBus: an event result to each ResultEffect, a state result conflated; removed, a key's results stop.
fn results_pass_between_screens() {
    let bus = ResultEventBus();
    let received = log();
    let (b, r) = (bus.clone(), received.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let r = r.clone();
        let decorator = remember_result_event_bus_nav_entry_decorator(b.clone());
        let entries = remember_decorated_nav_entries(vec!["home".to_string()], vec![decorator], move |k: String| {
            let r = r.clone();
            NavEntry::new(k.clone(), SlotKey::new(k), Metadata::new(), move |_| {
                let r = r.clone();
                ResultEffect(
                    on_result = move |name: String| {
                        let r = r.clone();
                        async move { r.borrow_mut().push(name) }
                    },
                );
                let latest = LocalResultEventBus.current().conflate_as_state(0_i32);
                BasicText(format!("son {}", latest.get()));
            })
        });
        entries[0].Content();
    }));
    bus.send_result("kedi".to_string());
    bus.send_result(3_i32);
    rule.wait_for_idle();
    bus.send_result("köpek".to_string());
    bus.send_result(9_i32);
    rule.wait_for_idle();
    assert_eq!(taken(&received), ["kedi", "köpek"]);
    assert!(shown(&rule, "son 9"));
    bus.remove_result::<String>();
    rule.wait_for_idle();
}

// ---- NavDisplay ---------------------------------------------------------------------------------------------------

/// A NavDisplay of Home and Detail keys, its entries telling their lifecycle state.
fn app(stack: NavBackStack<Rc<dyn NavKey>>) -> impl Content + Clone {
    kompozable_lambda!(move || {
        NavDisplay(
            stack.clone(),
            entry_provider = entry_provider(|s: &mut EntryProviderScope<Rc<dyn NavKey>>| {
                s.entry::<Home>(content = |_: Home| Screen("home".into()));
                s.entry::<Detail>(content = |d: Detail| Screen(format!("detail {}", d.id)));
                s.entry::<Settings>(
                    metadata = DialogSceneStrategy::dialog(DialogProperties::default()),
                    content = |_: Settings| Screen("settings".into()),
                );
            }),
            scene_strategies = vec![
                Rc::new(DialogSceneStrategy()) as Rc<dyn SceneStrategy<Rc<dyn NavKey>>>,
                Rc::new(SinglePaneSceneStrategy()),
            ],
        );
    })
}

#[kompozable]
fn Screen(name: String) {
    let state = LocalLifecycleOwner.current().lifecycle().current_state_as_state().get();
    BasicText(name.clone());
    BasicText(format!("{name} {}", state.name()));
}

/// NavDisplay: the last entry shown; a navigation animated (both shown, the new one STARTED till settled), back pops
/// it (Escape), the popped one leaves; back not taken with one entry.
fn nav_display_shows_the_top_and_animates_navigations_and_pops() {
    let stack = NavBackStack(vec![key(Home)]);
    let rule = KompozTestRule::new(app(stack.clone()));
    assert!(shown(&rule, "home RESUMED"));
    assert!(!escape(&rule));
    rule.main_clock().set_auto_advance(false);
    stack.add(Detail { id: 1 });
    rule.advance_time_by(100);
    assert!(shown(&rule, "home") && shown(&rule, "detail 1"));
    assert!(shown(&rule, "detail 1 STARTED"));
    rule.advance_time_by(1000);
    rule.main_clock().set_auto_advance(true);
    rule.wait_for_idle();
    assert!(!shown(&rule, "home"));
    assert!(shown(&rule, "detail 1 RESUMED"));
    assert!(escape(&rule));
    assert_eq!(stack.size(), 1);
    assert!(shown(&rule, "home RESUMED") && !shown(&rule, "detail 1"));
    let empty = panics(|| {
        let _ = KompozTestRule::new(app(NavBackStack(Vec::new())));
    });
    assert_eq!(empty, "IllegalArgumentException: NavDisplay backstack cannot be empty");
}

/// Predictive back: the gesture seeks the transition to the previous scene (both shown), cancelled it comes back,
/// completed it pops.
fn predictive_back_seeks_to_the_previous_scene() {
    let stack = NavBackStack(vec![key(Home), key(Detail { id: 2 })]);
    let rule = KompozTestRule::new(app(stack.clone()));
    assert!(shown(&rule, "detail 2") && !shown(&rule, "home"));
    let host = rule.host();
    host.back_started(step(0.0));
    host.back_progressed(step(0.5));
    rule.wait_for_idle();
    assert!(shown(&rule, "home") && shown(&rule, "detail 2"));
    host.back_cancelled();
    rule.wait_for_idle();
    assert!(!shown(&rule, "home") && shown(&rule, "detail 2"));
    assert_eq!(stack.size(), 2);
    host.back_started(step(0.0));
    host.back_progressed(step(0.7));
    rule.wait_for_idle();
    assert!(host.back_invoked());
    rule.wait_for_idle();
    assert_eq!(stack.size(), 1);
    assert!(shown(&rule, "home RESUMED") && !shown(&rule, "detail 2"));
}

/// A dialog scene: the entry in a dialog over the scene of the entries before it (STARTED below it, RESUMED in it);
/// back pops the dialog's entry.
fn dialog_scenes_show_entries_over_the_others() {
    let stack = NavBackStack(vec![key(Home)]);
    let rule = KompozTestRule::new(app(stack.clone()));
    stack.add(Settings);
    rule.wait_for_idle();
    assert!(shown(&rule, "home STARTED") && shown(&rule, "settings RESUMED"));
    assert!(escape(&rule));
    assert_eq!(stack.size(), 1);
    assert!(shown(&rule, "home RESUMED") && !shown(&rule, "settings"));
}

/// The transitions: NavDisplay's, else the scene's metadata (its last entry's by default): an entry's 100 ms
/// transition settles before the default 700 ms one would.
fn transitions_come_from_the_scene_metadata_first() {
    let stack = NavBackStack(vec!["a".to_string()]);
    let s = stack.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        NavDisplay(
            s.clone(),
            entry_provider = |k: String| {
                let fast = k == "fast";
                let meta = if fast {
                    metadata(|m| {
                        m.put(
                            NavDisplay::TransitionKey,
                            |_: &kompoz_animation::AnimatedContentTransitionScope<AnyScene>| {
                                let spec = || -> std::sync::Arc<dyn kompoz_animation::FiniteAnimationSpec<f32>> {
                                    std::sync::Arc::new(named_args!(kompoz_animation::tween(100)))
                                };
                                named_args!(kompoz_animation::ContentTransform(
                                    kompoz_animation::fade_in(animation_spec = spec()),
                                    kompoz_animation::fade_out(animation_spec = spec())
                                ))
                            },
                        )
                    })
                } else {
                    Metadata::new()
                };
                NavEntry::new(k.clone(), SlotKey::new(k), meta, |k: String| BasicText(k))
            },
        );
    }));
    rule.main_clock().set_auto_advance(false);
    stack.add("fast".to_string());
    rule.advance_time_by(300);
    assert!(shown(&rule, "fast") && !shown(&rule, "a"));
    stack.add("slow".to_string());
    rule.advance_time_by(300);
    assert!(shown(&rule, "fast") && shown(&rule, "slow"));
    rule.main_clock().set_auto_advance(true);
    rule.wait_for_idle();
}

/// A strategy of its own: two panes of the last two entries; the entries shown by one scene only (moved between them),
/// the previous scene's entries what predictive back shows; a scene decorator around the scene's content.
fn scene_strategies_and_decorators_make_scenes() {
    #[derive(PartialEq, Debug)]
    struct TwoPane(NavEntry<String>, NavEntry<String>, Vec<NavEntry<String>>);
    impl Scene<String> for TwoPane {
        fn key(&self) -> SlotKey {
            SlotKey::new(("two", self.0.content_key()))
        }
        fn entries(&self) -> Vec<NavEntry<String>> {
            vec![self.0.clone(), self.1.clone()]
        }
        fn previous_entries(&self) -> Vec<NavEntry<String>> {
            self.2.clone()
        }
        fn content(&self) {
            let (a, b) = (self.0.clone(), self.1.clone());
            kompozable_block!({
                Row(content = move || {
                    a.Content();
                    b.Content();
                });
            });
        }
    }
    #[derive(Debug)]
    struct Framed(Rc<dyn Scene<String>>);
    impl PartialEq for Framed {
        fn eq(&self, other: &Self) -> bool {
            *self.0 == *other.0
        }
    }
    impl Scene<String> for Framed {
        fn key(&self) -> SlotKey {
            self.0.key()
        }
        fn entries(&self) -> Vec<NavEntry<String>> {
            self.0.entries()
        }
        fn previous_entries(&self) -> Vec<NavEntry<String>> {
            self.0.previous_entries()
        }
        fn content(&self) {
            kompozable_block!({
                BasicText("çerçeve");
            });
            self.0.content();
        }
    }
    let two_panes = SceneStrategy(|_: &SceneStrategyScope<String>, entries: Vec<NavEntry<String>>| {
        let n = entries.len();
        (n >= 2).then(|| {
            Rc::new(TwoPane(entries[n - 2].clone(), entries[n - 1].clone(), entries[..n - 1].to_vec()))
                as Rc<dyn Scene<String>>
        })
    });
    let frame = SceneDecoratorStrategy(|_: &SceneDecoratorStrategyScope<String>, scene: Rc<dyn Scene<String>>| {
        Rc::new(Framed(scene)) as Rc<dyn Scene<String>>
    });
    let scope = SceneStrategyScope::<String>();
    let e = |k: &str| NavEntry::new(k.to_string(), SlotKey::new(k.to_string()), Metadata::new(), |_| {});
    let scene = two_panes.calculate_scene(&scope, vec![e("a"), e("b"), e("c")]).unwrap();
    let keys: Vec<SlotKey> = scene.entries().iter().map(NavEntry::content_key).collect();
    assert_eq!(keys, [SlotKey::new("b".to_string()), SlotKey::new("c".to_string())]);
    assert!(two_panes.calculate_scene(&scope, vec![e("a")]).is_none());
    let stack = NavBackStack(vec!["a".to_string(), "b".to_string()]);
    let (s, t, f) = (stack.clone(), two_panes.clone(), frame.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        NavDisplay(
            s.clone(),
            scene_strategies = vec![t.clone()],
            scene_decorator_strategies = vec![f.clone()],
            entry_provider =
                |k: String| NavEntry::new(k.clone(), SlotKey::new(k), Metadata::new(), |k: String| BasicText(k)),
        );
    }));
    assert!(shown(&rule, "a") && shown(&rule, "b") && shown(&rule, "çerçeve"));
    stack.add("c".to_string());
    rule.wait_for_idle();
    let texts = rule.drawn_texts();
    assert_eq!(texts.iter().filter(|t| *t == "b").count(), 1, "{texts:?}");
    assert!(shown(&rule, "c") && !shown(&rule, "a"));
}

/// The scene state: overlays first, the current scene below them, the previous scenes; its navigation back handler
/// pops as many entries as the scene's back does.
fn scene_states_tell_overlays_current_and_previous_scenes() {
    let held: Rc<RefCell<Option<SceneState<String>>>> = Rc::default();
    let stack = NavBackStack(vec!["a".to_string(), "b".to_string(), "dialog".to_string()]);
    let (h, s) = (held.clone(), stack.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let st = s.clone();
        let entries = remember_decorated_nav_entries(
            s.clone(),
            entry_provider = |k: String| {
                let meta = if k == "dialog" {
                    DialogSceneStrategy::dialog(DialogProperties::default())
                } else {
                    Metadata::new()
                };
                NavEntry::new(k.clone(), SlotKey::new(k), meta, |k: String| BasicText(k))
            },
        );
        let state = remember_scene_state(
            entries,
            vec![Rc::new(DialogSceneStrategy()) as Rc<dyn SceneStrategy<String>>],
            on_back = move || {
                st.remove_last_or_null();
            },
        );
        *h.borrow_mut() = Some(state.clone());
        let st = s.clone();
        SceneNavigationBackHandler(
            state.clone(),
            on_back_completed = move || {
                st.remove_last_or_null();
            },
        );
        NavDisplayOfSceneState(state.clone(), remember_navigation_event_state_of_scene_state(state));
    }));
    let state = held.borrow().clone().unwrap();
    assert_eq!(state.overlay_scenes().len(), 1);
    assert_eq!(state.current_scene().entries().len(), 1);
    assert_eq!(state.previous_scenes().len(), 1);
    assert!(shown(&rule, "dialog") && shown(&rule, "b"));
    assert!(escape(&rule));
    assert_eq!(stack.size(), 2);
}

/// The ViewModel store decorator: a ViewModel per entry, cleared once its entry is popped.
fn view_models_are_scoped_to_their_entries() {
    struct Count {
        base: ViewModelImpl,
        cleared: Rc<Cell<i32>>,
    }
    thread_local!(static CLEARED: Rc<Cell<i32>> = Rc::new(Cell::new(0)));
    impl ViewModel for Count {
        fn view_model_impl(&self) -> &ViewModelImpl {
            &self.base
        }
        fn on_cleared(&self) {
            self.cleared.set(self.cleared.get() + 1);
        }
        fn new_instance(_: &kompoz_lifecycle::CreationExtras) -> Option<Self> {
            Some(Count { base: ViewModelImpl::new(), cleared: CLEARED.with(|c| c.clone()) })
        }
    }
    let seen: Rc<RefCell<Vec<(String, usize)>>> = Rc::default();
    let stack = NavBackStack(vec!["a".to_string()]);
    let (s, v) = (stack.clone(), seen.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let v = v.clone();
        NavDisplay(
            s.clone(),
            entry_decorators = vec![
                remember_saveable_state_holder_nav_entry_decorator(),
                remember_view_model_store_nav_entry_decorator(),
            ],
            entry_provider = move |k: String| {
                let v = v.clone();
                NavEntry::new(k.clone(), SlotKey::new(k), Metadata::new(), move |k: String| {
                    let vm: Rc<Count> = view_model();
                    v.borrow_mut().push((k.clone(), Rc::as_ptr(&vm) as usize));
                    BasicText(k);
                })
            },
        );
    }));
    stack.add("b".to_string());
    rule.wait_for_idle();
    let ptr = |k: &str| seen.borrow().iter().rev().find(|(n, _)| n == k).unwrap().1;
    assert_ne!(ptr("a"), ptr("b"));
    stack.remove_last_or_null();
    rule.wait_for_idle();
    assert_eq!(CLEARED.with(|c| c.get()), 1);
}

/// LocalNavAnimatedContentScope: given to entries; read outside a NavDisplay it throws.
fn the_animated_content_scope_is_local_to_entries() {
    let stack = NavBackStack(vec!["a".to_string()]);
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        NavDisplay(
            stack.clone(),
            entry_provider = |k: String| {
                NavEntry::new(k.clone(), SlotKey::new(k), Metadata::new(), |_| {
                    let _scope = LocalNavAnimatedContentScope.current();
                    BasicText("kapsam");
                })
            },
        );
    }));
    assert!(shown(&rule, "kapsam"));
    let outside = panics(|| {
        let _ = KompozTestRule::new(kompozable_lambda!(|| {
            let _ = LocalNavAnimatedContentScope.current();
        }));
    });
    assert!(outside.contains("Unexpected access to LocalNavAnimatedContentScope"), "{outside}");
}

// ---- samples ------------------------------------------------------------------------------------------------------

/// SceneStateSamples.kt's SceneStateSample: a scene state, its navigation event state and back handler hoisted.
#[kompozable]
fn SceneStateSample() {
    let back_stack = remember_saveable(|| mutable_state_list_of(vec!["a".to_string(), "b".to_string()]));
    let entries = remember_decorated_nav_entries(
        back_stack.clone(),
        entry_provider = |key: String| NavEntry(key, content = |key: String| BasicText(format!("Key = {key}"))),
    );
    let b = back_stack.clone();
    let scene_state = remember_scene_state(
        entries.clone(),
        vec![Rc::new(SinglePaneSceneStrategy()) as Rc<dyn SceneStrategy<String>>],
        on_back = move || {
            b.remove_last_key();
        },
    );
    let current_scene = scene_state.current_scene();
    let navigation_event_state = remember_navigation_event_state(
        SceneInfo(current_scene.clone()),
        back_info = scene_state.previous_scenes().into_iter().map(SceneInfo).collect(),
    );
    let (b, n) = (back_stack.clone(), entries.len() - current_scene.previous_entries().len());
    NavigationBackHandler(
        navigation_event_state.clone(),
        is_back_enabled = !current_scene.previous_entries().is_empty(),
        on_back_completed = move || {
            // Remove entries from the back stack until we've removed all popped entries
            for _ in 0..n {
                b.remove_last_key();
            }
        },
    );
    NavDisplayOfSceneState(scene_state, navigation_event_state);
}

fn scene_state_sample() {
    let rule = KompozTestRule::new(SceneStateSample);
    assert!(shown(&rule, "Key = b"));
    assert!(escape(&rule));
    assert!(shown(&rule, "Key = a") && !shown(&rule, "Key = b"));
    assert!(!escape(&rule));
}

/// SceneStrategySamples.kt's SceneStrategyOnBackSample: a dialog scene for every entry above the first, its dialog's
/// dismissal the scope's onBack.
struct SimpleDialogScene<T> {
    entry: NavEntry<T>,
    previous_entries: Vec<NavEntry<T>>,
    on_back: Rc<dyn Fn()>,
}

impl<T: std::fmt::Debug> std::fmt::Debug for SimpleDialogScene<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SimpleDialogScene({:?})", self.entry)
    }
}

impl<T: PartialEq> PartialEq for SimpleDialogScene<T> {
    fn eq(&self, other: &Self) -> bool {
        self.entry == other.entry && self.previous_entries == other.previous_entries
    }
}

impl<T: NavEntryKey> Scene<T> for SimpleDialogScene<T> {
    fn key(&self) -> SlotKey {
        self.entry.content_key()
    }
    fn entries(&self) -> Vec<NavEntry<T>> {
        vec![self.entry.clone()]
    }
    fn previous_entries(&self) -> Vec<NavEntry<T>> {
        self.previous_entries.clone()
    }
    fn content(&self) {
        let (on_back, entry) = (self.on_back.clone(), self.entry.clone());
        kompozable_block!({
            kompoz_ui::window::Dialog(on_dismiss_request = move || on_back(), content = move || entry.Content());
        });
    }
    fn as_overlay_scene(self: Rc<Self>) -> Option<Rc<dyn OverlayScene<T>>> {
        Some(self)
    }
}

impl<T: NavEntryKey> OverlayScene<T> for SimpleDialogScene<T> {
    fn overlaid_entries(&self) -> Vec<NavEntry<T>> {
        self.previous_entries.clone()
    }
}

#[derive(Default)]
struct SimpleDialogSceneStrategy;

impl<T: NavEntryKey> SceneStrategy<T> for SimpleDialogSceneStrategy {
    fn calculate_scene(&self, scope: &SceneStrategyScope<T>, entries: Vec<NavEntry<T>>) -> Option<Rc<dyn Scene<T>>> {
        let entry = entries.last()?.clone();
        let previous_entries = entries[..entries.len() - 1].to_vec();
        // Only show this as a dialog if there is something 'underneath' the dialog
        (!previous_entries.is_empty()).then(|| {
            Rc::new(SimpleDialogScene { entry, previous_entries, on_back: scope.on_back() }) as Rc<dyn Scene<T>>
        })
    }
}

fn scene_strategy_on_back_sample() {
    let stack = NavBackStack(vec!["ana".to_string(), "pencere".to_string()]);
    let s = stack.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        NavDisplay(
            s.clone(),
            scene_strategies = vec![Rc::new(SimpleDialogSceneStrategy) as Rc<dyn SceneStrategy<String>>],
            entry_provider = |k: String| NavEntry(k, content = |k: String| BasicText(k)),
        );
    }));
    assert!(shown(&rule, "ana") && shown(&rule, "pencere"));
    // (the dialog's own back: its dismissal, the scope's onBack, the display's)
    assert!(escape(&rule));
    assert_eq!(stack.size(), 1);
}

/// OverlaySamples.kt's DialogSample.
#[derive(Saveable, Clone, PartialEq, Debug)]
struct OverlaidNavKey;
impl NavKey for OverlaidNavKey {}
#[derive(Saveable, Clone, PartialEq, Debug)]
struct DialogNavKey;
impl NavKey for DialogNavKey {}

#[kompozable]
fn DialogSample() {
    let back_stack = remember_nav_back_stack(vec![key(OverlaidNavKey)]);
    let (b1, b2) = (back_stack.clone(), back_stack.clone());
    NavDisplay(
        back_stack,
        on_back = move || {
            b1.remove_last_or_null();
        },
        scene_strategies = vec![Rc::new(DialogSceneStrategy()) as Rc<dyn SceneStrategy<Rc<dyn NavKey>>>],
        entry_provider = entry_provider(move |s: &mut EntryProviderScope<Rc<dyn NavKey>>| {
            let b = b2.clone();
            s.entry::<OverlaidNavKey>(
                content = move |_: OverlaidNavKey| {
                    let b = b.clone();
                    Column(
                        content = move || {
                            BasicText("Overlaid Entry");
                            let b = b.clone();
                            Box(Modifier.test_tag("Open Dialog").clickable(
                                on_click = move || {
                                    b.add(DialogNavKey);
                                },
                            ));
                        },
                    );
                },
            );
            s.entry::<DialogNavKey>(
                metadata = metadata(|m| m.put(DialogKey, DialogProperties::default())),
                content = |_: DialogNavKey| BasicText("Dialog"),
            );
        }),
    );
}

fn dialog_sample() {
    let rule = KompozTestRule::new(DialogSample);
    assert!(shown(&rule, "Overlaid Entry") && !shown(&rule, "Dialog"));
    rule.on_node_with_tag("Open Dialog").perform_click();
    rule.wait_for_idle();
    assert!(shown(&rule, "Overlaid Entry") && shown(&rule, "Dialog"));
    assert!(escape(&rule));
    assert!(!shown(&rule, "Dialog"));
}

/// CustomSceneSamples.kt: a scene's metadata gives the transitions (SceneDefaultTransitionsSample), overriding its
/// entry's (SceneOverrideEntryTransitionsSample): here the scene's 100 ms one, not the entry's or NavDisplay's 5 s.
#[derive(PartialEq, Debug)]
struct OverridingScene<T> {
    current_entry: NavEntry<T>,
    previous_entries: Vec<NavEntry<T>>,
}

fn fast() -> kompoz_animation::ContentTransform {
    let spec = || -> std::sync::Arc<dyn kompoz_animation::FiniteAnimationSpec<f32>> {
        std::sync::Arc::new(named_args!(kompoz_animation::tween(100)))
    };
    named_args!(kompoz_animation::ContentTransform(
        kompoz_animation::fade_in(animation_spec = spec()),
        kompoz_animation::fade_out(animation_spec = spec())
    ))
}

fn slow(_: &kompoz_animation::AnimatedContentTransitionScope<AnyScene>) -> kompoz_animation::ContentTransform {
    let spec = || -> std::sync::Arc<dyn kompoz_animation::FiniteAnimationSpec<f32>> {
        std::sync::Arc::new(named_args!(kompoz_animation::tween(5000)))
    };
    named_args!(kompoz_animation::ContentTransform(
        kompoz_animation::fade_in(animation_spec = spec()),
        kompoz_animation::fade_out(animation_spec = spec())
    ))
}

impl<T: NavEntryKey> Scene<T> for OverridingScene<T> {
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
    // super.metadata defaults to [entries.last()]'s metadata, so this overrides the last entry's transitions
    fn metadata(&self) -> Metadata {
        self.current_entry.metadata()
            + metadata(|m| {
                m.put(NavDisplay::TransitionKey, |_: &kompoz_animation::AnimatedContentTransitionScope<AnyScene>| {
                    fast()
                });
                m.put(
                    NavDisplay::PopTransitionKey,
                    |_: &kompoz_animation::AnimatedContentTransitionScope<AnyScene>| fast(),
                );
            })
    }
}

fn scene_override_entry_transitions_sample() {
    let stack = NavBackStack(vec!["A".to_string()]);
    let s = stack.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        NavDisplay(
            s.clone(),
            transition_spec = slow,
            pop_transition_spec = slow,
            scene_strategies = vec![SceneStrategy(|_: &SceneStrategyScope<String>, entries: Vec<NavEntry<String>>| {
                let last = entries.last()?.clone();
                Some(Rc::new(OverridingScene {
                    current_entry: last,
                    previous_entries: entries[..entries.len() - 1].to_vec(),
                }) as Rc<dyn Scene<String>>)
            })],
            entry_provider = |k: String| {
                let meta = metadata(|m| {
                    m.put(
                        NavDisplay::TransitionKey,
                        |s: &kompoz_animation::AnimatedContentTransitionScope<AnyScene>| slow(s),
                    )
                });
                NavEntry(k, metadata = meta, content = |k: String| BasicText(k))
            },
        );
    }));
    rule.main_clock().set_auto_advance(false);
    stack.add("B".to_string());
    rule.advance_time_by(400);
    assert!(shown(&rule, "B") && !shown(&rule, "A"));
    rule.main_clock().set_auto_advance(true);
    rule.wait_for_idle();
}

/// ComplexBackStackSamples.kt's MultipleBackStackSample and ConcatenatedBackStackSample: back stacks switched with their
/// decorators' states, or concatenated.
/// The sample's getHomeTabEntries / getUserTabEntries.
#[kompozable]
fn get_tab_entries(name: String, back_stack: NavBackStack<String>) -> Vec<NavEntry<String>> {
    if back_stack.is_empty() {
        back_stack.add(name.clone());
    }
    let decorators =
        vec![remember_saveable_state_holder_nav_entry_decorator(), remember_view_model_store_nav_entry_decorator()];
    remember_decorated_nav_entries(back_stack.clone(), decorators, |k: String| {
        NavEntry(
            k,
            content = |k: String| {
                let count = remember_saveable(|| mutable_state_of(0));
                BasicText(format!("{k} {}", count.get()));
            },
        )
    })
}

fn multiple_and_concatenated_back_stack_samples() {
    let home = NavBackStack(vec!["Home".to_string()]);
    let user = NavBackStack(vec!["User".to_string()]);
    let tab = mutable_state_of(0);
    let concatenated = mutable_state_of(false);
    let (h, u, t, c) = (home.clone(), user.clone(), tab.clone(), concatenated.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let home_entries = get_tab_entries("Home".into(), h.clone());
        let user_entries = get_tab_entries("User".into(), u.clone());
        let entries = match (t.get(), c.get()) {
            (0, _) => home_entries,
            (_, false) => user_entries,
            (_, true) => [home_entries, user_entries].concat(),
        };
        let (h, u, t2) = (h.clone(), u.clone(), t.clone());
        NavDisplayOfEntries(
            entries,
            on_back = move || {
                if t2.get() == 0 {
                    h.remove_last_or_null();
                } else {
                    u.remove_last_or_null();
                }
            },
        );
    }));
    assert!(shown(&rule, "Home 0"));
    home.add("Detail in Home".to_string());
    rule.wait_for_idle();
    assert!(shown(&rule, "Detail in Home 0"));
    tab.set(1);
    rule.wait_for_idle();
    assert!(shown(&rule, "User 0"));
    tab.set(0);
    rule.wait_for_idle();
    assert!(shown(&rule, "Detail in Home 0"));
    concatenated.set(true);
    tab.set(1);
    rule.wait_for_idle();
    assert!(shown(&rule, "User 0"));
    assert!(escape(&rule) || user.is_empty());
}

/// BasicSamples.kt's SceneNav (its Profile's ViewModel, Dashboard's predictive pop by swipe edge) in a NavDisplay with
/// the saveable state and ViewModel decorators; SceneNavSharedElementSample / SceneNavSharedEntrySample: entries shared
/// in a SharedTransitionLayout.
#[derive(Saveable, Clone, PartialEq, Debug)]
struct Profile;
impl NavKey for Profile {}
#[derive(Saveable, Clone, PartialEq, Debug)]
struct Dashboard {
    #[saveable(default)]
    user_id: Option<String>,
}
impl NavKey for Dashboard {}

struct ProfileViewModel {
    base: ViewModelImpl,
    name: String,
}
impl ViewModel for ProfileViewModel {
    fn view_model_impl(&self) -> &ViewModelImpl {
        &self.base
    }
    fn new_instance(_: &kompoz_lifecycle::CreationExtras) -> Option<Self> {
        Some(ProfileViewModel { base: ViewModelImpl::new(), name: "no user".into() })
    }
}

#[kompozable]
fn SceneNav() {
    let back_stack = remember_nav_back_stack(vec![key(Profile)]);
    let (b1, b2) = (back_stack.clone(), back_stack.clone());
    NavDisplay(
        back_stack,
        entry_decorators =
            vec![remember_saveable_state_holder_nav_entry_decorator(), remember_view_model_store_nav_entry_decorator()],
        on_back = move || {
            b1.remove_at(b1.last_index());
        },
        entry_provider = entry_provider(
            |k: Rc<dyn NavKey>| NavEntry(k, content = |_| BasicText("Invalid Key")),
            move |s: &mut EntryProviderScope<Rc<dyn NavKey>>| {
                let b = b2.clone();
                s.entry::<Profile>(
                    content = move |_: Profile| {
                        let view_model: Rc<ProfileViewModel> = view_model();
                        BasicText(format!("{} Profile", view_model.name));
                        let b = b.clone();
                        Box(Modifier.test_tag("Dashboard").clickable(
                            on_click = move || {
                                b.add(Dashboard { user_id: Some("no value given".into()) });
                            },
                        ));
                    },
                );
                s.entry::<Dashboard>(metadata = metadata(|m| {
                        m.put(NavDisplay::PredictivePopTransitionKey, |_: &kompoz_animation::AnimatedContentTransitionScope<AnyScene>, swipe_edge: i32| {
                            let spec = || -> std::sync::Arc<dyn kompoz_animation::FiniteAnimationSpec<kompoz_graphics::IntOffset>> {
                                std::sync::Arc::new(named_args!(kompoz_animation::tween(700)))
                            };
                            let edge = if swipe_edge == kompoz_navigationevent::NavigationEvent::EDGE_RIGHT { 1 } else { -1 };
                            kompoz_animation::slide_in_horizontally(spec(), move |it: i32| edge * it / 2)
                                .together_with(kompoz_animation::slide_out_horizontally(spec(), move |it: i32| -edge * it / 2))
                        });
                    }), content = |dashboard_args: Dashboard| BasicText(dashboard_args.user_id.unwrap_or_default()));
            },
        ),
    );
}

fn scene_nav_sample() {
    let rule = KompozTestRule::new(SceneNav);
    assert!(shown(&rule, "no user Profile"));
    rule.on_node_with_tag("Dashboard").perform_click();
    rule.wait_for_idle();
    assert!(shown(&rule, "no value given"));
    let host = rule.host();
    host.back_started(BackEventCompat {
        progress: 0.0,
        swipe_edge: kompoz_ui::back::SwipeEdge::Right,
        ..Default::default()
    });
    host.back_progressed(BackEventCompat {
        progress: 0.5,
        swipe_edge: kompoz_ui::back::SwipeEdge::Right,
        ..Default::default()
    });
    rule.wait_for_idle();
    assert!(shown(&rule, "no user Profile") && shown(&rule, "no value given"));
    assert!(host.back_invoked());
    rule.wait_for_idle();
    assert!(shown(&rule, "no user Profile") && !shown(&rule, "no value given"));
}

#[derive(Saveable, Clone, PartialEq, Debug)]
struct CatList;
impl NavKey for CatList {}
#[derive(Saveable, Clone, PartialEq, Debug)]
struct Cat {
    image_id: i32,
    name: String,
    description: String,
}
#[derive(Saveable, Clone, PartialEq, Debug)]
struct CatDetail {
    cat: Cat,
}
impl NavKey for CatDetail {}

fn cats() -> Vec<Cat> {
    vec![
        Cat { image_id: 1, name: "happy".into(), description: "cat lying down".into() },
        Cat { image_id: 2, name: "lucky".into(), description: "cat playing".into() },
    ]
}

#[kompozable]
fn SharedCats(shared_entries: bool) {
    let back_stack = remember_nav_back_stack(vec![key(CatList)]);
    kompoz_animation::SharedTransitionLayout(
        content = move |scope: &kompoz_animation::SharedTransitionScope| {
            let (b1, b2, sc) = (back_stack.clone(), back_stack.clone(), scope.clone());
            NavDisplay(
                back_stack.clone(),
                on_back = move || {
                    b1.remove_at(b1.last_index());
                },
                shared_transition_scope = shared_entries.then(|| sc.clone()),
                entry_provider = entry_provider(move |s: &mut EntryProviderScope<Rc<dyn NavKey>>| {
                    let (b, sc1, sc2) = (b2.clone(), sc.clone(), sc.clone());
                    s.entry::<CatList>(
                        content = move |_: CatList| {
                            let (b, sc) = (b.clone(), sc1.clone());
                            Column(
                                content = move || {
                                    for cat in cats() {
                                        let state = sc.remember_shared_content_state(SlotKey::new(cat.image_id));
                                        let image = sc.shared_element(
                                            Modifier,
                                            state,
                                            LocalNavAnimatedContentScope.current().animated_visibility_scope(),
                                        );
                                        let b = b.clone();
                                        let c = cat.clone();
                                        Box(image.size(100.dp()).test_tag(cat.name.clone()).clickable(
                                            on_click = move || {
                                                b.add(CatDetail { cat: c.clone() });
                                            },
                                        ));
                                        BasicText(cat.name.clone());
                                    }
                                },
                            );
                        },
                    );
                    s.entry::<CatDetail>(
                        content = move |args: CatDetail| {
                            let state = sc2.remember_shared_content_state(SlotKey::new(args.cat.image_id));
                            let image = sc2.shared_element(
                                Modifier,
                                state,
                                LocalNavAnimatedContentScope.current().animated_visibility_scope(),
                            );
                            Box(image.size(300.dp()));
                            BasicText(args.cat.description.clone());
                        },
                    );
                }),
            );
        },
    );
}

fn scene_nav_shared_samples() {
    for shared_entries in [false, true] {
        let rule = KompozTestRule::new(kompozable_lambda!(move || SharedCats(shared_entries)));
        assert!(shown(&rule, "happy"));
        rule.on_node_with_tag("lucky").perform_click();
        rule.wait_for_idle();
        assert!(shown(&rule, "cat playing") && !shown(&rule, "happy"), "{shared_entries} {:?}", rule.drawn_texts());
        assert!(escape(&rule));
        assert!(shown(&rule, "happy"));
    }
}

/// NavBackStackSamples.kt and RememberNavBackStackSamples.kt / NavBackStackSerializerSamples.kt: open polymorphism (a
/// back stack of NavKeys: each key's class registered, Kotlin's `polymorphic(NavKey::class) { subclass(…) }`, or
/// Android's reflection), closed polymorphism (a sealed key type), encodeToSavedState/decodeFromSavedState.
fn nav_back_stack_samples() {
    register_nav_key::<Home>();
    let back_stack = NavBackStack(vec![key(Home), key(Detail { id: 42 })]);
    let stack_serializer = NavBackStackSerializer(NavKeySerializer());
    let encoded = kompoz_lifecycle::encode_to_saved_state_with(&stack_serializer, &back_stack);
    let decoded = kompoz_lifecycle::decode_from_saved_state_with(&stack_serializer, &encoded);
    assert_eq!(decoded.to_list(), back_stack.to_list());
    let closed = NavBackStack::<SealedKey>::new(Vec::new());
    let saver: Saver<NavBackStack<SealedKey>> = serializer::<NavBackStack<SealedKey>>();
    assert!(saver.restore(&saver.save(&closed)).unwrap().is_empty());
}

fn main() {
    let tests: &[(&str, fn())] = &[
        ("entry_providers_find_entries_by_key_then_class", entry_providers_find_entries_by_key_then_class),
        ("metadata_is_a_map_of_typed_keys", metadata_is_a_map_of_typed_keys),
        ("back_stacks_are_lists_saved_with_their_keys", back_stacks_are_lists_saved_with_their_keys),
        (
            "remembered_back_stacks_come_back_with_the_saved_state",
            remembered_back_stacks_come_back_with_the_saved_state,
        ),
        ("decorators_wrap_entries_and_clean_up_popped_keys", decorators_wrap_entries_and_clean_up_popped_keys),
        ("saveable_state_is_kept_per_entry_until_popped", saveable_state_is_kept_per_entry_until_popped),
        ("results_pass_between_screens", results_pass_between_screens),
        (
            "nav_display_shows_the_top_and_animates_navigations_and_pops",
            nav_display_shows_the_top_and_animates_navigations_and_pops,
        ),
        ("predictive_back_seeks_to_the_previous_scene", predictive_back_seeks_to_the_previous_scene),
        ("dialog_scenes_show_entries_over_the_others", dialog_scenes_show_entries_over_the_others),
        ("transitions_come_from_the_scene_metadata_first", transitions_come_from_the_scene_metadata_first),
        ("scene_strategies_and_decorators_make_scenes", scene_strategies_and_decorators_make_scenes),
        (
            "scene_states_tell_overlays_current_and_previous_scenes",
            scene_states_tell_overlays_current_and_previous_scenes,
        ),
        ("view_models_are_scoped_to_their_entries", view_models_are_scoped_to_their_entries),
        ("the_animated_content_scope_is_local_to_entries", the_animated_content_scope_is_local_to_entries),
        ("scene_state_sample", scene_state_sample),
        ("scene_strategy_on_back_sample", scene_strategy_on_back_sample),
        ("dialog_sample", dialog_sample),
        ("scene_override_entry_transitions_sample", scene_override_entry_transitions_sample),
        ("multiple_and_concatenated_back_stack_samples", multiple_and_concatenated_back_stack_samples),
        ("scene_nav_sample", scene_nav_sample),
        ("scene_nav_shared_samples", scene_nav_shared_samples),
        ("nav_back_stack_samples", nav_back_stack_samples),
    ];
    let filter = std::env::args().nth(1).filter(|a| !a.starts_with('-'));
    let mut failed = 0;
    for (name, test) in tests {
        if filter.as_ref().is_some_and(|f| !name.contains(f.as_str())) {
            continue;
        }
        match std::panic::catch_unwind(test) {
            Ok(()) => println!("test {name} ... ok"),
            Err(_) => {
                println!("test {name} ... FAILED");
                failed += 1;
            }
        }
    }
    println!("{} run, {failed} failed", tests.len());
    if failed > 0 {
        std::process::exit(1);
    }
}
