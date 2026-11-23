//! Kotlin's `kotlin.text.Regex` as Kotlin/JVM has it ([`Regex`], [`RegexOption`], [`MatchResult`], [`MatchGroup`],
//! `toRegex()` ([`KtToRegex`])): Java's `java.util.regex.Pattern` syntax and meanings, which Kotlin/JVM compiles its
//! patterns with (and Kotlin's own `split`, `replace(transform)`, `findAll`, `next()` on top).
//!
//! A backtracking matcher of Java's syntax: literals and escapes (`\t \n \r \f \a \e`, `\0oct`, `\xhh`, `\x{h…}`,
//! `\uhhhh`, `\cX`, any other non-alphanumeric character escaped), `\Q…\E` quoting (as Java's: quoted text first made
//! escapes), `.`, the classes `\d \D \s \S \w \W \h \H \v \V` and `\R`, character classes (`[^/]`, `[\s\S]`, ranges,
//! escapes and quoting inside, nested classes as unions, `&&` intersections, negation), the properties `\p{…}`/`\P{…}`
//! of the POSIX classes (`Lower`, `Upper`, `ASCII`, `Alpha`, `Digit`, `Alnum`, `Punct`, `Graph`, `Print`, `Blank`,
//! `Cntrl`, `XDigit`, `Space`), of `java.lang.Character` (`javaLowerCase`, `javaUpperCase`, `javaWhitespace`) and the
//! binary properties `IsAlphabetic`, `IsWhite_Space`, `IsUppercase`, `IsLowercase`; groups (capturing, named
//! `(?<name>…)`, `(?:…)`, inline flags `(?imsdux-imsdux)` and `(?imsdux-imsdux:…)`, lookahead and lookbehind
//! `(?=…) (?!…) (?<=…) (?<!…)`, atomic `(?>…)`), back references (`\1`, `\k<name>`), alternation, the quantifiers
//! `* + ? {n} {n,} {n,m}` greedy, lazy (`*?`) and possessive (`*+`), the anchors and boundaries `^ $ \A \z \Z \b \B
//! \G` (each with Java's line terminators: `$` also before a last `\r\n`, `\u{85}`, `\u{2028}`…). Case-insensitive
//! matching is Java's: Kotlin's `IGNORE_CASE` is `CASE_INSENSITIVE | UNICODE_CASE` (Unicode simple case mappings:
//! the one-character case mappings of Rust's `char`), an inline `(?i)` without `u` ASCII's.
//!
//! What Java has and this doesn't — Unicode scripts, blocks and general categories (`\p{L}`, `\p{IsLatin}`,
//! `\p{InGreek}`), `\N{name}`, `\X`, `\b{g}`, `UNICODE_CHARACTER_CLASS` (`(?U)`) and `CANON_EQ` — panics as a
//! `PatternSyntaxException` saying so, as a pattern Java can't read does (a programmer's error). `\b`'s word
//! characters are Rust's `is_alphanumeric` and `_` (Java's `isLetterOrDigit`, near it), without Java's look at
//! combining marks.
//!
//! Indices (`range`, `start_index`) are Kotlin's: UTF-16 units, as [`KtStr`](super::KtStr)'s. The matcher works on
//! characters (code points), as Java's nodes do on supplementary ones; an index inside a surrogate pair is taken as
//! the character after it.
//!
//! Navigation's deep links (kompoz-navigation's NavDeepLink, as Kotlin's `Regex`) are made of it too.

use super::panic_kt;
use super::range::{KtProgression, until};
use super::text::KtToString;
use std::fmt;
use std::sync::Arc;

// ---- Options ----

/// Kotlin's `RegexOption` (Kotlin/JVM's: each a `java.util.regex.Pattern` flag). Java's `CANON_EQ` isn't offered.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum RegexOption {
    /// Kotlin: `RegexOption.IGNORE_CASE` (Java's `CASE_INSENSITIVE`, with `UNICODE_CASE` as Kotlin adds it).
    IgnoreCase,
    /// Kotlin: `RegexOption.MULTILINE` (`^` and `$` at line terminators too).
    Multiline,
    /// Kotlin: `RegexOption.LITERAL` (the pattern is the text it matches).
    Literal,
    /// Kotlin: `RegexOption.UNIX_LINES` (only `\n` ends a line).
    UnixLines,
    /// Kotlin: `RegexOption.COMMENTS` (white space and `#` comments in the pattern left out).
    Comments,
    /// Kotlin: `RegexOption.DOT_MATCHES_ALL` (`.` matches line terminators too).
    DotMatchesAll,
}

const UNIX_LINES: u32 = 0x01;
const CASE_INSENSITIVE: u32 = 0x02;
const COMMENTS: u32 = 0x04;
const MULTILINE: u32 = 0x08;
const LITERAL: u32 = 0x10;
const DOTALL: u32 = 0x20;
const UNICODE_CASE: u32 = 0x40;

impl RegexOption {
    /// Java's flag of it (Kotlin's `value`).
    /// Kotlin: `value: Int`
    pub fn value(self) -> i32 {
        (match self {
            RegexOption::IgnoreCase => CASE_INSENSITIVE,
            RegexOption::Multiline => MULTILINE,
            RegexOption::Literal => LITERAL,
            RegexOption::UnixLines => UNIX_LINES,
            RegexOption::Comments => COMMENTS,
            RegexOption::DotMatchesAll => DOTALL,
        }) as i32
    }

    /// The flags it sets (Kotlin's `mask`: its value).
    /// Kotlin: `mask: Int`
    pub fn mask(self) -> i32 {
        self.value()
    }

    const ALL: [RegexOption; 6] = [
        RegexOption::IgnoreCase,
        RegexOption::Multiline,
        RegexOption::Literal,
        RegexOption::UnixLines,
        RegexOption::Comments,
        RegexOption::DotMatchesAll,
    ];
}

// ---- Characters ----

/// `c`'s simple upper case (Java's `Character.toUpperCase`): Rust's mapping when it is one character.
fn simple_upper(c: char) -> char {
    let mut up = c.to_uppercase();
    match (up.next(), up.next()) {
        (Some(u), None) => u,
        _ => c,
    }
}

/// `c`'s simple lower case (Java's `Character.toLowerCase`).
fn simple_lower(c: char) -> char {
    let mut low = c.to_lowercase();
    match (low.next(), low.next()) {
        (Some(l), None) => l,
        _ => c,
    }
}

/// What Java compares characters by without case (`toLowerCase(toUpperCase(c))`).
fn fold(c: char) -> char {
    simple_lower(simple_upper(c))
}

/// How a node compares characters: exactly, without ASCII case, without Unicode case.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Case {
    Exact,
    Ascii,
    Unicode,
}

impl Case {
    fn of(flags: u32) -> Case {
        if flags & CASE_INSENSITIVE == 0 {
            Case::Exact
        } else if flags & UNICODE_CASE != 0 {
            Case::Unicode
        } else {
            Case::Ascii
        }
    }

    /// Whether `c` is the pattern's `x` (Java's `single(x)`: `Single`, `SingleI`, `SingleU`).
    fn single(self, x: char, c: char) -> bool {
        c == x
            || match self {
                Case::Exact => false,
                Case::Ascii => x.is_ascii_alphabetic() && c.eq_ignore_ascii_case(&x),
                Case::Unicode => {
                    let up = simple_upper(x);
                    let low = simple_lower(up);
                    up != low && (c == low || fold(c) == low)
                }
            }
    }

    /// Whether `c` is in `a..=b` (Java's `range`, `CIRange`, `CIRangeU`).
    fn range(self, a: char, b: char, c: char) -> bool {
        let inside = |c: char| a <= c && c <= b;
        inside(c)
            || match self {
                Case::Exact => false,
                Case::Ascii => c.is_ascii() && (inside(c.to_ascii_uppercase()) || inside(c.to_ascii_lowercase())),
                Case::Unicode => {
                    let up = simple_upper(c);
                    inside(up) || inside(simple_lower(up))
                }
            }
    }

    /// Whether `c` is a character class's `x` (Java's bits of a class: a Latin-1 character's cases within Latin-1;
    /// any other a `single`).
    fn class_char(self, x: char, c: char) -> bool {
        if (x as u32) >= 256 {
            return self.single(x, c);
        }
        c == x
            || match self {
                Case::Exact => false,
                _ if x.is_ascii() => c.is_ascii() && x.is_ascii_alphabetic() && c.eq_ignore_ascii_case(&x),
                Case::Ascii => false,
                Case::Unicode => (c as u32) < 256 && (c == simple_lower(x) || c == simple_upper(x)),
            }
    }

    /// Whether two texts' characters are the same (a back reference: Java's `BackRef`, `CIBackRef`).
    fn same(self, a: char, b: char) -> bool {
        a == b
            || match self {
                Case::Exact => false,
                Case::Ascii => a.is_ascii() && b.is_ascii() && a.eq_ignore_ascii_case(&b),
                Case::Unicode => fold(a) == fold(b),
            }
    }
}

/// Java's line terminators (`\n` alone in `UNIX_LINES`).
fn is_line_terminator(c: char, unix: bool) -> bool {
    if unix { c == '\n' } else { matches!(c, '\n' | '\r' | '\u{85}' | '\u{2028}' | '\u{2029}') }
}

/// The titlecase letters (Unicode's `Lt`, Java's `isTitleCase`).
fn is_title_case(c: char) -> bool {
    matches!(
        c,
        '\u{1C5}'
            | '\u{1C8}'
            | '\u{1CB}'
            | '\u{1F2}'
            | '\u{1F88}'..='\u{1F8F}'
            | '\u{1F98}'..='\u{1F9F}'
            | '\u{1FA8}'..='\u{1FAF}'
            | '\u{1FBC}'
            | '\u{1FCC}'
            | '\u{1FFC}'
    )
}

/// A named set of characters.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Builtin {
    /// `\d`: `[0-9]`.
    Digit,
    /// `\s`: `[ \t\n\x0B\f\r]`.
    Space,
    /// `\w`: `[a-zA-Z_0-9]`.
    Word,
    /// `\h`: horizontal white space.
    Horizontal,
    /// `\v`: vertical white space.
    Vertical,
    PosixLower,
    PosixUpper,
    Ascii,
    Alpha,
    Alnum,
    Punct,
    Graph,
    Print,
    Blank,
    Cntrl,
    XDigit,
    /// `\p{Lower}`/`\p{Upper}` without case: the ASCII letters.
    AsciiCased,
    JavaLowerCase,
    JavaUpperCase,
    JavaWhitespace,
    Alphabetic,
    WhiteSpace,
    /// `\p{javaLowerCase}`, `\p{IsUppercase}`… without case: lower, upper or title case.
    Cased,
}

