//! What the front end says about the Kotlin it reads: errors and warnings at Kotlin positions, with notes.
//!
//! The macros report each at the span of its position (`compile_error!` for an error, a deprecation for a
//! warning: stable Rust has no other way to warn from a macro), its message prefixed with the `.kt` file's
//! `file:line:column` for `kompoz_file!` (whose tokens have no spans of their own).

use crate::pos::Loc;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Severity {
    Error,
    Warning,
}

/// A remark attached to a diagnostic: where something it refers to is (a previous declaration of a name), or
/// what to do.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Note {
    pub message: String,
    pub loc: Option<Loc>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Diagnostic {
    pub severity: Severity,
    pub message: String,
    /// What it is about: the span of its start is where it is reported.
    pub loc: Loc,
    pub notes: Vec<Note>,
}

impl Diagnostic {
    pub fn error(loc: Loc, message: impl Into<String>) -> Diagnostic {
        Diagnostic { severity: Severity::Error, message: message.into(), loc, notes: Vec::new() }
    }

    pub fn warning(loc: Loc, message: impl Into<String>) -> Diagnostic {
        Diagnostic { severity: Severity::Warning, message: message.into(), loc, notes: Vec::new() }
    }

    /// With a note (at a place, or none).
    pub fn note(mut self, message: impl Into<String>, loc: Option<Loc>) -> Diagnostic {
        self.notes.push(Note { message: message.into(), loc });
        self
    }

    pub fn is_error(&self) -> bool {
        self.severity == Severity::Error
    }

    /// The text a macro reports: `kompoz: message`, or `kompoz: file:line:column: message` when the Kotlin came
    /// from a file (`file`); each note on a line of its own (`note: …`, with its position when it has one).
    pub fn render(&self, file: Option<&str>) -> String {
        let mut out = String::from("kompoz: ");
        if let Some(file) = file {
            out += &format!("{file}:{}: ", self.loc.start);
        }
        out += &self.message;
        for n in &self.notes {
            out += "\nnote: ";
            match (n.loc, file) {
                (Some(loc), Some(file)) => out += &format!("{file}:{}: ", loc.start),
                (Some(loc), None) => out += &format!("at {}: ", loc.start),
                (None, _) => {}
            }
            out += &n.message;
        }
        out
    }
}

/// The diagnostics of one translation, in the order they were found.
#[derive(Clone, Debug, Default)]
pub struct Diagnostics {
    pub list: Vec<Diagnostic>,
}

impl Diagnostics {
    pub fn push(&mut self, d: Diagnostic) {
        self.list.push(d);
    }

    pub fn error(&mut self, loc: Loc, message: impl Into<String>) {
        self.push(Diagnostic::error(loc, message));
    }

    pub fn warning(&mut self, loc: Loc, message: impl Into<String>) {
        self.push(Diagnostic::warning(loc, message));
    }

    pub fn has_errors(&self) -> bool {
        self.list.iter().any(Diagnostic::is_error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pos::Pos;

    #[test]
    fn rendered_with_and_without_a_file() {
        let at = Loc::at(Pos { offset: 10, line: 3, column: 7 });
        let d = Diagnostic::error(at, "expected `)`").note("the `(` is here", Some(Loc::at(Pos::start(2, 1))));
        assert_eq!(d.render(Some("ui.kt")), "kompoz: ui.kt:3:7: expected `)`\nnote: ui.kt:2:1: the `(` is here");
        assert_eq!(d.render(None), "kompoz: expected `)`\nnote: at 2:1: the `(` is here");
    }
}
