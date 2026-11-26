//! The generator's own diagnostics: each at its Kotlin line:column (the text's base position counted in), one
//! declaration's errors leaving the others translated.

use kompoz_kotlin::{OneSpan, Options, Pos, translate};

/// The diagnostics of `text` (starting at `line`:1), as `line:column: message`, and the Rust it gave.
fn diagnose_at(text: &str, line: u32) -> (Vec<String>, String) {
    let t = translate(text, Pos::start(line, 1), &OneSpan::default(), &Options::default());
    let d = t.diagnostics.iter().map(|d| format!("{}: {}", d.loc.start, d.message)).collect();
    (d, t.tokens.to_string())
}

fn diagnose(text: &str) -> Vec<String> {
    diagnose_at(text, 1).0
}

const LATER: &str = "isn't translated by KompozR's Kotlin syntax yet (it comes with stage 14)";

#[test]
fn stage_14_constructs_at_their_positions() {
    let text = "\
sealed class A
fun f(x: Int): Int {
    try { return 1 } catch (e: Exception) { return 2 }
}
fun g(x: String): Boolean = x is String
fun h() = listOf(1).map(::println)
";
    assert_eq!(
        diagnose(text),
        [
            format!("1:1: `sealed` on a class {LATER}"),
            format!("3:5: `try` {LATER}"),
            "6:5: write `h`'s return type: its expression's type isn't one KompozR can tell (`fun h(…): Type = …`)"
                .to_string(),
            format!("6:25: a callable reference (`::name`) {LATER}"),
        ]
    );
}

#[test]
fn positions_count_from_the_base_and_in_characters() {
    // (the text starts at line 40 of its file; `ğ` and `ü` are one column each)
    let (d, _) = diagnose_at("fun ğü(x: Int) {\n    val y = 1\n    y = 2\n}", 40);
    assert_eq!(d, ["42:5: `y` is a `val`: it can't be assigned"]);
}

#[test]
fn errors_leave_the_other_declarations_translated() {
    let (d, rust) = diagnose_at("fun bad() = undefinedThing(1)\nfun good(): Int = 2\n", 1);
    assert_eq!(
        d,
        ["1:5: write `bad`'s return type: its expression's type isn't one KompozR can tell (`fun bad(…): Type = …`)"]
    );
    assert!(rust.contains("fn good"), "{rust}");
    assert!(!rust.contains("fn bad"), "{rust}");
}

#[test]
fn calls_named_wrongly_or_out_of_place() {
    let text = "\
@Kompozable
fun Labeled(label: String, value: Int = 7) {}
@Kompozable
fun Uses() {
    Labeled(label = \"a\", colour = 3)
    Labeled()
}
fun later() {
    delay(10)
}
";
    assert_eq!(
        diagnose(text),
        [
            "5:5: `Labeled`: no parameter `colour`",
            "6:5: `Labeled` needs `label`",
            "9:5: `delay` suspends: call it from a coroutine (`LaunchedEffect`, `launch`, a `suspend fun`)",
        ]
    );
}

#[test]
fn jumps_and_receivers_where_there_are_none() {
    let text = "\
fun f(xs: List<Int>) {
    xs.forEach { if (it > 1) return }
    break
    for (x in xs) { continue@outer }
    println(this)
}
";
    assert_eq!(
        diagnose(text),
        [
            format!("2:30: a `return` from inside a lambda (Kotlin's non-local return) {LATER}"),
            format!("3:5: `break`/`continue` outside a loop (or out of a lambda) {LATER}"),
            "4:21: no loop labeled `outer` around it".to_string(),
            "5:13: `this` here has nothing to be (no extension function or receiver lambda around it)".to_string(),
        ]
    );
}

#[test]
fn top_level_vals_have_their_values_types_or_written_ones() {
    assert_eq!(
        diagnose(
            "val answer = 42\nvar count: Int = 0\nval ok: Int = 1\nval vague = unknownThing()\nval twice = answer * 2"
        ),
        [
            format!("2:1: a top-level `var` {LATER}"),
            "4:5: write `vague`'s type: KompozR can't tell its value's type (`val vague: Type = …`)".to_string(),
        ]
    );
}

