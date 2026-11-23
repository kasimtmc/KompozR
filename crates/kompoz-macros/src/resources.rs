//! `compose_resources!`: a crate's `composeResources/` directory as its `Res` (what CMP's Gradle plugin does at
//! build time: GenerateResourceAccessorsTask, XmlValuesConverterTask, GeneratedResClassSpec). The directory's
//! first-level directories are resource types with their qualifiers (`drawable-dark-xxhdpi`); `values*/*.xml` are
//! converted into CMP's records (`values*/<name>.commonMain.cvr`: `version:0`, then `type|key|base64` lines,
//! sorted); every file (and every converted file) is embedded in the program at its path in the resources
//! (`composeResources/<package>/…`), and `Res` has an accessor for each resource — `Res::drawable::logo()`,
//! `Res::string::app_name()`, `Res::array::…`, `Res::plurals::…`, `Res::font::…` (CMP's `Res.drawable.logo`) — with
//! an item for each set of qualifiers, `Res::read_bytes(path)`/`Res::get_uri(path)` for `files/`, and
//! `Res::all_drawable_resources()` … (CMP's `Res.allDrawableResources`). What CMP's build refuses (a file at the top,
//! `string*/`, qualifiers on `files/`, an unknown type or qualifier, a qualifier twice, a region without a language
//! or before it, a key twice, a values file that isn't valid) is a compile error here.

#[path = "../../kompoz-resources/src/vector/xmldom.rs"]
#[allow(dead_code)]
mod xmldom;

use proc_macro2::{Span, TokenStream as Ts};
use quote::{format_ident, quote};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::{Ident, LitBool, LitStr, Token};

/// `compose_resources!(…)`'s options.
pub(crate) struct Options {
    dir: Option<String>,
    package: Option<String>,
    name: Option<Ident>,
    public: bool,
}

enum Opt {
    Dir(LitStr),
    Package(LitStr),
    Name(Ident),
    Public(LitBool),
}

impl Parse for Opt {
    fn parse(input: ParseStream) -> syn::Result<Opt> {
        let key: Ident = input.parse()?;
        input.parse::<Token![=]>()?;
        match key.to_string().as_str() {
            "dir" => Ok(Opt::Dir(input.parse()?)),
            "package" => Ok(Opt::Package(input.parse()?)),
            "name" => Ok(Opt::Name(input.parse()?)),
            "public" => Ok(Opt::Public(input.parse()?)),
            _ => Err(syn::Error::new(
                key.span(),
                "kompoz: unknown option (`dir = \"…\"`, `package = \"…\"`, `name = Res`, `public = true`)",
            )),
        }
    }
}

impl Parse for Options {
    fn parse(input: ParseStream) -> syn::Result<Options> {
        let mut o = Options { dir: None, package: None, name: None, public: false };
        for opt in Punctuated::<Opt, Token![,]>::parse_terminated(input)? {
            match opt {
                Opt::Dir(s) => o.dir = Some(s.value()),
                Opt::Package(s) => o.package = Some(s.value()),
                Opt::Name(i) => o.name = Some(i),
                Opt::Public(b) => o.public = b.value,
            }
        }
        Ok(o)
    }
}

/// CMP's `ResourceType`: its directory's name and its accessor's.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum ResourceType {
    Drawable,
    String,
    StringArray,
    PluralString,
    Font,
}

impl ResourceType {
    const ALL: [ResourceType; 5] = [
        ResourceType::Drawable,
        ResourceType::String,
        ResourceType::StringArray,
        ResourceType::PluralString,
        ResourceType::Font,
    ];
    fn type_name(self) -> &'static str {
        match self {
            ResourceType::Drawable => "drawable",
            ResourceType::String => "string",
            ResourceType::StringArray => "string-array",
            ResourceType::PluralString => "plurals",
            ResourceType::Font => "font",
        }
    }
    fn accessor_name(self) -> &'static str {
        match self {
            ResourceType::Drawable => "drawable",
            ResourceType::String => "string",
            ResourceType::StringArray => "array",
            ResourceType::PluralString => "plurals",
            ResourceType::Font => "font",
        }
    }
    fn class_name(self) -> &'static str {
        match self {
            ResourceType::Drawable => "DrawableResource",
            ResourceType::String => "StringResource",
            ResourceType::StringArray => "StringArrayResource",
            ResourceType::PluralString => "PluralStringResource",
            ResourceType::Font => "FontResource",
        }
    }
    fn requires_key_name(self) -> bool {
        matches!(self, ResourceType::String | ResourceType::StringArray | ResourceType::PluralString)
    }
    fn from_string(s: &str) -> Option<ResourceType> {
        ResourceType::ALL.into_iter().find(|t| t.type_name().eq_ignore_ascii_case(s))
    }
}

