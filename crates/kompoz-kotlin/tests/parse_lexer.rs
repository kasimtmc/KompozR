//! The lexer: every literal form, names, operators by longest match, comments, newlines, templates (nested,
//! with their real positions), raw strings, positions in characters, and going on after mistakes.

mod common;

use common::*;
use kompoz_kotlin::Pos;
use kompoz_kotlin::ast::NumberKind;
use kompoz_kotlin::lexer::{StringPiece, Token, TokenKind};

fn ok(text: &str) -> Vec<Token> {
    let (t, d) = tokens(text);
    assert!(d.is_empty(), "{text:?}:\n{}", render(&d));
    t
}

/// The tokens written short: names as they are, `#text` numbers, `"…"` strings, `'c'`, operators, `⏎`.
fn short(text: &str) -> String {
    ok(text)
        .iter()
        .filter(|t| t.kind != TokenKind::Eof)
        .map(|t| match &t.kind {
            TokenKind::Ident { backticked: true } => format!("`{}`", t.text),
            TokenKind::Ident { .. } | TokenKind::Op => t.text.clone(),
            TokenKind::Number { .. } => format!("#{}", t.text),
            TokenKind::String { raw, .. } => if *raw { "\"\"\"…\"\"\"" } else { "\"…\"" }.into(),
            TokenKind::Char(c) => format!("'{}'", c.escape_debug()),
            TokenKind::Newline => "⏎".into(),
            TokenKind::Eof => unreachable!(),
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn number(text: &str) -> (NumberKind, u32, String) {
    let t = ok(text);
    assert_eq!(t.len(), 2, "{text:?} is one token: {t:?}");
    match &t[0].kind {
        TokenKind::Number { kind, radix } => (*kind, *radix, t[0].text.clone()),
        other => panic!("{other:?}"),
    }
}

fn errors(text: &str) -> String {
    render(&tokens(text).1)
}

/// The pieces of the one string `text` is: text as it is, `$name`, `${…}` with its tokens written short.
fn pieces(text: &str) -> Vec<String> {
    let t = ok(text);
    let TokenKind::String { pieces, .. } = &t[0].kind else { panic!("{:?}", t[0]) };
    pieces
        .iter()
        .map(|p| match p {
            StringPiece::Text { text, .. } => text.clone(),
            StringPiece::Name { name, backticked: false, .. } => format!("${name}"),
            StringPiece::Name { name, backticked: true, .. } => format!("$`{name}`"),
            StringPiece::Expr { tokens, .. } => format!("${{{} tokens}}", tokens.len() - 1),
        })
        .collect()
}

#[test]
fn numbers_in_every_form() {
    use NumberKind::*;
    let cases: &[(&str, NumberKind, u32)] = &[
        ("0", Int, 10),
        ("16", Int, 10),
        ("1_000", Int, 10),
        ("1__000_000", Int, 10),
        ("2147483648", Int, 10),
        ("10L", Long, 10),
        ("1_000L", Long, 10),
        ("1u", UInt, 10),
        ("7U", UInt, 10),
        ("1uL", ULong, 10),
        ("1UL", ULong, 10),
        ("0xFF", Int, 16),
        ("0XfF_00", Int, 16),
        ("0xFF00FF00", Int, 16),
        ("0x1F", Int, 16),
        ("0xFFL", Long, 16),
        ("0xFFu", UInt, 16),
        ("0xFFuL", ULong, 16),
        ("0b1010", Int, 2),
        ("0B1010_0101", Int, 2),
        ("0b1L", Long, 2),
        ("1f", Float, 10),
        ("3F", Float, 10),
        ("01f", Float, 10),
        ("1.5f", Float, 10),
        (".5f", Float, 10),
        ("1e3f", Float, 10),
        ("1.0", Double, 10),
        ("1.5", Double, 10),
        (".5", Double, 10),
        ("0.5", Double, 10),
        ("00.5", Double, 10),
        ("1_000.000_1", Double, 10),
        ("1e10", Double, 10),
        ("1E10", Double, 10),
        ("1.5e-3", Double, 10),
        ("2.5E+3", Double, 10),
        ("1e1_0", Double, 10),
    ];
    for &(text, kind, radix) in cases {
        assert_eq!(number(text), (kind, radix, text.to_string()), "{text}");
    }
}

#[test]
fn a_dot_after_a_number() {
    // `1.` is 1 and then a `.`: a member (16.dp), a range (1..2), a member of a Double (1.5.dp)
    assert_eq!(short("16.dp"), "#16 . dp");
    assert_eq!(short("1.dp.toPx()"), "#1 . dp . toPx ( )");
    assert_eq!(short("1..2"), "#1 .. #2");
    assert_eq!(short("1..<2"), "#1 ..< #2");
    assert_eq!(short("1.0..2.0"), "#1.0 .. #2.0");
    assert_eq!(short("1.5.dp"), "#1.5 . dp");
    assert_eq!(short("1.e3"), "#1 . e3");
    assert_eq!(short("x.5f"), "x #.5f");
    assert_eq!(short("-1"), "- #1");
}

#[test]
fn malformed_numbers() {
    assert_eq!(errors("0x"), "1:1: expected hex digits after `0x`");
    assert_eq!(errors("0b"), "1:1: expected binary digits after `0b`");
    assert_eq!(errors("0b2"), "1:1: expected binary digits after `0b`\n1:1: unexpected characters after a number");
    assert_eq!(errors("1dp"), "1:1: unexpected characters after a number");
    assert_eq!(errors("1_"), "1:1: unexpected characters after a number");
    assert_eq!(errors("1e"), "1:1: unexpected characters after a number");
    assert_eq!(errors("1.0D"), "1:1: unexpected characters after a number");
    assert_eq!(
        errors("1.5L"),
        "1:4: a number with a fraction or an exponent is a Double (or a Float with `f`): no `L` or `u`"
    );
    assert_eq!(errors("012"), "1:1: a number doesn't start with `0` (Kotlin has no octal numbers)");
    assert_eq!(errors("0_1L"), "1:1: a number doesn't start with `0` (Kotlin has no octal numbers)");
    assert_eq!(errors("0 0L 0u 0.0 0f 0x0"), "");
}

#[test]
fn names_keywords_and_backticks() {
    let t = ok("fun `in` `my test` _ ğüşıöçİ ölçü2 data `fun`");
    let kinds: Vec<(bool, &str)> = t
        .iter()
        .filter_map(|t| match t.kind {
            TokenKind::Ident { backticked } => Some((backticked, t.text.as_str())),
            _ => None,
        })
        .collect();
    assert_eq!(
        kinds,
        [
            (false, "fun"),
            (true, "in"),
            (true, "my test"),
            (false, "_"),
            (false, "ğüşıöçİ"),
            (false, "ölçü2"),
            (false, "data"),
            (true, "fun"),
        ]
    );
    assert!(t[0].is_word("fun") && !t[7].is_word("fun"));
    // (a backticked name's position covers its backticks)
    assert_eq!((t[2].loc.start.column, t[2].loc.end.column), (10, 19));
    assert_eq!(errors("`open\nx"), "1:1: unterminated `name`");
    assert_eq!(errors("``"), "1:1: a backticked name can't be empty");
    assert_eq!(errors("a 😀 b"), "1:3: unexpected character `😀`");
}

#[test]
fn operators_by_longest_match() {
    assert_eq!(short("a?.b ?: c!!"), "a ?. b ?: c !!");
    assert_eq!(short("a===b!==c==d!=e"), "a === b !== c == d != e");
    assert_eq!(short("0..<n ..m"), "#0 ..< n .. m");
    assert_eq!(short("x->y::z"), "x -> y :: z");
    assert_eq!(short("a+=1-=2*=3/=4%=5"), "a += #1 -= #2 *= #3 /= #4 %= #5");
    assert_eq!(short("i++ --j"), "i ++ -- j");
    assert_eq!(short("a<=b>=c<d>e"), "a <= b >= c < d > e");
    assert_eq!(short("a&&b||!c"), "a && b || ! c");
    assert_eq!(short("@Ann lbl@{ return@lbl }"), "@ Ann lbl @ { return @ lbl }");
    assert_eq!(short("x as? T & Any"), "x as ? T & Any");
    assert_eq!(short("a !in b !is c"), "a ! in b ! is c");
    assert_eq!(short("f(*xs, [1]); x?"), "f ( * xs , [ #1 ] ) ; x ?");
    assert_eq!(
        errors("a # b $ c | d ~ e"),
        "1:3: unexpected character `#`\n1:7: unexpected character `$`\n1:11: unexpected character `|`\n1:15: unexpected character `~`"
    );
}

#[test]
fn comments_and_newlines() {
    assert_eq!(short("a // line\nb"), "a ⏎ b");
    assert_eq!(short("a /* one */ b"), "a b");
    assert_eq!(short("a /* outer /* inner */ still */ b"), "a b");
    assert_eq!(short("/** KDoc\n * @param x the x\n */\nfun f()"), "⏎ fun f ( )");
    // a block comment over several lines ends the line it is on
    assert_eq!(short("a /*\n*/ b"), "a ⏎ b");
    assert_eq!(short("a\r\nb\n\nc"), "a ⏎ b ⏎ ⏎ c");
    assert_eq!(errors("a /* never /* closed */"), "1:3: unterminated comment");
    // (space before a token, a comment counting as space, a newline not)
    let t = ok("a b/**/c\nd");
    let spaces: Vec<bool> = t.iter().map(|t| t.space_before).collect();
    assert_eq!(spaces, [false, true, true, false, false, false]);
}

#[test]
fn a_shebang_is_skipped() {
    assert_eq!(short("#!/usr/bin/env kotlin\nfun main() {}"), "⏎ fun main ( ) { }");
    assert_eq!(errors("x\n#!/no"), "2:1: unexpected character `#`");
}

#[test]
fn strings_and_their_escapes() {
    assert_eq!(pieces(r#""a\tb\b\n\r\'\"\\\$c""#), ["a\tb\u{8}\n\r'\"\\$c"]);
    assert_eq!(pieces(r#""\u0041\u00e7\u{1F600}\u{41}""#), ["Açç😀A".replace("çç", "ç")]);
    assert_eq!(pieces(r#""""#), Vec::<String>::new());
    assert_eq!(pieces(r#""ğü 😀""#), ["ğü 😀"]);
    assert_eq!(errors(r#""\d""#), "1:2: unknown escape `\\d`");
    assert_eq!(errors(r#""\u12""#), "1:2: `\\u` takes four hex digits");
    assert_eq!(errors(r#""\uD83D""#), "1:2: `\\uD83D` is half of a surrogate pair: write the character itself");
    assert_eq!(errors(r#""\u{110000}""#), "1:2: `\\u{…}` takes the hex digits of a character");
    assert_eq!(errors("\"abc\nx"), "1:1: unterminated string");
    assert_eq!(errors("\"abc"), "1:1: unterminated string");
}

#[test]
fn templates() {
    assert_eq!(pieces(r#""Count: $count""#), ["Count: ", "$count"]);
    assert_eq!(pieces(r#""$a$b-$`c d`.""#), ["$a", "$b", "-", "$`c d`", "."]);
    // `$` not before a name or `{` is text
    assert_eq!(pieces(r#""$ $1 a$""#), ["$ $1 a$"]);
    assert_eq!(pieces(r#""${x + 1} = ${'$'}""#), ["${3 tokens}", " = ", "${1 tokens}"]);
    // a brace or quote inside the expression is its own
    assert_eq!(pieces(r#""${map["}"]}!""#), ["${4 tokens}", "!"]);
    assert_eq!(pieces(r#""${list.map { it }}""#), ["${6 tokens}"]);
    assert_eq!(errors(r#""${a"#), "1:2: unterminated `${…}`\n1:1: unterminated string");
}

#[test]
fn templates_nested_three_deep_at_their_positions() {
    let source = "  \"ğ ${ \"ü ${ \"ş $x\" } ö\" }!\"";
    let t = ok(source);
    assert_eq!(t[0].loc.start, Pos { offset: 2, line: 1, column: 3 });
    let TokenKind::String { pieces: p1, .. } = &t[0].kind else { panic!() };
    let [
        StringPiece::Text { text, loc },
        StringPiece::Expr { tokens: inner, loc: l1 },
        StringPiece::Text { text: bang, .. },
    ] = &p1[..]
    else {
        panic!("{p1:?}")
    };
    assert_eq!((text.as_str(), loc.start.column, loc.end.column), ("ğ ", 4, 6));
    assert_eq!((l1.start.column, bang.as_str()), (6, "!"));
    let TokenKind::String { pieces: p2, .. } = &inner[0].kind else { panic!("{inner:?}") };
    assert_eq!(inner[0].loc.start.column, 9);
    let StringPiece::Expr { tokens: inner2, .. } = &p2[1] else { panic!("{p2:?}") };
    let TokenKind::String { pieces: p3, .. } = &inner2[0].kind else { panic!("{inner2:?}") };
    let StringPiece::Name { name, loc, .. } = &p3[1] else { panic!("{p3:?}") };
    // `x` of the innermost: character columns (`ğ`, `ü`, `ş` one each)
    assert_eq!((name.as_str(), loc.start.column, loc.start.offset), ("x", 19, source.find('x').unwrap()));
    assert_eq!(t[0].loc.end.column, source.chars().count() as u32 + 1);
}

#[test]
fn raw_strings() {
    let raw = |text: &str| {
        let t = ok(text);
        let TokenKind::String { raw: true, pieces } = &t[0].kind else { panic!("{:?}", t[0]) };
        pieces
            .iter()
            .map(|p| match p {
                StringPiece::Text { text, .. } => text.clone(),
                StringPiece::Name { name, .. } => format!("${name}"),
                StringPiece::Expr { .. } => "${…}".into(),
            })
            .collect::<Vec<_>>()
    };
    // no escapes, newlines and indentation kept (trimIndent's work is the generator's)
    assert_eq!(raw("\"\"\"\n    a\\tb \"q\"\n      c\n\"\"\""), ["\n    a\\tb \"q\"\n      c\n"]);
    assert_eq!(raw(r#""""C:\path\d+""""#), [r"C:\path\d+"]);
    // the closing quotes are the last three of a run
    assert_eq!(raw(r#""""say "hi"""""#), [r#"say "hi""#]);
    assert_eq!(raw(r#""""a""""""#), [r#"a"""#]);
    assert_eq!(raw(r#""""$name ${x} ${'$'}5""""#), ["$name", " ", "${…}", " ", "${…}", "5"]);
    let t = ok("\"\"\"a\nb\"\"\" x");
    assert_eq!((t[0].loc.end.line, t[0].loc.end.column, t[1].loc.start.line), (2, 5, 2));
    assert_eq!(errors("\"\"\"never\nclosed"), "1:1: unterminated string");
}

#[test]
fn characters() {
    let c = |text: &str| match ok(text)[0].kind {
        TokenKind::Char(c) => c,
        ref other => panic!("{other:?}"),
    };
    assert_eq!(c("'a'"), 'a');
    assert_eq!(c("'ğ'"), 'ğ');
    assert_eq!(c("'\\n'"), '\n');
    assert_eq!(c("'\\''"), '\'');
    assert_eq!(c("'\"'"), '"');
    assert_eq!(c("'$'"), '$');
    assert_eq!(c("'\\u0041'"), 'A');
    assert_eq!(c("'\\u{e7}'"), 'ç');
    assert_eq!(errors("'ab'"), "1:1: a character literal holds one character");
    assert_eq!(errors("''"), "1:1: a character literal can't be empty");
    assert_eq!(errors("'a"), "1:1: unterminated character literal");
    assert_eq!(errors("'😀'"), "1:1: a character literal holds one UTF-16 unit, and `😀` is two (a String holds it)");
}

#[test]
fn positions_count_characters() {
    let t = ok("val ğüş = \"😀😀\" // İ\n  x");
    let at = |i: usize| (t[i].loc.start.line, t[i].loc.start.column, t[i].loc.end.column);
    assert_eq!(at(1), (1, 5, 8));
    assert_eq!(at(2), (1, 9, 10));
    assert_eq!(at(3), (1, 11, 15));
    assert_eq!(t[3].loc.start.offset, "val ğüş = ".len());
    assert_eq!(t[4].kind, TokenKind::Newline);
    assert_eq!(at(5), (2, 3, 4));
    // from a base position (a `kompoz!` text starts at its line and column 1)
    let mut d = kompoz_kotlin::Diagnostics::default();
    let t = kompoz_kotlin::lexer::lex("ö x", Pos::start(42, 1), &mut d);
    assert_eq!((t[1].loc.start.line, t[1].loc.start.column, t[1].loc.start.offset), (42, 3, 3));
}

#[test]
fn going_on_after_mistakes() {
    let (t, d) = tokens("val a = \"open\nval b = 'xy' # 0x\nval c = `d");
    assert_eq!(
        render(&d),
        "1:9: unterminated string\n2:9: a character literal holds one character\n2:14: unexpected character `#`\n\
         2:16: expected hex digits after `0x`\n3:9: unterminated `name`"
    );
    let words: Vec<&str> = t.iter().filter(|t| t.is_ident()).map(|t| t.text.as_str()).collect();
    assert_eq!(words, ["val", "a", "val", "b", "val", "c", "d"]);
    assert_eq!(t.last().unwrap().kind, TokenKind::Eof);
}
