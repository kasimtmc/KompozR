//! Android vector drawables read into vectors (CMP's `vector` package: XmlVectorParser.kt and ValueParsers.kt,
//! Android's parsing logic without references to other resources): `<vector>`'s size (dp or px), viewport and
//! auto-mirroring; `<group>`s, `<clip-path>`s (a group of their own, closed with the group they are in), `<path>`s
//! with their fills and strokes, colors written out or `<aapt:attr>` gradients (linear, radial, sweep; `<item>`
//! stops or start/center/end colors).

pub mod xmldom;

use kompoz_graphics::vector::{
    DefaultPivotX, DefaultPivotY, DefaultRotation, DefaultScaleX, DefaultScaleY, DefaultTranslationX,
    DefaultTranslationY, ImageVector, VectorGroup, VectorPath, add_path_nodes,
};
use kompoz_graphics::{Brush, Color, Density, Dp, Offset, PathFillType, SolidColor, StrokeCap, StrokeJoin, TileMode};
use std::fmt;
use xmldom::{Element, MalformedXMLException};

const ANDROID_NS: &str = "http://schemas.android.com/apk/res/android";
const AAPT_NS: &str = "http://schemas.android.com/aapt";

/// Why a vector drawable can't be read: its XML (CMP's MalformedXMLException, the JVM's SAXParseException), a
/// value (Kotlin's NumberFormatException, IllegalArgumentException, UnsupportedOperationException).
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum VectorParseError {
    Xml(MalformedXMLException),
    Value(String),
}

impl fmt::Display for VectorParseError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            VectorParseError::Xml(e) => write!(f, "{e}"),
            VectorParseError::Value(m) => f.write_str(m),
        }
    }
}

impl std::error::Error for VectorParseError {}

impl From<MalformedXMLException> for VectorParseError {
    fn from(e: MalformedXMLException) -> Self {
        VectorParseError::Xml(e)
    }
}

type Result<T> = std::result::Result<T, VectorParseError>;

/// A number as Java's `Float.parseFloat` (Kotlin's `String.toFloat()`) reads it: blanks around it trimmed, an `f`
/// or `d` suffix allowed, `NaN`, `Infinity`; anything else a NumberFormatException.
pub(crate) fn parse_float(s: &str) -> Result<f32> {
    let t = s.trim_matches(|c: char| c <= ' ');
    let body = t.strip_suffix(['f', 'F', 'd', 'D']).filter(|b| !b.ends_with(|c: char| c.is_alphabetic())).unwrap_or(t);
    let unsigned = body.trim_start_matches(['+', '-']);
    let value = match unsigned {
        "NaN" => Some(f32::NAN),
        "Infinity" => Some(if body.starts_with('-') { f32::NEG_INFINITY } else { f32::INFINITY }),
        _ if unsigned.is_empty() || unsigned.contains(|c: char| c.is_alphabetic() && c != 'e' && c != 'E') => None,
        _ => body.parse::<f32>().ok(),
    };
    value.ok_or_else(|| VectorParseError::Value(format!("NumberFormatException: For input string: \"{s}\"")))
}

/// `#RGB`, `#ARGB`, `#RRGGBB`, `#AARRGGBB` as an ARGB int (CMP's `parseColorValue`, Android's VdUtil's): another
/// length is opaque black.
pub(crate) fn parse_color_value(color: &str) -> Result<u32> {
    let Some(hex) = color.strip_prefix('#') else {
        return Err(VectorParseError::Value(format!("Invalid color value {color}")));
    };
    const ALPHA_MASK: u32 = 0xFF00_0000;
    let parse = || {
        u32::from_str_radix(hex, 16)
            .map_err(|_| VectorParseError::Value(format!("NumberFormatException: Invalid number format: '{hex}'")))
    };
    Ok(match color.len() {
        7 => parse()? | ALPHA_MASK,
        9 => parse()?,
        4 => {
            let v = parse()?;
            ((v >> 8 & 0xF) * 0x11_0000) | ((v >> 4 & 0xF) * 0x1100) | ((v & 0xF) * 0x11) | ALPHA_MASK
        }
        5 => {
            let v = parse()?;
            ((v >> 12 & 0xF) * 0x1100_0000)
                | ((v >> 8 & 0xF) * 0x11_0000)
                | ((v >> 4 & 0xF) * 0x1100)
                | ((v & 0xF) * 0x11)
                | ALPHA_MASK
        }
        _ => ALPHA_MASK,
    })
}

fn unsupported<T>(what: &str, value: &str) -> Result<T> {
    Err(VectorParseError::Value(format!("UnsupportedOperationException: unknown {what}: {value}")))
}

fn parse_fill_type(v: &str) -> Result<PathFillType> {
    match v {
        "nonZero" => Ok(PathFillType::NonZero),
        "evenOdd" => Ok(PathFillType::EvenOdd),
        _ => unsupported("fillType", v),
    }
}

