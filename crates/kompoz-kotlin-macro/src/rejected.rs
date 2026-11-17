//! What Rust's lexer rejects of the Kotlin in a `kompoz! { … }`. rustc reads a macro's input before the macro does,
//! reports what it can't read (the build fails whatever the macro does) and runs the macro on what it could; these
//! errors say what it was and what to write instead — or that a `.kt` file read by `kompoz_file!`, which Rust's
//! lexer never reads, takes it as it is.
//!
//! Found from the leaf tokens rustc gave and the text they came from:
//! - source between the tokens that is neither spacing nor a comment: what the lexer dropped — a backticked name's
//!   backticks (`` `in` ``), a backslash outside a string (exact text only: a rebuilt one has no source between
//!   its tokens);
//! - an escape Rust doesn't have, in a string or a character: Kotlin's `\$`, `\b` and `\uXXXX`, and a Kotlin raw
//!   string's backslashes (Rust reads `"""\d"""` as `""`, `"\d"`, `""`);
//! - a name right before a quote (`else"y"`, `else'c'`): Rust reserves it as a literal's prefix (edition 2021 on;
//!   the macro can't see the crate's edition, and KompozR's crates and apps are 2021 or later). Exact text only.
//!
//! Brackets a template leaves unbalanced as Rust splits it (`"${ "(" }"`: `"${ "`, `(`, `" }"`) stop rustc before
//! any macro runs: only its own error is reported then.

use crate::text::{Entry, Leaf};
use kompoz_kotlin::{Diagnostic, Loc, Pos, TextSource};

/// Why, at the start of every message.
const RUST_READS_FIRST: &str = "Rust's lexer, which reads `kompoz! { … }` before KompozR does,";

/// What a `.kt` file spares, at the end of every message.
const IN_A_FILE: &str =
    "put this Kotlin in a `.kt` file read by `kompoz_file!(\"….kt\")`, which Rust's lexer doesn't read";

/// What Rust's lexer rejected of `text` (at `base`; its leaf tokens `entries`, by start offset), in order.
pub(crate) fn find(text: &str, base: Pos, entries: &[Entry], source: TextSource) -> Vec<Diagnostic> {
    let mut found: Vec<(usize, usize, String)> = Vec::new();
    if source == TextSource::Exact {
        dropped(text, entries, &mut found);
        prefixes(text, entries, &mut found);
    }
    escapes(text, entries, &mut found);
    found.sort_by_key(|f| (f.0, f.1));
    let at = |offset: usize| base.advanced_by(&text[..offset]);
    found.into_iter().map(|(start, end, message)| Diagnostic::error(Loc::new(at(start), at(end)), message)).collect()
}

/// What no token covers and isn't spacing or a comment.
fn dropped(text: &str, entries: &[Entry], found: &mut Vec<(usize, usize, String)>) {
    let mut gaps = Vec::new();
    let mut covered = 0;
    for e in entries {
        if e.start > covered {
            gaps.push(covered..e.start);
        }
        covered = covered.max(e.end);
    }
    if covered < text.len() {
        gaps.push(covered..text.len());
    }
    // (past a backticked name's closing backtick, which is in a later gap when the name is two words)
    let mut skip_to = 0;
    for gap in gaps {
        let mut i = gap.start.max(skip_to);
        while i < gap.end {
            let rest = &text[i..gap.end];
            let Some(c) = rest.chars().next() else { break };
            if rust_whitespace(c) {
                i += c.len_utf8();
            } else if rest.starts_with("//") {
                i += rest.find('\n').unwrap_or(rest.len());
            } else if rest.starts_with("/*") {
                i += block_comment_len(rest);
            } else if let Some(close) = (c == '`').then(|| backticked_end(text, i)).flatten() {
                found.push((
                    i,
                    close,
                    format!("{RUST_READS_FIRST} can't read the backticked name `` {} ``: {IN_A_FILE}", &text[i..close]),
                ));
                skip_to = close;
                i = close;
            } else {
                let end = i + rest.find(rust_whitespace).unwrap_or(rest.len());
                found.push((i, end, format!("{RUST_READS_FIRST} can't read `{}`: {IN_A_FILE}", &text[i..end])));
                i = end;
            }
        }
    }
}

