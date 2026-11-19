//! Android's resources as Compose's XML parsers read them, without Android's resource system: references
//! (`@animator/spin`, `@android:interpolator/fast_out_slow_in`, `?attr/colorAccent`) looked up in what the caller
//! supplies ([`XmlResources`]) and in the framework's public interpolators; attribute values compiled as Android's
//! resource compiler (aapt2) compiles them for their attribute's format (`#F00` a color, `300` an integer, `0.5` a
//! float, an enum's name its value, else a string where the format takes one); and read as `TypedArray` reads
//! them (`getFloat` of an integer, `getColor` of a color state list, `peekValue`'s type).

use super::xml::{ANDROID_NS, XmlElement, XmlPullParser, read_document};
use kompoz_graphics::Color;
use std::fmt;
use std::rc::Rc;

// ---- What the caller supplies -------------------------------------------------------------------------------------

/// A resource an XML resource refers to (Android's `@[package:]type/name`, or a theme attribute's
/// `?[package:][attr/]name`): `@animator/spin` is type `animator`, name `spin`, no package; `@android:color/white`
/// package `android`.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct ResourceReference {
    /// The package (`android` for the framework's); none for the app's own.
    pub package: Option<String>,
    /// Its type: `drawable`, `animator`, `anim`, `interpolator`, `color`, `integer`, `dimen`, `string`, … (`attr`
    /// for a theme attribute).
    pub r#type: String,
    pub name: String,
    /// A theme attribute (`?attr/…`): its value in the theme.
    pub is_theme_attribute: bool,
}

impl fmt::Display for ResourceReference {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let sigil = if self.is_theme_attribute { '?' } else { '@' };
        match &self.package {
            Some(p) => write!(f, "{sigil}{p}:{}/{}", self.r#type, self.name),
            None => write!(f, "{sigil}{}/{}", self.r#type, self.name),
        }
    }
}

/// The resources references in XML resources are looked up in (KompozR has no Android resource system: the caller
/// supplies them; a closure `Fn(&ResourceReference) -> Option<String>` is one). [`resolve`](XmlResources::resolve)
/// gives a resource's text: a file resource's XML (an `@drawable/` `<vector>`, an `@animator/` `<set>` or
/// `<objectAnimator>`, an `@interpolator/` or `@anim/` interpolator, an `@color/` `<selector>`), or a value
/// resource's value as `res/values` writes it (`@color/red`: `#FFFF0000`; `@integer/duration`: `300`; a theme
/// attribute: its value, itself maybe a reference). None: there is no such resource (reading fails as Android's
/// `Resources.NotFoundException`).
///
/// The framework's public interpolators (`@android:interpolator/…`, `@android:anim/…_interpolator`) are always
/// there and aren't asked for; other `@android:` resources are.
pub trait XmlResources {
    fn resolve(&self, reference: &ResourceReference) -> Option<String>;
}

impl<F: Fn(&ResourceReference) -> Option<String>> XmlResources for F {
    fn resolve(&self, reference: &ResourceReference) -> Option<String> {
        self(reference)
    }
}

/// No resources of the app's: only the framework's interpolators resolve.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoXmlResources;

impl XmlResources for NoXmlResources {
    fn resolve(&self, _: &ResourceReference) -> Option<String> {
        None
    }
}

/// Why a resource couldn't be read: the Java exception Android (or Compose's parser) throws there, and its message.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct XmlResourceError {
    /// The exception's class: `XmlPullParserException` (malformed XML, an unknown tag), `NotFoundException` (a
    /// reference to nothing), `RuntimeException` (an unknown interpolator), `IllegalArgumentException` (a path
    /// interpolator's path), `NumberFormatException`, `NullPointerException` (a required attribute missing),
    /// `UnsupportedOperationException` (a value of the wrong type), `ClassCastException`.
    pub exception: &'static str,
    pub message: String,
}

impl XmlResourceError {
    pub(crate) fn new(exception: &'static str, message: impl Into<String>) -> XmlResourceError {
        XmlResourceError { exception, message: message.into() }
    }
}

impl fmt::Display for XmlResourceError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}: {}", self.exception, self.message)
    }
}

impl std::error::Error for XmlResourceError {}

