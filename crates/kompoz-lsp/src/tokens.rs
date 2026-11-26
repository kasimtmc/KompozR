//! Semantic tokens. The Kotlin's are the server's own, read with kompoz-kotlin's lexer as the C++ kompozc's language
//! server reads them (keywords, strings with their templates' code as code, numbers, comments, annotations, labels,
//! and what names are by where they stand: functions and classes declared and called, values declared, parameters,
//! properties after a dot, named arguments), then told apart by what the generator found each name to be (the
//! translation's [`Analysis`]: a local, a declaration of the crate, an API item). The Rust's are rust-analyzer's,
//! kept off the Kotlin. One legend for both: rust-analyzer's (1.99's, the toolchain's) with what the Kotlin needs
//! added; rust-analyzer's answer is read by its own legend's names, so another version's order doesn't matter.

use crate::json::Json;
use kompoz_kotlin::analysis::{Analysis, DeclKind, LocalKind, Target};
use kompoz_kotlin::index;
use kompoz_kotlin::lexer::{StringPiece, Token, TokenKind};
use std::collections::HashMap;

/// The token types, by LSP's names: rust-analyzer's, then the Kotlin's it lacks.
pub const TYPES: &[&str] = &[
    "comment",
    "decorator",
    "enumMember",
    "enum",
    "function",
    "interface",
    "keyword",
    "macro",
    "method",
    "namespace",
    "number",
    "operator",
    "parameter",
    "property",
    "string",
    "struct",
    "typeParameter",
    "variable",
    "type",
    "label",
    "angle",
    "arithmetic",
    "attributeBracket",
    "attribute",
    "bitwise",
    "boolean",
    "brace",
    "bracket",
    "builtinAttribute",
    "builtinType",
    "character",
    "colon",
    "comma",
    "comparison",
    "constParameter",
    "const",
    "deriveHelper",
    "derive",
    "dot",
    "escapeSequence",
    "formatSpecifier",
    "generic",
    "invalidEscapeSequence",
    "lifetime",
    "logical",
    "macroBang",
    "negation",
    "parenthesis",
    "procMacro",
    "punctuation",
    "selfKeyword",
    "selfTypeKeyword",
    "semicolon",
    "static",
    "toolModule",
    "typeAlias",
    "union",
    "unresolvedReference",
    // (the Kotlin's)
    "class",
];

/// The token modifiers (a bit each, in this order): rust-analyzer's.
pub const MODIFIERS: &[&str] = &[
    "async",
    "documentation",
    "declaration",
    "static",
    "defaultLibrary",
    "deprecated",
    "associated",
    "attribute",
    "callable",
    "constant",
    "consuming",
    "controlFlow",
    "crateRoot",
    "injected",
    "intraDocLink",
    "library",
    "macro",
    "mutable",
    "procMacro",
    "public",
    "reference",
    "trait",
    "unsafe",
];

/// The legend, as `initialize` tells it.
pub fn legend() -> Json {
    Json::object([
        ("tokenTypes", TYPES.iter().map(|t| Json::from(*t)).collect::<Vec<_>>().into()),
        ("tokenModifiers", MODIFIERS.iter().map(|t| Json::from(*t)).collect::<Vec<_>>().into()),
    ])
}

fn type_index(name: &str) -> u32 {
    TYPES.iter().position(|t| *t == name).unwrap_or(0) as u32
}

fn modifier_bit(name: &str) -> u32 {
    MODIFIERS.iter().position(|m| *m == name).map_or(0, |i| 1 << i)
}

/// A token in a document: its line, its start and length (UTF-16 units), its type and modifiers (the legend's).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct SemanticToken {
    pub line: u32,
    pub start: u32,
    pub length: u32,
    pub ty: u32,
    pub modifiers: u32,
}

/// Tokens as LSP sends them: each relative to the one before (sorted first).
pub fn encode(mut tokens: Vec<SemanticToken>) -> Json {
    tokens.sort();
    tokens.dedup_by(|a, b| (a.line, a.start) == (b.line, b.start));
    let mut data = Vec::with_capacity(tokens.len() * 5);
    let (mut line, mut start) = (0, 0);
    for t in tokens {
        data.push(Json::from(t.line - line));
        data.push(Json::from(if t.line == line { t.start - start } else { t.start }));
        data.push(Json::from(t.length));
        data.push(Json::from(t.ty));
        data.push(Json::from(t.modifiers));
        (line, start) = (t.line, t.start);
    }
    Json::Array(data)
}

