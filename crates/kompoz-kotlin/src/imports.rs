//! (stage 14 WPD) The JVM classes a file imports that the API has under another Rust name: Kotlin's `Locale` is
//! Compose's text locale (`androidx.compose.ui.text.intl.Locale`) unless the file imports `java.util.Locale`, as
//! in Kotlin, and the kt shims' `java.util.Locale` is `JavaLocale` (the index's [`TYPES`](crate::index::TYPES) says
//! so: ``Kotlin: `java.util.Locale` ``). An import naming such a class by its whole name (`import java.util.Locale`,
//! or `… as JLocale`) makes the file's name for it the Rust type's, before the file is parsed: its type
//! (`val l: Locale`), constructor (`Locale("he")`) and companion (`Locale.getDefault()`) are that type's.

use crate::lexer::{StringPiece, Token, TokenKind};

/// Renames, in `tokens`, the names of the JVM classes their imports name whose Rust types are named otherwise.
pub(crate) fn rename_imported_jvm_classes(tokens: &mut [Token]) {
    let renames = jvm_imports(tokens);
    if !renames.is_empty() {
        rename(tokens, &renames);
    }
}

/// (the file's name of the class, the Rust type) of each such import.
fn jvm_imports(tokens: &[Token]) -> Vec<(String, &'static str)> {
    let mut renames = Vec::new();
    let mut i = 0;
    while i < tokens.len() {
        if !tokens[i].is_word("import") {
            i += 1;
            continue;
        }
        let mut path: Vec<&str> = Vec::new();
        let mut j = i + 1;
        while let Some(t) = tokens.get(j).filter(|t| t.is_ident()) {
            path.push(&t.text);
            j += 1;
            if tokens.get(j).is_some_and(|t| t.is_op(".")) && tokens.get(j + 1).is_some_and(|t| t.is_ident()) {
                j += 1;
            } else {
                break;
            }
        }
        let alias = (tokens.get(j).is_some_and(|t| t.is_word("as")))
            .then(|| tokens.get(j + 1).filter(|t| t.is_ident()).map(|t| t.text.clone()))
            .flatten();
        if path.len() > 1
            && let Some(rust) = crate::index::rust_type(&path.join("."))
        {
            let name = alias.unwrap_or_else(|| path.last().expect("a path").to_string());
            if name != rust {
                renames.push((name, rust));
            }
        }
        i = j.max(i + 1);
    }
    renames
}

fn rename(tokens: &mut [Token], renames: &[(String, &'static str)]) {
    for k in 0..tokens.len() {
        // (a member of another value of that name, `x.Locale`, isn't the class)
        let member = k > 0 && (tokens[k - 1].is_op(".") || tokens[k - 1].is_op("?."));
        let t = &mut tokens[k];
        match &mut t.kind {
            TokenKind::Ident { backticked: false } if !member => {
                if let Some((_, rust)) = renames.iter().find(|(name, _)| *name == t.text) {
                    t.text = rust.to_string();
                }
            }
            TokenKind::String { pieces, .. } => {
                for piece in pieces {
                    match piece {
                        StringPiece::Expr { tokens, .. } => rename(tokens, renames),
                        StringPiece::Name { name, backticked: false, .. } => {
                            if let Some((_, rust)) = renames.iter().find(|(n, _)| n == name) {
                                *name = rust.to_string();
                            }
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diag::Diagnostics;
    use crate::pos::Pos;

    fn renamed(text: &str) -> Vec<String> {
        let mut tokens = crate::lexer::lex(text, Pos::start(1, 1), &mut Diagnostics::default());
        rename_imported_jvm_classes(&mut tokens);
        tokens.iter().filter(|t| t.is_ident()).map(|t| t.text.clone()).collect()
    }

    #[test]
    fn javas_locale_by_its_import_only() {
        assert_eq!(
            renamed("import java.util.Locale\nval l: Locale = Locale.getDefault()\nval c = x.Locale"),
            [
                "import",
                "java",
                "util",
                "Locale",
                "val",
                "l",
                "JavaLocale",
                "JavaLocale",
                "getDefault",
                "val",
                "c",
                "x",
                "Locale"
            ]
        );
        assert_eq!(renamed("val l = Locale(\"he\")"), ["val", "l", "Locale"]);
        assert_eq!(
            renamed("import androidx.compose.ui.text.intl.Locale\nval l = Locale.current"),
            ["import", "androidx", "compose", "ui", "text", "intl", "Locale", "val", "l", "Locale", "current"]
        );
        let aliased = renamed("import java.util.Locale as JLocale\nval l = JLocale(\"he\")");
        assert_eq!(aliased[aliased.len() - 1], "JavaLocale");
        // (in a string template's expression too)
        let mut tokens = crate::lexer::lex(
            "import java.util.Locale\nval s = \"${Locale.getDefault()}\"",
            Pos::start(1, 1),
            &mut Diagnostics::default(),
        );
        rename_imported_jvm_classes(&mut tokens);
        let template = tokens.iter().find_map(|t| match &t.kind {
            TokenKind::String { pieces, .. } => pieces.iter().find_map(|p| match p {
                StringPiece::Expr { tokens, .. } => Some(tokens[0].text.clone()),
                _ => None,
            }),
            _ => None,
        });
        assert_eq!(template.as_deref(), Some("JavaLocale"));
    }
}
