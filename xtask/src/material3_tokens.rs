//! `cargo xtask material3-tokens [--check]`: material3's design tokens (Compose's generated `tokens/*.kt`, every
//! file of material3 1.5.0-alpha17's) converted to Rust, into `crates/kompoz-material3/src/tokens/` (committed): a
//! file a token object (`internal object ButtonSmallTokens { val ContainerHeight = 40.0.dp … }` → a unit struct
//! with associated constants), the key enums (`ColorSchemeKeyTokens`, `ShapeKeyTokens`, `TypographyKeyTokens`,
//! `MotionSchemeKeyTokens`) as Rust enums, `TypographyTokens` (a class of a font family) with its styles as methods,
//! and `mod.rs` declaring them.
//!
//! The values are written as Rust constants of KompozR's types: `56.0.dp` → `Dp(56.0)`, `16.sp` →
//! `TextUnit::new(16.0, TextUnitType::Sp)`, `0.38f` → `0.38`, `Color(red = …)` → `Color::from_argb(0xFF……)`,
//! `RoundedCornerShape(…)`, `CornerSize(…)`, `CircleShape`, `RectangleShape`, `CubicBezierEasing(…)`,
//! `FontFamily.X`, `FontWeight.X`, another token (`ElevationTokens.Level1`, `PaletteTokens.Neutral98`, the same
//! object's `Primary`). A form it doesn't know fails the conversion (a newer material3 needs it taught).
//!
//! It reads the sources jar from Gradle's cache; `--check` converts and compares with what is committed where the
//! jar is, and says it skipped where it isn't (CI).

use crate::jar::Jar;
use std::collections::BTreeMap;
use std::fmt::Write as _;

use std::path::Path;

const OUT: &str = "crates/kompoz-material3/src/tokens";
const GROUP: &str = "androidx.compose.material3";
const ARTIFACT: &str = "material3-android";
pub const VERSION: &str = "1.5.0-alpha17";
const PREFIX: &str = "commonMain/androidx/compose/material3/tokens/";

pub fn run(root: &Path, check: bool) -> Result<(), String> {
    let Some(jar) = Jar::find(GROUP, ARTIFACT, VERSION, "-sources.jar") else {
        if check {
            println!("material3-tokens: skipped (no {GROUP}:{ARTIFACT}:{VERSION} sources jar in Gradle's cache)");
            return Ok(());
        }
        return Err(format!("no {GROUP}:{ARTIFACT}:{VERSION} sources jar in Gradle's cache"));
    };
    let jar = Jar::open(&jar)?;
    let files = convert(&jar)?;
    crate::write_generated(root, OUT, &files, check, "material3-tokens")
}

/// A token file, parsed.
struct TokenFile {
    /// The object's (or enum's, class's) name: the file's, but where the file holds another
    /// (`IconButtonTokens.kt`'s `StandardIconButtonTokens`).
    kotlin_name: String,
    /// The file's name (without `.kt`), as the generated file's header says where it comes from.
    file_name: String,
    version: Option<String>,
    kind: FileKind,
}

enum FileKind {
    /// `internal enum class X { A, B }`.
    Enum(Vec<String>),
    /// `internal object X { val A = … }`.
    Object(Vec<(String, String)>),
    /// `internal class TypographyTokens(val fontFamily: FontFamily? = null) { val A = … }` and its file's top-level
    /// vals.
    TypographyClass { members: Vec<(String, String)>, top_level: Vec<(String, String)> },
}

/// A token's Rust value: its type, its expression, and whether it is a constant (else a function).
#[derive(Clone)]
struct Value {
    ty: String,
    expr: String,
}