fn not_found(what: impl fmt::Display) -> XmlResourceError {
    XmlResourceError::new("NotFoundException", format!("Resource {what} not found"))
}

// ---- Resources --------------------------------------------------------------------------------------------------

/// A resource a value refers to: an inline one (an `<aapt:attr>`'s element) or a named one.
#[derive(Clone, Debug)]
pub(crate) enum Resource {
    Inline(Rc<XmlElement>),
    Named(ResourceReference),
}

/// The framework's public interpolators, as their files are (Android 37's `res/anim/*_interpolator.xml` and
/// `res/interpolator/*.xml`).
const FRAMEWORK_INTERPOLATORS: &[(&str, &str, &str)] = &[
    ("anim", "accelerate_decelerate_interpolator", "<accelerateDecelerateInterpolator />"),
    ("anim", "accelerate_interpolator", "<accelerateInterpolator />"),
    ("anim", "anticipate_interpolator", "<anticipateInterpolator />"),
    ("anim", "anticipate_overshoot_interpolator", "<anticipateOvershootInterpolator />"),
    ("anim", "bounce_interpolator", "<bounceInterpolator />"),
    ("anim", "cycle_interpolator", "<cycleInterpolator />"),
    ("anim", "decelerate_interpolator", "<decelerateInterpolator />"),
    ("anim", "linear_interpolator", "<linearInterpolator />"),
    ("anim", "overshoot_interpolator", "<overshootInterpolator />"),
    ("interpolator", "accelerate_quad", "<accelerateInterpolator />"),
    ("interpolator", "decelerate_quad", "<decelerateInterpolator />"),
    (
        "interpolator",
        "accelerate_cubic",
        r#"<accelerateInterpolator xmlns:android="http://schemas.android.com/apk/res/android" android:factor="1.5" />"#,
    ),
    (
        "interpolator",
        "decelerate_cubic",
        r#"<decelerateInterpolator xmlns:android="http://schemas.android.com/apk/res/android" android:factor="1.5" />"#,
    ),
    (
        "interpolator",
        "accelerate_quint",
        r#"<accelerateInterpolator xmlns:android="http://schemas.android.com/apk/res/android" android:factor="2.5" />"#,
    ),
    (
        "interpolator",
        "decelerate_quint",
        r#"<decelerateInterpolator xmlns:android="http://schemas.android.com/apk/res/android" android:factor="2.5" />"#,
    ),
    ("interpolator", "accelerate_decelerate", "<accelerateDecelerateInterpolator />"),
    ("interpolator", "anticipate", "<anticipateInterpolator />"),
    ("interpolator", "overshoot", "<overshootInterpolator />"),
    ("interpolator", "anticipate_overshoot", "<anticipateOvershootInterpolator />"),
    ("interpolator", "bounce", "<bounceInterpolator />"),
    ("interpolator", "linear", "<linearInterpolator />"),
    ("interpolator", "cycle", "<cycleInterpolator />"),
    (
        "interpolator",
        "fast_out_slow_in",
        r#"<pathInterpolator xmlns:android="http://schemas.android.com/apk/res/android" android:controlX1="0.4" android:controlY1="0" android:controlX2="0.2" android:controlY2="1"/>"#,
    ),
    (
        "interpolator",
        "linear_out_slow_in",
        r#"<pathInterpolator xmlns:android="http://schemas.android.com/apk/res/android" android:controlX1="0" android:controlY1="0" android:controlX2="0.2" android:controlY2="1"/>"#,
    ),
    (
        "interpolator",
        "fast_out_linear_in",
        r#"<pathInterpolator xmlns:android="http://schemas.android.com/apk/res/android" android:controlX1="0.4" android:controlY1="0" android:controlX2="1" android:controlY2="1"/>"#,
    ),
    (
        "interpolator",
        "fast_out_extra_slow_in",
        r#"<pathInterpolator xmlns:android="http://schemas.android.com/apk/res/android" android:pathData="M 0,0 C 0.05, 0, 0.133333, 0.06, 0.166666, 0.4 C 0.208333, 0.82, 0.25, 1, 1, 1"/>"#,
    ),
];