/// CMP's build-time `ResourceItem`.
#[derive(Clone, Debug)]
struct Item {
    kind: ResourceType,
    qualifiers: Vec<String>,
    name: String,
    /// Its path relative to the directory, `/`-separated.
    path: String,
    content_hash: i32,
    offset: i64,
    size: i64,
}

/// A file to embed: its path in the resources, its bytes (or its file's), and the file it came from.
struct Embedded {
    path: String,
    source: PathBuf,
    /// Converted bytes (values records); none: the source file's, included.
    converted: Option<Vec<u8>>,
}

/// CMP's `asUnderscoredIdentifier`: `-` as `_`, a leading digit after `_`.
fn underscored(s: &str) -> String {
    let s = s.replace('-', "_");
    if s.starts_with(|c: char| c.is_ascii_digit()) { format!("_{s}") } else { s }
}

/// Java's `Arrays.hashCode(byte[])` (Kotlin's `contentHashCode`).
fn content_hash(bytes: &[u8]) -> i32 {
    bytes.iter().fold(1i32, |h, b| h.wrapping_mul(31).wrapping_add(*b as i8 as i32))
}

/// Java's `String.hashCode()`.
fn string_hash(s: &str) -> i32 {
    s.encode_utf16().fold(0i32, |h, c| h.wrapping_mul(31).wrapping_add(c as i32))
}

/// Not hidden (a name starting with `.`), in name order.
fn list_not_hidden(dir: &Path) -> Result<Vec<PathBuf>, String> {
    let mut out: Vec<PathBuf> = fs::read_dir(dir)
        .map_err(|e| format!("can't read {}: {e}", dir.display()))?
        .flatten()
        .map(|e| e.path())
        .filter(|p| !p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with('.')))
        .collect();
    out.sort();
    Ok(out)
}

fn file_name(p: &Path) -> String {
    p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

fn name_without_extension(p: &Path) -> String {
    let n = file_name(p);
    match n.rfind('.') {
        Some(i) if i > 0 => n[..i].to_string(),
        _ => n,
    }
}

fn extension(p: &Path) -> String {
    let n = file_name(p);
    match n.rfind('.') {
        Some(i) if i > 0 => n[i + 1..].to_string(),
        _ => String::new(),
    }
}

/// CMP's `handleSpecialCharacters`: `\n`, `\t`, `\uXXXX` (not right after a `\\`), then `\\` as `\`.
fn handle_special_characters(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    // (the starts of `\\` pairs, found left to right without overlapping)
    let mut double_slashes = Vec::new();
    let mut i = 0;
    while i + 1 < chars.len() {
        if chars[i] == '\\' && chars[i + 1] == '\\' {
            double_slashes.push(i);
            i += 2;
        } else {
            i += 1;
        }
    }
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '\\' && i + 1 < chars.len() {
            let after_double = i > 0 && double_slashes.contains(&(i - 1));
            let next = chars[i + 1];
            let unicode =
                next == 'u' && i + 6 <= chars.len() && chars[i + 2..i + 6].iter().all(|c| c.is_ascii_hexdigit());
            if next == 'n' || next == 't' || unicode {
                let len = if unicode { 6 } else { 2 };
                if after_double {
                    out.extend(&chars[i..i + len]);
                } else if next == 'n' {
                    out.push('\n');
                } else if next == 't' {
                    out.push('\t');
                } else {
                    let hex: String = chars[i + 2..i + 6].iter().collect();
                    let code = u32::from_str_radix(&hex, 16).unwrap_or(0xFFFD);
                    // (Kotlin's `toChar()`: a UTF-16 unit; a lone surrogate can't be a Rust char)
                    out.push(char::from_u32(code).unwrap_or('\u{FFFD}'));
                }
                i += len;
                continue;
            }
        }
        out.push(chars[i]);
        i += 1;
    }
    out.replace("\\\\", "\\")
}

fn base64(bytes: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n =
            (chunk[0] as u32) << 16 | (*chunk.get(1).unwrap_or(&0) as u32) << 8 | *chunk.get(2).unwrap_or(&0) as u32;
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 { T[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if chunk.len() > 2 { T[n as usize & 63] as char } else { '=' });
    }
    out
}