fn convert(jar: &Jar) -> Result<BTreeMap<String, String>, String> {
    let mut parsed = Vec::new();
    for name in jar.names(PREFIX, ".kt") {
        let text = jar.text(&name)?;
        let kotlin_name = name.trim_start_matches(PREFIX).trim_end_matches(".kt").to_string();
        parsed.push(parse_file(&kotlin_name, &text).map_err(|e| format!("{name}: {e}"))?);
    }
    // The enums' entries and every object's values, resolved in order (a value may name another object's).
    let enums: BTreeMap<String, Vec<String>> = parsed
        .iter()
        .filter_map(|f| match &f.kind {
            FileKind::Enum(entries) => Some((f.kotlin_name.clone(), entries.clone())),
            _ => None,
        })
        .collect();
    let mut known: BTreeMap<(String, String), Value> = BTreeMap::new();
    let mut pending: Vec<(&str, &str, &str)> = Vec::new();
    for f in &parsed {
        if let FileKind::Object(members) = &f.kind {
            for (n, e) in members {
                pending.push((&f.kotlin_name, n, e));
            }
        }
    }
    // (in passes: a value naming one not known yet waits for the next)
    while !pending.is_empty() {
        let before = pending.len();
        let mut next = Vec::new();
        let mut last_error = String::new();
        for (object, name, expr) in pending {
            match value(object, expr, &enums, &known) {
                Ok(v) => {
                    known.insert((object.to_string(), name.to_string()), v);
                }
                Err(e) => {
                    last_error = format!("{object}.{name} = {expr}: {e}");
                    next.push((object, name, expr));
                }
            }
        }
        if next.len() == before {
            return Err(last_error);
        }
        pending = next;
    }
    let mut out = BTreeMap::new();
    let mut modules = Vec::new();
    for f in &parsed {
        let module = snake(&f.kotlin_name);
        let mut s = header(&f.file_name, f.version.as_deref());
        match &f.kind {
            FileKind::Enum(entries) => {
                let _ = writeln!(s, "\n/// Compose's `{}`.", f.kotlin_name);
                s.push_str("#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]\n");
                let _ = writeln!(s, "pub(crate) enum {} {{", f.kotlin_name);
                for e in entries {
                    let _ = writeln!(s, "    {e},");
                }
                s.push_str("}\n");
                let _ = write!(
                    s,
                    "\nimpl {} {{\n    /// Every entry, in Kotlin's order (Kotlin's `entries`).\n",
                    f.kotlin_name
                );
                let _ = writeln!(s, "    pub(crate) const ENTRIES: [{}; {}] = [", f.kotlin_name, entries.len());
                for e in entries {
                    let _ = writeln!(s, "        {}::{e},", f.kotlin_name);
                }
                s.push_str("    ];\n}\n");
            }
            FileKind::Object(members) => {
                s.push_str("\n#[allow(unused_imports)]\nuse super::prelude::*;\n");
                let _ = writeln!(s, "\n/// Compose's `{}`.", f.kotlin_name);
                let _ = writeln!(s, "pub(crate) struct {};", f.kotlin_name);
                let _ = writeln!(s, "\nimpl {} {{", f.kotlin_name);
                for (n, _) in members {
                    let v = &known[&(f.kotlin_name.clone(), n.clone())];
                    let _ = writeln!(s, "    pub(crate) const {n}: {} = {};", v.ty, v.expr);
                }
                s.push_str("}\n");
            }
            FileKind::TypographyClass { members, top_level } => {
                s.push_str("\n#[allow(unused_imports)]\nuse super::prelude::*;\n");
                s.push_str(&typography_class(&f.kotlin_name, members, top_level, &enums, &known)?);
            }
        }
        out.insert(format!("{module}.rs"), s);
        modules.push((module, f.kotlin_name.clone()));
    }
    // (by module name, as rustfmt orders the `mod`s and `use`s: a file named otherwise than its object sorts by the object)
    modules.sort();
    out.insert("mod.rs".to_string(), mod_rs(&modules));
    Ok(out)
}

fn header(kotlin_name: &str, version: Option<&str>) -> String {
    let version = version.map(|v| format!(" (VERSION: {v})")).unwrap_or_default();
    format!(
        "//! Generated by `cargo xtask material3-tokens` from material3 {VERSION}'s tokens/{kotlin_name}.kt{version}: \
         do not edit.\n"
    )
}

