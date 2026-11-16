//! Mistakes: each a diagnostic at its exact line and column, the declaration (or statement) it is in left as
//! `Decl::Failed` (or an error statement) with its name when read, and everything else still parsed.

mod common;

use common::*;
use kompoz_kotlin::ast::*;

/// The declarations written out and the diagnostics.
fn recovered(text: &str) -> (String, String) {
    let (f, d) = parse_text(text);
    (f.decls.iter().map(decl).collect::<Vec<_>>().join("\n"), render(&d))
}

#[test]
fn broken_declarations_and_the_good_ones_between() {
    let text = "@Kompozable
fun Good1() {
    Text(\"a\")
}
fun broken(x: ) {}
val = 3
class C(val a Int)
@Kompozable
fun Good2() { Text(\"b\") }
fun alsoBroken( {
}
object
fun Good3() = 1";
    let (decls, d) = recovered(text);
    assert_eq!(
        d,
        "5:15: expected a type, found `)`\n\
         6:5: expected a name, found `=`\n\
         7:15: expected `:` and the parameter's type, found `Int`\n\
         10:17: expected a parameter's name, found `{`\n\
         13:1: expected the object's name, found keyword `fun`"
    );
    assert_eq!(
        decls,
        "(fun @Kompozable Good1() {(call Text \"a\")})\n<failed broken>\n<failed >\n<failed C>\n\
         (fun @Kompozable Good2() {(call Text \"b\")})\n<failed alsoBroken>\n<failed >\n(fun Good3() = 1)"
    );
    // a failed declaration covers what was skipped
    let (f, _) = parse_text(text);
    let Decl::Failed { loc, .. } = &f.decls[1] else { panic!() };
    assert_eq!(loc.text(text), "fun broken(x: ) {}");
    let Decl::Failed { loc, .. } = &f.decls[5] else { panic!() };
    assert_eq!(loc.text(text), "fun alsoBroken( {\n}");
}

#[test]
fn broken_statements_in_a_body() {
    let text = "fun f() {
    val a = 1 +
    val b = (2
    val c = 3
    Text(\"x\") \"y\"
    foo(1, , 2)
    bar()
}
fun g() {}";
    let (decls, d) = recovered(text);
    assert_eq!(
        d,
        "3:5: expected an expression, found keyword `val`\n\
         4:5: expected `)`, found keyword `val`\n\
         5:15: expected a new line or `;` after the statement, found a string\n\
         6:12: expected an expression, found `,`"
    );
    assert_eq!(
        decls,
        "(fun f() {<failed a>; <failed b>; (val c = 3); (call Text \"x\"); <error>; (call bar)})\n(fun g() {})"
    );
}

#[test]
fn broken_members_of_a_class() {
    let (decls, d) = recovered(
        "class K {
    val ok = 1
    fun bad(: Int) {}
    fun good() = 2
    val =
}
val after = 3",
    );
    assert_eq!(d, "3:13: expected a parameter's name, found `:`\n5:9: expected a name, found `=`");
    assert_eq!(decls, "(class K {(val ok = 1); <failed bad>; (fun good() = 2); <failed >})\n(val after = 3)");
}

#[test]
fn a_parenthesis_left_open_doesnt_hide_the_next_declaration() {
    let (decls, d) = recovered("fun a(x: Int\nfun b() {}\nclass C(\nval d = 1");
    assert_eq!(d, "2:1: expected `)`, found keyword `fun`\n4:7: expected `:` and the parameter's type, found `=`");
    assert_eq!(decls, "<failed a>\nfun b() {})\n<failed C>".replace("fun b", "(fun b"));
}

#[test]
fn a_brace_left_open_at_the_end() {
    let (decls, d) = recovered("fun a() {\n    Text(\"x\")");
    assert_eq!(d, "2:14: expected `}`, found the end");
    assert_eq!(decls, "<failed a>");
}

#[test]
fn a_statement_in_a_lambda_in_arguments() {
    let (decls, d) =
        recovered("fun f() {\n    Column(Modifier.x) {\n        Text(1 +)\n        Text(\"ok\")\n    }\n}");
    assert_eq!(d, "3:17: expected an expression, found `)`");
    assert_eq!(decls, "(fun f() {(call Column (. Modifier x) {<error>; (call Text \"ok\")})})");
}

#[test]
fn lexical_mistakes_and_templates() {
    let (decls, d) =
        recovered("fun a() {\n    val s = \"open\n    val t = 1\n    val u = \"${1 +}\"\n    val v = \"$if\"\n}");
    assert_eq!(
        d,
        "2:13: unterminated string\n4:19: expected an expression, found the end\n\
         5:15: `if` is a keyword: a template names a value (`${…}` takes an expression)"
    );
    assert_eq!(decls, "(fun a() {(val s = \"open\"); (val t = 1); (val u = \"${<error>}\"); (val v = \"$<error>\")})");
}

#[test]
fn what_the_tree_has_no_place_for() {
    let (decls, d) = recovered("fun <T> f(x: T & Any) = 1\nval r = List<Int>::size\nval a = [1, 2]\nval ok = 2");
    assert_eq!(
        d,
        "1:14: a definitely non-nullable type (`T & Any`) has no place in KompozR's syntax: write `T` with a non-null bound\n\
         2:9: a callable reference on a type with type arguments (`List<Int>::size`) has no place in KompozR's syntax\n\
         3:9: a collection literal `[…]` is Kotlin's for annotation arguments only, and has no place in KompozR's syntax: write `arrayOf(…)`"
    );
    assert_eq!(decls, "(fun <T> f(x: <error>) = 1)\n(val r = <error>)\n(val a = <error>)\n(val ok = 2)");
}

#[test]
fn warnings_for_what_has_no_effect() {
    let (decls, d) = recovered("fun f() {\n    x@ 1 + 2\n    for (@Ann i in xs) {}\n    @Ann x = 1\n}");
    assert_eq!(
        d,
        "2:5: warning: the label `x@` names nothing `return@`, `break@` or `continue@` can go to (only a lambda or a loop): it is ignored\n\
         3:10: warning: an annotation on a loop's variable has no effect here (it is ignored)\n\
         4:5: warning: an annotation on an assignment has no effect here (it is ignored)"
    );
    assert_eq!(decls, "(fun f() {(+ 1 2); (for i in xs {}); (= x 1)})");
}

#[test]
fn other_mistakes() {
    let cases = [
        (
            "fun f() { val x = 1 val y = 2 }",
            "1:21: expected a new line or `;` after the statement, found keyword `val`",
        ),
        ("private private fun f() {}", "1:9: `private` is written twice"),
        ("val x get() val y = 1", "1:13: expected the getter's body (`= …` or `{ … }`), found keyword `val`"),
        ("fun f() { try { } }", "1:11: a `try` needs a `catch` or a `finally`"),
        (
            "enum class E { A, B fun f() }",
            "1:21: expected `,`, `;` or `}` (members come after the entries and a `;`), found keyword `fun`",
        ),
        ("fun f() = when (x) { 1 }", "1:24: expected `->`, found `}`"),
        ("fun f() { x = }", "1:15: expected an expression, found `}`"),
        ("}\nfun ok() {}", "1:1: unexpected `}`: nothing to close"),
        ("import a.b\nfun f() {}\nimport c.d", "3:1: imports go before the declarations"),
        ("fun f(val x: Int) {}", "1:7: a function's parameter is no property: `val`/`var` are a class constructor's"),
        (
            "fun f() { val g = fun named() {} }",
            "1:23: an anonymous function has no name (a named one is a declaration, a statement of its own)",
        ),
        (
            "fun f() { f(@Ann { }) ; g() @Ann { } }",
            "1:29: an annotated lambda goes inside the call's parentheses: `f(@Ann { … })`",
        ),
        (
            "context(Scope) fun f() {}",
            "1:1: context parameters (`context(…)`, experimental in Kotlin) have no place in KompozR's syntax",
        ),
        ("fun f() { val c = 'ab' + 1 }", "1:19: a character literal holds one character"),
    ];
    for (text, expected) in cases {
        let (_, d) = recovered(text);
        assert_eq!(d, expected, "{text}");
    }
}
