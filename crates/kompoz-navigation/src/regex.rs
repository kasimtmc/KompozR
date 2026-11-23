//! The regular expressions navigation matches links with, as Java's (Kotlin's `Regex` on the JVM, which
//! `NavDeepLink` builds its patterns for): a backtracking matcher of the part of Java's syntax those patterns
//! use — literals and escapes (`\Q…\E` quoting, `\s \S \w \W \d \D`, `\.` …), `.`, classes (`[^/]`, `[\s\S]`,
//! `[*]`, ranges), groups (capturing, `(?:…)`), alternation, the quantifiers `* + ? {n,m}` greedy and lazy (`*?`),
//! and the anchors `^` and `$` — case-insensitively too (Kotlin's `IGNORE_CASE`, Unicode case as Kotlin asks).
//!
//! A pattern that isn't of that syntax is a programmer's error (Java's `PatternSyntaxException`): a panic.

/// A compiled pattern (Kotlin's `Regex`).
#[derive(Clone, Debug)]
pub(crate) struct Regex {
    node: Node,
    groups: usize,
    ignore_case: bool,
}

/// A match: where it is and what each group took (`None`: the group took no part).
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Match {
    pub range: std::ops::Range<usize>,
    groups: Vec<Option<(usize, usize)>>,
}

impl Match {
    /// Group `i`'s text in `text` (the one matched), group 0 the whole match.
    pub fn group<'t>(&self, text: &'t str, i: usize) -> Option<&'t str> {
        if i == 0 {
            return Some(&text[self.range.clone()]);
        }
        self.groups.get(i - 1).copied().flatten().map(|(s, e)| &text[s..e])
    }
}

#[derive(Clone, Debug)]
enum Node {
    Empty,
    Char(char),
    /// `.`: anything but a line terminator.
    Any,
    Class(Class),
    Start,
    End,
    Concat(Vec<Node>),
    Alt(Vec<Node>),
    /// A group: capturing (its number, 1-based) or not.
    Group(Box<Node>, Option<usize>),
    Repeat {
        node: Box<Node>,
        min: usize,
        max: Option<usize>,
        greedy: bool,
    },
}

#[derive(Clone, Debug)]
struct Class {
    negated: bool,
    items: Vec<ClassItem>,
}

#[derive(Clone, Debug)]
enum ClassItem {
    Range(char, char),
    Named(Named),
}

#[derive(Clone, Copy, Debug)]
enum Named {
    Space(bool),
    Word(bool),
    Digit(bool),
}

impl Named {
    fn matches(self, c: char) -> bool {
        match self {
            Named::Space(neg) => matches!(c, ' ' | '\t' | '\n' | '\u{b}' | '\u{c}' | '\r') != neg,
            Named::Word(neg) => (c.is_ascii_alphanumeric() || c == '_') != neg,
            Named::Digit(neg) => c.is_ascii_digit() != neg,
        }
    }
}

fn is_line_terminator(c: char) -> bool {
    matches!(c, '\n' | '\r' | '\u{85}' | '\u{2028}' | '\u{2029}')
}

/// The case variants of `c` compared by a case-insensitive match (its simple lower and upper case).
fn case_variants(c: char) -> [char; 3] {
    let lower = c.to_lowercase().next().filter(|_| c.to_lowercase().count() == 1).unwrap_or(c);
    let upper = c.to_uppercase().next().filter(|_| c.to_uppercase().count() == 1).unwrap_or(c);
    [c, lower, upper]
}

impl Class {
    fn matches(&self, c: char, ignore_case: bool) -> bool {
        let one = |c: char| {
            self.items.iter().any(|item| match item {
                ClassItem::Range(a, b) => *a <= c && c <= *b,
                ClassItem::Named(n) => n.matches(c),
            })
        };
        let found = if ignore_case { case_variants(c).into_iter().any(one) } else { one(c) };
        found != self.negated
    }
}

struct Parser<'p> {
    chars: Vec<char>,
    pos: usize,
    groups: usize,
    pattern: &'p str,
}