impl Builtin {
    fn matches(self, c: char) -> bool {
        let punct = |c: char| c.is_ascii_punctuation();
        match self {
            Builtin::Digit => c.is_ascii_digit(),
            Builtin::Space => matches!(c, ' ' | '\t' | '\n' | '\u{B}' | '\u{C}' | '\r'),
            Builtin::Word => c.is_ascii_alphanumeric() || c == '_',
            Builtin::Horizontal => matches!(
                c,
                ' ' | '\t' | '\u{A0}' | '\u{1680}' | '\u{180E}' | '\u{2000}'
                    ..='\u{200A}' | '\u{202F}' | '\u{205F}' | '\u{3000}'
            ),
            Builtin::Vertical => matches!(c, '\n' | '\u{B}' | '\u{C}' | '\r' | '\u{85}' | '\u{2028}' | '\u{2029}'),
            Builtin::PosixLower => c.is_ascii_lowercase(),
            Builtin::PosixUpper => c.is_ascii_uppercase(),
            Builtin::Ascii => c.is_ascii(),
            Builtin::Alpha | Builtin::AsciiCased => c.is_ascii_alphabetic(),
            Builtin::Alnum => c.is_ascii_alphanumeric(),
            Builtin::Punct => punct(c),
            Builtin::Graph => c.is_ascii_graphic(),
            Builtin::Print => c.is_ascii_graphic() || c == ' ',
            Builtin::Blank => c == ' ' || c == '\t',
            Builtin::Cntrl => c.is_ascii_control(),
            Builtin::XDigit => c.is_ascii_hexdigit(),
            Builtin::JavaLowerCase => c.is_lowercase(),
            Builtin::JavaUpperCase => c.is_uppercase(),
            Builtin::JavaWhitespace => matches!(
                c,
                '\t' | '\n' | '\u{B}' | '\u{C}' | '\r' | '\u{1C}'..='\u{1F}' | ' ' | '\u{1680}' | '\u{2000}'..='\u{2006}'
                    | '\u{2008}'..='\u{200A}' | '\u{2028}' | '\u{2029}' | '\u{205F}' | '\u{3000}'
            ),
            Builtin::Alphabetic => c.is_alphabetic(),
            Builtin::WhiteSpace => c.is_whitespace(),
            Builtin::Cased => c.is_lowercase() || c.is_uppercase() || is_title_case(c),
        }
    }
}

/// What one character is tested by: a class's parts.
#[derive(Clone, Debug)]
enum Pred {
    /// A class's character.
    Char(char, Case),
    Range(char, char, Case),
    Builtin(Builtin),
    Not(Box<Pred>),
    Union(Vec<Pred>),
    And(Vec<Pred>),
}

impl Pred {
    fn matches(&self, c: char) -> bool {
        match self {
            Pred::Char(x, case) => case.class_char(*x, c),
            Pred::Range(a, b, case) => case.range(*a, *b, c),
            Pred::Builtin(b) => b.matches(c),
            Pred::Not(p) => !p.matches(c),
            Pred::Union(ps) => ps.iter().any(|p| p.matches(c)),
            Pred::And(ps) => ps.iter().all(|p| p.matches(c)),
        }
    }
}

// ---- The pattern's tree ----

#[derive(Clone, Copy, Debug, PartialEq)]
enum Assertion {
    /// `^` (not multiline), `\A`: the input's start.
    Begin,
    /// `^` multiline: the start, or after a line terminator (not at the input's end).
    Caret { unix: bool },
    /// `$` (multiline or not), `\Z`.
    Dollar { multiline: bool, unix: bool },
    /// `\z`.
    End,
    /// `\b` (`\B` negated).
    Bound { negated: bool },
    /// `\G`: where the search started.
    SearchStart,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Mode {
    Greedy,
    Lazy,
}

#[derive(Clone, Debug)]
enum Node {
    Empty,
    Char(char, Case),
    /// `.`.
    Dot {
        all: bool,
        unix: bool,
    },
    Pred(Pred),
    Assert(Assertion),
    Concat(Vec<Node>),
    Alt(Vec<Node>),
    /// A group: capturing (its number, 1-based) or not.
    Group(Box<Node>, Option<usize>),
    Repeat {
        node: Box<Node>,
        min: usize,
        max: Option<usize>,
        mode: Mode,
    },
    BackRef(usize, Case),
    Look {
        node: Box<Node>,
        behind: bool,
        negative: bool,
        min: usize,
        max: usize,
    },
    Atomic(Box<Node>),
}

impl Node {
    /// The fewest and the most characters it matches (`None`: no most).
    fn lengths(&self) -> (usize, Option<usize>) {
        match self {
            Node::Empty | Node::Assert(_) | Node::Look { .. } => (0, Some(0)),
            Node::Char(..) | Node::Dot { .. } | Node::Pred(_) => (1, Some(1)),
            Node::Concat(items) => items.iter().fold((0, Some(0)), |(min, max), n| {
                let (a, b) = n.lengths();
                (min + a, max.zip(b).map(|(x, y)| x + y))
            }),
            Node::Alt(branches) => {
                let all: Vec<_> = branches.iter().map(Node::lengths).collect();
                let min = all.iter().map(|l| l.0).min().unwrap_or(0);
                let max = all.iter().try_fold(0, |m, l| l.1.map(|b| m.max(b)));
                (min, max)
            }
            Node::Group(n, _) | Node::Atomic(n) => n.lengths(),
            Node::Repeat { node, min, max, .. } => {
                let (a, b) = node.lengths();
                (
                    a * min,
                    match (b, max) {
                        (Some(0), _) => Some(0),
                        (Some(b), Some(m)) => Some(b * m),
                        _ => None,
                    },
                )
            }
            Node::BackRef(..) => (0, None),
        }
    }
}

// ---- Parsing ----

/// Java's `RemoveQEQuoting`: each `\Q…\E` made escapes (quoted letters as they are, digits `\x3n`, any other ASCII
/// character escaped, others as they are).
fn remove_qe_quoting(p: &[char]) -> Vec<char> {
    let mut out = Vec::with_capacity(p.len());
    let mut i = 0;
    while i < p.len() {
        if p[i] == '\\' && p.get(i + 1) == Some(&'Q') {
            i += 2;
            while i < p.len() {
                if p[i] == '\\' && p.get(i + 1) == Some(&'E') {
                    i += 2;
                    break;
                }
                let c = p[i];
                if !c.is_ascii() || c.is_ascii_alphabetic() {
                    out.push(c);
                } else if c.is_ascii_digit() {
                    out.extend(['\\', 'x', '3', c]);
                } else {
                    out.extend(['\\', c]);
                }
                i += 1;
            }
        } else if p[i] == '\\' && i + 1 < p.len() {
            out.extend([p[i], p[i + 1]]);
            i += 2;
        } else {
            out.push(p[i]);
            i += 1;
        }
    }
    out
}

/// What an escape is.
enum Escape {
    Char(char),
    Pred(Pred),
    Node(Node),
}

struct Parser<'p> {
    chars: Vec<char>,
    pos: usize,
    flags: u32,
    groups: usize,
    names: Vec<(String, usize)>,
    max_back_ref: usize,
    pattern: &'p str,
}

