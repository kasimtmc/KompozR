//! Expressions and statements: Kotlin's precedence, where a newline ends an expression and where it goes on,
//! calls (named and spread arguments, trailing lambdas, type arguments), lambdas, `if`/`when`/`try`, jumps and
//! labels, object literals, anonymous functions, callable references, templates, loops, assignments, local
//! declarations.

mod common;

use common::*;

#[test]
fn precedence() {
    let cases = [
        ("a || b && c", "(|| a (&& b c))"),
        ("a && b || c", "(|| (&& a b) c)"),
        ("a == b && c != d", "(&& (== a b) (!= c d))"),
        ("a === b || a !== c", "(|| (=== a b) (!== a c))"),
        ("a < b == c >= d", "(== (< a b) (>= c d))"),
        ("x in a..b == y !in c", "(== (in x (.. a b)) (!in y c))"),
        ("x is String && y !is Int?", "(&& (is x String) (!is y Int?))"),
        ("a ?: b ?: c", "(?: (?: a b) c)"),
        ("a < b ?: c", "(< a (?: b c))"),
        ("a ?: b until c", "(?: a (until b c))"),
        ("0 until n + 1", "(until 0 (+ n 1))"),
        ("10 downTo 0 step 2", "(step (downTo 10 0) 2)"),
        ("a to b to c", "(to (to a b) c)"),
        ("a..b + 1", "(.. a (+ b 1))"),
        ("0..<n", "(..< 0 n)"),
        ("a + b * c - d / e % f", "(- (+ a (* b c)) (% (/ d e) f))"),
        ("a * b as Float", "(* a (as b Float))"),
        ("-x as Int", "(as (- x) Int)"),
        ("x as? String ?: \"\"", "(?: (as? x String) \"\")"),
        ("!a.b()", "(! (call (. a b)))"),
        ("-a.b!!", "(- ((. a b) !!))"),
        ("!!done", "(! (! done))"),
        ("x++ + ++y", "(+ (x ++) (++ y))"),
        ("a?.b?.c ?: d", "(?: (?. (?. a b) c) d)"),
        ("a!!.b[0]?.c", "(?. ([] (. (a !!) b) 0) c)"),
        ("(a + b) * c", "(* (paren (+ a b)) c)"),
        ("m[i, j]", "([] m i j)"),
        ("-1", "(- 1)"),
        ("+x", "(+ x)"),
        ("--i", "(-- i)"),
        ("i--", "(i --)"),
    ];
    for (text, tree) in cases {
        assert_eq!(expr_of(text), tree, "{text}");
    }
}

#[test]
fn literals() {
    assert_eq!(expr_of("16"), "16");
    assert_eq!(expr_of("0xFF_00L"), "0xFF_00L");
    assert_eq!(expr_of("1.5f"), "1.5f");
    assert_eq!(expr_of("'ç'"), "'ç'");
    assert_eq!(expr_of("true && false"), "(&& true false)");
    assert_eq!(expr_of("null"), "null");
    assert_eq!(expr_of("\"a\\tb\""), "\"a\\tb\"");
    assert_eq!(expr_of("\"\"\"raw $x\"\"\""), "\"\"\"raw $x\"\"\"");
}

#[test]
fn templates_parse_their_expressions() {
    assert_eq!(expr_of("\"$a and ${b + 1}\""), "\"$a and ${(+ b 1)}\"");
    assert_eq!(expr_of("\"$this ${this@Outer.x}\""), "\"$this ${(. this@Outer x)}\"");
    assert_eq!(expr_of("\"${if (a) \"x\" else \"y\"}\""), "\"${(if a \"x\" else \"y\")}\"");
    assert_eq!(
        expr_of("\"${map[\"k\"]} ${list.map { it * 2 }}\""),
        "\"${([] map \"k\")} ${(call (. list map) {(* it 2)})}\""
    );
    // nested three deep
    assert_eq!(expr_of("\"a${\"b${\"c$d\"}\"}\""), "\"a${\"b${\"c$d\"}\"}\"");
    assert_eq!(expr_of("\"$`a b`\""), "\"$`a b`\"");
}

