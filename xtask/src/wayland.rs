//! `cargo xtask wayland [--check]`: the Wayland protocols KompozR speaks (their XML, as C++ Kompoz's
//! scripts/wayland-protocols.sh took them: crates/kompoz-platform/src/linux/wayland/protocols/*.xml) turned into Rust
//! over the platform's own libwayland binding (`linux::wayland::ffi`), into files that are committed
//! (crates/kompoz-platform/src/linux/wayland/generated/): building KompozR needs no generator, only changing a
//! protocol does. What wayland-scanner's `client-header` and `private-code` are to C, for each protocol:
//!
//! - its interfaces' tables, as libwayland reads them (`wl_interface` and `wl_message` statics: names, signatures
//!   with their since-versions and nullable marks, the interfaces their objects and new ids are, as the scanner
//!   writes them), so that libwayland-client, loaded at run time, needn't have any of them;
//! - a type for each interface (an owned object, its destructor request sent when it is dropped) with a function
//!   for each request (a new id's object returned; a destructor consuming it), over the runtime's checked
//!   marshalling (`wl_proxy_marshal_array_flags`);
//! - an enum of each interface's events with their arguments decoded, and one of them all ([`Event`] in the
//!   generated `mod.rs`) that the runtime's queue is read as;
//! - each enum and bitfield as a newtype over its value with a constant for each entry;
//! - the descriptions as doc comments, shortened to their first paragraph's first sentences.
//!
//! The output is formatted by rustfmt with the workspace's settings (what `cargo fmt` would leave as it is).
//! `--check` generates elsewhere and fails if what is committed differs (CI).

use crate::xml::{self, Element};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::fs;
use std::io::Write as _;
use std::path::Path;
use std::process::{Command, Stdio};

const PROTOCOLS: &str = "crates/kompoz-platform/src/linux/wayland/protocols";
const GENERATED: &str = "crates/kompoz-platform/src/linux/wayland/generated";
/// Where the generated code finds the runtime.
const FFI: &str = "crate::linux::wayland::ffi";

pub fn run(root: &Path, check: bool) -> Result<(), String> {
    let files = generate(&root.join(PROTOCOLS), root)?;
    let generated = root.join(GENERATED);
    let expected: BTreeSet<&str> = files.iter().map(|(name, _)| name.as_str()).collect();
    let present: Vec<String> = match fs::read_dir(&generated) {
        Ok(dir) => dir.filter_map(|e| e.ok()?.file_name().into_string().ok()).collect(),
        Err(_) => Vec::new(),
    };
    let orphans: Vec<&String> = present.iter().filter(|name| !expected.contains(name.as_str())).collect();
    let shown = |name: &str| format!("{GENERATED}/{name}");
    if check {
        let mut stale: Vec<String> = files
            .iter()
            .filter(|(name, data)| fs::read_to_string(generated.join(name)).ok().as_ref() != Some(data))
            .map(|(name, _)| shown(name))
            .collect();
        stale.extend(orphans.iter().map(|name| shown(name)));
        if !stale.is_empty() {
            for s in &stale {
                eprintln!("out of date: {s}");
            }
            return Err("run `cargo xtask wayland` and commit what it writes".to_string());
        }
        println!("wayland: up to date");
        return Ok(());
    }
    fs::create_dir_all(&generated).map_err(|e| format!("{}: {e}", generated.display()))?;
    for (name, data) in &files {
        let path = generated.join(name);
        if fs::read_to_string(&path).ok().as_ref() != Some(data) {
            fs::write(&path, data).map_err(|e| format!("{}: {e}", path.display()))?;
            println!("wrote {}", shown(name));
        }
    }
    for name in orphans {
        let path = generated.join(name);
        fs::remove_file(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        println!("removed {}", shown(name));
    }
    Ok(())
}

/// Every generated file (its name, its text) from the protocols in `dir`, formatted with `root`'s rustfmt.toml.
fn generate(dir: &Path, root: &Path) -> Result<Vec<(String, String)>, String> {
    let mut paths: Vec<_> = fs::read_dir(dir)
        .map_err(|e| format!("{}: {e}", dir.display()))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "xml"))
        .collect();
    paths.sort();
    let mut protocols = Vec::new();
    for path in &paths {
        let text = fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let stem = path.file_stem().and_then(|s| s.to_str()).ok_or("a protocol's name isn't UTF-8")?;
        let protocol = read_protocol(&text, &stem.replace('-', "_")).map_err(|e| format!("{}: {e}", path.display()))?;
        protocols.push(protocol);
    }
    let files = emit(&protocols)?;
    files.into_iter().map(|(name, code)| Ok((name, rustfmt(&code, root)?))).collect()
}