impl Parser<'_> {
    /// Java's `PatternSyntaxException`: what, where, the pattern and a caret under the place.
    #[track_caller]
    fn error(&self, what: &str) -> ! {
        let index = self.pos.saturating_sub(1);
        let caret =
            if index < self.pattern.chars().count() { format!("\n{}^", " ".repeat(index)) } else { String::new() };
        panic_kt("PatternSyntaxException", &format!("{what} near index {index}\n{}{caret}", self.pattern))
    }

    fn case(&self) -> Case {
        Case::of(self.flags)
    }

    /// The next character, past white space and comments in `COMMENTS` mode.
    fn peek(&mut self) -> Option<char> {
        if self.flags & COMMENTS != 0 {
            loop {
                match self.chars.get(self.pos) {
                    Some(' ' | '\t' | '\n' | '\u{B}' | '\u{C}' | '\r') => self.pos += 1,
                    Some('#') => {
                        while self.chars.get(self.pos).is_some_and(|c| !is_line_terminator(*c, false)) {
                            self.pos += 1;
                        }
                    }
                    _ => break,
                }
            }
        }
        self.chars.get(self.pos).copied()
    }

    fn eat(&mut self, c: char) -> bool {
        if self.peek() == Some(c) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    /// The next character as it is (after a `\`).
    fn raw(&mut self) -> Option<char> {
        let c = self.chars.get(self.pos).copied();
        if c.is_some() {
            self.pos += 1;
        }
        c
    }

    fn expr(&mut self) -> Node {
        let mut branches = vec![self.sequence()];
        while self.eat('|') {
            branches.push(self.sequence());
        }
        if branches.len() == 1 { branches.pop().expect("a branch") } else { Node::Alt(branches) }
    }

    fn sequence(&mut self) -> Node {
        let mut items = Vec::new();
        while let Some(c) = self.peek() {
            let atom = match c {
                '|' | ')' => break,
                '(' => {
                    self.pos += 1;
                    match self.group() {
                        Some(node) => node,
                        None => continue,
                    }
                }
                '[' => {
                    self.pos += 1;
                    Node::Pred(self.class())
                }
                '\\' => {
                    self.pos += 1;
                    match self.escape(false) {
                        Escape::Char(c) => Node::Char(c, self.case()),
                        Escape::Pred(p) => Node::Pred(p),
                        Escape::Node(n) => n,
                    }
                }
                '^' => {
                    self.pos += 1;
                    if self.flags & MULTILINE != 0 {
                        Node::Assert(Assertion::Caret { unix: self.flags & UNIX_LINES != 0 })
                    } else {
                        Node::Assert(Assertion::Begin)
                    }
                }
                '$' => {
                    self.pos += 1;
                    Node::Assert(Assertion::Dollar {
                        multiline: self.flags & MULTILINE != 0,
                        unix: self.flags & UNIX_LINES != 0,
                    })
                }
                '.' => {
                    self.pos += 1;
                    Node::Dot { all: self.flags & DOTALL != 0, unix: self.flags & UNIX_LINES != 0 }
                }
                '*' | '+' | '?' | '{' => {
                    self.pos += 1;
                    self.error(&format!("Dangling meta character '{c}'"))
                }
                c => {
                    self.pos += 1;
                    Node::Char(c, self.case())
                }
            };
            let node = self.closure(atom);
            items.push(node);
        }
        match items.len() {
            0 => Node::Empty,
            1 => items.pop().expect("an item"),
            _ => Node::Concat(items),
        }
    }

    /// A number of a counted repetition.
    fn count(&mut self) -> Option<usize> {
        let mut n: Option<usize> = None;
        while let Some(d) = self.chars.get(self.pos).and_then(|c| c.to_digit(10)) {
            self.pos += 1;
            n = Some(n.unwrap_or(0).checked_mul(10).and_then(|n| n.checked_add(d as usize)).unwrap_or_else(|| {
                self.error("Illegal repetition range");
            }));
        }
        n
    }

    /// The quantifier after an atom, if one is there.
    fn closure(&mut self, atom: Node) -> Node {
        let (min, max) = match self.peek() {
            Some('?') => (0, Some(1)),
            Some('*') => (0, None),
            Some('+') => (1, None),
            Some('{') => {
                if !self.chars.get(self.pos + 1).is_some_and(char::is_ascii_digit) {
                    self.pos += 1;
                    self.error("Illegal repetition");
                }
                self.pos += 1;
                let min = self.count().expect("a digit");
                let max = if self.chars.get(self.pos) == Some(&',') {
                    self.pos += 1;
                    self.count()
                } else {
                    Some(min)
                };
                if self.chars.get(self.pos) != Some(&'}') {
                    self.pos += 1;
                    self.error("Unclosed counted closure");
                }
                if max.is_some_and(|m| m < min) {
                    self.pos += 1;
                    self.error("Illegal repetition range");
                }
                (min, max)
            }
            _ => return atom,
        };
        self.pos += 1;
        let repeat = |mode| Node::Repeat { node: Box::new(atom), min, max, mode };
        if self.eat('?') {
            repeat(Mode::Lazy)
        } else if self.eat('+') {
            Node::Atomic(Box::new(repeat(Mode::Greedy)))
        } else {
            repeat(Mode::Greedy)
        }
    }

    /// After a `(`: a group, or `None` for flags alone (`(?i)`, which stay set to the enclosing group's end).
    fn group(&mut self) -> Option<Node> {
        let saved = self.flags;
        let node = if self.chars.get(self.pos) == Some(&'?') {
            self.pos += 1;
            let c = self.raw().unwrap_or_else(|| self.error("Unknown inline modifier"));
            match c {
                ':' => Node::Group(Box::new(self.expr()), None),
                '=' | '!' => {
                    let inner = self.expr();
                    Node::Look { node: Box::new(inner), behind: false, negative: c == '!', min: 0, max: 0 }
                }
                '>' => Node::Atomic(Box::new(self.expr())),
                '<' => {
                    let d = self.raw().unwrap_or_else(|| self.error("Unknown look-behind group"));
                    if d == '=' || d == '!' {
                        let inner = self.expr();
                        let (min, max) = inner.lengths();
                        let Some(max) = max else {
                            self.error("Look-behind group does not have an obvious maximum length")
                        };
                        Node::Look { node: Box::new(inner), behind: true, negative: d == '!', min, max }
                    } else {
                        self.pos -= 1;
                        let name = self.group_name();
                        if self.names.iter().any(|(n, _)| *n == name) {
                            self.error(&format!("Named capturing group <{name}> is already defined"));
                        }
                        self.groups += 1;
                        let number = self.groups;
                        self.names.push((name, number));
                        Node::Group(Box::new(self.expr()), Some(number))
                    }
                }
                '$' | '@' => self.error("Unknown group type"),
                _ => {
                    self.pos -= 1;
                    self.inline_flags();
                    match self.raw() {
                        Some(')') => return None,
                        Some(':') => Node::Group(Box::new(self.expr()), None),
                        _ => self.error("Unknown inline modifier"),
                    }
                }
            }
        } else {
            self.groups += 1;
            let number = self.groups;
            Node::Group(Box::new(self.expr()), Some(number))
        };
        if !self.eat(')') {
            self.pos += 1;
            self.error("Unclosed group");
        }
        self.flags = saved;
        Some(node)
    }

    /// A named group's name and its `>` (Java's `groupname`).
    fn group_name(&mut self) -> String {
        let mut name = String::new();
        if !self.chars.get(self.pos).is_some_and(char::is_ascii_alphabetic) {
            self.pos += 1;
            self.error("capturing group name does not start with a Latin letter");
        }
        while let Some(c) = self.chars.get(self.pos).copied().filter(char::is_ascii_alphanumeric) {
            name.push(c);
            self.pos += 1;
        }
        if self.raw() != Some('>') {
            self.error("named capturing group is missing trailing '>'");
        }
        name
    }

    /// `imsdux` on, then after a `-` off (Java's `addFlag`/`subFlag`).
    fn inline_flags(&mut self) {
        let mut on = true;
        while let Some(c) = self.chars.get(self.pos).copied() {
            let flag = match c {
                'i' => CASE_INSENSITIVE,
                'm' => MULTILINE,
                's' => DOTALL,
                'd' => UNIX_LINES,
                'u' => UNICODE_CASE,
                'x' => COMMENTS,
                'c' | 'U' => {
                    self.pos += 1;
                    self.error(&format!("Inline flag '{c}' (CANON_EQ, UNICODE_CHARACTER_CLASS) isn't supported"))
                }
                '-' if on => {
                    on = false;
                    self.pos += 1;
                    continue;
                }
                _ => return,
            };
            if on {
                self.flags |= flag;
            } else {
                self.flags &= !flag;
            }
            self.pos += 1;
        }
    }

    /// After a `[`: the class up to its `]` (Java's `clazz`): `^` negating all of it, `&&` intersecting.
    fn class(&mut self) -> Pred {
        let negated = self.chars.get(self.pos) == Some(&'^');
        if negated {
            self.pos += 1;
        }
        let mut operands: Vec<Pred> = Vec::new();
        let mut items: Vec<Pred> = Vec::new();
        let mut first = true;
        loop {
            let c = self.peek().unwrap_or_else(|| self.error("Unclosed character class"));
            match c {
                ']' if !first => {
                    self.pos += 1;
                    break;
                }
                '[' => {
                    self.pos += 1;
                    items.push(self.class());
                }
                '&' if self.chars.get(self.pos + 1) == Some(&'&') => {
                    self.pos += 2;
                    if !items.is_empty() {
                        operands.push(Pred::Union(std::mem::take(&mut items)));
                    }
                }
                _ => {
                    let item = self.class_item();
                    items.push(item);
                }
            }
            first = false;
        }
        if !items.is_empty() {
            operands.push(Pred::Union(items));
        }
        let pred = match operands.len() {
            0 => Pred::Union(Vec::new()),
            1 => operands.pop().expect("an operand"),
            _ => Pred::And(operands),
        };
        if negated { Pred::Not(Box::new(pred)) } else { pred }
    }

    /// One character, a range or an escaped set in a class.
    fn class_item(&mut self) -> Pred {
        let start = match self.class_char() {
            Ok(c) => c,
            Err(pred) => return pred,
        };
        if self.peek() == Some('-') {
            let after = self.chars.get(self.pos + 1).copied();
            if after != Some(']') && after != Some('[') && after.is_some() {
                self.pos += 1;
                let end = match self.class_char() {
                    Ok(c) => c,
                    Err(_) => self.error("Illegal character range"),
                };
                if end < start {
                    self.error("Illegal character range");
                }
                return Pred::Range(start, end, self.case());
            }
        }
        Pred::Char(start, self.case())
    }

    /// A class's character (`Err`: an escaped set).
    fn class_char(&mut self) -> Result<char, Pred> {
        let c = self.peek().unwrap_or_else(|| self.error("Unclosed character class"));
        self.pos += 1;
        if c != '\\' {
            return Ok(c);
        }
        match self.escape(true) {
            Escape::Char(c) => Ok(c),
            Escape::Pred(p) => Err(p),
            Escape::Node(_) => self.error("Illegal/unsupported escape sequence"),
        }
    }

    /// After a `\` (Java's `escape`).
    fn escape(&mut self, in_class: bool) -> Escape {
        let c = self.raw().unwrap_or_else(|| self.error("Unexpected internal error"));
        let builtin = |b| Escape::Pred(Pred::Builtin(b));
        let not = |b| Escape::Pred(Pred::Not(Box::new(Pred::Builtin(b))));
        let unix = self.flags & UNIX_LINES != 0;
        let outside = |p: &Self, node: Node| {
            if in_class {
                p.error("Illegal/unsupported escape sequence");
            }
            Escape::Node(node)
        };
        match c {
            '0' => Escape::Char(self.octal()),
            '1'..='9' => {
                if in_class {
                    self.error("Illegal/unsupported escape sequence");
                }
                let mut number = c.to_digit(10).expect("a digit") as usize;
                while let Some(d) = self.chars.get(self.pos).and_then(|c| c.to_digit(10)) {
                    let more = number * 10 + d as usize;
                    if more > self.groups {
                        break;
                    }
                    number = more;
                    self.pos += 1;
                }
                self.max_back_ref = self.max_back_ref.max(number);
                Escape::Node(Node::BackRef(number, self.case()))
            }
            'A' => outside(self, Node::Assert(Assertion::Begin)),
            'B' => outside(self, Node::Assert(Assertion::Bound { negated: true })),
            'D' => not(Builtin::Digit),
            'G' => outside(self, Node::Assert(Assertion::SearchStart)),
            'H' => not(Builtin::Horizontal),
            'P' | 'p' => {
                let pred = self.property();
                Escape::Pred(if c == 'P' { Pred::Not(Box::new(pred)) } else { pred })
            }
            'R' => {
                let breaks = Pred::Union(vec![
                    Pred::Range('\n', '\r', Case::Exact),
                    Pred::Char('\u{85}', Case::Exact),
                    Pred::Range('\u{2028}', '\u{2029}', Case::Exact),
                ]);
                let crlf = Node::Concat(vec![Node::Char('\r', Case::Exact), Node::Char('\n', Case::Exact)]);
                outside(self, Node::Atomic(Box::new(Node::Alt(vec![crlf, Node::Pred(breaks)]))))
            }
            'S' => not(Builtin::Space),
            'V' => not(Builtin::Vertical),
            'W' => not(Builtin::Word),
            'Z' => outside(self, Node::Assert(Assertion::Dollar { multiline: false, unix })),
            'a' => Escape::Char('\u{7}'),
            'b' => {
                if self.chars.get(self.pos) == Some(&'{') {
                    self.error("\\b{g} (grapheme cluster boundaries) isn't supported");
                }
                outside(self, Node::Assert(Assertion::Bound { negated: false }))
            }
            'c' => {
                let x = self.raw().unwrap_or_else(|| self.error("Illegal control escape sequence"));
                Escape::Char(
                    char::from_u32(x as u32 ^ 64).unwrap_or_else(|| self.error("Illegal control escape sequence")),
                )
            }
            'd' => builtin(Builtin::Digit),
            'e' => Escape::Char('\u{1B}'),
            'f' => Escape::Char('\u{C}'),
            'h' => builtin(Builtin::Horizontal),
            'k' => {
                if in_class {
                    self.error("Illegal/unsupported escape sequence");
                }
                if self.raw() != Some('<') {
                    self.error("\\k is not followed by '<' for named capturing group");
                }
                let name = self.group_name();
                let Some(number) = self.names.iter().find(|(n, _)| *n == name).map(|(_, g)| *g) else {
                    self.error(&format!("named capturing group <{name}> does not exist"))
                };
                Escape::Node(Node::BackRef(number, self.case()))
            }
            'n' => Escape::Char('\n'),
            'r' => Escape::Char('\r'),
            's' => builtin(Builtin::Space),
            't' => Escape::Char('\t'),
            'u' => Escape::Char(self.unicode()),
            'v' => builtin(Builtin::Vertical),
            'w' => builtin(Builtin::Word),
            'x' => Escape::Char(self.hex()),
            'z' => outside(self, Node::Assert(Assertion::End)),
            'N' => self.error("\\N{name} (characters by their names) isn't supported"),
            'X' => self.error("\\X (grapheme clusters) isn't supported"),
            c if c.is_ascii_alphabetic() => self.error("Illegal/unsupported escape sequence"),
            c => Escape::Char(c),
        }
    }

    /// `\0n`, `\0nn`, `\0mnn` (m ≤ 3).
    fn octal(&mut self) -> char {
        let digit = |p: &Self, at: usize| p.chars.get(at).and_then(|c| c.to_digit(8));
        let Some(n) = digit(self, self.pos) else { self.error("Illegal octal escape sequence") };
        self.pos += 1;
        let Some(m) = digit(self, self.pos) else { return char::from_u32(n).expect("an octal") };
        self.pos += 1;
        match digit(self, self.pos) {
            Some(o) if n <= 3 => {
                self.pos += 1;
                char::from_u32(n * 64 + m * 8 + o).expect("an octal")
            }
            _ => char::from_u32(n * 8 + m).expect("an octal"),
        }
    }

    fn hex_digits(&mut self, count: usize) -> Option<u32> {
        let mut v = 0;
        for _ in 0..count {
            v = v * 16 + self.chars.get(self.pos)?.to_digit(16)?;
            self.pos += 1;
        }
        Some(v)
    }

    /// `\xhh`, `\x{h…}`.
    fn hex(&mut self) -> char {
        if self.chars.get(self.pos) == Some(&'{') {
            self.pos += 1;
            let mut v: u32 = 0;
            let mut any = false;
            while let Some(d) = self.chars.get(self.pos).and_then(|c| c.to_digit(16)) {
                v = v.saturating_mul(16).saturating_add(d);
                any = true;
                self.pos += 1;
            }
            if !any || self.raw() != Some('}') || v > 0x10FFFF {
                self.error("Illegal hexadecimal escape sequence");
            }
            return char::from_u32(v).unwrap_or_else(|| self.error("Unpaired surrogates aren't supported"));
        }
        let v = self.hex_digits(2).unwrap_or_else(|| self.error("Illegal hexadecimal escape sequence"));
        char::from_u32(v).expect("a Latin-1 character")
    }

    /// `\uhhhh`, a surrogate pair's two escapes one character.
    fn unicode(&mut self) -> char {
        let v = self.hex_digits(4).unwrap_or_else(|| self.error("Illegal Unicode escape sequence"));
        if (0xD800..0xDC00).contains(&v)
            && self.chars.get(self.pos) == Some(&'\\')
            && self.chars.get(self.pos + 1) == Some(&'u')
        {
            let at = self.pos;
            self.pos += 2;
            match self.hex_digits(4) {
                Some(low) if (0xDC00..0xE000).contains(&low) => {
                    return char::from_u32(0x10000 + ((v - 0xD800) << 10) + (low - 0xDC00)).expect("a pair");
                }
                _ => self.pos = at,
            }
        }
        char::from_u32(v).unwrap_or_else(|| self.error("Unpaired surrogates aren't supported"))
    }

    /// After `\p` or `\P`: `{name}` or a one-letter name.
    fn property(&mut self) -> Pred {
        let name = if self.chars.get(self.pos) == Some(&'{') {
            self.pos += 1;
            let mut name = String::new();
            loop {
                match self.raw() {
                    Some('}') => break,
                    Some(c) => name.push(c),
                    None => self.error("Unclosed character family"),
                }
            }
            name
        } else {
            self.raw().unwrap_or_else(|| self.error("Illegal character family")).to_string()
        };
        let ci = self.flags & CASE_INSENSITIVE != 0;
        let builtin = match name.as_str() {
            "Lower" | "Upper" if ci => Builtin::AsciiCased,
            "Lower" => Builtin::PosixLower,
            "Upper" => Builtin::PosixUpper,
            "ASCII" => Builtin::Ascii,
            "Alpha" => Builtin::Alpha,
            "Digit" => Builtin::Digit,
            "Alnum" => Builtin::Alnum,
            "Punct" => Builtin::Punct,
            "Graph" => Builtin::Graph,
            "Print" => Builtin::Print,
            "Blank" => Builtin::Blank,
            "Cntrl" => Builtin::Cntrl,
            "XDigit" => Builtin::XDigit,
            "Space" => Builtin::Space,
            "javaLowerCase" | "javaUpperCase" | "IsUppercase" | "IsLowercase" if ci => Builtin::Cased,
            "javaLowerCase" | "IsLowercase" => Builtin::JavaLowerCase,
            "javaUpperCase" | "IsUppercase" => Builtin::JavaUpperCase,
            "javaWhitespace" => Builtin::JavaWhitespace,
            "IsAlphabetic" => Builtin::Alphabetic,
            "IsWhite_Space" | "IsWhiteSpace" => Builtin::WhiteSpace,
            _ => self.error(&format!(
                "Unsupported character property name {{{name}}} (KompozR's Regex reads the POSIX classes, \
                 javaLowerCase, javaUpperCase, javaWhitespace, IsAlphabetic, IsWhite_Space, IsUppercase and \
                 IsLowercase)"
            )),
        };
        Pred::Builtin(builtin)
    }
}