/// Resource types whose resources are files (an attribute referring to one holds its file, read when it is
/// loaded); `color` is a value or a file (a color state list).
const FILE_TYPES: &[&str] = &[
    "anim",
    "animator",
    "drawable",
    "font",
    "interpolator",
    "layout",
    "menu",
    "mipmap",
    "navigation",
    "raw",
    "transition",
    "xml",
];

/// How deep references to references go before it is taken for a loop.
const MAX_REFERENCE_DEPTH: usize = 32;

/// Compose's `Resources` (and its theme) as the parsers use them: references looked up in the framework's
/// interpolators, then in what the caller supplies.
#[derive(Clone, Copy)]
pub(crate) struct Resources<'a> {
    pub(crate) app: &'a dyn XmlResources,
}

impl<'a> Resources<'a> {
    pub(crate) fn new(app: &'a dyn XmlResources) -> Resources<'a> {
        Resources { app }
    }

    /// A named resource's text: the framework's interpolators', else the caller's.
    fn text_of(&self, reference: &ResourceReference) -> Result<String, XmlResourceError> {
        if reference.package.as_deref() == Some("android") && !reference.is_theme_attribute {
            let found = FRAMEWORK_INTERPOLATORS.iter().find(|(t, n, _)| *t == reference.r#type && *n == reference.name);
            if let Some((_, _, xml)) = found {
                return Ok(xml.to_string());
            }
        }
        self.app.resolve(reference).ok_or_else(|| not_found(reference))
    }

    /// A resource's XML element (Android's `getXml(id)`, as an element).
    fn element(&self, resource: &Resource) -> Result<Rc<XmlElement>, XmlResourceError> {
        match resource {
            Resource::Inline(e) => Ok(e.clone()),
            Resource::Named(r) => read_document(&self.text_of(r)?),
        }
    }

    /// A pull parser over a resource (Android's `Resources.getXml(id)`), at its document's start.
    pub(crate) fn get_xml(&self, resource: &Resource) -> Result<XmlPullParser, XmlResourceError> {
        Ok(XmlPullParser::new(self.element(resource)?))
    }

    /// A resource's XML text whole (an inline one's as it is written in its document).
    pub(crate) fn xml_text(&self, resource: &Resource) -> Result<String, XmlResourceError> {
        match resource {
            Resource::Inline(e) => Ok(e.text().to_string()),
            Resource::Named(r) => self.text_of(r),
        }
    }
}

// ---- Attributes, as aapt2 compiles them ---------------------------------------------------------------------------

/// What an attribute's values may be (Android's `ResTable_map` type mask: an `<attr format="…">`).
pub(crate) mod format {
    pub(crate) const REFERENCE: u32 = 1 << 0;
    pub(crate) const STRING: u32 = 1 << 1;
    pub(crate) const INTEGER: u32 = 1 << 2;
    pub(crate) const BOOLEAN: u32 = 1 << 3;
    pub(crate) const COLOR: u32 = 1 << 4;
    pub(crate) const FLOAT: u32 = 1 << 5;
    pub(crate) const DIMENSION: u32 = 1 << 6;
    pub(crate) const FRACTION: u32 = 1 << 7;
}

/// An attribute in Android's namespace as the framework declares it (attrs.xml): its name, format and enum values.
pub(crate) struct AttrDef {
    pub(crate) name: &'static str,
    pub(crate) format: u32,
    pub(crate) enums: &'static [(&'static str, i32)],
}

const fn attr(name: &'static str, format: u32) -> AttrDef {
    AttrDef { name, format, enums: &[] }
}

use format::*;