fn mod_rs(modules: &[(String, String)]) -> String {
    let mut s = format!(
        "//! material3's design tokens, as Compose's `androidx.compose.material3.tokens` (internal there): generated \
         by\n//! `cargo xtask material3-tokens` from material3 {VERSION}'s sources, do not edit. The components read them \
         as\n//! they come (a token set unused until its component is here; Compose's names, `ShapeKeyTokens`' all\n//! `Corner…`).\n\n#![allow(dead_code, unused_imports, clippy::enum_variant_names)]\n\n"
    );
    s.push_str(
        "/// What the token files name.\nmod prelude {\n    pub(super) use super::*;\n    \
         pub(super) use kompoz_animation::CubicBezierEasing;\n    pub(super) use kompoz_graphics::{\n        \
         CircleShape, Color, CornerSize, Dp, RectangleShape, RoundedCornerShape, TextUnit, TextUnitType,\n    };\n    \
         pub(super) use kompoz_text::style::line_height_style::{Alignment, Mode, Trim};\n    \
         pub(super) use kompoz_text::{FontFamily, FontWeight, LineHeightStyle, TextStyle};\n}\n\n",
    );
    for (m, _) in modules {
        let _ = writeln!(s, "#[rustfmt::skip]\nmod {m};");
    }
    s.push('\n');
    for (m, k) in modules {
        let _ = writeln!(s, "pub(crate) use {m}::{k};");
        // (the typography tokens' file's constants too, where rustfmt orders their `use`)
        if k == "TypographyTokens" {
            s.push_str("pub(crate) use typography_tokens::{DefaultLineHeightStyle, DefaultTextStyle};\n");
        }
    }
    s
}

// ---- Parsing ----------------------------------------------------------------------------------------------------

/// The text without comments.
fn strip_comments(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let b = text.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i..].starts_with(b"/*") {
            i = text[i + 2..].find("*/").map_or(b.len(), |e| i + 2 + e + 2);
        } else if b[i..].starts_with(b"//") {
            i = text[i..].find('\n').map_or(b.len(), |e| i + e);
        } else {
            let c = text[i..].chars().next().unwrap_or(' ');
            out.push(c);
            i += c.len_utf8();
        }
    }
    out
}

fn parse_file(kotlin_name: &str, text: &str) -> Result<TokenFile, String> {
    let version = text.lines().find_map(|l| l.trim().strip_prefix("// VERSION:")).map(|v| v.trim().to_string());
    let code = strip_comments(text);
    if let Some(at) = code.find("internal enum class ") {
        let body = braces(&code[at..]).ok_or("an enum without a body")?;
        let entries = body
            .split(',')
            .map(str::trim)
            .filter(|e| !e.is_empty())
            .map(|e| {
                if e.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
                    Ok(e.to_string())
                } else {
                    Err(format!("an enum entry `{e}`"))
                }
            })
            .collect::<Result<Vec<_>, _>>()?;
        return Ok(TokenFile {
            kotlin_name: kotlin_name.to_string(),
            file_name: kotlin_name.to_string(),
            version,
            kind: FileKind::Enum(entries),
        });
    }
    if let Some(at) = code.find("internal object ") {
        let body = braces(&code[at..]).ok_or("an object without a body")?;
        let members = vals(body)?;
        // (the object's own name: IconButtonTokens.kt holds `StandardIconButtonTokens`)
        let object = code[at + "internal object ".len()..]
            .split(|c: char| !c.is_ascii_alphanumeric() && c != '_')
            .next()
            .filter(|n| !n.is_empty())
            .ok_or("an object without a name")?;
        return Ok(TokenFile {
            kotlin_name: object.to_string(),
            file_name: kotlin_name.to_string(),
            version,
            kind: FileKind::Object(members),
        });
    }
    if let Some(at) = code.find("internal class TypographyTokens(val fontFamily: FontFamily? = null)") {
        let body = braces(&code[at..]).ok_or("a class without a body")?;
        let members = vals(body)?;
        let end = at + code[at..].find(body).unwrap_or(0) + body.len();
        let rest = &code[end..];
        let rest = rest.trim_start().strip_prefix('}').unwrap_or(rest);
        let top_level = vals(&rest.replace("internal val ", "val "))?;
        return Ok(TokenFile {
            kotlin_name: kotlin_name.to_string(),
            file_name: kotlin_name.to_string(),
            version,
            kind: FileKind::TypographyClass { members, top_level },
        });
    }
    Err("neither an enum class nor an object".into())
}

/// What is inside the first `{ … }` of `s`.
fn braces(s: &str) -> Option<&str> {
    let open = s.find('{')?;
    let mut depth = 0;
    for (i, c) in s[open..].char_indices() {
        match c {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(&s[open + 1..open + i]);
                }
            }
            _ => {}
        }
    }
    None
}