/// A compiled pattern: its tree, its groups.
#[derive(Debug)]
struct Program {
    node: Node,
    groups: usize,
    names: Vec<(String, usize)>,
}

fn compile(pattern: &str, flags: u32) -> Program {
    let chars: Vec<char> = pattern.chars().collect();
    if flags & LITERAL != 0 {
        let case = Case::of(flags);
        let node = Node::Concat(chars.into_iter().map(|c| Node::Char(c, case)).collect());
        return Program { node, groups: 0, names: Vec::new() };
    }
    let mut p = Parser {
        chars: remove_qe_quoting(&chars),
        pos: 0,
        flags,
        groups: 0,
        names: Vec::new(),
        max_back_ref: 0,
        pattern,
    };
    let node = p.expr();
    if p.pos != p.chars.len() {
        p.pos += 1;
        if p.chars[p.pos - 1] == ')' {
            p.error("Unmatched closing ')'");
        }
        p.error("Unexpected internal error");
    }
    if p.max_back_ref > p.groups {
        p.error(&format!("No such group {}", p.max_back_ref));
    }
    Program { node, groups: p.groups, names: p.names }
}

// ---- Matching ----

type Captures = Vec<Option<(usize, usize)>>;
type Next<'k> = &'k mut dyn FnMut(usize, &mut Captures) -> bool;

/// A text as the matcher reads it: its characters, each one's byte offset and UTF-16 index (and the end's).
struct Text {
    input: String,
    chars: Vec<char>,
    bytes: Vec<usize>,
    utf16: Vec<usize>,
}

impl Text {
    fn new(input: &str) -> Text {
        let chars: Vec<char> = input.chars().collect();
        let mut bytes = Vec::with_capacity(chars.len() + 1);
        let mut utf16 = Vec::with_capacity(chars.len() + 1);
        let (mut b, mut u) = (0, 0);
        for c in &chars {
            bytes.push(b);
            utf16.push(u);
            b += c.len_utf8();
            u += c.len_utf16();
        }
        bytes.push(b);
        utf16.push(u);
        Text { input: input.to_string(), chars, bytes, utf16 }
    }

    /// The UTF-16 length (Kotlin's `length`).
    fn length(&self) -> usize {
        self.utf16[self.chars.len()]
    }

    /// The character at or after UTF-16 index `index`.
    fn char_at_utf16(&self, index: usize) -> usize {
        self.utf16.partition_point(|u| *u < index)
    }

    fn slice(&self, from: usize, to: usize) -> &str {
        &self.input[self.bytes[from]..self.bytes[to]]
    }
}

struct Matcher<'t> {
    chars: &'t [char],
    /// Where the search started (what `\G` matches at).
    search_start: usize,
}

fn is_word(c: char) -> bool {
    c == '_' || c.is_alphanumeric()
}

