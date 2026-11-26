//! The analysis a translation records for a language server (`Options::analyze`): declarations with their headers
//! and Rust names, locals and where they can be named, what names refer to, the types of names and calls.

use kompoz_kotlin::analysis::{Analysis, DeclKind, LocalKind, Target};
use kompoz_kotlin::{OneSpan, Options, Pos, translate};

const TEXT: &str = "\
data class Note(val id: Int, val title: String = \"\")

object Tally {
    var total = 0
    fun add(n: Int = 1): Int {
        total += n
        return total
    }
}

fun label(note: Note): String {
    val shown = note.title + \"!\"
    val sizes = listOf(1, 2).map { it * 2 }
    return shown + sizes.size + Tally.add()
}

@Composable
fun NoteRow(note: Note) {
    Text(label(note))
}
";

fn analysis() -> Analysis {
    let opts = Options { analyze: true, ..Default::default() };
    let t = translate(TEXT, Pos::start(1, 1), &OneSpan::default(), &opts);
    assert!(t.diagnostics.is_empty(), "{:?}", t.diagnostics);
    t.analysis.expect("asked for")
}

/// The byte offset of the `n`th (0-based) `needle` in the text, plus `delta`.
fn at(needle: &str, n: usize, delta: usize) -> usize {
    TEXT.match_indices(needle).nth(n).map(|(i, _)| i + delta).unwrap_or_else(|| panic!("no {needle} #{n}"))
}

#[test]
fn not_recorded_unless_asked() {
    let t = translate(TEXT, Pos::start(1, 1), &OneSpan::default(), &Options::default());
    assert!(t.analysis.is_none());
}

#[test]
fn declarations_with_headers_and_rust_names() {
    let a = analysis();
    let shown: Vec<(&str, &str, DeclKind, &str)> =
        a.declarations.iter().map(|d| (d.kotlin.as_str(), d.rust.as_str(), d.kind, d.detail.as_str())).collect();
    assert_eq!(
        shown,
        [
            ("Note", "Note", DeclKind::DataClass, "data class Note(val id: Int, val title: String = \"\")"),
            ("Tally", "Tally", DeclKind::Object, "object Tally"),
            ("Tally.total", "total", DeclKind::Property, "var total"),
            ("Tally.add", "add", DeclKind::Function, "fun add(n: Int = 1): Int"),
            ("label", "label", DeclKind::Function, "fun label(note: Note): String"),
            ("NoteRow", "NoteRow", DeclKind::Composable, "@Composable fun NoteRow(note: Note)"),
        ]
    );
    let add = &a.declarations[3];
    assert_eq!(&TEXT[add.name.start.offset..add.name.end.offset], "add");
}

#[test]
fn locals_where_they_can_be_named() {
    let a = analysis();
    let shown: Vec<(&str, LocalKind, &str)> =
        a.locals.iter().map(|l| (l.name.as_str(), l.kind, l.ty.as_str())).collect();
    for local in [
        ("n", LocalKind::Param, "Int"),
        ("note", LocalKind::Param, "Note"),
        ("shown", LocalKind::Val, "String"),
        ("sizes", LocalKind::Val, "List<Int>"),
    ] {
        assert!(shown.contains(&local), "{local:?} not in {shown:?}");
    }
    // (in `label`'s return: its parameter and both vals; `n` of `Tally.add` is another function's)
    let names: Vec<&str> = a.locals_at(at("return shown", 0, 0)).iter().map(|l| l.name.as_str()).collect();
    assert_eq!(names, ["sizes", "shown", "note"]);
    // (before `shown` is declared, only the parameter)
    let names: Vec<&str> = a.locals_at(at("val shown", 0, 0)).iter().map(|l| l.name.as_str()).collect();
    assert_eq!(names, ["note"]);
}

#[test]
fn references_and_types() {
    let a = analysis();
    // (`note` in `note.title`: the parameter)
    let note_param = a.locals.iter().find(|l| l.name == "note" && l.loc.start.offset < at("val shown", 0, 0)).unwrap();
    let r = a.reference_at(at("note.title", 0, 1)).expect("a reference");
    assert_eq!(r.target, Target::Local(note_param.loc));
    // (`Tally.add()`: the object's function; `label(note)`: the file's function; `listOf`: the API's)
    assert_eq!(
        a.reference_at(at("Tally.add()", 0, 7)).map(|r| &r.target),
        Some(&Target::Declaration("Tally.add".into()))
    );
    assert_eq!(a.reference_at(at("label(note)", 0, 1)).map(|r| &r.target), Some(&Target::Declaration("label".into())));
    match a.reference_at(at("listOf", 0, 1)).map(|r| &r.target) {
        Some(Target::Api { kotlin, krate, .. }) => {
            assert_eq!((kotlin.as_str(), krate.as_str()), ("listOf", "kompoz_runtime"))
        }
        other => panic!("{other:?}"),
    }
    // (the types of a member read and of a call)
    assert_eq!(a.type_at(at("note.title", 0, 6)).map(|t| t.ty.as_str()), Some("String"));
    assert_eq!(a.type_at(at("Tally.add()", 0, 7)).map(|t| t.ty.as_str()), Some("Int"));
}
