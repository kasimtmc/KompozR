//! Compose's own inline functions in a coroutine, in Kotlin's syntax: their lambdas suspend with it, as Kotlin's do —
//! `buildAnnotatedString` with `withStyle`/`withLink` (and, in the composition, the builder's receiver lambdas),
//! `PathData`, `ImageVector.Builder`'s `group`/`path` (through their builders: named arguments; the builder's name
//! Compose's default), `takeOrElse` (DpOffset's and DpSize's too), `TextFieldState.edit` (the state being edited
//! while its lambda is suspended), `MutatorMutex.tryMutate` (the lock held meanwhile). The lambdas really suspend: the
//! coroutine is seen part way. And in a `.kt` file (kompoz_file!): `withLink`, `takeOrElse`, `TextFieldState.edit`, a
//! `withStyle` whose lambda doesn't suspend left plain.

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
            withLink(LinkAnnotation.Url("https://kompoz.dev")) { append("c") }
        }
        text(plain)
        LaunchedEffect(go) {
            if (!go) return@LaunchedEffect
            val built = buildAnnotatedString {
                append("x")
                withStyle(SpanStyle(color = Color.Blue)) {
                    append("${later(1)}")
                    withLink(LinkAnnotation.Url("https://kompoz.dev")) {
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
            val offset = DpOffset.Unspecified.takeOrElse {
                delay(1)
                DpOffset(1.dp, 2.dp)
            }
            val size = DpSize(4.dp, 5.dp).takeOrElse { DpSize(later(6).dp, 6.dp) }
            log("${color == Color.Green} $width ${align == TextAlign.Center} ${kept == Color.Red}")
            log("${offset == DpOffset(1.dp, 2.dp)} ${size == DpSize(4.dp, 5.dp)}")
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

// A draw scope in a coroutine, as Kotlin holds it (a reference: Rust's `&mut DrawScope<'_>`, given on reborrowed): a
// parameter of it, `with(scope)`, extension functions of it plain and suspending, the transforms' suspending twins
// (`inset(h, v)`, `inset(x)`, `scale(x)` by Kotlin's names) — and the same drawn without suspending.
kompoz! {
    fun DrawScope.ring(color: Color) {
        drawCircle(color, radius = 3f)
    }

    suspend fun DrawScope.slowRing(color: Color) {
        yield()
        drawCircle(color, radius = 3f, center = Offset(5f, 5f))
    }

    fun corner(scope: DrawScope) {
        with(scope) { drawRect(Color.Red, size = Size(4f, 4f)) }
    }

    suspend fun paint(scope: DrawScope, log: (String) -> Unit) {
        val half = scope.size.width / 2
        corner(scope)
        with(scope) {
            translate(left = 2f) {
                yield()
                drawRect(Color.Red, size = Size(1f, 1f))
                slowRing(Color.Blue)
            }
            withTransform({ translate(1f, 1f) }) {
                yield()
                drawRect(Color.Blue)
            }
            inset(4f, 2f) {
                yield()
                ring(Color.Green)
            }
            inset(4f) { drawRect(Color.Red) }
            scale(2f) {
                yield()
                drawRect(Color.Green)
            }
            drawIntoCanvas {
                yield()
                it.drawRect(Rect(0f, 0f, 1f, 1f), Paint())
            }
        }
        val same = scope
        same.slowRing(Color.Red)
        log("painted $half")
    }

    fun paintAtOnce(scope: DrawScope) {
        corner(scope)
        with(scope) {
            translate(left = 2f) {
                drawRect(Color.Red, size = Size(1f, 1f))
                drawCircle(Color.Blue, radius = 3f, center = Offset(5f, 5f))
            }
            withTransform({ translate(1f, 1f) }) { drawRect(Color.Blue) }
            inset(horizontal = 4f, vertical = 2f) { ring(Color.Green) }
            inset(4f, 4f, 4f, 4f) { drawRect(Color.Red) }
            scale(2f, 2f) { drawRect(Color.Green) }
            drawIntoCanvas { it.drawRect(Rect(0f, 0f, 1f, 1f), Paint()) }
        }
        scope.drawCircle(Color.Red, radius = 3f, center = Offset(5f, 5f))
    }
}

/// A future polled to its end (what it awaits ready at once or at the next poll: `yield()`).
fn block_on<F: std::future::Future>(f: F) -> F::Output {
    let mut f = std::pin::pin!(f);
    let mut cx = std::task::Context::from_waker(std::task::Waker::noop());
    loop {
        if let std::task::Poll::Ready(v) = f.as_mut().poll(&mut cx) {
            return v;
        }
    }
}

/// What `draw` draws in a 100 x 80 scope over a picture.
fn picture_of(draw: impl FnOnce(&mut kompoz::ui::draw::DrawScope)) -> String {
    let mut p = kompoz::ui::draw::Picture::new();
    {
        let mut d =
            kompoz::ui::draw::DrawScope::new(&mut p, Size::new(100.0, 80.0), LayoutDirection::Ltr, Density::default());
        draw(&mut d);
    }
    format!("{p:?}")
}

fn a_draw_scope_in_a_coroutine() {
    let log: Rc<RefCell<Vec<String>>> = Rc::default();
    let l = log.clone();
    let suspending = picture_of(|d| block_on(paint(d, move |s: String| l.borrow_mut().push(s))));
    let at_once = picture_of(paint_at_once);
    assert_eq!(suspending, at_once);
    assert!(suspending.contains("PushTransform"), "{suspending}");
    assert_eq!(*log.borrow(), ["painted 50.0"]);
}

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
    // (the effect's delays, 291 ms of them: the clock taken past them, a delay not waited for)
    rule.advance_time_by(291);
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
        ImageVector::Builder(DefaultGroupName, 8.dp(), 8.dp(), 8.0, 8.0, Color::Unspecified, BlendMode::SrcIn, false);
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
    assert_eq!(*log.borrow(), ["true 3.0.dp true true", "true true"]);
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
    // (the rest of tryMutate's `delay(100)`: the clock taken past it, a delay not waited for)
    rule.advance_time_by(100);
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
    // (the effect's delays, 120 ms of them: the clock taken past them, a delay not waited for)
    rule.advance_time_by(120);
    assert_eq!(*log.borrow(), ["linked abc", "size 7.0", "typed 42"]);
    assert_eq!(state.text(), "42");
}

tests![
    builders_suspend_with_the_coroutine,
    edits_and_mutations_hold_on_while_suspended,
    compose_inline_functions_suspend_in_a_kt_file,
    a_draw_scope_in_a_coroutine,
];