impl Parser<'_> {
    fn error(&self, what: &str) -> ! {
        panic!("{what} near index {} in the pattern {:?}", self.pos, self.pattern)
    }

    fn peek(&self) -> Option<char> {
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

    fn alternation(&mut self) -> Node {
        let mut branches = vec![self.concat()];
        while self.eat('|') {
            branches.push(self.concat());
        }
        if branches.len() == 1 { branches.pop().expect("a branch") } else { Node::Alt(branches) }
    }

    fn concat(&mut self) -> Node {
        let mut items = Vec::new();
        while let Some(c) = self.peek() {
            if c == '|' || c == ')' {
                break;
            }
            if c == '\\' && self.chars.get(self.pos + 1) == Some(&'Q') {
                // \Q…\E: everything up to \E (or the end) as it is.
                self.pos += 2;
                while self.pos < self.chars.len() {
                    if self.chars[self.pos] == '\\' && self.chars.get(self.pos + 1) == Some(&'E') {
                        self.pos += 2;
                        break;
                    }
                    items.push(Node::Char(self.chars[self.pos]));
                    self.pos += 1;
                }
                continue;
            }
            if c == '\\' && self.chars.get(self.pos + 1) == Some(&'E') {
                // (an \E without its \Q is nothing)
                self.pos += 2;
                continue;
            }
            let atom = self.atom();
            items.push(self.quantified(atom));
        }
        match items.len() {
            0 => Node::Empty,
            1 => items.pop().expect("an item"),
            _ => Node::Concat(items),
        }
    }

    fn quantified(&mut self, atom: Node) -> Node {
        let (min, max) = match self.peek() {
            Some('*') => (0, None),
            Some('+') => (1, None),
            Some('?') => (0, Some(1)),
            Some('{') if self.counted_repetition_follows() => {
                self.pos += 1;
                let mut text = String::new();
                while let Some(c) = self.peek() {
                    self.pos += 1;
                    if c == '}' {
                        break;
                    }
                    text.push(c);
                }
                let (min, max) = match text.split_once(',') {
                    None => {
                        let n = text.parse().unwrap_or_else(|_| self.error("a repetition count"));
                        (n, Some(n))
                    }
                    Some((a, b)) => (
                        a.parse().unwrap_or_else(|_| self.error("a repetition count")),
                        if b.is_empty() { None } else { Some(b.parse().unwrap_or_else(|_| self.error("a count"))) },
                    ),
                };
                return self.lazy_or_greedy(atom, min, max);
            }
            _ => return atom,
        };
        self.pos += 1;
        self.lazy_or_greedy(atom, min, max)
    }

    /// Whether a `{n}`, `{n,}` or `{n,m}` is next.
    fn counted_repetition_follows(&self) -> bool {
        let rest = &self.chars[self.pos + 1..];
        let Some(close) = rest.iter().position(|c| *c == '}') else { return false };
        let inside = &rest[..close];
        inside.first().is_some_and(char::is_ascii_digit) && inside.iter().all(|c| c.is_ascii_digit() || *c == ',')
    }

    fn lazy_or_greedy(&mut self, atom: Node, min: usize, max: Option<usize>) -> Node {
        let greedy = !self.eat('?');
        if self.peek() == Some('+') {
            self.error("possessive quantifiers aren't supported");
        }
        let node = Node::Repeat { node: Box::new(atom), min, max, greedy };
        // (a quantifier after a quantifier: Java reads it as applying to the repetition)
        self.quantified(node)
    }

    fn atom(&mut self) -> Node {
        let c = self.peek().unwrap_or_else(|| self.error("an atom"));
        self.pos += 1;
        match c {
            '.' => Node::Any,
            '^' => Node::Start,
            '$' => Node::End,
            '(' => {
                let number = if self.chars[self.pos..].starts_with(&['?', ':']) {
                    self.pos += 2;
                    None
                } else if self.peek() == Some('?') {
                    self.error("only (…) and (?:…) groups are supported")
                } else {
                    self.groups += 1;
                    Some(self.groups)
                };
                let inner = self.alternation();
                if !self.eat(')') {
                    self.error("an unclosed group");
                }
                Node::Group(Box::new(inner), number)
            }
            '[' => Node::Class(self.class()),
            '\\' => match self.escape() {
                ClassItem::Range(a, _) => Node::Char(a),
                ClassItem::Named(n) => Node::Class(Class { negated: false, items: vec![ClassItem::Named(n)] }),
            },
            '*' | '+' | '?' => self.error("a dangling quantifier"),
            c => Node::Char(c),
        }
    }

    /// After a `\`: a named class or an escaped character.
    fn escape(&mut self) -> ClassItem {
        let c = self.peek().unwrap_or_else(|| self.error("an escape"));
        self.pos += 1;
        let one = |c: char| ClassItem::Range(c, c);
        match c {
            's' => ClassItem::Named(Named::Space(false)),
            'S' => ClassItem::Named(Named::Space(true)),
            'w' => ClassItem::Named(Named::Word(false)),
            'W' => ClassItem::Named(Named::Word(true)),
            'd' => ClassItem::Named(Named::Digit(false)),
            'D' => ClassItem::Named(Named::Digit(true)),
            't' => one('\t'),
            'n' => one('\n'),
            'r' => one('\r'),
            'f' => one('\u{c}'),
            c if c.is_ascii_alphanumeric() => self.error("an unsupported escape"),
            c => one(c),
        }
    }

    fn class(&mut self) -> Class {
        let negated = self.eat('^');
        let mut items = Vec::new();
        let mut first = true;
        loop {
            let c = self.peek().unwrap_or_else(|| self.error("an unclosed character class"));
            if c == ']' && !first {
                self.pos += 1;
                break;
            }
            first = false;
            self.pos += 1;
            let start = if c == '\\' {
                match self.escape() {
                    ClassItem::Range(a, _) => a,
                    named => {
                        items.push(named);
                        continue;
                    }
                }
            } else if c == '[' {
                self.error("nested classes aren't supported")
            } else {
                c
            };
            if self.peek() == Some('-') && self.chars.get(self.pos + 1).is_some_and(|c| *c != ']') {
                self.pos += 1;
                let end = self.peek().expect("a range's end");
                self.pos += 1;
                let end = if end == '\\' {
                    match self.escape() {
                        ClassItem::Range(a, _) => a,
                        ClassItem::Named(_) => self.error("a range ending in a class"),
                    }
                } else {
                    end
                };
                items.push(ClassItem::Range(start, end));
            } else {
                items.push(ClassItem::Range(start, start));
            }
        }
        Class { negated, items }
    }
}