#[test]
fn template_positions() {
    let f = parse_ok("val s = \"ğ $x ${y.z}\"");
    let kompoz_kotlin::ast::Decl::Property(p) = &f.decls[0] else { panic!() };
    let kompoz_kotlin::ast::PropertyInit::Value(v) = &p.init else { panic!() };
    let kompoz_kotlin::ast::ExprKind::String(s) = &v.kind else { panic!() };
    use kompoz_kotlin::ast::StringPart;
    let cols: Vec<(u32, u32)> = s
        .parts
        .iter()
        .map(|p| match p {
            StringPart::Text { loc, .. } | StringPart::Expr { loc, .. } => (loc.start.column, loc.end.column),
        })
        .collect();
    assert_eq!(cols, [(10, 12), (12, 14), (14, 15), (15, 21)]);
    let StringPart::Expr { expr, .. } = &s.parts[3] else { panic!() };
    assert_eq!((expr.loc.start.column, expr.loc.end.column), (17, 20));
}

#[test]
fn newlines_end_or_continue() {
    // `.` and `?.` on the next line go on
    assert_eq!(stmts("a\n  .b()\n  ?.c"), "(?. (call (. a b)) c)");
    // so do `&&`, `||`, `?:`, `as` there, and any operator at a line's end
    assert_eq!(stmts("a\n&& b\n|| c"), "(|| (&& a b) c)");
    assert_eq!(stmts("a\n?: b"), "(?: a b)");
    assert_eq!(stmts("a\nas B"), "(as a B)");
    assert_eq!(stmts("a +\nb *\nc"), "(+ a (* b c))");
    assert_eq!(stmts("a ==\nb"), "(== a b)");
    assert_eq!(stmts("val x =\n  1"), "(val x = 1)");
    // other operators on the next line start a statement of their own
    assert_eq!(stmts("a\n+ b"), "a\n(+ b)");
    assert_eq!(stmts("a\n- b"), "a\n(- b)");
    assert_eq!(stmts("a\n!b"), "a\n(! b)");
    assert_eq!(stmts("a\n++b"), "a\n(++ b)");
    // a call's `(`, an index's `[`, a trailing lambda's `{` must be on the line
    assert_eq!(stmts("f\n(x)"), "f\n(paren x)");
    assert_eq!(stmts("f()\n{ x }"), "(call f)\n{x}");
    assert_eq!(stmts("Column\n{ }"), "Column\n{}");
    // an infix call, `in` and `is` too
    assert_eq!(stmts("a\nto(b)"), "a\n(call to b)");
    // inside parentheses and brackets newlines don't count
    assert_eq!(stmts("f(a\n+ b,\nc\n(d))"), "(call f (+ a b) (call c d))");
    assert_eq!(stmts("m[a\n- 1]"), "([] m (- a 1))");
    // and a lambda's braces inside them count again
    assert_eq!(stmts("f({ a\n+ b })"), "(call f {a; (+ b)})");
    // `else`, `catch`, `finally` across newlines and a `;`
    assert_eq!(stmts("if (a) b\n\nelse c"), "(if a b else c)");
    assert_eq!(stmts("if (a) b; else c"), "(if a b else c)");
    assert_eq!(stmts("try {\n} \ncatch (e: E) {\n}\nfinally {\n}"), "(try {} (catch e: E {}) (finally {}))");
    // `return`'s value is on its line
    assert_eq!(stmts("return\nx"), "(return)\nx");
    assert_eq!(stmts("return x"), "(return x)");
    // two statements on a line need a `;`
    assert_eq!(stmts("a; b ;c"), "a\nb\nc");
}