/// `code` as rustfmt formats it with the workspace's settings.
fn rustfmt(code: &str, root: &Path) -> Result<String, String> {
    let rustfmt = std::env::var("RUSTFMT").unwrap_or_else(|_| "rustfmt".to_string());
    let mut child = Command::new(&rustfmt)
        .args(["--edition", "2024", "--emit", "stdout", "--config-path"])
        .arg(root.join("rustfmt.toml"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("{rustfmt}: {e}"))?;
    let mut stdin = child.stdin.take().ok_or("rustfmt's stdin")?;
    let input = code.to_string();
    let writer = std::thread::spawn(move || stdin.write_all(input.as_bytes()));
    let out = child.wait_with_output().map_err(|e| format!("{rustfmt}: {e}"))?;
    writer.join().map_err(|_| "rustfmt's writer")?.map_err(|e| format!("rustfmt: {e}"))?;
    if !out.status.success() {
        return Err(format!("rustfmt failed on generated code:\n{}", String::from_utf8_lossy(&out.stderr)));
    }
    String::from_utf8(out.stdout).map_err(|_| "rustfmt wrote other than UTF-8".to_string())
}

// ---- The protocols, as read ----

#[derive(Clone, Debug)]
struct Protocol {
    /// Its file's name as a module's ("xdg_shell").
    module: String,
    name: String,
    description: Option<String>,
    interfaces: Vec<Interface>,
}

#[derive(Clone, Debug)]
struct Interface {
    name: String,
    version: u32,
    description: Option<String>,
    requests: Vec<Message>,
    events: Vec<Message>,
    enums: Vec<Enum>,
}

#[derive(Clone, Debug)]
struct Message {
    name: String,
    since: u32,
    destructor: bool,
    deprecated_since: Option<u32>,
    description: Option<String>,
    args: Vec<Arg>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind {
    Int,
    Uint,
    Fixed,
    String,
    Object,
    NewId,
    Array,
    Fd,
}

#[derive(Clone, Debug)]
struct Arg {
    name: String,
    kind: Kind,
    interface: Option<String>,
    nullable: bool,
    /// `enum="name"` or `enum="interface.name"`.
    r#enum: Option<String>,
    summary: Option<String>,
}

#[derive(Clone, Debug)]
struct Enum {
    name: String,
    bitfield: bool,
    since: u32,
    description: Option<String>,
    entries: Vec<Entry>,
}

#[derive(Clone, Debug)]
struct Entry {
    name: String,
    value: u32,
    since: u32,
    summary: Option<String>,
}

fn number(s: &str) -> Result<u32, String> {
    let parsed = match s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        Some(hex) => u32::from_str_radix(hex, 16),
        None => s.parse(),
    };
    parsed.map_err(|_| format!("{s:?} isn't a number"))
}

fn required<'a>(e: &'a Element, name: &str) -> Result<&'a str, String> {
    e.attribute(name).ok_or_else(|| format!("<{}> without {name}", e.name))
}

/// The description of `e` (its `<description>`: the summary, then the text), as one text.
fn description(e: &Element) -> Option<String> {
    let d = e.element("description")?;
    let mut out = d.attribute("summary").map(str::trim).unwrap_or("").to_string();
    let text = d.text();
    if !text.trim().is_empty() {
        out.push_str("\n\n");
        out.push_str(&text);
    }
    (!out.trim().is_empty()).then_some(out)
}

fn read_protocol(text: &str, module: &str) -> Result<Protocol, String> {
    let root = xml::parse(text)?;
    if root.name != "protocol" {
        return Err(format!("the root is <{}>, not <protocol>", root.name));
    }
    let mut interfaces = Vec::new();
    for i in root.elements_named("interface") {
        let mut interface = Interface {
            name: required(i, "name")?.to_string(),
            version: number(required(i, "version")?)?,
            description: description(i),
            requests: Vec::new(),
            events: Vec::new(),
            enums: Vec::new(),
        };
        for m in i.elements() {
            match m.name.as_str() {
                "request" | "event" => {
                    let message = read_message(m)?;
                    if m.name == "request" {
                        interface.requests.push(message);
                    } else {
                        interface.events.push(message);
                    }
                }
                "enum" => {
                    let mut entries = Vec::new();
                    for entry in m.elements_named("entry") {
                        entries.push(Entry {
                            name: required(entry, "name")?.to_string(),
                            value: number(required(entry, "value")?)?,
                            since: entry.attribute("since").map(number).transpose()?.unwrap_or(1),
                            summary: entry.attribute("summary").map(str::to_string),
                        });
                    }
                    interface.enums.push(Enum {
                        name: required(m, "name")?.to_string(),
                        bitfield: m.attribute("bitfield") == Some("true"),
                        since: m.attribute("since").map(number).transpose()?.unwrap_or(1),
                        description: description(m),
                        entries,
                    });
                }
                _ => {}
            }
        }
        interfaces.push(interface);
    }
    Ok(Protocol {
        module: module.to_string(),
        name: required(&root, "name")?.to_string(),
        description: description(&root),
        interfaces,
    })
}

fn read_message(m: &Element) -> Result<Message, String> {
    let mut args = Vec::new();
    for a in m.elements_named("arg") {
        let kind = match required(a, "type")? {
            "int" => Kind::Int,
            "uint" => Kind::Uint,
            "fixed" => Kind::Fixed,
            "string" => Kind::String,
            "object" => Kind::Object,
            "new_id" => Kind::NewId,
            "array" => Kind::Array,
            "fd" => Kind::Fd,
            other => return Err(format!("an argument of type {other}")),
        };
        args.push(Arg {
            name: required(a, "name")?.to_string(),
            kind,
            interface: a.attribute("interface").map(str::to_string),
            nullable: a.attribute("allow-null") == Some("true"),
            r#enum: a.attribute("enum").map(str::to_string),
            summary: a.attribute("summary").map(str::to_string),
        });
    }
    Ok(Message {
        name: required(m, "name")?.to_string(),
        since: m.attribute("since").map(number).transpose()?.unwrap_or(1),
        destructor: m.attribute("type") == Some("destructor"),
        deprecated_since: m.attribute("deprecated-since").map(number).transpose()?,
        description: description(m),
        args,
    })
}

// ---- Names ----