fn parse_stroke_cap(v: &str) -> Result<StrokeCap> {
    match v {
        "butt" => Ok(StrokeCap::Butt),
        "round" => Ok(StrokeCap::Round),
        "square" => Ok(StrokeCap::Square),
        _ => unsupported("strokeCap", v),
    }
}

fn parse_stroke_join(v: &str) -> Result<StrokeJoin> {
    match v {
        "miter" => Ok(StrokeJoin::Miter),
        "round" => Ok(StrokeJoin::Round),
        "bevel" => Ok(StrokeJoin::Bevel),
        _ => unsupported("strokeJoin", v),
    }
}

fn parse_tile_mode(v: &str) -> Result<TileMode> {
    match v {
        "clamp" => Ok(TileMode::Clamp),
        "repeated" => Ok(TileMode::Repeated),
        "mirror" => Ok(TileMode::Mirror),
        _ => unsupported("tileMode", v),
    }
}

/// A size in dp: `24dp`, or `48px` at `density`; none, 0 (CMP's `parseDp`).
fn parse_dp(v: Option<&str>, density: Density) -> Result<Dp> {
    match v {
        None => Ok(Dp(0.0)),
        Some(s) if s.ends_with("dp") => Ok(Dp(parse_float(&s[..s.len() - 2])?)),
        Some(s) if s.ends_with("px") => Ok(Dp(parse_float(&s[..s.len() - 2])? / density.density)),
        Some(_) => {
            Err(VectorParseError::Value("UnsupportedOperationException: value should ends with dp or px".into()))
        }
    }
}

/// Attribute `name` of `namespace`, none if it is blank (CMP's `attributeOrNull`).
fn attribute<'a>(e: &'a Element, namespace: &str, name: &str) -> Option<&'a str> {
    let v = e.get_attribute_ns(namespace, name);
    (!v.trim().is_empty()).then_some(v)
}

fn float_or(e: &Element, name: &str, default: f32) -> Result<f32> {
    attribute(e, ANDROID_NS, name).map_or(Ok(default), parse_float)
}

/// Whether the build context's group was in the XML or added for a `<clip-path>` (CMP's `BuildContext.Group`).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Group {
    Real,
    Virtual,
}

/// The `<vector>` element as a vector (CMP's `Element.toImageVector(density)`).
pub fn element_to_image_vector(element: &Element, density: Density) -> Result<ImageVector> {
    let mut groups: Vec<Group> = Vec::new();
    let mut builder = ImageVector::Builder(
        kompoz_graphics::vector::DefaultGroupName,
        parse_dp(attribute(element, ANDROID_NS, "width"), density)?,
        parse_dp(attribute(element, ANDROID_NS, "height"), density)?,
        float_or(element, "viewportWidth", 0.0)?,
        float_or(element, "viewportHeight", 0.0)?,
        Color::Unspecified,
        kompoz_graphics::vector::DefaultTintBlendMode,
        attribute(element, ANDROID_NS, "autoMirrored") == Some("true"),
    );
    parse_vector_nodes(element, &mut builder, &mut groups)?;
    Ok(builder.build())
}

/// An Android vector drawable's bytes as a vector at `density` (CMP's `ByteArray.decodeToImageVector(density)`,
/// `@ExperimentalResourceApi`); its XML or a value not as Android writes it is an error.
pub fn decode_to_image_vector(bytes: &[u8], density: Density) -> Result<ImageVector> {
    element_to_image_vector(&xmldom::parse_xml_bytes(bytes)?, density)
}

fn parse_vector_nodes(
    e: &Element,
    builder: &mut kompoz_graphics::vector::ImageVectorBuilder,
    groups: &mut Vec<Group>,
) -> Result<()> {
    for child in e.child_elements() {
        match child.node_name.as_str() {
            "path" => parse_path(child, builder)?,
            "clip-path" => {
                builder.add_group_of(VectorGroup {
                    name: attribute(child, ANDROID_NS, "name").unwrap_or("").into(),
                    clip_path_data: add_path_nodes(attribute(child, ANDROID_NS, "pathData").unwrap_or("")),
                    ..VectorGroup::default()
                });
                groups.push(Group::Virtual);
            }
            "group" => {
                builder.add_group_of(VectorGroup {
                    name: attribute(child, ANDROID_NS, "name").unwrap_or("").into(),
                    rotation: float_or(child, "rotation", DefaultRotation)?,
                    pivot_x: float_or(child, "pivotX", DefaultPivotX)?,
                    pivot_y: float_or(child, "pivotY", DefaultPivotY)?,
                    scale_x: float_or(child, "scaleX", DefaultScaleX)?,
                    scale_y: float_or(child, "scaleY", DefaultScaleY)?,
                    translation_x: float_or(child, "translateX", DefaultTranslationX)?,
                    translation_y: float_or(child, "translateY", DefaultTranslationY)?,
                    ..VectorGroup::default()
                });
                groups.push(Group::Real);
                parse_vector_nodes(child, builder, groups)?;
                // (the group, and the clip paths' groups opened in it, closed)
                loop {
                    let removed = groups.pop();
                    builder.clear_group();
                    if removed != Some(Group::Virtual) {
                        break;
                    }
                }
            }
            _ => {}
        }
    }
    Ok(())
}