/// rust-analyzer's tokens (`data`, in its `legend`) in this legend; those of a type this legend lacks are left out.
pub fn decode(data: &Json, legend: &Json) -> Vec<SemanticToken> {
    let types: Vec<Option<u32>> = legend["tokenTypes"]
        .array()
        .iter()
        .map(|t| TYPES.iter().position(|n| *n == t.str()).map(|i| i as u32))
        .collect();
    let modifiers: Vec<u32> = legend["tokenModifiers"].array().iter().map(|m| modifier_bit(m.str())).collect();
    let data = data.array();
    let mut out = Vec::with_capacity(data.len() / 5);
    let (mut line, mut start) = (0u32, 0u32);
    for chunk in data.as_chunks::<5>().0 {
        let dl = chunk[0].int() as u32;
        line += dl;
        start = if dl > 0 { chunk[1].int() as u32 } else { start + chunk[1].int() as u32 };
        let Some(Some(ty)) = types.get(chunk[3].int() as usize) else { continue };
        let bits = chunk[4].int() as u64;
        let mods =
            modifiers.iter().enumerate().filter(|(i, _)| *i < 64 && bits >> i & 1 == 1).fold(0, |m, (_, b)| m | b);
        out.push(SemanticToken { line, start, length: chunk[2].int() as u32, ty: *ty, modifiers: mods });
    }
    out
}

/// A Kotlin token over a stretch of its text (byte offsets), before its place in the document is known.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KotlinToken {
    pub from: usize,
    pub to: usize,
    pub ty: &'static str,
    pub declaration: bool,
}

/// What the names of the text are, by their place: the analysis' references told by what they refer to.
pub struct Names<'a> {
    pub analysis: Option<&'a Analysis>,
    /// The kinds of the declarations names refer to (the text's and the module table's), by Kotlin's qualified name.
    pub declarations: &'a HashMap<String, DeclKind>,
}

impl Names<'_> {
    /// The type of the name at `from` when the generator knows what it is.
    fn of(&self, from: usize) -> Option<&'static str> {
        let a = self.analysis?;
        let r = a.references.iter().find(|r| r.loc.start.offset == from)?;
        Some(match &r.target {
            Target::Local(at) => {
                let local = a.locals.iter().find(|l| l.loc == *at)?;
                match local.kind {
                    LocalKind::Param | LocalKind::LambdaParam => "parameter",
                    LocalKind::Val | LocalKind::Var => "variable",
                }
            }
            Target::Declaration(k) => match self.declarations.get(k)? {
                DeclKind::Function | DeclKind::Composable | DeclKind::Constructor => "function",
                DeclKind::Property if k.contains('.') => "property",
                DeclKind::Property => "variable",
                DeclKind::Class | DeclKind::DataClass | DeclKind::Object | DeclKind::Companion => "class",
                DeclKind::Enum => "enum",
                DeclKind::EnumEntry => "enumMember",
                DeclKind::Interface => "interface",
                DeclKind::TypeAlias => "typeAlias",
            },
            Target::Api { kotlin, rust, krate, module, owner } => {
                let item = index::lookup(kotlin)
                    .iter()
                    .find(|i| i.rust == rust && i.krate == krate && i.module == module && i.owner == owner)?;
                match item.kind {
                    index::Kind::Function | index::Kind::Overload => "function",
                    index::Kind::Method => "method",
                    index::Kind::Property => "property",
                    index::Kind::Value if item.owner == item.rust && !item.owner.is_empty() => "class",
                    index::Kind::Value => "variable",
                    index::Kind::Constant => "property",
                    index::Kind::Variant => "enumMember",
                    index::Kind::FunInterface => "interface",
                }
            }
        })
    }
}

fn hard_keyword(w: &str) -> bool {
    [
        "as",
        "break",
        "class",
        "continue",
        "do",
        "else",
        "false",
        "for",
        "fun",
        "if",
        "in",
        "interface",
        "is",
        "null",
        "object",
        "package",
        "return",
        "super",
        "this",
        "throw",
        "true",
        "try",
        "typealias",
        "typeof",
        "val",
        "var",
        "when",
        "while",
        "catch",
        "finally",
        "import",
    ]
    .contains(&w)
}

/// Kotlin's soft keywords and modifiers: keywords where a declaration or a name follows (`data class`,
/// `override fun`, `by lazy`, `companion object`); names elsewhere (a value called `data`).
fn soft_keyword(w: &str) -> bool {
    [
        "abstract",
        "actual",
        "annotation",
        "by",
        "companion",
        "const",
        "constructor",
        "crossinline",
        "data",
        "enum",
        "expect",
        "external",
        "field",
        "final",
        "get",
        "infix",
        "init",
        "inline",
        "inner",
        "internal",
        "lateinit",
        "noinline",
        "open",
        "operator",
        "out",
        "override",
        "private",
        "protected",
        "public",
        "reified",
        "sealed",
        "set",
        "suspend",
        "tailrec",
        "vararg",
        "value",
        "where",
    ]
    .contains(&w)
}

