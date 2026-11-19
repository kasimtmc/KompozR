//! Android vector drawables read into vectors, as Compose's `ImageVector.vectorResource` (its XmlVectorParser)
//! reads them.

use super::image_vector::{ImageVector, ImageVectorBuilder, VectorGroup, VectorPath};
use super::parser::add_path_nodes;
use super::xml::{XmlNode, read_xml_tree, xml_color, xml_number};
use crate::brush::{BlendMode, Brush, SolidColor, TileMode};
use crate::color::Color;
use crate::geometry::Offset;
use crate::path::{PathFillType, StrokeCap, StrokeJoin};
use crate::unit::Dp;
use std::collections::BTreeMap;

/// What a color reference (`@color/…`, `?attr/…`) is: a caller's resources, else none.
type ColorReferences<'a> = &'a dyn Fn(&str) -> Option<Color>;

/// Attribute `key` of `a` as a color: `#…` as Android writes it; another value a reference, its color as
/// `references` gives it, else black.
fn color_of(a: &BTreeMap<String, String>, key: &str, references: ColorReferences) -> Option<Color> {
    let v = a.get(key)?;
    if v.is_empty() || v.starts_with('#') {
        return xml_color(v);
    }
    Some(references(v).unwrap_or(Color::Black))
}

/// An aapt:attr `<gradient>` (its `<item>`s, else its start, center and end colors) as a brush, in its path's units
/// (its groups move it with the path when it is drawn); none without colors. A radial one of no radius (Android
/// refuses it): its first color.
fn gradient_brush(
    at: &BTreeMap<String, String>,
    mut stops: Vec<(f32, Color)>,
    references: ColorReferences,
) -> Option<Brush> {
    if stops.is_empty() {
        for (offset, key) in [(0.0, "startColor"), (0.5, "centerColor"), (1.0, "endColor")] {
            if let Some(c) = color_of(at, key, references) {
                stops.push((offset, c));
            }
        }
    }
    let first = stops.first()?.1;
    let value = |key: &str| at.get(key).map_or("", String::as_str);
    let mode = match value("tileMode") {
        "repeat" => TileMode::Repeated,
        "mirror" => TileMode::Mirror,
        _ => TileMode::Clamp,
    };
    let point = |x: &str, y: &str| Offset::new(xml_number(at, x, 0.0), xml_number(at, y, 0.0));
    Some(match value("type") {
        "radial" => {
            let radius = xml_number(at, "gradientRadius", 0.0);
            if radius <= 0.0 {
                return Some(SolidColor(first));
            }
            Brush::radial_gradient_stops(&stops, point("centerX", "centerY"), radius, mode)
        }
        "sweep" => Brush::sweep_gradient_stops(&stops, point("centerX", "centerY")),
        _ => Brush::linear_gradient_stops(&stops, point("startX", "startY"), point("endX", "endY"), mode),
    })
}