/// The framework's attributes Compose's parsers read (Android 37's attrs.xml).
pub(crate) mod attrs {
    use super::*;
    pub(crate) const DRAWABLE: AttrDef = attr("drawable", REFERENCE);
    pub(crate) const NAME: AttrDef = attr("name", STRING);
    pub(crate) const ANIMATION: AttrDef = attr("animation", REFERENCE);
    pub(crate) const INTERPOLATOR: AttrDef = attr("interpolator", REFERENCE);
    pub(crate) const DURATION: AttrDef = attr("duration", INTEGER);
    pub(crate) const START_OFFSET: AttrDef = attr("startOffset", INTEGER);
    pub(crate) const REPEAT_COUNT: AttrDef =
        AttrDef { name: "repeatCount", format: INTEGER, enums: &[("infinite", -1)] };
    pub(crate) const REPEAT_MODE: AttrDef =
        AttrDef { name: "repeatMode", format: 0, enums: &[("restart", 1), ("reverse", 2)] };
    pub(crate) const VALUE_FROM: AttrDef = attr("valueFrom", FLOAT | INTEGER | COLOR | DIMENSION | STRING);
    pub(crate) const VALUE_TO: AttrDef = attr("valueTo", FLOAT | INTEGER | COLOR | DIMENSION | STRING);
    pub(crate) const VALUE_TYPE: AttrDef = AttrDef {
        name: "valueType",
        format: 0,
        enums: &[("floatType", 0), ("intType", 1), ("pathType", 2), ("colorType", 3)],
    };
    pub(crate) const PROPERTY_NAME: AttrDef = attr("propertyName", STRING);
    pub(crate) const PATH_DATA: AttrDef = attr("pathData", STRING);
    pub(crate) const PROPERTY_X_NAME: AttrDef = attr("propertyXName", STRING);
    pub(crate) const PROPERTY_Y_NAME: AttrDef = attr("propertyYName", STRING);
    pub(crate) const ORDERING: AttrDef =
        AttrDef { name: "ordering", format: 0, enums: &[("together", 0), ("sequentially", 1)] };
    pub(crate) const VALUE: AttrDef = attr("value", STRING | INTEGER | COLOR | FLOAT | BOOLEAN);
    pub(crate) const FRACTION_ATTR: AttrDef = attr("fraction", FLOAT);
    pub(crate) const FACTOR: AttrDef = attr("factor", FLOAT);
    pub(crate) const TENSION: AttrDef = attr("tension", FLOAT);
    pub(crate) const EXTRA_TENSION: AttrDef = attr("extraTension", FLOAT);
    pub(crate) const CYCLES: AttrDef = attr("cycles", FLOAT);
    pub(crate) const CONTROL_X_1: AttrDef = attr("controlX1", FLOAT);
    pub(crate) const CONTROL_Y_1: AttrDef = attr("controlY1", FLOAT);
    pub(crate) const CONTROL_X_2: AttrDef = attr("controlX2", FLOAT);
    pub(crate) const CONTROL_Y_2: AttrDef = attr("controlY2", FLOAT);
    // (a color state list's items)
    pub(crate) const COLOR_ATTR: AttrDef = attr("color", COLOR | REFERENCE);
    pub(crate) const ALPHA: AttrDef = attr("alpha", FLOAT);
}

/// What kind of integer a value is (Android's TypedValue types from TYPE_FIRST_INT to TYPE_LAST_INT).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum IntKind {
    Dec,
    Hex,
    Boolean,
    ColorArgb8,
    ColorRgb8,
    ColorArgb4,
    ColorRgb4,
}