#[test]
fn names_rust_cant_have() {
    let (d, _) = diagnose_at("fun `two words`() {}\nfun `self`() {}", 1);
    assert_eq!(d, ["1:5: `two words` can't be a name in Rust", "2:5: `self` can't be a name in Rust"]);
}

#[test]
fn a_method_of_several_types_on_a_value_of_a_type_kompozr_cant_tell_asks_for_the_type() {
    // (`animateTo` is Animatable's, AnchoredDraggableState's, AnimationState's, DeferredTransitionState's and
    // SeekableTransitionState's: which one Kotlin calls its type tells, never a guess by the arguments)
    let text = "\
@Composable
fun Animated() {
    val x = madeElsewhere()
    LaunchedEffect(Unit) {
        x.animateTo(1f)
        madeElsewhere().animateTo(1f)
        listOf(madeElsewhere()).forEach { it.animateTo(1f) }
        listOf(madeElsewhere()).forEach { a -> a.snapTo(1f) }
        madeElsewhere().forEach { }
    }
}
";
    let owners = "(`AnchoredDraggableState`, `Animatable`, `AnimationState`, `DeferredTransitionState` and \
                  `SeekableTransitionState` have one)";
    // (WP5: pull to refresh — PullToRefreshState's `snapTo(targetValue: Float)` too; (WP7: search/tooltips)
    // SearchBarState's `snapTo(fraction: Float)` too)
    let snap_owners = "(`AnchoredDraggableState`, `Animatable`, `PullToRefreshState`, `SearchBarState` and \
                       `SeekableTransitionState` have one)";
    assert_eq!(
        diagnose(text),
        [
            format!(
                "5:9: write `x`'s type: KompozR can't tell which `animateTo` it calls {owners} (`val x: Type = …`)"
            ),
            format!(
                "6:9: write the type of the value `animateTo` is called on: KompozR can't tell which `animateTo` it \
                 calls {owners} (a `val x: Type = …` of it, then `x.animateTo(…)`)"
            ),
            format!(
                "7:43: name the lambda's parameter and write its type: KompozR can't tell which `animateTo` it calls \
                 {owners} (`{{ x: Type -> x.animateTo(…) }}`)"
            ),
            format!(
                "8:48: write `a`'s type: KompozR can't tell which `snapTo` it calls {snap_owners} (`{{ a: Type -> … }}`)"
            ),
            // (a list's, a map's — Kotlin's standard types, the kt shims' —, DraggableAnchors' or the adaptive pane
            // scaffolds' orders' and motion data's)
            "9:9: write the type of the value `forEach` is called on: KompozR can't tell which `forEach` it calls \
             (Kotlin's standard types, `DraggableAnchors`, `PaneScaffoldHorizontalOrder` and \
             `PaneScaffoldMotionDataProviderExt` have one) (a `val x: Type = …` of it, then \
             `x.forEach(…)`)"
                .to_string(),
        ]
    );
}

#[test]
fn a_method_on_a_value_of_a_type_kompozr_tells_resolves_by_it() {
    // (its type written, given by `remember { … }`'s block — its own locals too —, a delegate's, a parameter's, a
    // lambda's declared function type, a receiver lambda's (`with`, `run`), a call's result: no error; a method only
    // one type has, on a value of a type it can't tell, is that type's)
    let text = "\
@Composable
fun Animated(seekable: SeekableTransitionState<Int>, deferred: DeferredTransitionState<Int>) {
    val written: Animatable<Float, AnimationVector1D> = madeElsewhere()
    val remembered = remember { Animatable(0f) }
    val built = remember { val a = Animatable(0f); a }
    var delegated by remember { mutableStateOf(Animatable(0f)) }
    val unknown = madeElsewhere()
    LaunchedEffect(Unit) {
        written.animateTo(1f)
        remembered.animateTo(1f)
        built.animateTo(1f)
        delegated.animateTo(1f)
        seekable.animateTo(2)
        deferred.animateTo(3)
        with(remembered) { animateTo(2f) }
        remembered.run { animateTo(3f) }
        remember { Animatable(0f) }.animateTo(4f)
        unknown.updateBounds(upperBound = 1f)
        val stop: suspend (Animatable<Float, AnimationVector1D>) -> Unit = { it.stop() }
    }
}
";
    assert_eq!(diagnose(text), Vec::<String>::new());
}