#[test]
fn calls_and_arguments() {
    assert_eq!(expr_of("f()"), "(call f)");
    assert_eq!(expr_of("f (1)"), "(call f 1)");
    assert_eq!(
        expr_of("Text(\"a\", modifier = Modifier.padding(16.dp),)"),
        "(call Text \"a\" modifier=(call (. Modifier padding) (. 16 dp)))"
    );
    assert_eq!(expr_of("listOf(*xs, `in` = 2)"), "(call listOf *xs `in`=2)");
    assert_eq!(
        expr_of("Button(onClick = { n++ }) { Text(\"+\") }"),
        "(call Button onClick={(n ++)} {(call Text \"+\")})"
    );
    assert_eq!(expr_of("Column { }"), "(call Column {})");
    assert_eq!(expr_of("a.b(1) { }.c { }"), "(call (. (call (. a b) 1 {}) c) {})");
    assert_eq!(expr_of("f()()"), "(call (call f))");
    assert_eq!(expr_of("{ x: Int -> x }(1)"), "(call {x: Int -> x} 1)");
    assert_eq!(expr_of("xs.forEach lit@{ return@lit }"), "(call (. xs forEach) lit@{(return@lit)})");
}

#[test]
fn type_arguments_or_comparisons() {
    assert_eq!(expr_of("listOf<Int>()"), "(call listOf<Int>)");
    assert_eq!(expr_of("mapOf<String, List<Int>>(a to b)"), "(call mapOf<String, List<Int>> (to a b))");
    assert_eq!(expr_of("remember<MutableState<Int>> { s }"), "(call remember<MutableState<Int>> {s})");
    assert_eq!(expr_of("a.emptyList<T?>()"), "(call (. a emptyList)<T?>)");
    assert_eq!(expr_of("viewModel<VM>()"), "(call viewModel<VM>)");
    assert_eq!(expr_of("a < b"), "(< a b)");
    assert_eq!(expr_of("a < b && c > d"), "(&& (< a b) (> c d))");
    assert_eq!(expr_of("f(a < b, c > d)"), "(call f (< a b) (> c d))");
    assert_eq!(expr_of("a < b > c"), "(> (< a b) c)");
    assert_eq!(expr_of("i < n - 1"), "(< i (- n 1))");
}

#[test]
fn lambdas() {
    assert_eq!(expr_of("{ }"), "{}");
    assert_eq!(expr_of("{ it + 1 }"), "{(+ it 1)}");
    assert_eq!(expr_of("{ -> 1 }"), "{-> 1}");
    assert_eq!(expr_of("{ a, b -> a + b }"), "{a, b -> (+ a b)}");
    assert_eq!(expr_of("{ a: Int, b: String? -> }"), "{a: Int, b: String? -> }");
    assert_eq!(expr_of("{ (k, v), _ -> k }"), "{(k, v), _ -> k}");
    assert_eq!(expr_of("{ (k: String, _), i -> }"), "{(k: String, _), i -> }");
    assert_eq!(expr_of("{ a, -> a }"), "{a -> a}");
    assert_eq!(expr_of("{\n  a ->\n  val b = a\n  b\n}"), "{a -> (val b = a); b}");
    assert_eq!(expr_of("lbl@ { return@lbl 1 }"), "lbl@{(return@lbl 1)}");
    assert_eq!(expr_of("{ a }"), "{a}");
    assert_eq!(expr_of("{ a.b }"), "{(. a b)}");
    assert_eq!(expr_of("@Composable { Text(\"a\") }"), "(@Composable {(call Text \"a\")})");
}

#[test]
fn ifs() {
    assert_eq!(expr_of("if (a) 1 else 2"), "(if a 1 else 2)");
    assert_eq!(expr_of("if (a) { 1 } else if (b) 2 else { 3 }"), "(if a {1} else (if b 2 else {3}))");
    assert_eq!(stmts("if (a) x = 1 else y += 2"), "(if a (= x 1) else (+= y 2))");
    assert_eq!(stmts("if (a) ;"), "(if a [])");
    assert_eq!(stmts("if (a) else b"), "(if a [] else b)");
    assert_eq!(stmts("if (a)\n  b\nelse\n  c"), "(if a b else c)");
    assert_eq!(expr_of("if (x)\n{ 1 }\nelse\n{ 2 }"), "(if x {1} else {2})");
}