impl Matcher<'_> {
    fn assert(&self, a: Assertion, i: usize) -> bool {
        let n = self.chars.len();
        let at = |j: usize| self.chars[j];
        match a {
            Assertion::Begin => i == 0,
            Assertion::End => i == n,
            Assertion::SearchStart => i == self.search_start,
            Assertion::Caret { unix } => {
                if i == n {
                    return false;
                }
                if i > 0 {
                    let before = at(i - 1);
                    if !is_line_terminator(before, unix) {
                        return false;
                    }
                    if !unix && before == '\r' && at(i) == '\n' {
                        return false;
                    }
                }
                true
            }
            Assertion::Dollar { multiline, unix } => {
                if unix {
                    if i < n {
                        if at(i) != '\n' {
                            return false;
                        }
                        if !multiline && i != n - 1 {
                            return false;
                        }
                    }
                    return true;
                }
                if !multiline {
                    if i + 2 < n {
                        return false;
                    }
                    if i + 2 == n {
                        return at(i) == '\r' && at(i + 1) == '\n';
                    }
                }
                if i < n {
                    let c = at(i);
                    if c == '\n' {
                        if i > 0 && at(i - 1) == '\r' {
                            return false;
                        }
                    } else if !is_line_terminator(c, false) {
                        return false;
                    }
                }
                true
            }
            Assertion::Bound { negated } => {
                let left = i > 0 && is_word(at(i - 1));
                let right = i < n && is_word(at(i));
                (left != right) != negated
            }
        }
    }

    /// Whether `node` matches at character `i` with `k` matching the rest from where it ends.
    fn m(&self, node: &Node, i: usize, caps: &mut Captures, k: Next) -> bool {
        let n = self.chars.len();
        match node {
            Node::Empty => k(i, caps),
            Node::Char(c, case) => i < n && case.single(*c, self.chars[i]) && k(i + 1, caps),
            Node::Dot { all, unix } => i < n && (*all || !is_line_terminator(self.chars[i], *unix)) && k(i + 1, caps),
            Node::Pred(p) => i < n && p.matches(self.chars[i]) && k(i + 1, caps),
            Node::Assert(a) => self.assert(*a, i) && k(i, caps),
            Node::Concat(items) => self.concat(items, i, caps, k),
            Node::Alt(branches) => {
                for b in branches {
                    let saved = caps.clone();
                    if self.m(b, i, caps, k) {
                        return true;
                    }
                    *caps = saved;
                }
                false
            }
            Node::Group(inner, number) => {
                let start = i;
                let number = *number;
                self.m(inner, i, caps, &mut |end, caps: &mut Captures| {
                    let previous = number.map(|n| caps[n - 1]);
                    if let Some(n) = number {
                        caps[n - 1] = Some((start, end));
                    }
                    if k(end, caps) {
                        return true;
                    }
                    if let (Some(n), Some(p)) = (number, previous) {
                        caps[n - 1] = p;
                    }
                    false
                })
            }
            Node::Repeat { node, min, max, mode } => self.repeat(node, (*min, *max, *mode), 0, i, caps, k),
            Node::BackRef(group, case) => {
                let Some((s, e)) = caps[group - 1] else { return false };
                let len = e - s;
                i + len <= n && (0..len).all(|j| case.same(self.chars[s + j], self.chars[i + j])) && k(i + len, caps)
            }
            Node::Look { node, behind: false, negative, .. } => {
                if *negative {
                    let mut scratch = caps.clone();
                    !self.m(node, i, &mut scratch, &mut |_, _| true) && k(i, caps)
                } else {
                    let saved = caps.clone();
                    if self.m(node, i, caps, &mut |_, _| true) && k(i, caps) {
                        return true;
                    }
                    *caps = saved;
                    false
                }
            }
            Node::Look { node, behind: true, negative, min, max } => {
                let saved = caps.clone();
                let mut found = false;
                if i >= *min {
                    let lowest = i.saturating_sub(*max);
                    for j in (lowest..=i - min).rev() {
                        if self.m(node, j, caps, &mut |end, _| end == i) {
                            found = true;
                            break;
                        }
                        *caps = saved.clone();
                    }
                }
                if *negative {
                    *caps = saved;
                    !found && k(i, caps)
                } else {
                    if found && k(i, caps) {
                        return true;
                    }
                    *caps = saved;
                    false
                }
            }
            Node::Atomic(inner) => {
                let mut taken = caps.clone();
                let mut end = None;
                if !self.m(inner, i, &mut taken, &mut |j, _| {
                    end = Some(j);
                    true
                }) {
                    return false;
                }
                let saved = std::mem::replace(caps, taken);
                if k(end.expect("an end"), caps) {
                    return true;
                }
                *caps = saved;
                false
            }
        }
    }

    fn concat(&self, items: &[Node], i: usize, caps: &mut Captures, k: Next) -> bool {
        match items.split_first() {
            None => k(i, caps),
            Some((first, rest)) => self.m(first, i, caps, &mut |j, caps: &mut Captures| self.concat(rest, j, caps, k)),
        }
    }

    /// One more iteration of a repetition (`count` done), as Java's loops: an iteration matching nothing ends it.
    fn iterate(
        &self,
        node: &Node,
        bounds: (usize, Option<usize>, Mode),
        count: usize,
        i: usize,
        caps: &mut Captures,
        k: Next,
    ) -> bool {
        self.m(node, i, caps, &mut |j, caps: &mut Captures| {
            if j == i {
                return k(j, caps);
            }
            self.repeat(node, bounds, count + 1, j, caps, k)
        })
    }

    fn repeat(
        &self,
        node: &Node,
        bounds: (usize, Option<usize>, Mode),
        count: usize,
        i: usize,
        caps: &mut Captures,
        k: Next,
    ) -> bool {
        let (min, max, mode) = bounds;
        if count < min {
            return self.iterate(node, bounds, count, i, caps, k);
        }
        let more = max.is_none_or(|m| count < m);
        let saved = caps.clone();
        match mode {
            Mode::Greedy => {
                if more && self.iterate(node, bounds, count, i, caps, k) {
                    return true;
                }
                *caps = saved;
                k(i, caps)
            }
            Mode::Lazy => {
                if k(i, caps) {
                    return true;
                }
                *caps = saved;
                more && self.iterate(node, bounds, count, i, caps, k)
            }
        }
    }
}

// ---- Regex ----

/// Kotlin's `Regex` (Kotlin/JVM's: a `java.util.regex.Pattern`): a compiled pattern, shared by its clones.
///
/// # Panics
/// Making one of a pattern this doesn't read panics, as Java's `PatternSyntaxException` (see the module).
#[derive(Clone)]
pub struct Regex {
    pattern: Arc<str>,
    flags: u32,
    program: Arc<Program>,
}

/// Kotlin's `Regex(pattern)`.
/// Kotlin: `Regex(pattern: String): Regex`
#[allow(non_snake_case)]
#[track_caller]
pub fn Regex(pattern: &str) -> Regex {
    Regex::new(pattern)
}

impl fmt::Debug for Regex {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Regex({:?})", self.pattern)
    }
}

/// Its text is its pattern (Kotlin's `toString()`).
impl fmt::Display for Regex {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.pattern)
    }
}

impl KtToString for Regex {
    fn kt_write(&self, out: &mut String) {
        out.push_str(&self.pattern);
    }
}

#[track_caller]
fn check_start(start_index: i32, text: &Text) -> usize {
    if start_index < 0 || start_index as usize > text.length() {
        panic_kt(
            "IndexOutOfBoundsException",
            &format!("Start index out of bounds: {start_index}, input length: {}", text.length()),
        );
    }
    text.char_at_utf16(start_index as usize)
}

impl Regex {
    /// `pattern` compiled (Kotlin's `Regex(pattern)`).
    /// Kotlin: `Regex(pattern: String)`
    #[track_caller]
    pub fn new(pattern: &str) -> Regex {
        Regex::with_flags(pattern, 0)
    }

    /// `pattern` compiled with `option` (Kotlin's `Regex(pattern, option)`).
    /// Kotlin: `Regex(pattern: String, option: RegexOption)`
    #[track_caller]
    pub fn with_option(pattern: &str, option: RegexOption) -> Regex {
        Regex::with_options(pattern, [option])
    }

    /// `pattern` compiled with `options` (Kotlin's `Regex(pattern, options)`).
    /// Kotlin: `Regex(pattern: String, options: Set<RegexOption>)`
    #[track_caller]
    pub fn with_options(pattern: &str, options: impl IntoIterator<Item = RegexOption>) -> Regex {
        let mut flags = options.into_iter().fold(0, |f, o| f | o.value() as u32);
        // (Kotlin/JVM's ensureUnicodeCase: IGNORE_CASE is Unicode's)
        if flags & CASE_INSENSITIVE != 0 {
            flags |= UNICODE_CASE;
        }
        Regex::with_flags(pattern, flags)
    }

    #[track_caller]
    fn with_flags(pattern: &str, flags: u32) -> Regex {
        Regex { pattern: pattern.into(), flags, program: Arc::new(compile(pattern, flags)) }
    }

    /// A regex matching `literal` as it is (Kotlin's `Regex.fromLiteral`: the `LITERAL` option).
    /// Kotlin: `Regex.fromLiteral(literal: String): Regex`
    pub fn from_literal(literal: &str) -> Regex {
        Regex::with_flags(literal, LITERAL)
    }

    /// A pattern matching `literal` as it is (Kotlin/JVM's `Regex.escape`: Java's `Pattern.quote`, a `\Q…\E`
    /// quoting, its own `\E`s escaped).
    /// Kotlin: `Regex.escape(literal: String): String`
    pub fn escape(literal: &str) -> String {
        if !literal.contains("\\E") {
            return format!("\\Q{literal}\\E");
        }
        let mut out = String::from("\\Q");
        let mut rest = literal;
        while let Some(at) = rest.find("\\E") {
            out.push_str(&rest[..at]);
            out.push_str("\\E\\\\E\\Q");
            rest = &rest[at + 2..];
        }
        out.push_str(rest);
        out.push_str("\\E");
        out
    }

    /// A replacement giving `literal` as it is (Kotlin/JVM's `Regex.escapeReplacement`: Java's
    /// `Matcher.quoteReplacement`, each `\` and `$` escaped).
    /// Kotlin: `Regex.escapeReplacement(literal: String): String`
    pub fn escape_replacement(literal: &str) -> String {
        let mut out = String::with_capacity(literal.len());
        for c in literal.chars() {
            if c == '\\' || c == '$' {
                out.push('\\');
            }
            out.push(c);
        }
        out
    }

