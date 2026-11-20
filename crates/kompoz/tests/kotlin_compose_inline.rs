//! Compose's own inline functions in a coroutine, in Kotlin's syntax: their lambdas suspend with it, as Kotlin's do —
//! `buildAnnotatedString` with `withStyle`/`withLink` (and, in the composition, the builder's receiver lambdas),
//! `PathData`, `ImageVector.Builder`'s `group`/`path` (through their builders: named arguments), `takeOrElse`,
//! `TextFieldState.edit` (the state being edited while its lambda is suspended), `MutatorMutex.tryMutate` (the lock
//! held meanwhile). The lambdas really suspend: the coroutine is seen part way. And in a `.kt`
//! file (kompoz_file!): `withLink`, `takeOrElse`, `TextFieldState.edit`, a `withStyle` whose lambda doesn't suspend
//! left plain.

#![allow(non_snake_case, non_upper_case_globals)]

mod common;

use common::thrown;
use kompoz::prelude::*;
use kompoz::test::KompozTestRule;
use std::cell::RefCell;
use std::rc::Rc;

kompoz! {
    suspend fun later(n: Int): Int {
        delay(100)
        return n
    }

    @Composable
    fun Builders(
        go: Boolean,
        text: (AnnotatedString) -> Unit,
        nodes: (List<PathNode>) -> Unit,
        icon: (ImageVector) -> Unit,
        log: (String) -> Unit,
    ) {
        // (in the composition: withStyle's and withLink's lambdas are the builder's receiver lambdas)
        val plain = buildAnnotatedString {
            append("a")
            withStyle(SpanStyle(color = Color.Red)) { append("b") }
            withLink(LinkAnnotation.url("https://kompoz.dev")) { append("c") }
        }
        text(plain)
        LaunchedEffect(go) {
            if (!go) return@LaunchedEffect
            val built = buildAnnotatedString {
                append("x")
                withStyle(SpanStyle(color = Color.Blue)) {
                    append("${later(1)}")
                    withLink(LinkAnnotation.url("https://kompoz.dev")) {
                        delay(50)
                        append("y")
                    }
                }
                append("z")
            }
            text(built)
            nodes(PathData {
                moveTo(0f, 0f)
                delay(10)
                lineTo(4f, 4f)
                close()
            })
            icon(ImageVector.Builder(
                    name = "v",
                    defaultWidth = 8.dp,
                    defaultHeight = 8.dp,
                    viewportWidth = 8f,
                    viewportHeight = 8f,
                )
                .group(name = "g", rotate = 45f) {
                    delay(10)
                    path(fill = SolidColor(Color.Red)) {
                        moveTo(0f, 0f)
                        delay(10)
                        lineTo(8f, 8f)
                    }
                }
                .build())
            val color = Color.Unspecified.takeOrElse {
                delay(10)
                Color.Green
            }
            val width = Dp.Unspecified.takeOrElse { later(3).dp }
            val align = TextAlign.Unspecified.takeOrElse {
                delay(1)
                TextAlign.Center
            }
            val kept = Color.Red.takeOrElse { later(9).let { Color.Blue } }
            log("${color == Color.Green} $width ${align == TextAlign.Center} ${kept == Color.Red}")
        }
    }

    @Composable
    fun Editing(go: Boolean, state: TextFieldState, mutex: MutatorMutex, log: (String) -> Unit) {
        LaunchedEffect(go) {
            if (!go) return@LaunchedEffect
            state.edit {
                append("b")
                delay(100)
                append("c")
            }
            log("edited ${state.text}")
            val ran = mutex.tryMutate {
                log("locked")
                delay(100)
            }
            log("mutated $ran")
        }
    }
}

kompoz_file!("tests/kt/compose_inline.kt");

fn spans(s: &AnnotatedString) -> Vec<(usize, usize)> {
    s.span_styles().iter().map(|r| (r.start, r.end)).collect()
}

fn links(s: &AnnotatedString) -> Vec<(usize, usize)> {
    s.get_link_annotations(0, s.len()).iter().map(|r| (r.start, r.end)).collect()
}