/// The `val name = expr` (and `const val`) declarations of a body, in order, their expressions on one line.
fn vals(body: &str) -> Result<Vec<(String, String)>, String> {
    let mut out = Vec::new();
    let mut rest = body;
    loop {
        rest = rest.trim_start();
        if rest.is_empty() {
            break;
        }
        let decl = rest.strip_prefix("const ").unwrap_or(rest);
        let Some(decl) = decl.strip_prefix("val ") else {
            return Err(format!("a declaration that isn't a val: `{}`", rest.lines().next().unwrap_or("")));
        };
        let eq = decl.find('=').ok_or("a val without a value")?;
        let name = decl[..eq].trim();
        if name.contains(':') {
            return Err(format!("a typed val `{name}`"));
        }
        // (the value runs to the next declaration at depth 0)
        let value = &decl[eq + 1..];
        let mut depth = 0i32;
        let mut end = value.len();
        let bytes = value.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            match bytes[i] {
                b'(' => depth += 1,
                b')' => depth -= 1,
                b'\n' if depth == 0 => {
                    let next = value[i..].trim_start();
                    if next.starts_with("val ") || next.starts_with("const val ") || next.is_empty() {
                        end = i;
                        break;
                    }
                }
                _ => {}
            }
            i += 1;
        }
        let expr: String = value[..end].split_whitespace().collect::<Vec<_>>().join(" ");
        out.push((name.to_string(), expr.replace("( ", "(").replace(", )", ")").replace(",)", ")")));
        rest = &value[end..];
    }
    Ok(out)
}

// ---- Values -----------------------------------------------------------------------------------------------------

fn value(
    object: &str,
    expr: &str,
    enums: &BTreeMap<String, Vec<String>>,
    known: &BTreeMap<(String, String), Value>,
) -> Result<Value, String> {
    let e = expr.trim();
    let v = |ty: &str, expr: String| Ok(Value { ty: ty.to_string(), expr });
    // Numbers: dp, sp, Float, Double.
    if let Some(n) = e.strip_suffix(".dp") {
        return v("Dp", format!("Dp({})", float(n)?));
    }
    if let Some(n) = e.strip_suffix(".sp") {
        return v("TextUnit", format!("TextUnit::new({}, TextUnitType::Sp)", float(n)?));
    }
    if let Some(n) = e.strip_suffix('f')
        && is_number(n)
    {
        return v("f32", float(n)?);
    }
    if is_number(e) {
        return if e.contains('.') { v("f64", float(e)?) } else { v("i32", e.to_string()) };
    }
    match e {
        "CircleShape" => return v("RoundedCornerShape", "CircleShape".into()),
        "RectangleShape" => return v("RectangleShape", "RectangleShape".into()),
        _ => {}
    }
    if let Some(args) = call(e, "Color") {
        let a = named_args(args)?;
        let c = |k: &str| -> Result<u32, String> {
            a.get(k).ok_or(format!("Color without {k}"))?.parse::<u32>().map_err(|_| format!("Color's {k}"))
        };
        return v(
            "Color",
            format!("Color::from_argb(0x{:08X})", 0xFF00_0000 | c("red")? << 16 | c("green")? << 8 | c("blue")?),
        );
    }
    if let Some(args) = call(e, "CubicBezierEasing") {
        let p: Vec<String> =
            split_args(args).iter().map(|a| float(a.trim_end_matches('f'))).collect::<Result<_, _>>()?;
        if p.len() != 4 {
            return Err("CubicBezierEasing of other than four".into());
        }
        return v("CubicBezierEasing", format!("CubicBezierEasing::constant({})", p.join(", ")));
    }
    if let Some(args) = call(e, "CornerSize") {
        return v("CornerSize", corner_size(args)?);
    }
    if let Some(args) = call(e, "RoundedCornerShape") {
        let corners = if args.contains('=') {
            let a = named_args(args)?;
            let get = |k: &str| a.get(k).cloned().ok_or(format!("RoundedCornerShape without {k}"));
            [get("topStart")?, get("topEnd")?, get("bottomEnd")?, get("bottomStart")?]
        } else {
            let a = args.trim().to_string();
            [a.clone(), a.clone(), a.clone(), a]
        };
        let c: Vec<String> = corners.iter().map(|c| corner_size(c)).collect::<Result<_, _>>()?;
        return v(
            "RoundedCornerShape",
            format!(
                "RoundedCornerShape {{ top_start: {}, top_end: {}, bottom_end: {}, bottom_start: {} }}",
                c[0], c[1], c[2], c[3]
            ),
        );
    }
    // A qualified name: an enum's entry, a library constant, another object's token.
    if let Some((q, name)) = e.rsplit_once('.')
        && is_ident(name)
    {
        if let Some(entries) = enums.get(q) {
            if !entries.iter().any(|x| x == name) {
                return Err(format!("no entry {name} in {q}"));
            }
            return v(q, format!("{q}::{name}"));
        }
        match q {
            "FontFamily" => return v("FontFamily", format!("FontFamily::{name}")),
            "FontWeight" => return v("FontWeight", format!("FontWeight::{name}")),
            _ => {}
        }
        if is_ident(q) {
            return match known.get(&(q.to_string(), name.to_string())) {
                Some(k) => v(&k.ty, format!("{q}::{name}")),
                None => Err(format!("{q}.{name} not known (yet)")),
            };
        }
    }
    // The same object's.
    if is_ident(e) {
        return match known.get(&(object.to_string(), e.to_string())) {
            Some(k) => v(&k.ty, format!("Self::{e}")),
            None => Err(format!("{e} not known (yet)")),
        };
    }
    Err("a form the converter doesn't know".into())
}