#[test]
fn whens() {
    assert_eq!(
        expr_of(
            "when (n) {\n 0 -> \"a\"\n 1, 2, -> \"b\"\n in 3..5 -> \"c\"\n !in xs -> { d }\n is Int -> e\n !is String? -> f\n else -> g\n}"
        ),
        "(when (n) (0 -> \"a\") (1, 2 -> \"b\") (in (.. 3 5) -> \"c\") (!in xs -> {d}) (is Int -> e) (!is String? -> f) (else -> g))"
    );
    assert_eq!(expr_of("when {\n a > 0 -> 1\n else -> 2\n}"), "(when ((> a 0) -> 1) (else -> 2))");
    assert_eq!(expr_of("when (val r = f()) { is Ok -> r }"), "(when (val r = (call f)) (is Ok -> r))");
    assert_eq!(expr_of("when (val r: R = f()) { else -> r }"), "(when (val r: R = (call f)) (else -> r))");
    // Kotlin 2.1's guards
    assert_eq!(
        expr_of("when (s) {\n is Loaded if s.data.isEmpty() -> 1\n else if x -> 2\n else -> 3 }"),
        "(when (s) (is Loaded if (call (. (. s data) isEmpty)) -> 1) (else if x -> 2) (else -> 3))"
    );
    // branches need no separator; a branch's body ends where the next branch's conditions start
    assert_eq!(
        expr_of("when (n) { 0 -> \"a\" 1, 2 -> \"b\" in 3..5 -> \"c\" !is Int -> \"d\" else -> \"e\" }"),
        "(when (n) (0 -> \"a\") (1, 2 -> \"b\") (in (.. 3 5) -> \"c\") (!is Int -> \"d\") (else -> \"e\"))"
    );
    assert_eq!(expr_of("when (x) { 1 -> a in b else -> c }"), "(when (x) (1 -> (in a b)) (else -> c))");
    // an `if` in a branch doesn't take the next branch's `else ->`
    assert_eq!(expr_of("when (x) {\n 1 -> if (a) b\n else -> c\n}"), "(when (x) (1 -> (if a b)) (else -> c))");
    assert_eq!(
        expr_of("when (x) {\n 1 -> if (a) b else d\n else -> c\n}"),
        "(when (x) (1 -> (if a b else d)) (else -> c))"
    );
    assert_eq!(stmts("when (x) {\n 1 -> y = 2\n else -> {}\n}"), "(when (x) (1 -> (= y 2)) (else -> {}))");
}

#[test]
fn tries_jumps_and_labels() {
    assert_eq!(
        expr_of("try { a } catch (e: E) { b } catch (e: F) { c }"),
        "(try {a} (catch e: E {b}) (catch e: F {c}))"
    );
    assert_eq!(expr_of("try { a } finally { b }"), "(try {a} (finally {b}))");
    assert_eq!(expr_of("x ?: return"), "(?: x (return))");
    assert_eq!(
        expr_of("x ?: throw IllegalStateException(\"no\")"),
        "(?: x (throw (call IllegalStateException \"no\")))"
    );
    assert_eq!(
        stmts("outer@ for (i in xs) {\n for (j in ys) {\n  if (j) break@outer else continue\n }\n}"),
        "(for@outer i in xs {(for j in ys {(if j (break@outer) else (continue))})})"
    );
    assert_eq!(stmts("loop@while (true) { continue@loop }"), "(while@loop true {(continue@loop)})");
    assert_eq!(stmts("return@forEach"), "(return@forEach)");
    assert_eq!(
        expr_of("this@Outer.x + super.y + super<Base>@Outer.z"),
        "(+ (+ (. this@Outer x) (. super y)) (. super<Base>@Outer z))"
    );
}