/// A values XML converted into CMP's records file (XmlValuesConverterTask.convert).
fn convert_values(file: &Path) -> Result<String, String> {
    let bytes = fs::read(file).map_err(|e| format!("can't read {}: {e}", file.display()))?;
    let root = xmldom::parse_xml_bytes(&bytes)
        .map_err(|_| format!("XML file {} is not valid. Check the file content.", file.display()))?;
    let invalid = |m: String| format!("XML file {} is not valid. {m}", file.display());
    // (`getElementsByTagName("resources").item(0)`: the first such element, the root or within it)
    fn find(e: &xmldom::Element) -> Option<&xmldom::Element> {
        if e.node_name == "resources" {
            return Some(e);
        }
        e.child_elements().find_map(find)
    }
    let resources = find(&root).ok_or_else(|| invalid("Cannot invoke \"org.w3c.dom.Node.getChildNodes()\"".into()))?;
    let mut records: Vec<(ResourceType, String, String)> = Vec::new();
    for node in resources.child_elements().filter(|e| !e.attributes.is_empty()) {
        let kind = ResourceType::from_string(&node.node_name)
            .ok_or_else(|| invalid(format!("Unknown resource type: '{}'.", node.node_name)))?;
        if !node.has_attribute("name") {
            return Err(invalid("Attribute 'name' not found.".into()));
        }
        let key = node.get_attribute("name").to_string();
        let items = || node.child_elements().filter(|c| c.node_name == "item");
        let value = match kind {
            ResourceType::String => base64(handle_special_characters(&node.text_content()).as_bytes()),
            ResourceType::StringArray => items()
                .map(|c| base64(handle_special_characters(&c.text_content()).as_bytes()))
                .collect::<Vec<_>>()
                .join(","),
            ResourceType::PluralString => {
                let mut parts = Vec::new();
                for c in items() {
                    if !c.has_attribute("quantity") {
                        return Err(invalid("Cannot invoke \"org.w3c.dom.Node.getNodeValue()\"".into()));
                    }
                    let content = handle_special_characters(&c.text_content());
                    parts.push(format!(
                        "{}:{}",
                        c.get_attribute("quantity").to_uppercase(),
                        base64(content.as_bytes())
                    ));
                }
                parts.join(",")
            }
            _ => return Err(invalid(format!("Unknown string resource type: '{}'.", kind.type_name()))),
        };
        records.push((kind, key, value));
    }
    // (no key twice for one type)
    let mut by_key: BTreeMap<&str, Vec<ResourceType>> = BTreeMap::new();
    for (k, key, _) in &records {
        by_key.entry(key).or_default().push(*k);
    }
    for (key, kinds) in by_key {
        let mut unique = kinds.clone();
        unique.sort();
        unique.dedup();
        if unique.len() != kinds.len() {
            return Err(invalid(format!("Duplicated key '{key}'.")));
        }
    }
    let mut lines: Vec<String> = records.iter().map(|(k, key, v)| format!("{}|{key}|{v}", k.type_name())).collect();
    lines.sort();
    let mut out = String::from("version:0\n");
    for l in lines {
        out.push_str(&l);
        out.push('\n');
    }
    Ok(out)
}

/// The items of a converted values file: its lines after the first, each at its offset (getValueResourceItems).
fn value_items(content: &str, qualifiers: &[String], path: &str) -> Result<Vec<Item>, String> {
    let mut out = Vec::new();
    let mut offset: i64 = 0;
    for line in content.split_terminator('\n') {
        let size = line.len() as i64;
        if offset > 0 {
            let parts: Vec<&str> = line.split('|').collect();
            let kind = ResourceType::from_string(parts[0]).ok_or_else(|| format!("bad record in {path}"))?;
            out.push(Item {
                kind,
                qualifiers: qualifiers.to_vec(),
                name: underscored(parts[1]),
                path: path.to_string(),
                content_hash: string_hash(parts[2]),
                offset,
                size,
            });
        }
        offset += size + 1;
    }
    Ok(out)
}