/// A corner size's argument: `4.0.dp` (or a percent `50`).
fn corner_size(arg: &str) -> Result<String, String> {
    let a = arg.trim();
    if let Some(n) = a.strip_suffix(".dp") {
        return Ok(format!("CornerSize::dp(Dp({}))", float(n)?));
    }
    if is_number(a) && !a.contains('.') {
        return Ok(format!("CornerSize::percent({a})"));
    }
    Err(format!("a corner size `{a}`"))
}

/// `Name(args)` → `args`.
fn call<'a>(e: &'a str, name: &str) -> Option<&'a str> {
    e.strip_prefix(name)?.strip_prefix('(')?.strip_suffix(')')
}

fn split_args(args: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut depth = 0;
    let mut cur = String::new();
    for c in args.chars() {
        match c {
            '(' => depth += 1,
            ')' => depth -= 1,
            ',' if depth == 0 => {
                if !cur.trim().is_empty() {
                    out.push(cur.trim().to_string());
                }
                cur.clear();
                continue;
            }
            _ => {}
        }
        cur.push(c);
    }
    if !cur.trim().is_empty() {
        out.push(cur.trim().to_string());
    }
    out
}

fn named_args(args: &str) -> Result<BTreeMap<String, String>, String> {
    split_args(args)
        .into_iter()
        .map(|a| {
            let (k, v) = a.split_once('=').ok_or(format!("an argument without a name: `{a}`"))?;
            Ok((k.trim().to_string(), v.trim().to_string()))
        })
        .collect()
}

fn is_ident(s: &str) -> bool {
    !s.is_empty()
        && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        && !s.starts_with(|c: char| c.is_ascii_digit())
}

fn is_number(s: &str) -> bool {
    let s = s.strip_prefix('-').unwrap_or(s);
    !s.is_empty() && s.chars().all(|c| c.is_ascii_digit() || c == '.') && s.chars().filter(|&c| c == '.').count() <= 1
}

/// A Kotlin number as a Rust float literal (`16` → `16.0`).
fn float(n: &str) -> Result<String, String> {
    let n = n.trim();
    if !is_number(n) {
        return Err(format!("a number `{n}`"));
    }
    Ok(if n.contains('.') { n.to_string() } else { format!("{n}.0") })
}

/// `ButtonSmallTokens` → `button_small_tokens`.
pub fn snake(name: &str) -> String {
    let mut s = String::new();
    let chars: Vec<char> = name.chars().collect();
    for (i, &c) in chars.iter().enumerate() {
        if c.is_ascii_uppercase() {
            let prev_lower = i > 0 && (chars[i - 1].is_ascii_lowercase() || chars[i - 1].is_ascii_digit());
            let next_lower = chars.get(i + 1).is_some_and(|n| n.is_ascii_lowercase());
            if i > 0 && (prev_lower || (next_lower && chars[i - 1].is_ascii_uppercase())) {
                s.push('_');
            }
            s.push(c.to_ascii_lowercase());
        } else {
            s.push(c);
        }
    }
    s
}

