//! Whole sources: the Kotlin of the C++ Kompoz's kompozable tests (`tests/kotlin/*.kt`, taken from
//! `tests/kompozable_tests.cpp` and `kompozable_tests_other.cpp` with the C++ blanked out, so lines are the
//! originals') parsed without a diagnostic, every position in the tree right; the text `kompoz!` rebuilds from
//! tokens when it has no source text (rust-analyzer) parsed as the exact text is.

mod common;

use common::*;
use kompoz_kotlin::Pos;
use kompoz_kotlin::ast::*;

fn source(name: &str) -> String {
    let path = format!("{}/tests/kotlin/{name}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"))
}

/// The number after `key` at the start of `s`, and what follows it.
fn number_after<'a>(s: &'a str, key: &str) -> (usize, &'a str) {
    let s = s.strip_prefix(key).unwrap_or_else(|| panic!("{key} in {}", &s[..s.len().min(80)]));
    let digits = s.chars().take_while(char::is_ascii_digit).count();
    (s[..digits].parse().expect("a number"), &s[digits..])
}

/// Every position in `tree` (its `Debug`) agrees with `text`: its line and column are those of its offset, a
/// stretch's start is before its end, a name's stretch is the name as written. How many positions there were.
fn check_positions(text: &str, tree: &str) -> usize {
    // (each character's line and column, by its offset; the text's end too)
    let mut at = vec![None; text.len() + 1];
    let mut p = Pos::start(1, 1);
    for c in text.chars() {
        at[p.offset] = Some((p.line as usize, p.column as usize));
        p = p.advanced_over(c);
    }
    at[p.offset] = Some((p.line as usize, p.column as usize));
    let mut count = 0;
    for (i, _) in tree.match_indices("Pos { offset: ") {
        let (offset, rest) = number_after(&tree[i..], "Pos { offset: ");
        let (line, rest) = number_after(rest, ", line: ");
        let (column, _) = number_after(rest, ", column: ");
        assert_eq!(at[offset], Some((line, column)), "the position at offset {offset}");
        count += 1;
    }
    for (i, _) in tree.match_indices("Loc { start: Pos { offset: ") {
        let (start, rest) = number_after(&tree[i..], "Loc { start: Pos { offset: ");
        let rest = &rest[rest.find("end: Pos { offset: ").expect("an end")..];
        let (end, _) = number_after(rest, "end: Pos { offset: ");
        assert!(start <= end && text.is_char_boundary(start) && text.is_char_boundary(end), "{start}..{end}");
    }
    for (i, _) in tree.match_indices("Ident { name: \"") {
        let rest = &tree[i + "Ident { name: \"".len()..];
        let close = rest.find("\", backticked: ").expect("a name");
        let name = &rest[..close];
        let backticked = rest[close..].starts_with("\", backticked: true");
        let rest = &rest[rest.find("start: Pos { offset: ").expect("a start")..];
        let (start, rest) = number_after(rest, "start: Pos { offset: ");
        let rest = &rest[rest.find("end: Pos { offset: ").expect("an end")..];
        let (end, _) = number_after(rest, "end: Pos { offset: ");
        let written = &text[start..end];
        assert!(
            written == name || (backticked && written == format!("`{name}`")),
            "the name `{name}` at {start}..{end} is written `{written}`"
        );
    }
    count
}

#[test]
fn the_cpp_kompozable_tests_parse() {
    for name in ["kompozable_tests.kt", "kompozable_tests_other.kt"] {
        let text = source(name);
        let (file, d) = parse_text(&text);
        assert!(d.is_empty(), "{name}:\n{}", render(&d));
        assert!(!file.decls.iter().any(|d| matches!(d, Decl::Failed { .. })));
        // every `@Kompozable` at a line's start is a composable function's
        let kompozables = text.lines().filter(|l| l.starts_with("@Kompozable")).count();
        let functions = file.decls.iter().filter(|d| matches!(d, Decl::Function(f) if f.is_kompozable())).count();
        assert_eq!(functions, kompozables, "{name}");
        let positions = check_positions(&text, &format!("{file:?}"));
        assert!(positions > 100, "{name}: {positions} positions");
    }
}