/// The qualifiers of an item as the generated code makes them (addQualifiers): theme, density, language, region.
fn qualifiers_of(item: &Item, r: &Ts) -> Result<Vec<Ts>, String> {
    let mut theme = None;
    let mut density = None;
    let mut language: Option<String> = None;
    let mut region: Option<String> = None;
    let repeated = |a: &str, b: &str| format!("{} contains repetitive qualifiers: '{a}' and '{b}'.", item.path);
    for q in &item.qualifiers {
        let slot = match q.as_str() {
            "light" | "dark" => &mut theme,
            "mdpi" | "hdpi" | "xhdpi" | "xxhdpi" | "xxxhdpi" | "ldpi" => &mut density,
            _ if (2..=3).contains(&q.len()) && q.bytes().all(|c| c.is_ascii_lowercase()) => &mut language,
            _ if q.len() == 3 && q.starts_with('r') && q[1..].bytes().all(|c| c.is_ascii_uppercase()) => &mut region,
            _ => return Err(format!("{} contains unknown qualifier: '{q}'.", item.path)),
        };
        if let Some(previous) = slot {
            return Err(repeated(previous, q));
        }
        *slot = Some(q.clone());
    }
    let mut out = Vec::new();
    if let Some(t) = theme {
        let v = format_ident!("{}", if t == "dark" { "Dark" } else { "Light" });
        out.push(quote!(#r::Qualifier::Theme(#r::ThemeQualifier::#v)));
    }
    if let Some(d) = density {
        let mut name = d.clone();
        name[..1].make_ascii_uppercase();
        let v = format_ident!("{name}");
        out.push(quote!(#r::Qualifier::Density(#r::DensityQualifier::#v)));
    }
    if let Some(l) = &language {
        out.push(quote!(#r::Qualifier::Language(#r::LanguageQualifier::new(#l))));
    }
    if let Some(q) = region {
        let Some(lang) = &language else {
            return Err(format!("Region qualifier must be used only with language.\nFile: {}", item.path));
        };
        let both = format!("{lang}-{q}");
        if !item.path.contains(&format!("-{both}")) {
            return Err(format!("Region qualifier must be declared after language: '{both}'.\nFile: {}", item.path));
        }
        let code = &q[1..];
        out.push(quote!(#r::Qualifier::Region(#r::RegionQualifier::new(#code))));
    }
    Ok(out)
}

/// A name as a Rust identifier: a keyword raw (`r#type`), one that can't be raw with `_` after it, other characters
/// (Kotlin escapes them in backticks) `_`.
fn ident_of(name: &str) -> Ident {
    let mut s: String = name.chars().map(|c| if c.is_alphanumeric() || c == '_' { c } else { '_' }).collect();
    if s.is_empty() {
        s.push('_');
    }
    if matches!(s.as_str(), "self" | "Self" | "super" | "crate" | "_") {
        s.push('_');
        return Ident::new(&s, Span::call_site());
    }
    syn::parse_str::<Ident>(&s).unwrap_or_else(|_| Ident::new_raw(&s, Span::call_site()))
}

/// The resources' crate path where the macro is called.
fn resources_path() -> Ts {
    use proc_macro_crate::{FoundCrate, crate_name};
    match crate_name("kompoz") {
        Ok(FoundCrate::Itself) => return quote!(::kompoz::resources),
        Ok(FoundCrate::Name(n)) => {
            let i = format_ident!("{n}");
            return quote!(::#i::resources);
        }
        Err(_) => {}
    }
    match crate_name("kompoz-resources") {
        Ok(FoundCrate::Name(n)) => {
            let i = format_ident!("{n}");
            quote!(::#i)
        }
        Ok(FoundCrate::Itself) => quote!(crate),
        Err(_) => quote!(::kompoz_resources),
    }
}

pub(crate) fn expand(options: Options) -> Result<Ts, String> {
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").map_err(|_| "kompoz: no CARGO_MANIFEST_DIR")?);
    let root = manifest.join(options.dir.as_deref().unwrap_or("composeResources"));
    let package = match options.package {
        Some(p) => p,
        None => {
            let name = std::env::var("CARGO_PKG_NAME").unwrap_or_default().to_lowercase();
            format!("{}.generated.resources", underscored(&name))
        }
    };
    let module_dir = format!("composeResources/{package}/");
    let res_name = options.name.unwrap_or_else(|| format_ident!("Res"));
    let r = resources_path();

    let mut items: Vec<Item> = Vec::new();
    let mut embedded: Vec<Embedded> = Vec::new();
    let mut tracked: Vec<PathBuf> = Vec::new();
    if root.is_dir() {
        for dir in list_not_hidden(&root)? {
            if !dir.is_dir() {
                return Err(format!(
                    "{} is not directory! Raw files should be placed in '{}/files' directory.",
                    file_name(&dir),
                    file_name(&root)
                ));
            }
            let dir_name = file_name(&dir);
            let mut parts = dir_name.split('-');
            let type_string = parts.next().unwrap_or("").to_lowercase();
            let qualifiers: Vec<String> = parts.map(str::to_string).collect();
            if type_string == "string" {
                return Err(format!(
                    "Forbidden directory name '{dir_name}'! String resources should be declared in 'values/strings.xml'."
                ));
            }
            if type_string == "files" && !qualifiers.is_empty() {
                return Err(format!("The 'files' directory doesn't support qualifiers: '{dir_name}'."));
            }
            for file in walk(&dir)? {
                let relative = file.strip_prefix(&root).unwrap_or(&file).to_string_lossy().replace('\\', "/");
                let is_values_xml = type_string.starts_with("values")
                    && file.parent() == Some(dir.as_path())
                    && extension(&file).eq_ignore_ascii_case("xml");
                if is_values_xml && type_string == "values" {
                    let converted = convert_values(&file)?;
                    let path = format!("{dir_name}/{}.commonMain.cvr", name_without_extension(&file));
                    items.extend(value_items(&converted, &qualifiers, &path)?);
                    tracked.push(file.clone());
                    embedded.push(Embedded {
                        path: format!("{module_dir}{path}"),
                        source: file.clone(),
                        converted: Some(converted.into_bytes()),
                    });
                    continue;
                }
                if is_values_xml {
                    // (`values…` with another first part: CMP converts it too, then refuses its type)
                    return Err(format!("Unknown resource type: '{type_string}'."));
                }
                embedded.push(Embedded {
                    path: format!("{module_dir}{relative}"),
                    source: file.clone(),
                    converted: None,
                });
                if type_string == "files" || file.parent() != Some(dir.as_path()) {
                    continue; // (raw files; what is deeper isn't a resource of its own, but is packaged)
                }
                let kind = ResourceType::from_string(&type_string)
                    .ok_or_else(|| format!("Unknown resource type: '{type_string}'."))?;
                let bytes = fs::read(&file).map_err(|e| format!("can't read {}: {e}", file.display()))?;
                items.push(Item {
                    kind,
                    qualifiers: qualifiers.clone(),
                    name: underscored(&name_without_extension(&file)),
                    path: relative,
                    content_hash: content_hash(&bytes),
                    offset: -1,
                    size: -1,
                });
            }
        }
    }

    // type → name → items (sorted by path)
    let mut by_type: BTreeMap<ResourceType, BTreeMap<String, Vec<Item>>> = BTreeMap::new();
    for item in items {
        by_type.entry(item.kind).or_default().entry(item.name.clone()).or_default().push(item);
    }
    let mut accessor_mods = Vec::new();
    let mut all_fns = Vec::new();
    for kind in ResourceType::ALL {
        let resources = by_type.remove(&kind).unwrap_or_default();
        let class = format_ident!("{}", kind.class_name());
        let mut accessors = Vec::new();
        let mut collect = Vec::new();
        for (name, mut list) in resources {
            list.sort_by(|a, b| a.path.cmp(&b.path));
            let id = format!("{}:{name}", kind.type_name());
            let mut item_tokens = Vec::new();
            for item in &list {
                let quals = qualifiers_of(item, &r)?;
                let path = format!("{module_dir}{}", item.path);
                let (offset, size) = (item.offset, item.size);
                item_tokens.push(quote!(#r::ResourceItem::new(vec![#(#quals),*], #path, #offset, #size)));
            }
            let hash = list.iter().fold(0i32, |acc, i| acc.wrapping_mul(31).wrapping_add(i.content_hash));
            let make = if kind.requires_key_name() {
                quote!(#r::#class::new(#id, #name, vec![#(#item_tokens),*]))
            } else {
                quote!(#r::#class::new(#id, vec![#(#item_tokens),*]))
            };
            let f = ident_of(&name);
            let doc = format!("`{}` (CMP's `Res.{}.{name}`). ResourceContentHash: {hash}.", id, kind.accessor_name());
            accessors.push(quote! {
                #[doc = #doc]
                pub fn #f() -> #r::#class {
                    super::register();
                    static RESOURCE: #r::__private::OnceLock<#r::#class> = #r::__private::OnceLock::new();
                    RESOURCE.get_or_init(|| #make).clone()
                }
            });
            collect.push(quote!((#name, #f as fn() -> #r::#class)));
        }
        let m = format_ident!("{}", kind.accessor_name());
        let mod_doc = format!("CMP's `Res.{}`: the {} resources.", kind.accessor_name(), kind.type_name());
        accessor_mods.push(quote! {
            #[doc = #mod_doc]
            pub mod #m {
                #(#accessors)*
                pub(super) const ALL: &[(&str, fn() -> #r::#class)] = &[#(#collect),*];
            }
        });
        let all = format_ident!("all_{}s", snake(kind.class_name()));
        let all_doc = format!("Every {} by its name (CMP's `Res.all{}s`).", kind.class_name(), kind.class_name());
        all_fns.push(quote! {
            #[doc = #all_doc]
            pub fn #all() -> &'static ::std::collections::BTreeMap<::std::string::String, #r::#class> {
                static ALL: #r::__private::OnceLock<::std::collections::BTreeMap<::std::string::String, #r::#class>> =
                    #r::__private::OnceLock::new();
                ALL.get_or_init(|| #m::ALL.iter().map(|(n, f)| (n.to_string(), f())).collect())
            }
        });
    }

    let files: Vec<Ts> = embedded
        .iter()
        .map(|e| {
            let path = &e.path;
            let source = e.source.to_string_lossy().into_owned();
            let bytes = match &e.converted {
                Some(b) => {
                    let lit = syn::LitByteStr::new(b, Span::call_site());
                    quote!(#lit)
                }
                None => quote!(include_bytes!(#source)),
            };
            quote!(#r::__private::EmbeddedFile { path: #path, bytes: #bytes, source: #source })
        })
        .collect();
    let tracked: Vec<String> = tracked.iter().map(|p| p.to_string_lossy().into_owned()).collect();
    let vis = if options.public { quote!(pub) } else { quote!(pub(crate)) };
    let res_doc = format!(
        "The resources of `{}` (CMP's generated `Res` of package `{package}`): accessors by type, raw files by path.",
        root.display()
    );
    Ok(quote! {
        #[doc = #res_doc]
        #[allow(non_snake_case, non_upper_case_globals, dead_code, clippy::all)]
        #vis mod #res_name {
            // (the values files: their changes rebuild this)
            #(const _: &[u8] = include_bytes!(#tracked);)*

            const MD: &str = #module_dir;

            static FILES: &[#r::__private::EmbeddedFile] = &[#(#files),*];

            /// Its files made readable (once), its drawables known by name (for references between them).
            pub(crate) fn register() {
                static ONCE: ::std::sync::Once = ::std::sync::Once::new();
                ONCE.call_once(|| {
                    #r::__private::register_embedded_files(FILES);
                    #r::__private::register_drawables(MD, || {
                        drawable::ALL.iter().map(|(n, f)| (n.to_string(), f())).collect()
                    });
                });
            }

            /// A file's bytes by its path in this directory (CMP's `Res.readBytes("files/key.bin")`); panics if
            /// there is none (Kotlin's MissingResourceException; `try_read_bytes` gives it as an error).
            pub async fn read_bytes(path: &str) -> ::std::vec::Vec<u8> {
                try_read_bytes(path).await.unwrap_or_else(|e| ::std::panic!("{e}"))
            }

            /// The same, a missing file an error.
            pub async fn try_read_bytes(path: &str) -> ::std::result::Result<::std::vec::Vec<u8>, #r::MissingResourceException> {
                register();
                #r::read_resource_bytes(&::std::format!("{}{}", MD, path)).await
            }

            /// A file's URI by its path in this directory (CMP's `Res.getUri("files/key.bin")`); panics if there is
            /// none (Kotlin's MissingResourceException; `try_get_uri` gives it as an error).
            pub fn get_uri(path: &str) -> ::std::string::String {
                try_get_uri(path).unwrap_or_else(|e| ::std::panic!("{e}"))
            }

            /// The same, a missing file an error.
            pub fn try_get_uri(path: &str) -> ::std::result::Result<::std::string::String, #r::MissingResourceException> {
                register();
                #r::get_resource_uri(&::std::format!("{}{}", MD, path))
            }

            #(#accessor_mods)*
            #(#all_fns)*
        }
    })
}

/// `DrawableResource` → `drawable_resource`.
fn snake(s: &str) -> String {
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if c.is_ascii_uppercase() {
            if i > 0 {
                out.push('_');
            }
            out.push(c.to_ascii_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

/// The files in `dir` and below (not hidden), in order.
fn walk(dir: &Path) -> Result<Vec<PathBuf>, String> {
    let mut out = Vec::new();
    for p in list_not_hidden(dir)? {
        if p.is_dir() {
            out.extend(walk(&p)?);
        } else {
            out.push(p);
        }
    }
    Ok(out)
}