/// Rust's keywords, which a protocol's names are written as raw identifiers if they are one.
const KEYWORDS: &[&str] = &[
    "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum", "extern", "false", "fn",
    "for", "gen", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub", "ref", "return", "static",
    "struct", "trait", "true", "type", "unsafe", "use", "where", "while", "abstract", "become", "box", "do", "final",
    "macro", "override", "priv", "try", "typeof", "unsized", "virtual", "yield",
];

/// A snake_case name as an identifier (a keyword raw).
fn ident(name: &str) -> String {
    match name {
        "self" | "super" | "crate" | "Self" => format!("{name}_"),
        _ if KEYWORDS.contains(&name) => format!("r#{name}"),
        _ => name.to_string(),
    }
}

/// `wl_data_device` as `WlDataDevice`.
fn camel(name: &str) -> String {
    let mut out = String::new();
    for part in name.split('_').filter(|p| !p.is_empty()) {
        let mut chars = part.chars();
        if let Some(first) = chars.next() {
            out.extend(first.to_uppercase());
            out.push_str(chars.as_str());
        }
    }
    out
}

/// `wl_data_device` as `WL_DATA_DEVICE`; an entry named `90` as `_90`.
fn screaming(name: &str) -> String {
    let upper = name.to_uppercase();
    if upper.starts_with(|c: char| c.is_ascii_digit()) { format!("_{upper}") } else { upper }
}

// ---- Docs ----

/// A description as a doc comment's lines (`prefix` before each, `width` columns at most where words allow): the
/// summary (a sentence, capitalized), then the first paragraph's first two sentences, Markdown's special characters
/// escaped, no line starting what Markdown would take as a list, a heading or a quote.
fn doc_lines(text: &str, prefix: &str, width: usize) -> Vec<String> {
    let mut paragraphs = paragraphs(text).into_iter();
    let summary = paragraphs.next().unwrap_or_default();
    let mut out = sentence(&summary);
    if let Some(first) = paragraphs.find(|p| !p.is_empty()) {
        let first = sentences(&first, 2);
        if !first.is_empty() && !first.eq_ignore_ascii_case(&out) {
            if !out.is_empty() {
                out.push(' ');
            }
            out.push_str(&first);
        }
    }
    wrap(&escape(&out), prefix, width)
}

/// A text's paragraphs (blank lines between them), each one line.
fn paragraphs(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut words: Vec<&str> = Vec::new();
    for line in text.lines().chain(std::iter::once("")) {
        if line.trim().is_empty() {
            if !words.is_empty() {
                out.push(std::mem::take(&mut words).join(" "));
            }
        } else {
            words.extend(line.split_whitespace());
        }
    }
    out
}

/// A summary as a sentence: capitalized, ending in a full stop.
fn sentence(s: &str) -> String {
    let s = s.trim();
    if s.is_empty() {
        return String::new();
    }
    let mut chars = s.chars();
    let mut out: String = chars.next().map(|c| c.to_uppercase().collect()).unwrap_or_default();
    out.push_str(chars.as_str());
    if !out.ends_with(['.', '!', '?', ':']) {
        out.push('.');
    }
    out
}

/// The first `n` sentences of a paragraph (a full stop followed by a space ends one; "e.g." and "i.e." don't).
fn sentences(p: &str, n: usize) -> String {
    let mut count = 0;
    let bytes = p.as_bytes();
    for (i, &b) in bytes.iter().enumerate() {
        if matches!(b, b'.' | b'!' | b'?') && bytes.get(i + 1).is_none_or(|&c| c == b' ') {
            let before = &p[..i + 1];
            if before.ends_with("e.g.") || before.ends_with("i.e.") || before.ends_with("etc.") {
                continue;
            }
            count += 1;
            if count == n {
                return before.to_string();
            }
        }
    }
    p.to_string()
}

/// Markdown's link brackets, HTML's angle brackets and emphasis marks escaped (backticks are code, as written).
fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if matches!(c, '[' | ']' | '<' | '>' | '*' | '\\' | '_') {
            if c == '_' {
                out.push(c); // (inside words, as names have them: not emphasis)
                continue;
            }
            out.push('\\');
        }
        out.push(c);
    }
    out
}

fn wrap(text: &str, prefix: &str, width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut line = String::new();
    // (A word Markdown would take, at a line's start, as a list's item, a heading, a quote or a table.)
    let special = |w: &str| {
        matches!(w, "-" | "+" | "=" | "|")
            || w.starts_with('#')
            || w.trim_end_matches(['.', ')']).parse::<u32>().is_ok_and(|_| w.ends_with(['.', ')']))
    };
    for word in text.split_whitespace() {
        if !line.is_empty() && prefix.len() + line.len() + 1 + word.len() > width {
            // (A special word goes down with the one before it.)
            let carried = match line.rfind(' ') {
                Some(space) if special(word) => {
                    let last = line[space + 1..].to_string();
                    line.truncate(space);
                    last
                }
                _ => String::new(),
            };
            if !special(word) || !carried.is_empty() {
                lines.push(format!("{prefix}{line}"));
                line = carried;
            }
        }
        if !line.is_empty() {
            line.push(' ');
        }
        line.push_str(word);
    }
    if !line.is_empty() {
        lines.push(format!("{prefix}{line}"));
    }
    lines
}

// ---- Emitting ----

/// Where each interface's type is (its module), and each enum's type name, across the protocols.
struct Names {
    modules: BTreeMap<String, String>,
}