/// The tokens of a Kotlin text (a region's: offsets into it), sorted.
pub fn kotlin_tokens(text: &str, names: &Names) -> Vec<KotlinToken> {
    let mut diags = kompoz_kotlin::Diagnostics::default();
    let tokens = kompoz_kotlin::lexer::lex(text, kompoz_kotlin::Pos::start(1, 1), &mut diags);
    let mut r = Reader { text, names, out: Vec::new(), functions: Vec::new(), values: Vec::new() };
    // (first the names declared, then the tokens)
    r.read(&tokens, 0, text.len(), true);
    r.out.clear();
    r.read(&tokens, 0, text.len(), false);
    let mut out = r.out;
    out.sort_by_key(|t| t.from);
    out
}

struct Reader<'a> {
    text: &'a str,
    names: &'a Names<'a>,
    out: Vec<KotlinToken>,
    /// The values and functions the text declares (the first reading): a name's kind where it is used.
    functions: Vec<String>,
    values: Vec<String>,
}

#[derive(PartialEq)]
enum After {
    None,
    Fun,
    Parameters,
    Class,
    Value,
}

impl Reader<'_> {
    fn add(&mut self, from: usize, to: usize, ty: &'static str, declaration: bool) {
        if to > from {
            self.out.push(KotlinToken { from, to, ty, declaration });
        }
    }

    /// The comments in a gap between tokens (what the lexer skips: blanks and comments).
    fn comments(&mut self, mut from: usize, to: usize) {
        let text = self.text;
        while from < to {
            let Some(slash) = text[from..to].find('/').map(|i| i + from) else { return };
            match text.as_bytes().get(slash + 1) {
                Some(b'/') if slash + 1 < to => {
                    let end = text[slash..to].find('\n').map_or(to, |i| i + slash);
                    self.add(slash, end, "comment", false);
                    from = end;
                }
                Some(b'*') if slash + 1 < to => {
                    // (nested, as Kotlin's)
                    let mut depth = 0;
                    let mut i = slash;
                    let b = text.as_bytes();
                    while i + 1 < to {
                        if b[i] == b'/' && b[i + 1] == b'*' {
                            depth += 1;
                            i += 2;
                        } else if b[i] == b'*' && b[i + 1] == b'/' {
                            depth -= 1;
                            i += 2;
                            if depth == 0 {
                                break;
                            }
                        } else {
                            i += 1;
                        }
                    }
                    let end = i.min(to);
                    self.add(slash, end, "comment", false);
                    from = end;
                }
                _ => from = slash + 1,
            }
        }
    }

    /// The tokens of `tokens` (a text's, or a template's code), between `from` and `to`.
    fn read(&mut self, tokens: &[Token], from: usize, to: usize, collecting: bool) {
        let mut last = from;
        let mut code: Vec<&Token> = Vec::new();
        for t in tokens {
            if t.kind == TokenKind::Eof {
                break;
            }
            if !collecting {
                self.comments(last, t.loc.start.offset);
            }
            last = t.loc.end.offset;
            if t.kind != TokenKind::Newline {
                code.push(t);
            }
        }
        if !collecting {
            self.comments(last, to);
        }
        self.classify(&code, collecting);
    }

    fn classify(&mut self, code: &[&Token], collecting: bool) {
        let op = |i: usize, o: &str| code.get(i).is_some_and(|t| t.is_op(o));
        let ident = |i: usize| code.get(i).is_some_and(|t| t.is_ident());
        let prev_dot = |i: usize| i > 0 && (op(i - 1, ".") || op(i - 1, "?."));
        // (the names a lambda's arrow follows: `{ a, b -> }`, `{ (x, y) -> }`)
        let mut lambda_params: Vec<usize> = Vec::new();
        for i in 0..code.len() {
            if !op(i, "{") {
                continue;
            }
            let mut names = Vec::new();
            let mut j = i + 1;
            while j < code.len() && (ident(j) || [",", "(", ")", ":", "<", ">", ".", "?"].iter().any(|o| op(j, o))) {
                if ident(j) && !(op(j - 1, ":") || op(j - 1, "<") || op(j - 1, ".")) {
                    names.push(j);
                }
                j += 1;
            }
            if op(j, "->") {
                lambda_params.extend(names);
            }
        }
        let mut after = After::None;
        let (mut param_depth, mut depth): (i64, i64) = (-1, 0);
        let mut i = 0;
        while i < code.len() {
            let t = code[i];
            let (from, to) = (t.loc.start.offset, t.loc.end.offset);
            match &t.kind {
                TokenKind::Number { .. } if !collecting => self.add(from, to, "number", false),
                TokenKind::Char(_) if !collecting => self.add(from, to, "string", false),
                TokenKind::String { pieces, .. } if !collecting => self.string(t, pieces),
                TokenKind::Op => {
                    if t.text == "(" {
                        if after == After::Parameters {
                            param_depth = depth;
                            after = After::None;
                        }
                        depth += 1;
                    } else if t.text == ")" {
                        depth -= 1;
                        if depth == param_depth {
                            param_depth = -1;
                        }
                    } else if t.text == "@" && ident(i + 1) && code[i + 1].loc.start.offset == to {
                        let label = i > 0 && ident(i - 1) && code[i - 1].loc.end.offset == from;
                        if label {
                            // (return@name)
                            if !collecting {
                                self.add(code[i + 1].loc.start.offset, code[i + 1].loc.end.offset, "label", false);
                            }
                        } else {
                            // (an annotation, its dotted name too)
                            let mut j = i + 1;
                            let mut end = code[j].loc.end.offset;
                            while op(j + 1, ".") && ident(j + 2) {
                                j += 2;
                                end = code[j].loc.end.offset;
                            }
                            if !collecting {
                                self.add(from, end, "decorator", false);
                            }
                            i = j;
                        }
                    }
                }
                TokenKind::Ident { backticked } => {
                    let w = t.text.as_str();
                    let call =
                        op(i + 1, "(") || op(i + 1, "{") || (op(i + 1, "<") && call_after_type_args(code, i + 1));
                    if !backticked && hard_keyword(w) && !prev_dot(i) {
                        if !collecting {
                            self.add(from, to, "keyword", false);
                        }
                        after = match w {
                            "fun" => After::Fun,
                            "class" | "interface" | "object" | "typealias" => After::Class,
                            "val" | "var" => After::Value,
                            "this" | "super" => After::None,
                            _ => after,
                        };
                        i += 1;
                        continue;
                    }
                    if !backticked
                        && soft_keyword(w)
                        && !prev_dot(i)
                        && (ident(i + 1)
                            || (["get", "set", "constructor"].contains(&w) && op(i + 1, "("))
                            || (w == "init" && op(i + 1, "{")))
                    {
                        if !collecting {
                            self.add(from, to, "keyword", false);
                        }
                        i += 1;
                        continue;
                    }
                    if after == After::Fun {
                        // (fun <T> Receiver.name(: the last name before the parameters)
                        if op(i + 1, ".") {
                            self.emit(from, to, "class", false, collecting);
                        } else {
                            self.emit(from, to, "function", true, collecting);
                            if collecting {
                                self.functions.push(w.to_string());
                            }
                            after = After::Parameters;
                        }
                        i += 1;
                        continue;
                    }
                    if after == After::Class {
                        self.emit(from, to, "class", true, collecting);
                        after = After::None;
                        i += 1;
                        continue;
                    }
                    if after == After::Value {
                        // (val Receiver.name, val (a, b))
                        if op(i + 1, ".") {
                            self.emit(from, to, "class", false, collecting);
                        } else {
                            self.emit(from, to, "variable", true, collecting);
                            if collecting {
                                self.values.push(w.to_string());
                            }
                            if !op(i + 1, ",") {
                                after = After::None;
                            }
                        }
                        i += 1;
                        continue;
                    }
                    if collecting {
                        i += 1;
                        continue;
                    }
                    if lambda_params.contains(&i) || (param_depth >= 0 && depth == param_depth + 1 && op(i + 1, ":")) {
                        self.add(from, to, "parameter", true);
                    } else if let Some(known) = self.names.of(from) {
                        self.add(from, to, known, false);
                    } else if w == "it" && !op(i + 1, ":") {
                        self.add(from, to, "parameter", false);
                    } else if call || (self.functions.iter().any(|f| f == w) && i > 0 && op(i - 1, "::")) {
                        self.add(from, to, "function", false);
                    } else if self.values.iter().any(|v| v == w) && !prev_dot(i) {
                        // (a value the text declares)
                        self.add(from, to, "variable", false);
                    } else if op(i + 1, "=") && i > 0 && (op(i - 1, "(") || op(i - 1, ",")) {
                        // (a named argument)
                        self.add(from, to, "parameter", false);
                    } else if prev_dot(i) {
                        let upper = w.chars().next().is_some_and(char::is_uppercase);
                        self.add(from, to, if upper { "class" } else { "property" }, false);
                    } else if w.chars().next().is_some_and(char::is_uppercase) {
                        self.add(from, to, "class", false);
                    }
                }
                _ => {}
            }
            i += 1;
        }
    }

    fn emit(&mut self, from: usize, to: usize, ty: &'static str, declaration: bool, collecting: bool) {
        if !collecting {
            self.add(from, to, ty, declaration);
        }
    }

    /// A string: its text, and its templates' code read as code.
    fn string(&mut self, t: &Token, pieces: &[StringPiece]) {
        let mut at = t.loc.start.offset;
        for piece in pieces {
            match piece {
                StringPiece::Text { .. } => {}
                StringPiece::Name { name, loc, .. } => {
                    // (`$name`: the `$` is the string's)
                    self.add(at, loc.start.offset, "string", false);
                    let known = self.names.of(loc.start.offset);
                    let ty = known.or_else(|| self.values.iter().any(|v| v == name).then_some("variable"));
                    if let Some(ty) = ty {
                        self.add(loc.start.offset, loc.end.offset, ty, false);
                    }
                    at = loc.end.offset;
                }
                StringPiece::Expr { tokens, loc } => {
                    self.add(at, loc.start.offset + 2, "string", false);
                    let inner_end = loc.end.offset.saturating_sub(1);
                    self.read(tokens, loc.start.offset + 2, inner_end, false);
                    at = inner_end;
                }
            }
        }
        self.add(at, t.loc.end.offset, "string", false);
    }
}