#[test]
fn callable_references() {
    assert_eq!(expr_of("::foo"), "(:: foo)");
    assert_eq!(expr_of("String::length"), "(:: String length)");
    assert_eq!(expr_of("Foo::class.java"), "(. (:: Foo class) java)");
    assert_eq!(expr_of("a.b::c"), "(:: (. a b) c)");
    assert_eq!(expr_of("this::onClick"), "(:: this onClick)");
    assert_eq!(expr_of("xs.map(Item::name)"), "(call (. xs map) (:: Item name))");
}

#[test]
fn objects_and_anonymous_functions() {
    assert_eq!(
        expr_of("object : NestedScrollConnection {\n override fun onPreScroll(a: Offset): Offset = a\n}"),
        "(object : NestedScrollConnection {(fun override onPreScroll(a: Offset): Offset = a)})"
    );
    assert_eq!(expr_of("object : Base(1), I by d { }"), "(object : Base(1), I by d {})");
    assert_eq!(expr_of("object { val x = 1 }"), "(object {(val x = 1)})");
    assert_eq!(expr_of("fun(x: Int): Int { return x }"), "(fun (x: Int): Int {(return x)})");
    assert_eq!(expr_of("fun(x) = x"), "(fun (x) = x)");
    assert_eq!(expr_of("fun Int.(y: Int) = this + y"), "(fun Int.(y: Int) = (+ this y))");
    assert_eq!(expr_of("suspend fun() {}"), "(fun suspend () {})");
}

#[test]
fn statements_and_loops() {
    assert_eq!(
        stmts("x = 1\nx += 2\nx -= 3\nx *= 4\nx /= 5\nx %= 6"),
        "(= x 1)\n(+= x 2)\n(-= x 3)\n(*= x 4)\n(/= x 5)\n(%= x 6)"
    );
    assert_eq!(stmts("a.b[i] = c"), "(= ([] (. a b) i) c)");
    assert_eq!(stmts("for (i in 0 until n) Text(\"$i\")"), "(for i in (until 0 n) (call Text \"$i\"))");
    assert_eq!(stmts("for ((i, x) in xs.withIndex()) { }"), "(for (i, x) in (call (. xs withIndex)) {})");
    assert_eq!(stmts("for (x: Int in xs) ;"), "(for x: Int in xs [])");
    assert_eq!(stmts("while (x < 3)\n  x++"), "(while (< x 3) (x ++))");
    assert_eq!(stmts("do {\n x++\n} while (x < 3)"), "(do {(x ++)} while (< x 3))");
    assert_eq!(stmts("do x++\nwhile (x < 3)"), "(do (x ++) while (< x 3))");
    assert_eq!(stmts("count++"), "(count ++)");
}

#[test]
fn local_declarations() {
    assert_eq!(stmts("val x = 1\nvar y: Int? = null"), "(val x = 1)\n(var y: Int? = null)");
    assert_eq!(stmts("val (a, _, c: Int) = triple"), "(val (a, _, c: Int) = triple)");
    assert_eq!(
        stmts("var count by remember { mutableStateOf(0) }"),
        "(var count by (call remember {(call mutableStateOf 0)}))"
    );
    assert_eq!(stmts("fun helper(a: Int) = a * 2"), "(fun helper(a: Int) = (* a 2))");
    assert_eq!(stmts("class Local(val x: Int)"), "(class Local(val x: Int))");
    assert_eq!(stmts("data class P(val a: Int)"), "(class data P(val a: Int))");
    assert_eq!(stmts("object Once { }"), "(object Once {})");
    assert_eq!(stmts("@Suppress(\"UNUSED\") val z = 3"), "(val @Suppress(\"UNUSED\") z = 3)");
    assert_eq!(stmts("typealias L = List<Int>"), "(typealias L = List<Int>)");
    // soft keywords are names in a statement
    assert_eq!(
        stmts("value = 3\nopen(file)\ndata.size\ninline + 1\nfield = it"),
        "(= value 3)\n(call open file)\n(. data size)\n(+ inline 1)\n(= field it)"
    );
    // annotations at a statement's start are the whole statement's
    assert_eq!(stmts("@Suppress(\"x\") a + b"), "(@Suppress(\"x\") (+ a b))");
}