#[test]
fn the_cpp_tests_declarations_in_their_kinds() {
    let text = source("kompozable_tests.kt");
    let file = parse_ok(&text);
    let find = |name: &str| {
        file.decls.iter().find(|d| d.name().is_some_and(|n| n.is(name))).unwrap_or_else(|| panic!("{name}"))
    };
    let class = |name: &str| match find(name) {
        Decl::Class(c) => c.clone(),
        other => panic!("{other:?}"),
    };
    assert!(class("Note").is_data());
    assert!(class("LoadState").is_sealed());
    assert_eq!(class("Figure").kind, ClassKind::Interface);
    assert_eq!(class("Check").kind, ClassKind::FunInterface);
    assert_eq!(class("Rank").kind, ClassKind::Enum);
    assert_eq!(
        class("Calc").enum_entries.iter().filter(|e| e.body.is_some()).count(),
        class("Calc").enum_entries.len()
    );
    assert!(class("Ledger").companion().is_some());
    assert!(matches!(find("Tally"), Decl::Object(_)));
    assert!(matches!(find("Cells"), Decl::TypeAlias(a) if a.type_params.len() == 1));
    assert!(matches!(find("greeting"), Decl::Property(p) if p.is_delegated()));
    assert!(matches!(find("shout"), Decl::Property(p) if p.receiver.is_some() && p.getter.is_some()));
    assert!(matches!(find("doubled"), Decl::Property(p) if p.is_var() && p.getter.is_some() && p.setter.is_some()));
    assert!(matches!(find("framed"), Decl::Function(f) if f.receiver.is_some()));
    assert!(matches!(find("bag"), Decl::Function(f) if f.params[0].is_vararg()));
    // the whole Counter, as written
    assert_eq!(
        decl(find("Counter")),
        "(fun @Kompozable Counter(start: Int = 0) {(var count by (call remember {(call mutableStateOf start)})); \
         (call Column {(call Text \"Count: $count\"); (call Button {(count ++)} {(call Text \"+\")})})})"
    );
}