    /// Kotlin: `pattern: String`
    pub fn pattern(&self) -> &str {
        &self.pattern
    }

    /// The options it was made with (Kotlin's `options`, a set).
    /// Kotlin: `options: Set<RegexOption>`
    pub fn options(&self) -> Vec<RegexOption> {
        RegexOption::ALL.into_iter().filter(|o| self.flags & o.value() as u32 != 0).collect()
    }

    fn match_at(&self, text: &Arc<Text>, start: usize, search_start: usize, entire: bool) -> Option<MatchResult> {
        let matcher = Matcher { chars: &text.chars, search_start };
        let mut caps: Captures = vec![None; self.program.groups];
        let n = text.chars.len();
        let mut end = None;
        let found = matcher.m(&self.program.node, start, &mut caps, &mut |j, _| {
            if entire && j != n {
                return false;
            }
            end = Some(j);
            true
        });
        found.then(|| MatchResult {
            regex: self.clone(),
            text: text.clone(),
            range: (start, end.expect("an end")),
            caps,
        })
    }

    /// The first match from character `from` on, `\G` at `search_start`.
    fn search(&self, text: &Arc<Text>, from: usize, search_start: usize) -> Option<MatchResult> {
        (from..=text.chars.len()).find_map(|i| self.match_at(text, i, search_start, false))
    }

    /// Whether all of `input` matches (Kotlin's `matches`; Java's `Matcher.matches`).
    /// Kotlin: `infix fun matches(input: CharSequence): Boolean`
    pub fn matches(&self, input: &str) -> bool {
        self.match_entire(input).is_some()
    }

    /// The match of all of `input` (Kotlin's `matchEntire`).
    /// Kotlin: `matchEntire(input: CharSequence): MatchResult?`
    pub fn match_entire(&self, input: &str) -> Option<MatchResult> {
        let text = Arc::new(Text::new(input));
        self.match_at(&text, 0, 0, true)
    }

    /// Whether some part of `input` matches (Kotlin's `containsMatchIn`).
    /// Kotlin: `containsMatchIn(input: CharSequence): Boolean`
    pub fn contains_match_in(&self, input: &str) -> bool {
        self.find(input, 0).is_some()
    }

    /// The first match in `input` from `start_index` (UTF-16 units) on (Kotlin's `find`).
    /// Kotlin: `find(input: CharSequence, startIndex: Int = 0): MatchResult?`
    ///
    /// # Panics
    /// If `start_index` isn't within `input` (Java's `IndexOutOfBoundsException`).
    #[track_caller]
    pub fn find(&self, input: &str, start_index: i32) -> Option<MatchResult> {
        let text = Arc::new(Text::new(input));
        if start_index < 0 || start_index as usize > text.length() {
            panic_kt("IndexOutOfBoundsException", "Illegal start index");
        }
        let from = text.char_at_utf16(start_index as usize);
        self.search(&text, from, from)
    }

    /// Every match in `input` from `start_index` on, one after another (Kotlin's `findAll`, a sequence of `find`
    /// and then each match's `next()`).
    /// Kotlin: `findAll(input: CharSequence, startIndex: Int = 0): Sequence<MatchResult>`
    #[track_caller]
    pub fn find_all(&self, input: &str, start_index: i32) -> Vec<MatchResult> {
        let text = Arc::new(Text::new(input));
        let from = check_start(start_index, &text);
        let mut out = Vec::new();
        let mut next = self.search(&text, from, from);
        while let Some(m) = next {
            next = m.next();
            out.push(m);
        }
        out
    }

    /// The match starting at `index` (UTF-16 units) exactly, if there is one (Kotlin's `matchAt`).
    /// Kotlin: `matchAt(input: CharSequence, index: Int): MatchResult?`
    #[track_caller]
    pub fn match_at_index(&self, input: &str, index: i32) -> Option<MatchResult> {
        let text = Arc::new(Text::new(input));
        let at = check_start(index, &text);
        self.match_at(&text, at, at, false)
    }

    /// Whether a match starts at `index` exactly (Kotlin's `matchesAt`).
    /// Kotlin: `matchesAt(input: CharSequence, index: Int): Boolean`
    #[track_caller]
    pub fn matches_at(&self, input: &str, index: i32) -> bool {
        self.match_at_index(input, index).is_some()
    }

    /// Java's successive `find()`s: each from the last's end, a character further after an empty one.
    fn java_matches(&self, text: &Arc<Text>) -> Vec<MatchResult> {
        let mut out: Vec<MatchResult> = Vec::new();
        let mut from = 0;
        let mut last_end = 0;
        while from <= text.chars.len() {
            let Some(m) = self.search(text, from, last_end) else { break };
            last_end = m.range.1;
            from = if m.range.0 == m.range.1 { m.range.1 + 1 } else { m.range.1 };
            out.push(m);
        }
        out
    }

    /// `input` split around the matches (Kotlin's `split`: trailing empty texts kept; at most `limit` pieces, the
    /// last the rest, when it isn't 0).
    /// Kotlin: `split(input: CharSequence, limit: Int = 0): List<String>`
    ///
    /// # Panics
    /// If `limit` is negative (Kotlin's IllegalArgumentException).
    #[track_caller]
    pub fn split(&self, input: &str, limit: i32) -> Vec<String> {
        if limit < 0 {
            panic_kt("IllegalArgumentException", &format!("Limit must be non-negative, but was {limit}"));
        }
        let text = Arc::new(Text::new(input));
        let matches = if limit == 1 { Vec::new() } else { self.java_matches(&text) };
        if matches.is_empty() {
            return vec![input.to_string()];
        }
        let mut out = Vec::new();
        let mut last = 0;
        for m in matches {
            out.push(text.slice(last, m.range.0).to_string());
            last = m.range.1;
            if limit > 0 && out.len() as i32 == limit - 1 {
                break;
            }
        }
        out.push(text.slice(last, text.chars.len()).to_string());
        out
    }

    /// `input` with each match replaced by `replacement` (Kotlin/JVM's `replace`: Java's `Matcher.replaceAll`, `$n`
    /// and `${name}` the groups' texts, `\` escaping the next character).
    /// Kotlin: `replace(input: CharSequence, replacement: String): String`
    ///
    /// # Panics
    /// As Java throws, for a replacement referring to a group there isn't, or ending in `\` or `$`.
    #[track_caller]
    pub fn replace(&self, input: &str, replacement: &str) -> String {
        let text = Arc::new(Text::new(input));
        self.replace_matches(&text, self.java_matches(&text), replacement)
    }

    /// `input` with its first match replaced by `replacement` (Java's `Matcher.replaceFirst`).
    /// Kotlin: `replaceFirst(input: CharSequence, replacement: String): String`
    #[track_caller]
    pub fn replace_first(&self, input: &str, replacement: &str) -> String {
        let text = Arc::new(Text::new(input));
        let first = self.search(&text, 0, 0).into_iter().collect();
        self.replace_matches(&text, first, replacement)
    }

    #[track_caller]
    fn replace_matches(&self, text: &Text, matches: Vec<MatchResult>, replacement: &str) -> String {
        let mut out = String::new();
        let mut last = 0;
        for m in &matches {
            out.push_str(text.slice(last, m.range.0));
            m.append_replacement(&mut out, replacement);
            last = m.range.1;
        }
        out.push_str(text.slice(last, text.chars.len()));
        out
    }

    /// `input` with each match replaced by what `transform` gives for it (Kotlin's `replace(input, transform)`:
    /// matches one after another by `next()`, none at the very end).
    /// Kotlin: `replace(input: CharSequence, transform: (MatchResult) -> CharSequence): String`
    pub fn replace_with(&self, input: &str, mut transform: impl FnMut(&MatchResult) -> String) -> String {
        let text = Arc::new(Text::new(input));
        let Some(mut m) = self.search(&text, 0, 0) else { return input.to_string() };
        let n = text.chars.len();
        let mut out = String::new();
        let mut last = 0;
        loop {
            out.push_str(text.slice(last, m.range.0));
            out.push_str(&transform(&m));
            last = m.range.1;
            match m.next() {
                Some(next) if last < n => m = next,
                _ => break,
            }
        }
        if last < n {
            out.push_str(text.slice(last, n));
        }
        out
    }
}

/// Kotlin's `String.toRegex()` and its options' overloads, on anything that is text.
pub trait KtToRegex: AsRef<str> {
    /// Kotlin: `toRegex(): Regex`
    #[track_caller]
    fn to_regex(&self) -> Regex {
        Regex::new(self.as_ref())
    }
    /// Kotlin: `toRegex(option: RegexOption): Regex`
    #[track_caller]
    fn to_regex_with(&self, option: RegexOption) -> Regex {
        Regex::with_option(self.as_ref(), option)
    }
    /// Kotlin: `toRegex(options: Set<RegexOption>): Regex`
    #[track_caller]
    fn to_regex_with_options(&self, options: &[RegexOption]) -> Regex {
        Regex::with_options(self.as_ref(), options.iter().copied())
    }
}

impl<S: AsRef<str> + ?Sized> KtToRegex for S {}

// ---- Results ----

/// A group's part of a match (Kotlin's `MatchGroup`, a data class): its text and where it is (UTF-16 units).
#[derive(Clone, Debug, PartialEq)]
pub struct MatchGroup {
    pub value: String,
    pub range: KtProgression<i32>,
}

impl KtToString for MatchGroup {
    fn kt_write(&self, out: &mut String) {
        out.push_str("MatchGroup(value=");
        out.push_str(&self.value);
        out.push_str(", range=");
        self.range.kt_write(out);
        out.push(')');
    }
}

/// A match's groups (Kotlin's `MatchGroupCollection`, Kotlin/JVM's `MatchNamedGroupCollection`): group 0 the whole
/// match, `None` for a group that took no part.
#[derive(Clone, Debug, PartialEq)]
pub struct MatchGroupCollection {
    groups: Vec<Option<MatchGroup>>,
    names: Vec<(String, usize)>,
}

impl MatchGroupCollection {
    /// Kotlin: `size: Int`
    pub fn size(&self) -> i32 {
        self.groups.len() as i32
    }