impl Names {
    /// The path of interface `name`'s type (or table, with `suffix`) from module `from`.
    fn interface_path(&self, name: &str, from: &str, item: &str) -> Result<String, String> {
        let module = self.modules.get(name).ok_or_else(|| format!("no interface {name}"))?;
        Ok(if module == from { item.to_string() } else { format!("super::{module}::{item}") })
    }

    /// The type of an argument's enum (`enum="transform"` in `interface`, or `enum="wl_output.transform"`).
    fn enum_type(&self, r#enum: &str, interface: &str, from: &str) -> Result<String, String> {
        let (owner, name) = r#enum.split_once('.').unwrap_or((interface, r#enum));
        self.interface_path(owner, from, &format!("{}{}", camel(owner), camel(name)))
    }
}

/// The generated files: one for each protocol, and `mod.rs`.
fn emit(protocols: &[Protocol]) -> Result<Vec<(String, String)>, String> {
    let mut names = Names { modules: BTreeMap::new() };
    for p in protocols {
        for i in &p.interfaces {
            if names.modules.insert(i.name.clone(), p.module.clone()).is_some() {
                return Err(format!("interface {} is in two protocols", i.name));
            }
        }
    }
    let mut files = Vec::new();
    let mut with_events = Vec::new();
    for p in protocols {
        files.push((format!("{}.rs", p.module), emit_protocol(p, &names)?));
        for i in &p.interfaces {
            if !i.events.is_empty() {
                with_events.push((p.module.clone(), i.name.clone()));
            }
        }
    }
    files.push(("mod.rs".to_string(), emit_mod(protocols, &with_events)));
    Ok(files)
}

const HEADER: &str =
    concat!("// Generated by `cargo xtask wayland` from\n", "// crates/kompoz-platform/src/linux/wayland/protocols/");

fn emit_mod(protocols: &[Protocol], with_events: &[(String, String)]) -> String {
    let mut s = format!(
        "{HEADER}*.xml: don't edit, run it.\n\n\
         //! The Wayland protocols KompozR speaks, generated from their XML (`cargo xtask wayland`): each \
         interface's tables,\n//! its object's type with its requests, its events, its enums. [`Event`]: any \
         interface's event, as the runtime's queue\n//! is read.\n\n\
         #![allow(clippy::too_many_arguments, clippy::large_enum_variant, clippy::should_implement_trait)]\n\n"
    );
    for p in protocols {
        let _ = writeln!(s, "pub mod {};", p.module);
    }
    s.push('\n');
    for p in protocols {
        let _ = writeln!(s, "pub use self::{}::*;", p.module);
    }
    let _ = write!(
        s,
        "\nuse {FFI}::{{Args, ObjectId, RawEvent}};\n\n\
         /// An event of any interface's object (the interface's event enum, by its name).\n\
         #[derive(Debug)]\npub enum Event {{\n"
    );
    for (_, name) in with_events {
        let _ = writeln!(s, "    {}({}Event),", camel(name), camel(name));
    }
    s.push_str(
        "}\n\nimpl Event {\n    /// A queued event decoded: who sent it and what it is; none if it isn't one of the \
         protocols' (or its arguments\n    /// aren't what they should be).\n    \
         pub fn decode(raw: RawEvent) -> Option<(ObjectId, Event)> {\n        \
         let RawEvent { sender, interface, opcode, args } = raw;\n        \
         let mut args = Args::new(args);\n        \
         let event = match interface.name().to_bytes() {\n",
    );
    for (_, name) in with_events {
        let c = camel(name);
        let _ = writeln!(s, "            b\"{name}\" => Event::{c}({c}Event::decode(opcode, &mut args)?),");
    }
    s.push_str("            _ => return None,\n        };\n        Some((sender, event))\n    }\n}\n");
    s
}

/// The Rust type of a request's argument (`a` in interface `i` of module `m`).
fn request_param(a: &Arg, i: &Interface, m: &str, names: &Names) -> Result<String, String> {
    Ok(match a.kind {
        Kind::Int | Kind::Uint => match &a.r#enum {
            Some(e) => names.enum_type(e, &i.name, m)?,
            None => if a.kind == Kind::Int { "i32" } else { "u32" }.to_string(),
        },
        Kind::Fixed => "f64".to_string(),
        Kind::String => if a.nullable { "Option<&str>" } else { "&str" }.to_string(),
        Kind::Object => {
            let t = match &a.interface {
                Some(o) => names.interface_path(o, m, &camel(o))?,
                None => "Proxy".to_string(),
            };
            if a.nullable { format!("Option<&{t}>") } else { format!("&{t}") }
        }
        Kind::Array => "&[u8]".to_string(),
        Kind::Fd => "BorrowedFd<'_>".to_string(),
        Kind::NewId => unreachable!("new ids aren't parameters"),
    })
}

/// The Rust type of an event's argument.
fn event_field(a: &Arg, i: &Interface, m: &str, names: &Names) -> Result<String, String> {
    Ok(match a.kind {
        Kind::Int | Kind::Uint => match &a.r#enum {
            Some(e) => names.enum_type(e, &i.name, m)?,
            None => if a.kind == Kind::Int { "i32" } else { "u32" }.to_string(),
        },
        Kind::Fixed => "f64".to_string(),
        Kind::String => if a.nullable { "Option<String>" } else { "String" }.to_string(),
        Kind::Object => if a.nullable { "Option<ObjectId>" } else { "ObjectId" }.to_string(),
        Kind::NewId => {
            let o = a.interface.as_deref().ok_or("an event's new id without an interface")?;
            names.interface_path(o, m, &camel(o))?
        }
        Kind::Array => "Vec<u8>".to_string(),
        Kind::Fd => "OwnedFd".to_string(),
    })
}