/// Exact text and what `kompoz!` rebuilds from its tokens without their source text (WP0's text acquisition, its
/// output for these very inputs).
const REBUILT: &[(&str, &str)] = &[
    (
        "@Kompozable
fun Counter(start: Int = 0) {
    var count by remember { mutableStateOf(start) }
    Column {
        Text(\"Count: $count\")
        Button({ count++ }) { Text(\"+\") }
    }
}",
        "@Kompozable fun Counter(start: Int = 0) {var count by remember {mutableStateOf(start)}
Column {Text(\"Count: $count\")
Button({count ++}) {Text(\"+\")}}}",
    ),
    (
        "@Kompozable
fun Branches(n: Int) {
    Column {
        for (i in 0 until n) {
            Text(\"satır $i\")
        }
        for (i in 10 downTo 8 step 2) Text(\"geri $i\")
        if (n > 2) Text(\"çok\") else Text(\"az\")
        val word = when (n) {
            0 -> \"sıfır\"
            1, 2 -> \"biraz\"
            in 3..5 -> \"orta\"
            is Int -> \"sayı\"
            else -> \"bol\"
        }
        Text(word)
        Text(if (n % 2 == 0) \"çift\" else \"tek\")
    }
}",
        "@Kompozable fun Branches(n: Int) {Column {for(i in 0 until
n) {Text(\"satır $i\")}
for(i in 10 downTo
8 step
2)
Text(\"geri $i\")
if(n > 2)
Text(\"çok\") else Text(\"az\")
val word = when(n) {0 -> \"sıfır\"
1, 2 -> \"biraz\" in 3..5 -> \"orta\" is Int -> \"sayı\" else -> \"bol\"}
Text(word)
Text(if(n % 2 == 0)
\"çift\" else \"tek\")}}",
    ),
    (
        "data class Note(val id: Int, val title: String = \"\", val tags: List<String> = listOf())
sealed interface Shape
object Dot : Shape
enum class Level(val n: Int) {
    LOW(1), HIGH(9);
    fun next() = if (this == LOW) HIGH else LOW
}
class Counter(start: Int) : Base(start), Shape {
    var count = start
        private set
    val doubled: Int get() = count * 2
    init { count += 1 }
    companion object { const val MAX = 10 }
}",
        "data class Note(val id: Int, val title: String = \"\", val tags: List < String > = listOf())
sealed interface Shape
object Dot: Shape
enum class Level(val n: Int) {LOW(1), HIGH(9); fun next() = if(this == LOW)
HIGH else LOW}
class Counter(start: Int): Base(start), Shape {var count = start
private set val doubled: Int get() = count * 2
init {count += 1}
companion object {const val MAX = 10}}",
    ),
    (
        "fun Modifier.framed(color: Color): Modifier = this.border(1.dp, color).padding(4.dp)
fun <T> List<T>.second(): T = this[1]
fun report(items: List<String>?): String {
    val names = items?.filter { it.isNotEmpty() }?.map { name -> name.uppercase() } ?: emptyList()
    names.forEachIndexed { i, n -> println(\"$i: $n\") }
    val total = names.sumOf { it.length }.toFloat() / 2f
    return names.joinToString(\", \") + \" ${total}\" + \"${names.size}\"
}",
        "fun Modifier.framed(color: Color): Modifier = this.border(1.dp, color).padding(4.dp)
fun < T > List < T >.second(): T = this[1]
fun report(items: List < String >?): String {val names = items?.filter {it.isNotEmpty()}?.map {name -> name.uppercase()}?: emptyList()
names.forEachIndexed {i, n -> println(\"$i: $n\")}
val total = names.sumOf {it.length}.toFloat() / 2f
return names.joinToString(\", \") + \" ${total}\" + \"${names.size}\"}",
    ),
    (
        "fun control(x: Int?): Int {
    val y = x ?: return -1
    try {
        check(y > 0)
    } catch (e: IllegalStateException) {
        return 0
    } finally {
        log(\"done\")
    }
    var i = 0
    outer@ while (i < y) {
        i += 1
        if (i % 2 == 0) continue@outer
    }
    listOf(1, 2).forEach { if (it == 2) return@forEach }
    val f = fun(a: Int): Int { return a * 2 }
    val o = object : Runnable { override fun run() {} }
    val r = this::control
    return when {
        y > 10 && i != 0 -> 2
        else -> 1
    }
}",
        "fun control(x: Int?): Int {val y = x?: return - 1
try {check(y > 0)} catch(e: IllegalStateException) {return 0} finally {log(\"done\")}
var i = 0
outer@while(i < y) {i += 1
if(i % 2 == 0)
continue@outer}
listOf(1, 2).forEach {if(it == 2)
return@forEach}
val f = fun(a: Int): Int {return a * 2}
val o = object: Runnable {override fun run() {}}
val r = this:: control
return when {y > 10 && i!= 0 -> 2 else -> 1}}",
    ),
];

#[test]
fn rebuilt_text_parses_as_the_exact_text() {
    for (exact, rebuilt) in REBUILT {
        let a = parse_ok(exact);
        let b = parse_ok(rebuilt);
        assert_eq!(
            without_positions(&format!("{:?}", b.decls)),
            without_positions(&format!("{:?}", a.decls)),
            "\n{}\n---\n{}",
            b.decls.iter().map(decl).collect::<Vec<_>>().join("\n"),
            a.decls.iter().map(decl).collect::<Vec<_>>().join("\n"),
        );
        check_positions(rebuilt, &format!("{b:?}"));
    }
}

/// What the umbrella crate's Kotlin test (`crates/kompoz/tests/kotlin.rs`) gives `kompoz!` and `kompoz_file!`
/// translates without a diagnostic: its `.kt` file, its `kompoz!` block, the text rebuilt for a `macro_rules!`'s.
#[test]
fn the_umbrella_tests_kotlin_translates() {
    let path = format!("{}/../kompoz/tests/kt/empty.kt", env!("CARGO_MANIFEST_DIR"));
    let file = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
    let block = "    /** A composable that emits nothing. */\n    @Kompozable\n    fun Empty() {}\n\n    \
                 // (pasted Compose code: @Composable is @Kompozable)\n    @Composable private fun AlsoEmpty() {\n    }";
    let rebuilt = "@Kompozable fun EmptyToo() {}";
    for text in [file.as_str(), block, rebuilt] {
        let t = kompoz_kotlin::translate(
            text,
            Pos::start(1, 1),
            &kompoz_kotlin::OneSpan::default(),
            &kompoz_kotlin::Options::default(),
        );
        assert!(t.diagnostics.is_empty(), "{text}:\n{}", render(&t.diagnostics));
        assert!(!t.tokens.is_empty());
    }
}

#[test]
fn positions_with_turkish_and_emoji() {
    let text =
        "@Kompozable\nfun Selam(ad: String) {\n    Text(\"😀 ğüş $ad ${ad.length}\")\n    val çağrı = ad + \"İ\"\n}";
    let file = parse_ok(text);
    assert!(check_positions(text, &format!("{file:?}")) > 20);
    let Decl::Function(f) = &file.decls[0] else { panic!() };
    let Some(FunctionBody::Block(b)) = &f.body else { panic!() };
    let StmtKind::Expr(call) = &b.stmts[0].kind else { panic!() };
    let ExprKind::Call { args, .. } = &call.kind else { panic!() };
    let ExprKind::String(s) = &args[0].value.kind else { panic!() };
    let StringPart::Expr { expr, loc, .. } = &s.parts[1] else { panic!("{:?}", s.parts) };
    // `$ad` after an emoji and three Turkish letters: columns count characters
    assert_eq!((loc.start.line, loc.start.column, expr.loc.start.column), (3, 17, 18));
    let StmtKind::Decl(Decl::Property(p)) = &b.stmts[1].kind else { panic!() };
    assert_eq!(p.loc.text(text), "val çağrı = ad + \"İ\"");
    assert_eq!((p.loc.end.line, p.loc.end.column), (4, 25));
}