/// A compiled value (Android's TypedValue, its type and data).
#[derive(Clone, Debug)]
pub(crate) enum Value {
    /// TYPE_STRING: a string, or a file resource's path (its text kept where it was read).
    String(String, Option<String>),
    Float(f32),
    /// TYPE_DIMENSION: its value and unit as Android writes it back (`dip`, `sp`, `px`, …).
    Dimension(f32, &'static str),
    /// TYPE_FRACTION: its value and whether of the parent (`%p`).
    Fraction(f32, bool),
    Int(i32, IntKind),
}

/// An attribute's value as a TypedArray holds it: the value, and the resource it refers to (getResourceId).
#[derive(Clone, Debug)]
pub(crate) struct TypedValue {
    pub(crate) value: Value,
    pub(crate) resource: Option<Resource>,
}

impl TypedValue {
    /// Its type is one of the color types (Android's TYPE_FIRST_COLOR_INT..TYPE_LAST_COLOR_INT).
    pub(crate) fn is_color(&self) -> bool {
        matches!(
            self.value,
            Value::Int(_, IntKind::ColorArgb8 | IntKind::ColorRgb8 | IntKind::ColorArgb4 | IntKind::ColorRgb4)
        )
    }
}

/// A reference written in a value (aapt2's ParseReference): `@null`/`@empty` (nothing), `@[+][*][package:]type/name`,
/// `?[*][package:][attr/]name`.
enum Written {
    Null,
    Reference(ResourceReference),
    Text,
}

fn parse_reference(text: &str) -> Written {
    let t = text.trim();
    if t == "@null" || t == "@empty" {
        return Written::Null;
    }
    let (theme, rest) = if let Some(r) = t.strip_prefix('@') {
        (false, r)
    } else if let Some(r) = t.strip_prefix('?') {
        (true, r)
    } else {
        return Written::Text;
    };
    let rest = rest.strip_prefix('+').unwrap_or(rest);
    let rest = rest.strip_prefix('*').unwrap_or(rest);
    let (package, rest) = match rest.split_once(':') {
        Some((p, r)) => (Some(p.to_string()), r),
        None => (None, rest),
    };
    let (r#type, name) = match rest.split_once('/') {
        Some((t, n)) => (t.to_string(), n.to_string()),
        None if theme => ("attr".to_string(), rest.to_string()),
        None => return Written::Text,
    };
    if name.is_empty() || r#type.is_empty() || (theme && r#type != "attr") {
        return Written::Text;
    }
    Written::Reference(ResourceReference { package, r#type, name, is_theme_attribute: theme })
}

/// aapt2's TryParseColor: `#RGB`, `#ARGB`, `#RRGGBB`, `#AARRGGBB`.
fn parse_color(text: &str) -> Option<(i32, IntKind)> {
    let hex = text.trim().strip_prefix('#')?;
    if !hex.bytes().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let digits: Vec<u32> = hex.chars().map(|c| c.to_digit(16).unwrap_or(0)).collect();
    let (argb, kind) = match digits.len() {
        3 => (
            0xff00_0000 | ((digits[0] * 0x11) << 16) | ((digits[1] * 0x11) << 8) | (digits[2] * 0x11),
            IntKind::ColorRgb4,
        ),
        4 => (
            ((digits[0] * 0x11) << 24) | ((digits[1] * 0x11) << 16) | ((digits[2] * 0x11) << 8) | (digits[3] * 0x11),
            IntKind::ColorArgb4,
        ),
        6 => (0xff00_0000 | u32::from_str_radix(hex, 16).ok()?, IntKind::ColorRgb8),
        8 => (u32::from_str_radix(hex, 16).ok()?, IntKind::ColorArgb8),
        _ => return None,
    };
    Some((argb as i32, kind))
}

/// Android's ResTable::stringToInt: a decimal or `0x` hex integer (leading and trailing blanks allowed).
fn parse_int(text: &str) -> Option<(i32, IntKind)> {
    let s = text.trim();
    let (neg, digits) = match s.strip_prefix('-') {
        Some(d) => (true, d),
        None => (false, s),
    };
    if !digits.starts_with(|c: char| c.is_ascii_digit()) {
        return None;
    }
    if let Some(hex) = digits.strip_prefix("0x") {
        if hex.is_empty() {
            return None;
        }
        let v = u32::from_str_radix(hex, 16).ok()?;
        let v = if neg { (v as i32).wrapping_neg() } else { v as i32 };
        return Some((v, IntKind::Hex));
    }
    if !digits.bytes().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let v: i64 = digits.parse().ok()?;
    let v = if neg { -v } else { v };
    i32::try_from(v).ok().map(|v| (v, IntKind::Dec))
}

/// Android's ResTable::stringToFloat: a number (C's strtof of its digits, sign, point and exponent) and a unit: none
/// (a float), a dimension's (`px`, `dp`/`dip`, `sp`, `pt`, `in`, `mm`) or a fraction's (`%`, `%p`).
fn parse_float(text: &str) -> Option<Value> {
    let s = text.trim();
    let end = s.find(|c: char| !(c.is_ascii_digit() || matches!(c, '.' | '+' | '-' | 'e' | 'E'))).unwrap_or(s.len());
    let number = &s[..end];
    if number.is_empty() {
        return None;
    }
    let v: f32 = number.parse().ok()?;
    let unit = s[end..].trim();
    Some(match unit {
        "" => Value::Float(v),
        "px" => Value::Dimension(v, "px"),
        "dp" | "dip" => Value::Dimension(v, "dip"),
        "sp" => Value::Dimension(v, "sp"),
        "pt" => Value::Dimension(v, "pt"),
        "in" => Value::Dimension(v, "in"),
        "mm" => Value::Dimension(v, "mm"),
        "%" => Value::Fraction(v / 100.0, false),
        "%p" => Value::Fraction(v / 100.0, true),
        _ => return None,
    })
}

/// aapt2's TryParseItemForAttribute of a value that isn't a reference: a color, a boolean, an integer, a float
/// (a dimension, a fraction) as the format allows, in that order, then an enum's name; else a string if the format
/// takes one. None: incompatible with the attribute.
fn compile_text(text: &str, def: &AttrDef) -> Option<Value> {
    let f = def.format;
    if f & COLOR != 0
        && let Some((argb, kind)) = parse_color(text)
    {
        return Some(Value::Int(argb, kind));
    }
    if f & BOOLEAN != 0 {
        match text.trim() {
            "true" | "TRUE" | "True" => return Some(Value::Int(-1, IntKind::Boolean)),
            "false" | "FALSE" | "False" => return Some(Value::Int(0, IntKind::Boolean)),
            _ => {}
        }
    }
    if f & INTEGER != 0
        && let Some((v, kind)) = parse_int(text)
    {
        return Some(Value::Int(v, kind));
    }
    if f & (FLOAT | DIMENSION | FRACTION) != 0
        && let Some(v) = parse_float(text)
    {
        let allowed = match v {
            Value::Float(_) => f & FLOAT != 0,
            Value::Dimension(..) => f & DIMENSION != 0,
            Value::Fraction(..) => f & FRACTION != 0,
            _ => false,
        };
        if allowed {
            return Some(v);
        }
    }
    if let Some((_, v)) = def.enums.iter().find(|(n, _)| *n == text.trim()) {
        return Some(Value::Int(*v, IntKind::Dec));
    }
    if f & STRING != 0 {
        return Some(Value::String(text.to_string(), None));
    }
    None
}

impl<'a> Resources<'a> {
    /// An attribute's written value compiled and its references resolved (as `obtainStyledAttributes` resolves
    /// them): None for `@null`, `@empty`.
    fn typed_value(&self, text: &str, def: &AttrDef, depth: usize) -> Result<Option<TypedValue>, XmlResourceError> {
        match parse_reference(text) {
            Written::Null => Ok(None),
            Written::Text => match compile_text(text, def) {
                Some(value) => Ok(Some(TypedValue { value, resource: None })),
                None => Err(XmlResourceError::new(
                    "XmlPullParserException",
                    format!("'{text}' is incompatible with attribute {}", def.name),
                )),
            },
            Written::Reference(r) => {
                if depth > MAX_REFERENCE_DEPTH {
                    return Err(XmlResourceError::new("NotFoundException", format!("{r}: a loop of references")));
                }
                if !r.is_theme_attribute && FILE_TYPES.contains(&r.r#type.as_str()) {
                    let path = format!("res/{}/{}.xml", r.r#type, r.name);
                    return Ok(Some(TypedValue {
                        value: Value::String(path, None),
                        resource: Some(Resource::Named(r)),
                    }));
                }
                let text = self.text_of(&r)?;
                if text.trim_start().starts_with('<') {
                    // (a file: a color state list)
                    let path = format!("res/{}/{}.xml", r.r#type, r.name);
                    return Ok(Some(TypedValue {
                        value: Value::String(path, Some(text)),
                        resource: Some(Resource::Named(r)),
                    }));
                }
                let resolved = self.typed_value(&text, def, depth + 1)?;
                Ok(resolved.map(|v| TypedValue { resource: Some(Resource::Named(r)), ..v }))
            }
        }
    }

    /// A color written as a reference (`@color/…`, `?attr/…`) as these resources give it (a value, or a color
    /// state list's default); none if they don't (a vector drawable's reader then takes black).
    pub(crate) fn color_of_reference(&self, written: &str) -> Option<Color> {
        let value = self.typed_value(written, &attrs::COLOR_ATTR, 0).ok()??;
        let a = TypedArray { values: vec![Some(value)], res: *self };
        a.get_color(0, 0).ok().map(|argb| Color::from_argb(argb as u32))
    }

    /// The attributes `styleable` names of the element the parser is at (Compose's `attrs.attrs(res, theme,
    /// styleable) { a -> }`'s TypedArray), an inline `<aapt:attr>` one's as a reference to it.
    pub(crate) fn obtain_attributes(
        &self,
        parser: &XmlPullParser,
        styleable: &[&AttrDef],
    ) -> Result<TypedArray<'a>, XmlResourceError> {
        let element = parser.element();
        let mut values = Vec::with_capacity(styleable.len());
        for def in styleable {
            let value = if let Some(inline) = element.inline(ANDROID_NS, def.name) {
                let path = format!("res/inline/{}.xml", def.name);
                Some(TypedValue { value: Value::String(path, None), resource: Some(Resource::Inline(inline.clone())) })
            } else if let Some(text) = element.attribute(ANDROID_NS, def.name) {
                self.typed_value(text, def, 0)?
            } else {
                None
            };
            values.push(value);
        }
        Ok(TypedArray { values, res: *self })
    }
}

// ---- TypedArray ---------------------------------------------------------------------------------------------------

/// An element's attributes as Android's TypedArray reads them.
pub(crate) struct TypedArray<'a> {
    values: Vec<Option<TypedValue>>,
    res: Resources<'a>,
}

fn number_format(s: &str) -> XmlResourceError {
    XmlResourceError::new("NumberFormatException", format!("For input string: \"{s}\""))
}

/// Java's `Float.parseFloat`: blanks around, a sign, digits with a point, an exponent, an `f`/`d` suffix, `NaN`,
/// `Infinity`.
pub(crate) fn java_parse_float(s: &str) -> Result<f32, XmlResourceError> {
    let t = s.trim();
    let body = t.strip_suffix(['f', 'F', 'd', 'D']).unwrap_or(t);
    let unsigned = body.trim_start_matches(['+', '-']);
    if unsigned == "NaN" || unsigned == "Infinity" {
        let v = if unsigned == "NaN" { f32::NAN } else { f32::INFINITY };
        return Ok(if body.starts_with('-') { -v } else { v });
    }
    if unsigned.is_empty()
        || !unsigned.bytes().all(|c| c.is_ascii_digit() || matches!(c, b'.' | b'e' | b'E' | b'+' | b'-'))
    {
        return Err(number_format(s));
    }
    body.parse::<f32>().map_err(|_| number_format(s))
}

/// Android's `XmlUtils.convertValueToInt`: a sign, then `0x`/`#` hex, a leading `0` octal, or decimal.
fn convert_value_to_int(s: &str) -> Result<i32, XmlResourceError> {
    if s.is_empty() {
        return Err(number_format(s));
    }
    let (sign, rest) = match s.strip_prefix('-') {
        Some(r) => (-1i64, r),
        None => (1, s),
    };
    let (digits, base) = if rest == "0" {
        return Ok(0);
    } else if let Some(h) = rest.strip_prefix("0x").or_else(|| rest.strip_prefix("0X")) {
        (h, 16)
    } else if let Some(o) = rest.strip_prefix('0') {
        (o, 8)
    } else if let Some(h) = rest.strip_prefix('#') {
        (h, 16)
    } else {
        (rest, 10)
    };
    let v = i64::from_str_radix(digits, base).map_err(|_| number_format(s))?;
    i32::try_from(v).map(|v| (v as i64 * sign) as i32).map_err(|_| number_format(s))
}

impl TypedArray<'_> {
    fn at(&self, index: usize) -> Option<&TypedValue> {
        self.values[index].as_ref()
    }

    /// TypedValue.coerceToString.
    fn coerce_to_string(v: &Value) -> String {
        match v {
            Value::String(s, _) => s.clone(),
            Value::Float(f) => format!("{f:?}"),
            Value::Dimension(f, unit) => format!("{f:?}{unit}"),
            Value::Fraction(f, parent) => format!("{:?}{}", f * 100.0, if *parent { "%p" } else { "%" }),
            Value::Int(i, kind) => match kind {
                IntKind::Dec => i.to_string(),
                IntKind::Hex => format!("0x{:08x}", *i as u32),
                IntKind::Boolean => (*i != 0).to_string(),
                _ => format!("#{:08x}", *i as u32),
            },
        }
    }

    /// `getString`: a string's, else the value as a string; none if there is none.
    pub(crate) fn get_string(&self, index: usize) -> Option<String> {
        self.at(index).map(|v| Self::coerce_to_string(&v.value))
    }

    /// `getFloat`: a float's; an integer's (a color's too) as a float; a string's (a dimension's as Android writes
    /// it) through `Float.parseFloat`.
    pub(crate) fn get_float(&self, index: usize, default: f32) -> Result<f32, XmlResourceError> {
        match self.at(index).map(|v| &v.value) {
            None => Ok(default),
            Some(Value::Float(f)) => Ok(*f),
            Some(Value::Int(i, _)) => Ok(*i as f32),
            Some(v) => java_parse_float(&Self::coerce_to_string(v)),
        }
    }

    /// `getInt`: an integer's (a color's, a boolean's); else through `XmlUtils.convertValueToInt`.
    pub(crate) fn get_int(&self, index: usize, default: i32) -> Result<i32, XmlResourceError> {
        match self.at(index).map(|v| &v.value) {
            None => Ok(default),
            Some(Value::Int(i, _)) => Ok(*i),
            Some(v) => convert_value_to_int(&Self::coerce_to_string(v)),
        }
    }

    /// `getColor`: an integer's as ARGB; a color state list's default color; a float's or a dimension's: an error.
    pub(crate) fn get_color(&self, index: usize, default: i32) -> Result<i32, XmlResourceError> {
        match self.at(index) {
            None => Ok(default),
            Some(TypedValue { value: Value::Int(i, _), .. }) => Ok(*i),
            Some(TypedValue { value: Value::String(path, text), resource }) => {
                let text = match (text, resource) {
                    (Some(t), _) => t.clone(),
                    (None, Some(r @ Resource::Named(_))) => self.res.xml_text(r)?,
                    _ => {
                        return Err(XmlResourceError::new(
                            "NotFoundException",
                            format!("File {path} from color state list resource ID #0x0"),
                        ));
                    }
                };
                color_state_list_default(&read_document(&text)?, self.res)
            }
            Some(v) => Err(XmlResourceError::new(
                "UnsupportedOperationException",
                format!("Can't convert value at index {index} to color: {}", Self::coerce_to_string(&v.value)),
            )),
        }
    }

    /// `getResourceId`: the resource it refers to (none: Android's 0).
    pub(crate) fn get_resource_id(&self, index: usize) -> Option<Resource> {
        self.at(index).and_then(|v| v.resource.clone())
    }

    /// `peekValue`: its value as it is held.
    pub(crate) fn peek_value(&self, index: usize) -> Option<&TypedValue> {
        self.at(index)
    }

    /// `hasValue`.
    pub(crate) fn has_value(&self, index: usize) -> bool {
        self.at(index).is_some()
    }
}

/// A color state list's default color (Android's ColorStateList.getDefaultColor): its first item of no state, else
/// its first; its color with its alpha.
fn color_state_list_default(root: &Rc<XmlElement>, res: Resources) -> Result<i32, XmlResourceError> {
    if root.name != "selector" {
        return Err(XmlResourceError::new(
            "XmlPullParserException",
            format!("invalid color state list tag {}", root.name),
        ));
    }
    let items: Vec<&Rc<XmlElement>> = root.children.iter().filter(|c| c.name == "item").collect();
    let stateless =
        |e: &XmlElement| !e.attributes.iter().any(|a| a.namespace == ANDROID_NS && a.name.starts_with("state_"));
    let Some(item) = items.iter().find(|e| stateless(e)).or(items.first()) else {
        return Err(XmlResourceError::new("XmlPullParserException", "a color state list without items"));
    };
    let parser = XmlPullParser::new((*item).clone()).seek_to_start_tag()?;
    let a = res.obtain_attributes(&parser, &[&attrs::COLOR_ATTR, &attrs::ALPHA])?;
    let color = a.get_color(0, 0xffff_00ffu32 as i32)? as u32;
    let alpha = a.get_float(1, 1.0)?;
    let base = (color >> 24) as f32;
    let a8 = ((base * alpha + 0.5) as i32).clamp(0, 255) as u32;
    Ok(((a8 << 24) | (color & 0x00ff_ffff)) as i32)
}