fn builders_suspend_with_the_coroutine() {
    let log: Rc<RefCell<Vec<String>>> = Rc::default();
    let texts: Rc<RefCell<Vec<AnnotatedString>>> = Rc::default();
    let paths: Rc<RefCell<Vec<Vec<PathNode>>>> = Rc::default();
    let icons: Rc<RefCell<Vec<ImageVector>>> = Rc::default();
    let go = mutable_state_of(false);
    let (l, t, p, i, g) = (log.clone(), texts.clone(), paths.clone(), icons.clone(), go.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (l, t, p, i) = (l.clone(), t.clone(), p.clone(), i.clone());
        Builders(
            g.value(),
            move |s: AnnotatedString| t.borrow_mut().push(s),
            move |n: Vec<PathNode>| p.borrow_mut().push(n),
            move |v: ImageVector| i.borrow_mut().push(v),
            move |s: String| l.borrow_mut().push(s),
        );
    }));
    rule.wait_for_idle();
    // In the composition: the plain builder, its styles on what their lambdas appended.
    {
        let texts = texts.borrow();
        let plain = texts.last().expect("the plain text");
        assert_eq!(plain.text(), "abc");
        assert_eq!((spans(plain), links(plain)), (vec![(1, 2)], vec![(2, 3)]));
    }
    texts.borrow_mut().clear();
    go.set(true);
    rule.advance_time_by(130);
    // Part way through withLink's lambda (in withStyle's, in buildAnnotatedString's): nothing built yet.
    assert!(texts.borrow().iter().all(|t| t.text() == "abc"));
    rule.wait_for_idle();
    let built = texts.borrow().iter().find(|t| t.text() != "abc").cloned().expect("the built text");
    assert_eq!(built.text(), "x1yz");
    assert_eq!((spans(&built), links(&built)), (vec![(1, 3)], vec![(2, 3)]));
    // What the plain forms build.
    let mut b = AnnotatedStringBuilder::new();
    b.append("x");
    b.with_style(named_args!(SpanStyle(color = Color::Blue)), |b| {
        b.append("1");
        b.with_link(LinkAnnotation::url("https://kompoz.dev"), |b| b.append("y"));
    });
    b.append("z");
    assert_eq!(built, b.to_annotated_string());
    let expected_path = PathData(|p| {
        p.move_to(0.0, 0.0);
        p.line_to(4.0, 4.0);
        p.close();
    });
    assert_eq!(*paths.borrow(), [expected_path]);
    let mut expected_icon =
        ImageVector::Builder("v", 8.dp(), 8.dp(), 8.0, 8.0, Color::Unspecified, BlendMode::SrcIn, false);
    named_args!(expected_icon.group(
        name = "g",
        rotate = 45.0,
        block = |b| {
            named_args!(b.path(
                fill = SolidColor(Color::Red),
                path_builder = |p| {
                    p.move_to(0.0, 0.0);
                    p.line_to(8.0, 8.0);
                }
            ));
        }
    ));
    assert_eq!(*icons.borrow(), [expected_icon.build()]);
    assert_eq!(*log.borrow(), ["true 3.0.dp true true"]);
}

fn edits_and_mutations_hold_on_while_suspended() {
    let log: Rc<RefCell<Vec<String>>> = Rc::default();
    let go = mutable_state_of(false);
    let state = TextFieldState::new("a");
    let mutex = MutatorMutex::new();
    let (l, g, s, m) = (log.clone(), go.clone(), state.clone(), mutex.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let l = l.clone();
        Editing(g.value(), s.clone(), m.clone(), move |x: String| l.borrow_mut().push(x));
    }));
    rule.wait_for_idle();
    go.set(true);
    rule.advance_time_by(50);
    // Part way through edit's lambda: nothing committed, and the state is being edited (another edit can't start).
    assert_eq!(state.text(), "a");
    let nested = state.clone();
    let e = thrown(std::panic::AssertUnwindSafe(move || {
        nested.edit(|b| {
            b.append("!");
        })
    }))
    .expect("a nested edit");
    assert!(e.contains("concurrent or nested editing"), "{e}");
    rule.advance_time_by(100);
    assert_eq!(state.text(), "abc");
    // Part way through tryMutate's lambda: the lock held.
    assert_eq!(*log.borrow(), ["edited abc", "locked"]);
    assert!(!mutex.try_mutate(|| {}));
    rule.wait_for_idle();
    assert!(mutex.try_mutate(|| {}));
    assert_eq!(*log.borrow(), ["edited abc", "locked", "mutated true"]);
}

fn compose_inline_functions_suspend_in_a_kt_file() {
    let log: Rc<RefCell<Vec<String>>> = Rc::default();
    let go = mutable_state_of(false);
    let state = TextFieldState::new("");
    let (l, g, s) = (log.clone(), go.clone(), state.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let l = l.clone();
        Linked(g.value(), s.clone(), move |x: String| l.borrow_mut().push(x));
    }));
    rule.wait_for_idle();
    go.set(true);
    rule.advance_time_by(60);
    assert!(log.borrow().is_empty());
    rule.wait_for_idle();
    assert_eq!(*log.borrow(), ["linked abc", "size 7.0", "typed 42"]);
    assert_eq!(state.text(), "42");
}

tests![
    builders_suspend_with_the_coroutine,
    edits_and_mutations_hold_on_while_suspended,
    compose_inline_functions_suspend_in_a_kt_file,
];