/// `name<…>(` or `name<…> {`: the angle brackets close before a call's parenthesis or lambda.
fn call_after_type_args(code: &[&Token], lt: usize) -> bool {
    let mut open = 0;
    for j in lt..code.len() {
        let t = code[j];
        if t.is_op("<") {
            open += 1;
        } else if t.is_op(">") {
            open -= 1;
            if open == 0 {
                return code.get(j + 1).is_some_and(|n| n.is_op("(") || n.is_op("{"));
            }
        } else if !(t.is_ident() || t.is_op(",") || t.is_op(".") || t.is_op("?") || t.is_op("*")) {
            return false;
        }
    }
    false
}

/// A Kotlin token's type and modifiers in the legend.
pub fn legend_of(t: &KotlinToken) -> (u32, u32) {
    (type_index(t.ty), if t.declaration { modifier_bit("declaration") } else { 0 })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(text: &str) -> Vec<(String, &'static str, bool)> {
        let decls = HashMap::new();
        let names = Names { analysis: None, declarations: &decls };
        kotlin_tokens(text, &names).iter().map(|t| (text[t.from..t.to].to_string(), t.ty, t.declaration)).collect()
    }

    #[test]
    fn read_as_kompozc_reads_them() {
        let got = kinds(
            "@Composable\nfun Row(item: Item) { // note\n    Text(\"a ${item.title} $x\", style = s)\n    list.forEach { n -> n }\n}\nval Shout = 1",
        );
        for want in [
            ("@Composable", "decorator", false),
            ("fun", "keyword", false),
            ("Row", "function", true),
            ("item", "parameter", true),
            ("Item", "class", false),
            ("// note", "comment", false),
            ("Text", "function", false),
            ("\"a ${", "string", false),
            ("title", "property", false),
            ("style", "parameter", false),
            ("forEach", "function", false),
            ("Shout", "variable", true),
            ("1", "number", false),
        ] {
            assert!(got.iter().any(|g| (g.0.as_str(), g.1, g.2) == want), "{want:?} not in {got:?}");
        }
        let n: Vec<_> = got.iter().filter(|g| g.0 == "n").collect();
        assert_eq!(n[0].1, "parameter");
    }

    #[test]
    fn encoded_and_decoded() {
        let t = vec![
            SemanticToken { line: 2, start: 4, length: 3, ty: 6, modifiers: 0 },
            SemanticToken { line: 0, start: 1, length: 2, ty: 4, modifiers: 4 },
            SemanticToken { line: 2, start: 9, length: 1, ty: 10, modifiers: 0 },
        ];
        let data = encode(t.clone());
        assert_eq!(data.dump(), "[0,1,2,4,4,2,4,3,6,0,0,5,1,10,0]");
        let mut sorted = t;
        sorted.sort();
        assert_eq!(decode(&data, &legend()), sorted);
    }
}