// ---- TypographyTokens -------------------------------------------------------------------------------------------

fn typography_class(
    name: &str,
    members: &[(String, String)],
    top_level: &[(String, String)],
    enums: &BTreeMap<String, Vec<String>>,
    known: &BTreeMap<(String, String), Value>,
) -> Result<String, String> {
    let mut s = String::new();
    for (n, e) in top_level {
        let (ty, expr) = match n.as_str() {
            "DefaultLineHeightStyle" => {
                let args = call(e, "LineHeightStyle").ok_or("DefaultLineHeightStyle's form")?;
                let a = named_args(args)?;
                let alignment = a
                    .get("alignment")
                    .and_then(|v| v.strip_prefix("LineHeightStyle.Alignment."))
                    .ok_or("DefaultLineHeightStyle's alignment")?;
                let trim = a.get("trim").and_then(|v| v.strip_prefix("LineHeightStyle.Trim.")).ok_or("its trim")?;
                ("LineHeightStyle", format!("LineHeightStyle::new(Alignment::{alignment}, Trim::{trim}, Mode::Fixed)"))
            }
            "DefaultTextStyle" => {
                let args = call(e, "TextStyle.Default.copy").ok_or("DefaultTextStyle's form")?;
                let a = named_args(args)?;
                if a.get("platformStyle").map(String::as_str) != Some("defaultPlatformTextStyle()")
                    || a.get("lineHeightStyle").map(String::as_str) != Some("DefaultLineHeightStyle")
                    || a.len() != 2
                {
                    return Err("DefaultTextStyle's arguments".into());
                }
                // (the desktop's defaultPlatformTextStyle() is null: CMP's DefaultPlatformTextStyle.skiko.kt)
                (
                    "TextStyle",
                    "TextStyle { platform_style: None, line_height_style: Some(DefaultLineHeightStyle), ..TextStyle::Default }"
                        .to_string(),
                )
            }
            other => return Err(format!("a top-level val {other}")),
        };
        let _ = writeln!(s, "\n/// Compose's `{n}`.\npub(crate) const {n}: {ty} = {expr};");
    }
    let _ = write!(
        s,
        "\n/// Compose's `{name}(fontFamily)`: the type scale's styles, in `font_family` where given.\npub(crate) struct \
         {name} {{\n    pub(crate) font_family: Option<FontFamily>,\n}}\n\nimpl {name} {{\n    pub(crate) fn new(font_family: \
         Option<FontFamily>) -> {name} {{\n        {name} {{ font_family }}\n    }}\n"
    );
    for (n, e) in members {
        let args = call(e, "DefaultTextStyle.copy").ok_or(format!("{n}'s form"))?;
        let a = named_args(args)?;
        let mut fields = Vec::new();
        for (k, v) in &a {
            let field = match k.as_str() {
                "fontFamily" => {
                    let token = v.strip_prefix("fontFamily ?: ").ok_or(format!("{n}'s fontFamily"))?;
                    let t = value(name, token, enums, known)?;
                    format!("font_family: Some(self.font_family.clone().unwrap_or({}))", t.expr)
                }
                "fontWeight" => format!("font_weight: Some({})", value(name, v, enums, known)?.expr),
                "fontSize" => format!("font_size: {}", value(name, v, enums, known)?.expr),
                "lineHeight" => format!("line_height: {}", value(name, v, enums, known)?.expr),
                "letterSpacing" => format!("letter_spacing: {}", value(name, v, enums, known)?.expr),
                other => return Err(format!("{n}'s {other}")),
            };
            fields.push(field);
        }
        let _ = write!(s, "\n    pub(crate) fn {n}(&self) -> TextStyle {{\n        TextStyle {{\n");
        for f in &fields {
            let _ = writeln!(s, "            {f},");
        }
        s.push_str("            ..DefaultTextStyle\n        }\n    }\n");
    }
    s.push_str("}\n");
    Ok(s)
}