/// A message's signature as libwayland reads it (wayland-scanner's): its since-version, then each argument's type,
/// `?` before a nullable one, an untyped new id as `sun` (the interface's name, its version, the id).
fn signature(m: &Message) -> String {
    let mut s = if m.since > 1 { m.since.to_string() } else { String::new() };
    for a in &m.args {
        if a.nullable && matches!(a.kind, Kind::String | Kind::Object | Kind::NewId | Kind::Array) {
            s.push('?');
        }
        s.push_str(match a.kind {
            Kind::Int => "i",
            Kind::Uint => "u",
            Kind::Fixed => "f",
            Kind::String => "s",
            Kind::Object => "o",
            Kind::NewId if a.interface.is_none() => "sun",
            Kind::NewId => "n",
            Kind::Array => "a",
            Kind::Fd => "h",
        });
    }
    s
}

/// A message's types (each signature argument's interface, if it is an object's or a new id's).
fn types(m: &Message, names: &Names, module: &str) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    for a in &m.args {
        match (a.kind, &a.interface) {
            (Kind::NewId, None) => out.extend(["None".to_string(), "None".to_string(), "None".to_string()]),
            (Kind::Object | Kind::NewId, Some(o)) => {
                let path = names.interface_path(o, module, &format!("{}_INTERFACE", screaming(o)))?;
                out.push(format!("Some(&{path})"));
            }
            _ => out.push("None".to_string()),
        }
    }
    Ok(out)
}

fn emit_protocol(p: &Protocol, names: &Names) -> Result<String, String> {
    let m = &p.module;
    let mut s = format!("{HEADER}{}.xml: don't edit, run it.\n\n", m.replace('_', "-"));
    let _ = writeln!(s, "//! The `{}` protocol's client side (its copyright and license are in its XML).", p.name);
    if let Some(d) = &p.description {
        s.push_str("//!\n");
        for line in doc_lines(d, "//! ", 116) {
            let _ = writeln!(s, "{line}");
        }
    }
    s.push_str("\n#![allow(clippy::too_many_arguments, clippy::should_implement_trait)]\n\n");
    let body = emit_interfaces(p, names)?;
    // (Only what the body uses.)
    let mut uses = Vec::new();
    for (item, needle) in [
        ("Arg", "Arg::"),
        ("Args", "&mut Args"),
        ("Interface", "Interface"),
        ("Message", "Message::"),
        ("Object", "Object"),
        ("ObjectId", "ObjectId"),
        ("Proxy", "Proxy"),
        ("c_string", "c_string("),
    ] {
        if body.contains(needle) {
            uses.push(item);
        }
    }
    let _ = writeln!(s, "use {FFI}::{{{}}};", uses.join(", "));
    if body.contains("BorrowedFd") || body.contains("OwnedFd") {
        let fds: Vec<&str> = ["BorrowedFd", "OwnedFd"].into_iter().filter(|f| body.contains(f)).collect();
        let _ = writeln!(s, "use std::os::fd::{{{}}};", fds.join(", "));
    }
    if body.contains("bitfield!") {
        let _ = writeln!(s, "use {FFI}::bitfield;");
    }
    s.push('\n');
    s.push_str(&body);
    Ok(s)
}

fn doc(s: &mut String, text: Option<&str>, indent: &str) {
    if let Some(t) = text {
        for line in doc_lines(t, &format!("{indent}/// "), 120) {
            let _ = writeln!(s, "{line}");
        }
    }
}