    /// Group `index` (Kotlin's `groups[index]`).
    /// Kotlin: `get(index: Int): MatchGroup?`
    ///
    /// # Panics
    /// If there is no group `index` (Kotlin's IndexOutOfBoundsException).
    #[track_caller]
    pub fn get(&self, index: i32) -> Option<MatchGroup> {
        if index < 0 || index as usize >= self.groups.len() {
            panic_kt("IndexOutOfBoundsException", &format!("No group {index}"));
        }
        self.groups[index as usize].clone()
    }

    /// The group named `name` (Kotlin's `groups[name]`).
    /// Kotlin: `get(name: String): MatchGroup?`
    ///
    /// # Panics
    /// If there is no group of that name (Kotlin/JVM's IllegalArgumentException).
    #[track_caller]
    pub fn get_named(&self, name: &str) -> Option<MatchGroup> {
        match self.names.iter().find(|(n, _)| n == name) {
            Some((_, g)) => self.groups[*g].clone(),
            None => panic_kt("IllegalArgumentException", &format!("No group with name <{name}>")),
        }
    }

    pub fn iter(&self) -> std::slice::Iter<'_, Option<MatchGroup>> {
        self.groups.iter()
    }
}

impl std::ops::Index<usize> for MatchGroupCollection {
    type Output = Option<MatchGroup>;
    fn index(&self, index: usize) -> &Option<MatchGroup> {
        &self.groups[index]
    }
}

impl IntoIterator for MatchGroupCollection {
    type Item = Option<MatchGroup>;
    type IntoIter = std::vec::IntoIter<Option<MatchGroup>>;
    fn into_iter(self) -> Self::IntoIter {
        self.groups.into_iter()
    }
}

/// A match (Kotlin's `MatchResult`): its text, where it is (UTF-16 units), its groups; the next one after it.
#[derive(Clone)]
pub struct MatchResult {
    regex: Regex,
    text: Arc<Text>,
    /// Characters.
    range: (usize, usize),
    caps: Captures,
}

impl fmt::Debug for MatchResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MatchResult").field("value", &self.value()).field("range", &self.range()).finish()
    }
}

impl MatchResult {
    /// Group `g`'s characters (0: the match's).
    fn span(&self, g: usize) -> Option<(usize, usize)> {
        if g == 0 { Some(self.range) } else { self.caps[g - 1] }
    }

    fn group(&self, g: usize) -> Option<MatchGroup> {
        self.span(g).map(|(s, e)| MatchGroup {
            value: self.text.slice(s, e).to_string(),
            range: until(self.text.utf16[s] as i32, self.text.utf16[e] as i32),
        })
    }

    /// The matched text.
    /// Kotlin: `value: String`
    pub fn value(&self) -> String {
        self.text.slice(self.range.0, self.range.1).to_string()
    }

    /// Where it is in the input (UTF-16 units; empty, `start..start-1`, for an empty match).
    /// Kotlin: `range: IntRange`
    pub fn range(&self) -> KtProgression<i32> {
        until(self.text.utf16[self.range.0] as i32, self.text.utf16[self.range.1] as i32)
    }

    /// Kotlin: `groups: MatchGroupCollection`
    pub fn groups(&self) -> MatchGroupCollection {
        MatchGroupCollection {
            groups: (0..=self.caps.len()).map(|g| self.group(g)).collect(),
            names: self.regex.program.names.clone(),
        }
    }

    /// Each group's text, `""` for one that took no part.
    /// Kotlin: `groupValues: List<String>`
    pub fn group_values(&self) -> Vec<String> {
        (0..=self.caps.len()).map(|g| self.group(g).map(|m| m.value).unwrap_or_default()).collect()
    }

    /// The next match in the same input: from this one's end, a character further after an empty one (Kotlin's
    /// `next()`).
    /// Kotlin: `next(): MatchResult?`
    pub fn next(&self) -> Option<MatchResult> {
        let from = self.range.1 + usize::from(self.range.0 == self.range.1);
        (from <= self.text.chars.len()).then(|| self.regex.search(&self.text, from, from)).flatten()
    }