type Captures = Vec<Option<(usize, usize)>>;

struct Matcher<'t> {
    /// The text's characters with their byte offsets (and the text's length after them).
    chars: Vec<(usize, char)>,
    len: usize,
    ignore_case: bool,
    _text: &'t str,
}

impl Matcher<'_> {
    fn offset(&self, i: usize) -> usize {
        self.chars.get(i).map_or(self.len, |c| c.0)
    }

    fn char_eq(&self, a: char, b: char) -> bool {
        a == b || (self.ignore_case && case_variants(a).into_iter().any(|v| case_variants(b).contains(&v)))
    }

    /// Whether `node` matches at character `i` with `k` matching the rest from where it ends.
    fn m(&self, node: &Node, i: usize, caps: &mut Captures, k: &mut dyn FnMut(usize, &mut Captures) -> bool) -> bool {
        match node {
            Node::Empty => k(i, caps),
            Node::Char(c) => i < self.chars.len() && self.char_eq(self.chars[i].1, *c) && k(i + 1, caps),
            Node::Any => i < self.chars.len() && !is_line_terminator(self.chars[i].1) && k(i + 1, caps),
            Node::Class(class) => {
                i < self.chars.len() && class.matches(self.chars[i].1, self.ignore_case) && k(i + 1, caps)
            }
            Node::Start => i == 0 && k(i, caps),
            Node::End => {
                let n = self.chars.len();
                (i == n || (i + 1 == n && self.chars[i].1 == '\n')) && k(i, caps)
            }
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
            Node::Repeat { node, min, max, greedy } => self.repeat(node, *min, *max, *greedy, 0, i, caps, k),
        }
    }

    fn concat(
        &self,
        items: &[Node],
        i: usize,
        caps: &mut Captures,
        k: &mut dyn FnMut(usize, &mut Captures) -> bool,
    ) -> bool {
        match items.split_first() {
            None => k(i, caps),
            Some((first, rest)) => self.m(first, i, caps, &mut |j, caps: &mut Captures| self.concat(rest, j, caps, k)),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn repeat(
        &self,
        node: &Node,
        min: usize,
        max: Option<usize>,
        greedy: bool,
        count: usize,
        i: usize,
        caps: &mut Captures,
        k: &mut dyn FnMut(usize, &mut Captures) -> bool,
    ) -> bool {
        let can_more = max.is_none_or(|m| count < m);
        let more = |caps: &mut Captures, k: &mut dyn FnMut(usize, &mut Captures) -> bool| {
            can_more
                && self.m(node, i, caps, &mut |j, caps: &mut Captures| {
                    // (an empty iteration past the minimum ends the loop, as Java's does)
                    if j == i && count >= min {
                        return false;
                    }
                    self.repeat(node, min, max, greedy, count + 1, j, caps, k)
                })
        };
        if count < min {
            return more(caps, k);
        }
        let saved = caps.clone();
        if greedy {
            if more(caps, k) {
                return true;
            }
            *caps = saved;
            k(i, caps)
        } else {
            if k(i, caps) {
                return true;
            }
            *caps = saved;
            more(caps, k)
        }
    }
}

impl Regex {
    /// `pattern` compiled (Kotlin's `Regex(pattern)`).
    ///
    /// # Panics
    /// If `pattern` isn't of the syntax this reads (see the module).
    pub fn new(pattern: &str) -> Regex {
        Regex::with_case(pattern, false)
    }

    /// `pattern` matching regardless of case (Kotlin's `Regex(pattern, RegexOption.IGNORE_CASE)`).
    pub fn ignore_case(pattern: &str) -> Regex {
        Regex::with_case(pattern, true)
    }

    fn with_case(pattern: &str, ignore_case: bool) -> Regex {
        let mut p = Parser { chars: pattern.chars().collect(), pos: 0, groups: 0, pattern };
        let node = p.alternation();
        if p.pos != p.chars.len() {
            p.error("an unmatched ')'");
        }
        Regex { node, groups: p.groups, ignore_case }
    }

    /// Kotlin's `Regex.escape(literal)` (Java's `Pattern.quote`): a pattern matching `literal` as it is.
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

    fn matcher<'t>(&self, text: &'t str) -> Matcher<'t> {
        Matcher { chars: text.char_indices().collect(), len: text.len(), ignore_case: self.ignore_case, _text: text }
    }

    fn match_at(&self, m: &Matcher, start: usize, entire: bool) -> Option<Match> {
        let mut caps: Captures = vec![None; self.groups];
        let mut end = None;
        let n = m.chars.len();
        let found = m.m(&self.node, start, &mut caps, &mut |j, _| {
            if entire && j != n {
                return false;
            }
            end = Some(j);
            true
        });
        if !found {
            return None;
        }
        let end = end.expect("an end");
        Some(Match {
            range: m.offset(start)..m.offset(end),
            groups: caps.into_iter().map(|g| g.map(|(s, e)| (m.offset(s), m.offset(e)))).collect(),
        })
    }

    /// The match of the whole of `text` (Kotlin's `matchEntire`).
    pub fn match_entire(&self, text: &str) -> Option<Match> {
        let m = self.matcher(text);
        self.match_at(&m, 0, true)
    }

    /// Whether the whole of `text` matches (Kotlin's `matches`).
    pub fn matches(&self, text: &str) -> bool {
        self.match_entire(text).is_some()
    }

    /// The first match in `text` from byte `from` on (Kotlin's `find`).
    pub fn find_from(&self, text: &str, from: usize) -> Option<Match> {
        let m = self.matcher(text);
        let first = m.chars.iter().position(|(o, _)| *o >= from).unwrap_or(m.chars.len());
        (first..=m.chars.len()).find_map(|i| self.match_at(&m, i, false))
    }

    /// The first match in `text` (Kotlin's `find`).
    pub fn find(&self, text: &str) -> Option<Match> {
        self.find_from(text, 0)
    }

    /// Every match in `text`, one after another (Kotlin's `findAll`; after an empty match the next starts a
    /// character further).
    pub fn find_all(&self, text: &str) -> Vec<Match> {
        let mut out = Vec::new();
        let mut from = 0;
        while from <= text.len() {
            let Some(m) = self.find_from(text, from) else { break };
            from = if m.range.is_empty() {
                m.range.end + text[m.range.end..].chars().next().map_or(1, char::len_utf8)
            } else {
                m.range.end
            };
            out.push(m);
        }
        out
    }

    /// Whether `text` has a match (Kotlin's `containsMatchIn`).
    pub fn contains_match_in(&self, text: &str) -> bool {
        self.find(text).is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn literals_classes_and_quantifiers() {
        assert!(Regex::new("abc").matches("abc"));
        assert!(!Regex::new("abc").matches("abcd"));
        assert!(Regex::new("a.c").matches("abc") && !Regex::new("a.c").matches("a\nc"));
        assert!(Regex::new("[^/]*").matches("a b") && !Regex::new("[^/]*").matches("a/b"));
        assert!(Regex::new("^[a-zA-Z]+[+\\w\\-.]*:").contains_match_in("kompoz://x"));
        assert!(!Regex::new("^[a-zA-Z]+[+\\w\\-.]*:").contains_match_in("item/42"));
        assert!(Regex::new("^[-\\w*.]+/[-\\w+*.]+$").matches("image/*"));
        assert!(Regex::new("a{2,3}").matches("aaa") && !Regex::new("a{2,3}").matches("aaaa"));
        assert!(Regex::new("[\\s\\S]+").matches("a\nb"));
        assert!(Regex::new("[*]+").matches("**"));
    }

    #[test]
    fn groups_take_what_lazy_and_greedy_quantifiers_leave() {
        let r = Regex::new("^\\Qkompoz://item/\\E([^/]*?|)($|(\\?(.)*)|(#(.)*))");
        let t = "kompoz://item/42?x=1";
        let m = r.match_entire(t).expect("a match");
        assert_eq!(m.group(t, 1), Some("42"));
        assert_eq!(m.group(t, 2), Some("?x=1"));
        let lazy = Regex::new("([\\s\\S]+?)?_([\\s\\S]+?)?$");
        let t = "John_Doe";
        let m = lazy.match_entire(t).expect("a match");
        assert_eq!((m.group(t, 1), m.group(t, 2)), (Some("John"), Some("Doe")));
        let t = "_";
        let m = lazy.match_entire(t).expect("a match");
        assert_eq!((m.group(t, 1), m.group(t, 2)), (None, None));
    }

    #[test]
    fn case_is_ignored_when_asked_and_quoting_is_javas() {
        assert!(Regex::ignore_case("\\Qhttps://example.com/\\E([^/]*?|)").matches("HTTPS://Example.com/7"));
        assert!(!Regex::new("\\Qhttps\\E").matches("HTTPS"));
        assert_eq!(Regex::escape("a.b"), "\\Qa.b\\E");
        assert!(Regex::new(&Regex::escape("a\\Eb")).matches("a\\Eb"));
        let fill = Regex::new("\\{(.+?)\\}");
        let t = "a/{x}/{y}";
        let all: Vec<_> = fill.find_all(t).iter().map(|m| m.group(t, 1).unwrap().to_string()).collect();
        assert_eq!(all, ["x", "y"]);
        let m = Regex::new("(\\?|#|$)").find("a/b").expect("the end");
        assert_eq!(m.range, 3..3);
    }
}