fn emit_interfaces(p: &Protocol, names: &Names) -> Result<String, String> {
    let m = &p.module;
    let mut s = String::new();
    for i in &p.interfaces {
        let t = camel(&i.name);
        let upper = screaming(&i.name);
        // The tables.
        let tables = |list: &[Message], kind: &str| -> Result<String, String> {
            let mut out = String::new();
            for msg in list {
                let ty = types(msg, names, m)?;
                if !ty.is_empty() {
                    let _ = writeln!(
                        out,
                        "static {upper}_{kind}_{}_TYPES: [Option<&Interface>; {}] = [{}];",
                        screaming(&msg.name),
                        ty.len(),
                        ty.join(", ")
                    );
                }
            }
            let _ = writeln!(out, "static {upper}_{kind}S: [Message; {}] = [", list.len());
            for msg in list {
                let ty = types(msg, names, m)?;
                let types = if ty.is_empty() {
                    "&[]".to_string()
                } else {
                    format!("&{upper}_{kind}_{}_TYPES", screaming(&msg.name))
                };
                let _ = writeln!(out, "    Message::new(c\"{}\", c\"{}\", {types}),", msg.name, signature(msg));
            }
            out.push_str("];\n");
            Ok(out)
        };
        s.push_str(&tables(&i.requests, "REQUEST")?);
        s.push_str(&tables(&i.events, "EVENT")?);
        let destructor = i
            .requests
            .iter()
            .position(|r| r.destructor && r.args.is_empty())
            .map_or("None".to_string(), |o| format!("Some({o})"));
        let _ = writeln!(s, "\n/// The `{}` interface's table (libwayland's `wl_interface`).", i.name);
        let _ = writeln!(
            s,
            "pub static {upper}_INTERFACE: Interface = Interface::new(c\"{}\", {}, &{upper}_REQUESTS, &{upper}_EVENTS, \
             {destructor});\n",
            i.name, i.version
        );

        // The object's type.
        doc(&mut s, i.description.as_deref(), "");
        if i.name == "wl_display" {
            s.push_str(
                "///\n/// (The display is libwayland's: never destroyed by this type, which only borrows it.)\n",
            );
        }
        let _ = writeln!(s, "#[derive(Debug)]\npub struct {t}(Proxy);\n");
        let _ = writeln!(
            s,
            "impl Object for {t} {{\n    const INTERFACE: &'static Interface = &{upper}_INTERFACE;\n    \
             fn from_proxy(proxy: Proxy) -> Self {{\n        {t}(proxy)\n    }}\n    \
             fn proxy(&self) -> &Proxy {{\n        &self.0\n    }}\n}}\n"
        );
        if !i.requests.is_empty() {
            let _ = writeln!(s, "impl {t} {{");
            for (opcode, r) in i.requests.iter().enumerate() {
                emit_request(&mut s, i, r, opcode, names, m)?;
            }
            s.push_str("}\n\n");
        }

        // The events.
        if !i.events.is_empty() {
            let _ = writeln!(s, "/// The `{}` interface's events.\n#[derive(Debug)]\npub enum {t}Event {{", i.name);
            for e in &i.events {
                doc(&mut s, e.description.as_deref(), "    ");
                since_doc(&mut s, e, "    ");
                let variant = camel(&e.name);
                if e.args.is_empty() {
                    let _ = writeln!(s, "    {variant},");
                    continue;
                }
                let _ = writeln!(s, "    {variant} {{");
                for a in &e.args {
                    doc(&mut s, a.summary.as_deref(), "        ");
                    let _ = writeln!(s, "        {}: {},", ident(&a.name), event_field(a, i, m, names)?);
                }
                s.push_str("    },\n");
            }
            s.push_str("}\n\n");
            let _ = writeln!(
                s,
                "impl {t}Event {{\n    /// Event `opcode`'s arguments decoded (none if they aren't its).\n    \
                 pub fn decode(opcode: u16, args: &mut Args) -> Option<Self> {{\n        \
                 let _ = &args;\n        Some(match opcode {{"
            );
            for (opcode, e) in i.events.iter().enumerate() {
                let variant = camel(&e.name);
                if e.args.is_empty() {
                    let _ = writeln!(s, "            {opcode} => Self::{variant},");
                    continue;
                }
                let _ = writeln!(s, "            {opcode} => Self::{variant} {{");
                for a in &e.args {
                    let read = match a.kind {
                        Kind::Int | Kind::Uint => {
                            let raw = if a.kind == Kind::Int { "args.int()?" } else { "args.uint()?" };
                            match &a.r#enum {
                                Some(en) => {
                                    let ty = names.enum_type(en, &i.name, m)?;
                                    if a.kind == Kind::Int {
                                        format!("{ty}({raw} as u32)")
                                    } else {
                                        format!("{ty}({raw})")
                                    }
                                }
                                None => raw.to_string(),
                            }
                        }
                        Kind::Fixed => "args.fixed()?".to_string(),
                        Kind::String if a.nullable => "args.opt_string()?".to_string(),
                        Kind::String => "args.string()?".to_string(),
                        Kind::Object if a.nullable => "args.opt_object()?".to_string(),
                        Kind::Object => "args.object()?".to_string(),
                        Kind::NewId => "args.new_id()?".to_string(),
                        Kind::Array => "args.array()?".to_string(),
                        Kind::Fd => "args.fd()?".to_string(),
                    };
                    let _ = writeln!(s, "                {}: {read},", ident(&a.name));
                }
                s.push_str("            },\n");
            }
            s.push_str("            _ => return None,\n        })\n    }\n}\n\n");
        }

        // The enums.
        for e in &i.enums {
            let name = format!("{t}{}", camel(&e.name));
            match &e.description {
                Some(d) => doc(&mut s, Some(d), ""),
                None => {
                    let _ = writeln!(s, "/// `{}.{}`'s values.", i.name, e.name);
                }
            }
            if e.since > 1 {
                let _ = writeln!(s, "///\n/// Since version {}.", e.since);
            }
            let _ = writeln!(
                s,
                "#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]\npub struct {name}(pub u32);\n"
            );
            let _ = writeln!(s, "impl {name} {{");
            for entry in &e.entries {
                let mut text = entry.summary.clone().unwrap_or_default();
                if entry.since > 1 {
                    text = format!("{} (since version {})", text.trim(), entry.since);
                }
                if !text.trim().is_empty() {
                    for line in doc_lines(&text, "    /// ", 120) {
                        let _ = writeln!(s, "{line}");
                    }
                }
                let _ = writeln!(s, "    pub const {}: Self = Self({});", screaming(&entry.name), entry.value);
            }
            s.push_str("}\n\n");
            if e.bitfield {
                let _ = writeln!(s, "bitfield!({name});\n");
            }
        }
    }
    Ok(s)
}

fn since_doc(s: &mut String, m: &Message, indent: &str) {
    match (m.since > 1, m.deprecated_since) {
        (true, Some(d)) => {
            let _ = writeln!(s, "{indent}///\n{indent}/// Since version {}, deprecated since version {d}.", m.since);
        }
        (true, None) => {
            let _ = writeln!(s, "{indent}///\n{indent}/// Since version {}.", m.since);
        }
        (false, Some(d)) => {
            let _ = writeln!(s, "{indent}///\n{indent}/// Deprecated since version {d}.");
        }
        (false, None) => {}
    }
}