    /// Java's `appendReplacement`: `replacement` with its group references filled in.
    #[track_caller]
    fn append_replacement(&self, out: &mut String, replacement: &str) {
        let r: Vec<char> = replacement.chars().collect();
        let groups = self.caps.len();
        let mut i = 0;
        while i < r.len() {
            match r[i] {
                '\\' => {
                    i += 1;
                    let Some(c) = r.get(i) else {
                        panic_kt("IllegalArgumentException", "character to be escaped is missing")
                    };
                    out.push(*c);
                    i += 1;
                }
                '$' => {
                    i += 1;
                    let Some(&c) = r.get(i) else {
                        panic_kt("IllegalArgumentException", "Illegal group reference: group index is missing")
                    };
                    let group = if c == '{' {
                        i += 1;
                        let mut name = String::new();
                        while let Some(&c) = r.get(i).filter(|c| c.is_ascii_alphanumeric()) {
                            name.push(c);
                            i += 1;
                        }
                        if name.is_empty() {
                            panic_kt("IllegalArgumentException", "named capturing group has 0 length name");
                        }
                        if r.get(i) != Some(&'}') {
                            panic_kt("IllegalArgumentException", "named capturing group is missing trailing '}'");
                        }
                        i += 1;
                        if name.starts_with(|c: char| c.is_ascii_digit()) {
                            panic_kt(
                                "IllegalArgumentException",
                                &format!("capturing group name {{{name}}} starts with digit character"),
                            );
                        }
                        match self.regex.program.names.iter().find(|(n, _)| *n == name) {
                            Some((_, g)) => *g,
                            None => panic_kt("IllegalArgumentException", &format!("No group with name {{{name}}}")),
                        }
                    } else {
                        let Some(d) = c.to_digit(10) else {
                            panic_kt("IllegalArgumentException", "Illegal group reference")
                        };
                        i += 1;
                        let mut group = d as usize;
                        while let Some(d) = r.get(i).and_then(|c| c.to_digit(10)) {
                            let more = group * 10 + d as usize;
                            if more > groups {
                                break;
                            }
                            group = more;
                            i += 1;
                        }
                        group
                    };
                    if group > groups {
                        panic_kt("IndexOutOfBoundsException", &format!("No group {group}"));
                    }
                    if let Some((s, e)) = self.span(group) {
                        out.push_str(self.text.slice(s, e));
                    }
                }
                c => {
                    out.push(c);
                    i += 1;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn values(r: &Regex, input: &str) -> Vec<String> {
        r.find_all(input, 0).iter().map(MatchResult::value).collect()
    }

    fn panic_message(f: impl FnOnce() + std::panic::UnwindSafe) -> String {
        let e = std::panic::catch_unwind(f).expect_err("a panic");
        e.downcast_ref::<String>().cloned().or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string())).unwrap()
    }

    #[test]
    fn literals_classes_and_quantifiers() {
        assert!(Regex::new("abc").matches("abc"));
        assert!(!Regex::new("abc").matches("abcd"));
        assert!(Regex::new("a.c").matches("abc") && !Regex::new("a.c").matches("a\nc"));
        assert!(Regex::with_option("a.c", RegexOption::DotMatchesAll).matches("a\nc"));
        assert!(Regex::new("[^/]*").matches("a b") && !Regex::new("[^/]*").matches("a/b"));
        assert!(Regex::new("^[a-zA-Z][a-zA-Z0-9+\\-.]*:").contains_match_in("kompoz://x"));
        assert!(!Regex::new("^[a-zA-Z][a-zA-Z0-9+\\-.]*:").contains_match_in("item/42"));
        assert!(Regex::new("^[-\\w*.]+/[-\\w+*.]+$").matches("image/*"));
        assert!(Regex::new("a{2,3}").matches("aaa") && !Regex::new("a{2,3}").matches("aaaa"));
        assert!(Regex::new("a{2}").matches("aa") && Regex::new("a{2,}").matches("aaaaa"));
        assert!(Regex::new("[\\s\\S]+").matches("a\nb"));
        assert!(Regex::new("[*]+").matches("**"));
        assert!(Regex::new("http[s]?://").matches("https://") && Regex::new("http[s]?://").matches("http://"));
        assert!(Regex::new("\\d+\\.\\d*").matches("12.5") && !Regex::new("\\d+").matches("1a"));
        assert!(Regex::new("\\w+\\W\\s\\S").matches("ab_1- x"));
        assert!(Regex::new("[a-c&&[b-d]]").matches("b") && !Regex::new("[a-c&&[b-d]]").matches("a"));
        assert!(Regex::new("[a[0-9]]+").matches("a5a") && Regex::new("[^a[0-9]]").matches("b"));
        assert!(Regex::new("[]a]+").matches("]a]") && Regex::new("[a-]+").matches("-a"));
        assert!(Regex::new("\\x41\\u0042\\x{43}\\0104\\t").matches("ABCD\t"));
        assert!(Regex::new("\\p{Lower}+\\P{Lower}").matches("ab1") && Regex::new("\\p{IsAlphabetic}").matches("é"));
        assert!(Regex::new("a}]").matches("a}]"));
    }

    #[test]
    fn groups_take_what_lazy_and_greedy_quantifiers_leave() {
        let r = Regex::new("^\\Qkompoz://item/\\E([^/]*?|)($|(\\?(.)*)|(#(.)*))");
        let m = r.match_entire("kompoz://item/42?x=1").expect("a match");
        assert_eq!(m.group_values()[1], "42");
        assert_eq!(m.group_values()[2], "?x=1");
        let lazy = Regex::new("^\\Quser_\\E([\\s\\S]+?)?_([\\s\\S]+?)?$");
        let m = lazy.match_entire("user_John_Doe").expect("a match");
        assert_eq!(m.group_values(), ["user_John_Doe", "John", "Doe"]);
        let lazy = Regex::new("([\\s\\S]+?)?_([\\s\\S]+?)?$");
        let m = lazy.match_entire("_").expect("a match");
        assert_eq!(m.groups().get(1), None);
        assert_eq!(m.groups()[2], None);
        assert_eq!(m.group_values(), ["_", "", ""]);
        // (an iteration matching nothing ends a loop, its group taken: Java's)
        let m = Regex::new("(a|)*").find("b", 0).expect("a match");
        assert_eq!(m.groups()[1].as_ref().map(|g| g.value.as_str()), Some(""));
        let m = Regex::new("(?:(a)|b)+").match_entire("ab").expect("a match");
        assert_eq!(m.group_values(), ["ab", "a"]);
        let m = Regex::new("(?<year>\\d{4})-(?<month>\\d\\d)").find("on 2026-11", 0).unwrap();
        assert_eq!(m.groups().get_named("month").unwrap().value, "11");
        assert_eq!(m.groups().get(1).unwrap().range, until(3, 7));
    }

    #[test]
    fn find_find_all_next_and_ranges_are_kotlins() {
        let fill = Regex::new("\\{(.+?)\\}");
        let all: Vec<_> = fill.find_all("a/{x}/{y}", 0).iter().map(|m| m.group_values()[1].clone()).collect();
        assert_eq!(all, ["x", "y"]);
        let first = fill.find("a/{x}/{y}", 0).unwrap();
        assert_eq!(first.range(), until(2, 5));
        assert_eq!(first.range().kt_to_string(), "2..4");
        assert_eq!(first.next().unwrap().value(), "{y}");
        assert!(first.next().unwrap().next().is_none());
        assert_eq!(fill.find("a/{x}/{y}", 3).unwrap().value(), "{y}");
        let m = Regex::new("(\\?|#|$)").find("a/b", 0).expect("the end");
        assert_eq!(m.range(), until(3, 3));
        assert_eq!(values(&Regex::new(""), "ab"), ["", "", ""]);
        assert_eq!(values(&Regex::new("a*"), "baa"), ["", "aa", ""]);
        // (UTF-16 indices)
        let m = Regex::new("b").find("😀b", 0).unwrap();
        assert_eq!(m.range(), until(2, 3));
        assert_eq!(Regex::new(".").find("😀b", 0).unwrap().value(), "😀");
        assert!(Regex::new("a").matches_at("ba", 1) && !Regex::new("a").matches_at("ba", 0));
        assert_eq!(Regex::new("\\Ga").find_all("aab", 0).len(), 2);
        assert_eq!(
            MatchGroup { value: "ab".into(), range: until(0, 2) }.kt_to_string(),
            "MatchGroup(value=ab, range=0..1)"
        );
        let message = panic_message(|| {
            Regex::new("a").find_all("a", 2);
        });
        assert_eq!(message, "IndexOutOfBoundsException: Start index out of bounds: 2, input length: 1");
    }

    #[test]
    fn split_keeps_trailing_empty_texts() {
        let fill = Regex::new("\\{(.+?)\\}");
        assert_eq!(fill.split("user_{id}", 0), ["user_", ""]);
        assert_eq!(fill.split("{name}", 0), ["", ""]);
        assert_eq!(fill.split("{first}_{last}", 0), ["", "_", ""]);
        assert_eq!(fill.split("plain", 0), ["plain"]);
        assert_eq!(Regex::new(",").split("a,b,,", 0), ["a", "b", "", ""]);
        assert_eq!(Regex::new(",").split("a,b,c", 2), ["a", "b,c"]);
        assert_eq!(Regex::new(",").split("a,b,c", 1), ["a,b,c"]);
        assert_eq!(Regex::new("").split("ab", 0), ["", "a", "b", ""]);
    }

    #[test]
    fn replace_is_javas_and_replace_with_kotlins() {
        let r = Regex::new("(\\w)(\\d)");
        assert_eq!(r.replace("a1 b2", "$2$1"), "1a 2b");
        assert_eq!(r.replace("a1", "\\$1"), "$1");
        assert_eq!(r.replace("a1", "$21"), "11");
        assert_eq!(r.replace("x", "$9"), "x");
        assert_eq!(r.replace_first("a1 b2", "<$0>"), "<a1> b2");
        assert_eq!(Regex::new("(?<d>\\d)").replace("a1", "[${d}]"), "a[1]");
        assert_eq!(Regex::new("").replace("ab", "-"), "-a-b-");
        assert_eq!(Regex::new("").replace_with("ab", |_| "-".to_string()), "-a-b-");
        assert_eq!(Regex::new("a*").replace("baa", "-"), "-b--");
        assert_eq!(Regex::new("a*").replace_with("baa", |_| "-".to_string()), "-b-");
        assert_eq!(Regex::new("\\d").replace_with("a1b22", |m| format!("<{}>", m.value())), "a<1>b<2><2>");
        assert_eq!(Regex::escape_replacement("$1\\"), "\\$1\\\\");
        let message = panic_message(|| {
            Regex::new("(a)").replace("a", "$2");
        });
        assert_eq!(message, "IndexOutOfBoundsException: No group 2");
        let message = panic_message(|| {
            Regex::new("a").replace("a", "x$");
        });
        assert_eq!(message, "IllegalArgumentException: Illegal group reference: group index is missing");
    }

    #[test]
    fn case_is_ignored_as_kotlin_asks_and_quoting_is_javas() {
        let ci = |p| Regex::with_option(p, RegexOption::IgnoreCase);
        assert!(ci("\\Qhttps://example.com/\\E([^/]*?|)").matches("HTTPS://Example.com/7"));
        assert!(!Regex::new("\\Qhttps\\E").matches("HTTPS"));
        assert!(ci("straße").matches("STRAßE") && ci("é[à-ä]").matches("ÉÂ"));
        assert!(ci("k").matches("\u{212A}") && !ci("[k]").matches("\u{212A}"));
        assert!(Regex::new("(?i)abc").matches("AbC") && !Regex::new("(?i)é").matches("É"));
        assert!(Regex::new("(?iu)é").matches("É") && Regex::new("a(?i:b)c").matches("aBc"));
        assert!(!Regex::new("a(?i:b)c").matches("aBC") && Regex::new("(a)\\1").matches("aa"));
        assert!(ci("(a)\\1").matches("aA"));
        assert_eq!(Regex::escape("a.b"), "\\Qa.b\\E");
        assert!(Regex::new(&Regex::escape("a\\Eb")).matches("a\\Eb"));
        assert_eq!(Regex::escape("a\\Eb"), "\\Qa\\E\\\\E\\Qb\\E");
        assert!(Regex::new("\\Q.*\\E.*").matches(".*xyz") && !Regex::new("\\Q.*\\E").matches("ab"));
        assert!(Regex::new("\\Qab\\E+").matches("abbb"));
        assert!(Regex::new("\\Q12\\E").matches("12") && Regex::new("[\\Q]-\\E]+").matches("]-"));
        assert!(Regex::from_literal("a.b").matches("a.b") && !Regex::from_literal("a.b").matches("axb"));
        assert_eq!(ci("x").options(), [RegexOption::IgnoreCase]);
        assert_eq!("a+".to_regex().pattern(), "a+");
        assert!("A".to_regex_with(RegexOption::IgnoreCase).matches("a"));
    }

    #[test]
    fn anchors_lookaround_atomic_and_possessive() {
        assert!(Regex::new("a$").contains_match_in("a\n") && Regex::new("a$").contains_match_in("a\r\n"));
        assert!(!Regex::new("a$").contains_match_in("a\nb") && !Regex::new("a\\z").contains_match_in("a\n"));
        assert!(Regex::new("a\\Z").contains_match_in("a\n"));
        let ml = |p| Regex::with_option(p, RegexOption::Multiline);
        assert_eq!(values(&ml("^\\w"), "ab\ncd\r\nef"), ["a", "c", "e"]);
        assert_eq!(values(&ml("\\w$"), "ab\ncd"), ["b", "d"]);
        assert!(!Regex::new("^b").contains_match_in("a\nb") && Regex::new("(?m)^b").contains_match_in("a\nb"));
        assert_eq!(values(&Regex::new("\\bfoo\\b"), "foo food foo"), ["foo", "foo"]);
        assert_eq!(values(&Regex::new("\\Bo"), "foo"), ["o", "o"]);
        assert_eq!(values(&Regex::new("\\w+(?=!)"), "hi! yo"), ["hi"]);
        assert_eq!(values(&Regex::new("\\w+(?!\\w|!)"), "hi! yo"), ["yo"]);
        assert_eq!(values(&Regex::new("\\w+(?!\\w)"), "hi! yo"), ["hi", "yo"]);
        assert_eq!(values(&Regex::new("(?<=\\$)\\d+"), "a $12 34"), ["12"]);
        assert_eq!(values(&Regex::new("(?<!\\$|\\d)\\d+"), "a $12 34"), ["34"]);
        assert!(!Regex::new("(?>a+)a").matches("aaa") && !Regex::new("a++a").matches("aaa"));
        assert!(Regex::new("a+a").matches("aaa") && Regex::new("a*?b").matches("aab"));
        assert!(Regex::new("\\R").matches("\r\n") && Regex::new("\\h\\v").matches(" \n"));
        assert!(Regex::with_option("a b # comment\n c", RegexOption::Comments).matches("abc"));
        assert!(Regex::with_option("a\\ b", RegexOption::Comments).matches("a b"));
    }

    #[test]
    fn patterns_java_cant_read_panic_as_java_throws() {
        let bad = |p: &'static str| {
            panic_message(move || {
                Regex::new(p);
            })
        };
        assert!(bad("(ab").starts_with("PatternSyntaxException: Unclosed group"));
        assert!(bad("ab)").starts_with("PatternSyntaxException: Unmatched closing ')'"));
        assert!(bad("*a").starts_with("PatternSyntaxException: Dangling meta character '*'"));
        assert!(bad("a**").starts_with("PatternSyntaxException: Dangling meta character '*'"));
        assert!(bad("a{x}").starts_with("PatternSyntaxException: Illegal repetition"));
        assert!(bad("{a}").starts_with("PatternSyntaxException: Dangling meta character '{'"));
        assert!(bad("[ab").starts_with("PatternSyntaxException: Unclosed character class"));
        assert!(bad("[z-a]").starts_with("PatternSyntaxException: Illegal character range"));
        assert!(bad("\\y").starts_with("PatternSyntaxException: Illegal/unsupported escape sequence"));
        assert!(bad("(?<=a+)b").contains("does not have an obvious maximum length"));
        assert!(bad("\\p{L}").starts_with("PatternSyntaxException: Unsupported character property name {L}"));
        assert_eq!(bad("(ab"), "PatternSyntaxException: Unclosed group near index 3\n(ab");
        assert_eq!(bad("a{x}"), "PatternSyntaxException: Illegal repetition near index 1\na{x}\n ^");
    }
}