/// Spacing to Rust's lexer (Unicode's Pattern_White_Space: the marks of direction too, not a no-break space).
fn rust_whitespace(c: char) -> bool {
    matches!(
        c,
        '\t' | '\n' | '\u{b}' | '\u{c}' | '\r' | ' ' | '\u{85}' | '\u{200e}' | '\u{200f}' | '\u{2028}' | '\u{2029}'
    )
}

/// The length of the (nested) block comment `rest` starts with.
fn block_comment_len(rest: &str) -> usize {
    let mut depth = 0;
    let mut i = 0;
    while i < rest.len() {
        if rest[i..].starts_with("/*") {
            depth += 1;
            i += 2;
        } else if rest[i..].starts_with("*/") {
            depth -= 1;
            i += 2;
            if depth == 0 {
                return i;
            }
        } else {
            i += rest[i..].chars().next().map_or(1, char::len_utf8);
        }
    }
    rest.len()
}

/// Just past the backtick closing the name whose opening one is at `open`, on its line.
fn backticked_end(text: &str, open: usize) -> Option<usize> {
    let rest = &text[open + 1..];
    let close = rest.find(['`', '\n'])?;
    rest[close..].starts_with('`').then_some(open + 1 + close + 1)
}

/// A name right before a quote: a literal's prefix to Rust.
fn prefixes(text: &str, entries: &[Entry], found: &mut Vec<(usize, usize, String)>) {
    for pair in entries.windows(2) {
        let (name, next) = (&pair[0], &pair[1]);
        if name.leaf != Leaf::Ident || name.end != next.start {
            continue;
        }
        let quote = match next.leaf {
            Leaf::Literal => text[next.start..].starts_with(['"', '\'']),
            Leaf::Punct('\'') => true,
            _ => false,
        };
        if quote {
            let word = &text[name.start..name.end];
            let end = next.start + 1;
            found.push((
                name.start,
                end,
                format!(
                    "{RUST_READS_FIRST} takes `{}` for a literal's prefix, which it reserves: put a space after \
                     `{word}`, or {IN_A_FILE}",
                    &text[name.start..end]
                ),
            ));
        }
    }
}

/// The escapes Rust doesn't have in the strings and characters.
fn escapes(text: &str, entries: &[Entry], found: &mut Vec<(usize, usize, String)>) {
    for e in entries.iter().filter(|e| e.leaf == Leaf::Literal) {
        let literal = &text[e.start..e.end];
        if !literal.starts_with(['"', '\'']) {
            continue;
        }
        let mut chars = literal.char_indices().peekable();
        while let Some((i, c)) = chars.next() {
            if c != '\\' {
                continue;
            }
            let Some((_, escaped)) = chars.next() else { break };
            let start = e.start + i;
            let rest = &literal[i..];
            let (end, fix) = match escaped {
                'n' | 'r' | 't' | '\\' | '0' | '\'' | '"' | '\n' => continue,
                'x' if rest.get(2..4).is_some_and(|h| u8::from_str_radix(h, 16).is_ok_and(|v| v <= 0x7f)) => continue,
                'u' if chars.peek().is_some_and(|&(_, c)| c == '{') => continue,
                '$' => (start + 2, Some("write `${'$'}` for a `$` in a string, `'$'` for the character".to_string())),
                'b' => (start + 2, Some("write `\\u{8}`".to_string())),
                'u' => {
                    let digits: String = rest[2..].chars().take(4).take_while(char::is_ascii_hexdigit).collect();
                    let end = start + 2 + digits.len();
                    (end, (digits.len() == 4).then(|| format!("write `\\u{{{digits}}}`")))
                }
                other => (start + 1 + other.len_utf8(), None),
            };
            let written = &text[start..end];
            let message = match (escaped, fix) {
                (_, Some(fix)) => format!("{RUST_READS_FIRST} has no escape `{written}`: {fix}, or {IN_A_FILE}"),
                ('u', None) => format!("{RUST_READS_FIRST} has no escape `{written}`: {IN_A_FILE}"),
                _ => format!(
                    "{RUST_READS_FIRST} has no escape `{written}` (a backslash in a Kotlin raw string is one to it): \
                     {IN_A_FILE}"
                ),
            };
            found.push((start, end, message));
        }
    }
}