/// A `<vector>` element as a vector, read as Compose's XmlVectorParser reads it (and its alpha, fading its paths'
/// fills and strokes): its groups and clip-paths through the builder, its paths in their paints (gradients from
/// their aapt:attr children), color references as `references` gives them.
fn vector_of(vector: &XmlNode, references: ColorReferences) -> ImageVector {
    let number = |n: &XmlNode, key: &str, fallback: f32| xml_number(&n.attributes, key, fallback);
    let color = |n: &XmlNode, key: &str| color_of(&n.attributes, key, references);
    let path_data = |n: &XmlNode| add_path_nodes(n.attribute("pathData"));
    let (vw, vh) = (number(vector, "viewportWidth", 24.0), number(vector, "viewportHeight", 24.0));
    let tint_mode = match vector.attribute("tintMode") {
        "src_over" => BlendMode::SrcOver,
        "src_atop" => BlendMode::SrcAtop,
        "multiply" => BlendMode::Modulate,
        "screen" => BlendMode::Screen,
        "add" => BlendMode::Plus,
        _ => BlendMode::SrcIn,
    };
    let mut builder = ImageVector::Builder(
        "",
        Dp(number(vector, "width", vw)),
        Dp(number(vector, "height", vh)),
        vw,
        vh,
        color_of(&vector.attributes, "tint", references).unwrap_or(Color::Unspecified),
        tint_mode,
        vector.attribute("autoMirrored") == "true",
    );
    let alpha = number(vector, "alpha", 1.0);
    let path_of = |n: &XmlNode| {
        let mut p = VectorPath {
            name: n.attribute("name").to_string(),
            path_data: path_data(n),
            fill: color(n, "fillColor").map(SolidColor),
            stroke: color(n, "strokeColor").map(SolidColor),
            ..VectorPath::default()
        };
        // <aapt:attr name="android:fillColor"><gradient>…
        for attr in n.children.iter().filter(|a| a.name == "attr") {
            let Some(gradient) = attr.find("gradient") else { continue };
            let stops: Vec<(f32, Color)> = gradient
                .children
                .iter()
                .filter(|item| item.name == "item")
                .filter_map(|item| {
                    color_of(&item.attributes, "color", references)
                        .map(|c| (xml_number(&item.attributes, "offset", 0.0), c))
                })
                .collect();
            if let Some(brush) = gradient_brush(&gradient.attributes, stops, references) {
                if attr.attribute("name").contains("strokeColor") {
                    p.stroke = Some(brush);
                } else {
                    p.fill = Some(brush);
                }
            }
        }
        p.fill_alpha = number(n, "fillAlpha", 1.0) * alpha;
        p.stroke_alpha = number(n, "strokeAlpha", 1.0) * alpha;
        p.stroke_line_width = number(n, "strokeWidth", 1.0);
        p.stroke_line_miter = number(n, "strokeMiterLimit", 4.0);
        p.trim_path_start = number(n, "trimPathStart", 0.0);
        p.trim_path_end = number(n, "trimPathEnd", 1.0);
        p.trim_path_offset = number(n, "trimPathOffset", 0.0);
        p.stroke_line_cap = match n.attribute("strokeLineCap") {
            "round" => StrokeCap::Round,
            "square" => StrokeCap::Square,
            _ => StrokeCap::Butt,
        };
        p.stroke_line_join = match n.attribute("strokeLineJoin") {
            "round" => StrokeJoin::Round,
            "bevel" => StrokeJoin::Bevel,
            _ => StrokeJoin::Miter,
        };
        if n.attribute("fillType") == "evenOdd" {
            p.path_fill_type = PathFillType::EvenOdd;
        }
        p
    };
    // What is in `parent`: its groups, its paths; each <clip-path> a group of its own clipping what comes after it,
    // closed with `parent`.
    fn contents(
        parent: &XmlNode,
        builder: &mut ImageVectorBuilder,
        group_of: &dyn Fn(&XmlNode) -> VectorGroup,
        clip_of: &dyn Fn(&XmlNode) -> VectorGroup,
        path_of: &dyn Fn(&XmlNode) -> VectorPath,
    ) {
        let mut clips = 0;
        for n in &parent.children {
            match n.name.as_str() {
                "group" => {
                    builder.add_group_of(group_of(n));
                    contents(n, builder, group_of, clip_of, path_of);
                    builder.clear_group();
                }
                "clip-path" => {
                    builder.add_group_of(clip_of(n));
                    clips += 1;
                }
                "path" => {
                    builder.add_path_of(path_of(n));
                }
                _ => {}
            }
        }
        for _ in 0..clips {
            builder.clear_group();
        }
    }
    let group_of = |n: &XmlNode| VectorGroup {
        name: n.attribute("name").to_string(),
        rotation: number(n, "rotation", 0.0),
        pivot_x: number(n, "pivotX", 0.0),
        pivot_y: number(n, "pivotY", 0.0),
        scale_x: number(n, "scaleX", 1.0),
        scale_y: number(n, "scaleY", 1.0),
        translation_x: number(n, "translateX", 0.0),
        translation_y: number(n, "translateY", 0.0),
        ..VectorGroup::default()
    };
    let clip_of = |n: &XmlNode| VectorGroup {
        name: n.attribute("name").to_string(),
        clip_path_data: path_data(n),
        ..VectorGroup::default()
    };
    contents(vector, &mut builder, &group_of, &clip_of, &path_of);
    builder.build()
}

impl ImageVector {
    /// An Android vector drawable read as Compose's `ImageVector.vectorResource` reads it: `<vector>` (width,
    /// height (dp; another unit read as dp), viewport (24 where it isn't given; the default size the viewport's),
    /// autoMirrored, tint, tintMode, alpha), `<group>`s (name, rotation, pivot, scale, translate; nested) and
    /// `<path>`s (name, pathData, fillColor, strokeColor, strokeWidth (1 where it isn't given), their alphas, line
    /// cap, join, miter limit, fillType, trimPathStart, End and Offset; aapt:attr `<gradient>`s as brushes: linear,
    /// radial, sweep, their items or start, center and end colors, tile mode) and `<clip-path>`s (clipping what
    /// comes after them in their group). Colors are `#RGB`, `#ARGB`, `#RRGGBB` or `#AARRGGBB`; references
    /// (`@color/…`) aren't resolved (black: [`from_vector_xml_with`](Self::from_vector_xml_with) resolves them).
    /// The vector's alpha fades its paths' fills and strokes. An `<animated-vector>`'s drawable is read as it
    /// starts. None if there is no `<vector>`.
    pub fn from_vector_xml(xml: &str) -> Option<ImageVector> {
        ImageVector::from_vector_xml_with(xml, &|_| None)
    }

    /// [`from_vector_xml`](Self::from_vector_xml), each color written as a reference (`@color/…`, `?attr/…`, as it
    /// is written) the color `color_reference` gives it (none: black). What an animated vector's resources resolve
    /// its drawable's colors with.
    pub fn from_vector_xml_with(xml: &str, color_reference: &dyn Fn(&str) -> Option<Color>) -> Option<ImageVector> {
        let root = read_xml_tree(xml)?;
        let vector = root.find("vector")?;
        Some(vector_of(vector, color_reference))
    }
}