fn parse_path(e: &Element, builder: &mut kompoz_graphics::vector::ImageVectorBuilder) -> Result<()> {
    let brush = |name: &str| -> Result<Option<Brush>> {
        if let Some(v) = attribute(e, ANDROID_NS, name) {
            return Ok(Some(SolidColor(Color::from_argb(parse_color_value(v)?))));
        }
        match aapt_attr(e, ANDROID_NS, name) {
            Some(a) => parse_element_brush(a),
            None => Ok(None),
        }
    };
    builder.add_path_of(VectorPath {
        path_data: add_path_nodes(attribute(e, ANDROID_NS, "pathData").unwrap_or("")),
        path_fill_type: attribute(e, ANDROID_NS, "fillType").map_or(Ok(PathFillType::NonZero), parse_fill_type)?,
        name: attribute(e, ANDROID_NS, "name").unwrap_or("").into(),
        fill: brush("fillColor")?,
        fill_alpha: float_or(e, "fillAlpha", 1.0)?,
        stroke: brush("strokeColor")?,
        stroke_alpha: float_or(e, "strokeAlpha", 1.0)?,
        stroke_line_width: float_or(e, "strokeWidth", 1.0)?,
        stroke_line_cap: attribute(e, ANDROID_NS, "strokeLineCap").map_or(Ok(StrokeCap::Butt), parse_stroke_cap)?,
        stroke_line_join: attribute(e, ANDROID_NS, "strokeLineJoin")
            .map_or(Ok(StrokeJoin::Miter), parse_stroke_join)?,
        stroke_line_miter: float_or(e, "strokeMiterLimit", 1.0)?,
        trim_path_start: float_or(e, "trimPathStart", 0.0)?,
        trim_path_end: float_or(e, "trimPathEnd", 1.0)?,
        trim_path_offset: float_or(e, "trimPathOffset", 0.0)?,
    });
    Ok(())
}

/// An attribute given as a child element (`<aapt:attr name="android:fillColor"><gradient …/></aapt:attr>`): the
/// child whose name is the attribute's qualified name (CMP's `apptAttr`).
fn aapt_attr<'a>(e: &'a Element, namespace: &str, name: &str) -> Option<&'a Element> {
    let prefix = e.lookup_prefix(namespace)?;
    let qualified = format!("{prefix}:{name}");
    e.child_elements()
        .find(|c| c.namespace_uri == AAPT_NS && c.local_name == "attr" && c.get_attribute("name") == qualified)
}

fn parse_element_brush(e: &Element) -> Result<Option<Brush>> {
    match e.child_elements().find(|c| c.node_name == "gradient") {
        Some(g) => parse_gradient(g),
        None => Ok(None),
    }
}

fn parse_gradient(e: &Element) -> Result<Option<Brush>> {
    let offset = |x: &str, y: &str| -> Result<Offset> { Ok(Offset::new(float_or(e, x, 0.0)?, float_or(e, y, 0.0)?)) };
    let tile_mode = || attribute(e, ANDROID_NS, "tileMode").map_or(Ok(TileMode::Clamp), parse_tile_mode);
    Ok(match attribute(e, ANDROID_NS, "type") {
        Some("linear") => Some(Brush::linear_gradient_stops(
            &parse_color_stops(e)?,
            offset("startX", "startY")?,
            offset("endX", "endY")?,
            tile_mode()?,
        )),
        Some("radial") => Some(Brush::radial_gradient_stops(
            &parse_color_stops(e)?,
            offset("centerX", "centerY")?,
            float_or(e, "gradientRadius", 0.0)?,
            tile_mode()?,
        )),
        Some("sweep") => Some(Brush::sweep_gradient_stops(&parse_color_stops(e)?, offset("centerX", "centerY")?)),
        _ => None,
    })
}

fn parse_color_stops(e: &Element) -> Result<Vec<(f32, Color)>> {
    let items: Vec<&Element> = e.child_elements().filter(|c| c.node_name == "item").collect();
    let last = items.len().saturating_sub(1).max(1) as f32;
    let mut stops = Vec::new();
    for (index, item) in items.iter().enumerate() {
        let offset = float_or(item, "offset", index as f32 / last)?;
        if let Some(c) = attribute(item, ANDROID_NS, "color") {
            stops.push((offset, Color::from_argb(parse_color_value(c)?)));
        }
    }
    if stops.is_empty() {
        for (offset, name) in [(0.0, "startColor"), (0.5, "centerColor"), (1.0, "endColor")] {
            if let Some(c) = attribute(e, ANDROID_NS, name) {
                stops.push((offset, Color::from_argb(parse_color_value(c)?)));
            }
        }
    }
    Ok(stops)
}