fn emit_request(
    s: &mut String,
    i: &Interface,
    r: &Message,
    opcode: usize,
    names: &Names,
    m: &str,
) -> Result<(), String> {
    doc(s, r.description.as_deref(), "    ");
    since_doc(s, r, "    ");
    if r.since > 1 {
        let _ = writeln!(s, "    /// (Not sent to an object of an older version: what it makes is then none.)");
    }
    let documented: Vec<&Arg> = r.args.iter().filter(|a| a.kind != Kind::NewId && a.summary.is_some()).collect();
    if !documented.is_empty() {
        s.push_str("    ///\n");
        for a in documented {
            let text = format!("`{}`: {}", a.name, a.summary.as_deref().unwrap_or("").trim());
            for (n, line) in doc_lines(&text, "    /// ", 120).into_iter().enumerate() {
                // (A list's items, continued indented.)
                let line = if n == 0 { line.replacen("/// ", "/// - ", 1) } else { line.replacen("/// ", "///   ", 1) };
                let _ = writeln!(s, "{line}");
            }
        }
    }
    let new_id = r.args.iter().find(|a| a.kind == Kind::NewId);
    let mut params = Vec::new();
    let mut prelude = Vec::new();
    let mut args = Vec::new();
    let mut generic = "";
    for a in &r.args {
        let name = ident(&a.name);
        match a.kind {
            Kind::NewId if a.interface.is_none() => {
                generic = "<T: Object>";
                params.push("version: u32".to_string());
                args.push("Arg::Str(Some(T::INTERFACE.name()))".to_string());
                args.push("Arg::Uint(version)".to_string());
                args.push("Arg::NewId".to_string());
            }
            Kind::NewId => args.push("Arg::NewId".to_string()),
            _ => {
                params.push(format!("{name}: {}", request_param(a, i, m, names)?));
                args.push(match a.kind {
                    Kind::Int if a.r#enum.is_some() => format!("Arg::Int({name}.0 as i32)"),
                    Kind::Int => format!("Arg::Int({name})"),
                    Kind::Uint if a.r#enum.is_some() => format!("Arg::Uint({name}.0)"),
                    Kind::Uint => format!("Arg::Uint({name})"),
                    Kind::Fixed => format!("Arg::Fixed({name})"),
                    Kind::String => {
                        let c = format!("c_{}", a.name);
                        if a.nullable {
                            prelude.push(format!("let {c} = {name}.map(c_string);"));
                            format!("Arg::Str({c}.as_deref())")
                        } else {
                            prelude.push(format!("let {c} = c_string({name});"));
                            format!("Arg::Str(Some(&{c}))")
                        }
                    }
                    Kind::Object => {
                        let proxy = if a.interface.is_some() { "o.proxy()" } else { "o" };
                        if a.nullable {
                            format!("Arg::Object({name}.map(|o| {proxy}))")
                        } else if a.interface.is_some() {
                            format!("Arg::Object(Some({name}.proxy()))")
                        } else {
                            format!("Arg::Object(Some({name}))")
                        }
                    }
                    Kind::Array => format!("Arg::Array({name})"),
                    Kind::Fd => format!("Arg::Fd({name})"),
                    Kind::NewId => unreachable!(),
                });
            }
        }
    }
    let receiver = if r.destructor { "self" } else { "&self" };
    let ret = match new_id {
        Some(a) => match &a.interface {
            Some(o) => format!(" -> Option<{}>", names.interface_path(o, m, &camel(o))?),
            None => " -> Option<T>".to_string(),
        },
        None => String::new(),
    };
    if new_id.is_some() {
        s.push_str("    #[must_use = \"the object made is destroyed when dropped\"]\n");
    }
    let all_params = std::iter::once(receiver.to_string()).chain(params).collect::<Vec<_>>().join(", ");
    let _ = writeln!(s, "    pub fn {}{generic}({all_params}){ret} {{", ident(&r.name));
    for line in &prelude {
        let _ = writeln!(s, "        {line}");
    }
    let list = args.join(", ");
    let call = match (new_id, r.destructor) {
        (Some(a), _) if a.interface.is_none() => format!("self.0.create_versioned({opcode}, version, &[{list}])"),
        (Some(_), _) => format!("self.0.create({opcode}, &[{list}])"),
        (None, true) => format!("self.0.destroy_with({opcode}, &[{list}])"),
        (None, false) => format!("self.0.send({opcode}, &[{list}])"),
    };
    let _ = writeln!(s, "        {call}{}", if ret.is_empty() { ";" } else { "" });
    s.push_str("    }\n\n");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const PROTOCOL: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<protocol name="test_v1">
  <interface name="wl_thing" version="3">
    <description summary="a thing">
      A thing that is. It does things.

      Another paragraph.
    </description>
    <request name="destroy" type="destructor"/>
    <request name="make" since="2">
      <arg name="id" type="new_id" interface="wl_thing"/>
      <arg name="title" type="string" allow-null="true" summary="its [title]"/>
      <arg name="edge" type="uint" enum="wl_thing.edge"/>
    </request>
    <request name="bind">
      <arg name="name" type="uint"/>
      <arg name="id" type="new_id"/>
    </request>
    <request name="move">
      <arg name="other" type="object" interface="wl_thing" allow-null="true"/>
      <arg name="x" type="fixed"/>
      <arg name="fd" type="fd"/>
    </request>
    <event name="made" since="3">
      <arg name="thing" type="new_id" interface="wl_thing"/>
      <arg name="data" type="array"/>
      <arg name="edge" type="int" enum="edge"/>
    </event>
    <enum name="edge" bitfield="true">
      <entry name="none" value="0"/>
      <entry name="top" value="0x1" summary="the top"/>
      <entry name="90" value="2" since="2"/>
    </enum>
  </interface>
</protocol>"#;

    fn generated() -> String {
        let p = read_protocol(PROTOCOL, "test_v1").unwrap();
        let names = Names { modules: [("wl_thing".to_string(), "test_v1".to_string())].into() };
        emit_protocol(&p, &names).unwrap()
    }

    #[test]
    fn signatures_are_wayland_scanners() {
        let p = read_protocol(PROTOCOL, "test_v1").unwrap();
        let sigs: Vec<String> = p.interfaces[0].requests.iter().map(signature).collect();
        assert_eq!(sigs, ["", "2n?su", "usun", "?ofh"]);
        assert_eq!(signature(&p.interfaces[0].events[0]), "3nai");
    }

    #[test]
    fn tables_name_their_interfaces() {
        let s = generated();
        assert!(s.contains(
            "static WL_THING_REQUEST_MAKE_TYPES: [Option<&Interface>; 3] = [Some(&WL_THING_INTERFACE), None, None];"
        ));
        assert!(s.contains("static WL_THING_REQUEST_BIND_TYPES: [Option<&Interface>; 4] = [None, None, None, None];"));
        assert!(s.contains("Message::new(c\"destroy\", c\"\", &[]),"));
        assert!(s.contains("Message::new(c\"make\", c\"2n?su\", &WL_THING_REQUEST_MAKE_TYPES),"));
        assert!(s.contains(
            "pub static WL_THING_INTERFACE: Interface = Interface::new(c\"wl_thing\", 3, &WL_THING_REQUESTS, \
             &WL_THING_EVENTS, Some(0));"
        ));
    }

    #[test]
    fn requests_are_typed_functions() {
        let s = generated();
        assert!(s.contains("pub fn destroy(self) {\n        self.0.destroy_with(0, &[]);"));
        assert!(s.contains("pub fn make(&self, title: Option<&str>, edge: WlThingEdge) -> Option<WlThing> {"));
        assert!(s.contains("self.0.create(1, &[Arg::NewId, Arg::Str(c_title.as_deref()), Arg::Uint(edge.0)])"));
        assert!(s.contains("pub fn bind<T: Object>(&self, name: u32, version: u32) -> Option<T> {"));
        assert!(s.contains(
            "self.0.create_versioned(2, version, &[Arg::Uint(name), Arg::Str(Some(T::INTERFACE.name())), \
             Arg::Uint(version), Arg::NewId])"
        ));
        assert!(s.contains("pub fn r#move(&self, other: Option<&WlThing>, x: f64, fd: BorrowedFd<'_>) {"));
        assert!(s.contains("/// - `title`: its \\[title\\]"));
    }

    #[test]
    fn events_and_enums_are_decoded() {
        let s = generated();
        assert!(s.contains("Made {"));
        assert!(s.contains("thing: WlThing,"));
        assert!(s.contains("thing: args.new_id()?,"));
        assert!(s.contains("data: args.array()?,"));
        assert!(s.contains("edge: WlThingEdge(args.int()? as u32),"));
        assert!(s.contains("pub struct WlThingEdge(pub u32);"));
        assert!(s.contains("pub const TOP: Self = Self(1);"));
        assert!(s.contains("pub const _90: Self = Self(2);"));
        assert!(s.contains("bitfield!(WlThingEdge);"));
        assert!(
            s.contains("/// A thing. A thing that is. It does things.\n#[derive(Debug)]\npub struct WlThing(Proxy);")
        );
    }

    #[test]
    fn docs_are_shortened_and_escaped() {
        let lines = doc_lines("a summary\n\nOne. Two e.g. this. Three <b>.\n\nMore.", "/// ", 30);
        assert_eq!(lines, ["/// A summary. One. Two e.g.", "/// this."]);
        assert_eq!(doc_lines("x\n\nsee [a] and <b> or *c*", "", 80), ["X. see \\[a\\] and \\<b\\> or \\*c\\*"]);
        // (No line starts a Markdown list.)
        assert_eq!(wrap("aaaa bbbb - cccc 1. dddd", "", 8), ["aaaa", "bbbb -", "cccc 1.", "dddd"]);
    }

    #[test]
    fn names_are_rusts() {
        assert_eq!(camel("zwp_text_input_v3"), "ZwpTextInputV3");
        assert_eq!(screaming("flipped_90"), "FLIPPED_90");
        assert_eq!(screaming("270"), "_270");
        assert_eq!(ident("type"), "r#type");
        assert_eq!(ident("self"), "self_");
        assert_eq!(ident("serial"), "serial");
    }

    #[test]
    fn the_protocols_generate() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        let files = generate(&root.join(PROTOCOLS), root).unwrap();
        let names: Vec<&str> = files.iter().map(|(n, _)| n.as_str()).collect();
        assert!(names.contains(&"wayland.rs") && names.contains(&"xdg_shell.rs") && names.contains(&"mod.rs"));
        let xdg = &files.iter().find(|(n, _)| n == "xdg_shell.rs").unwrap().1;
        assert!(xdg.contains("Some(&super::wayland::WL_SURFACE_INTERFACE)"));
    }
}
