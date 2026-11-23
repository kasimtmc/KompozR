//! An SVG document (Skia's SVG module, SkSVGDOM, as CMP's desktop SvgPainter renders it): the XML read into its
//! elements — structure (`svg`, `g`, `defs`, `use`, `symbol`, `switch`), shapes (`path`, `rect`, `circle`,
//! `ellipse`, `line`, `polyline`, `polygon`), paint servers (`linearGradient`, `radialGradient`, `stop`) and
//! `clipPath` — each with its presentation attributes (and its `style` attribute's declarations, which win over
//! them) and its transform; unknown elements left out with what is in them, unknown attributes and values that
//! don't read ignored, as Skia's parser leaves them. Drawn by [`render`].

mod render;
#[cfg(test)]
mod tests;
mod values;

use super::SvgParseError;
use crate::vector::xmldom::{Element, parse_xml_bytes};
use kompoz_graphics::vector::add_path_nodes;
use kompoz_graphics::{Offset, Path, PathFillType, Rect, StrokeCap, StrokeJoin, TileMode, to_path};
use kompoz_ui::draw::DrawScope;
use std::collections::HashMap;
use values::{
    Affine, AspectRatio, Length, SvgColor, SvgPaint, absolute_px, parse_aspect_ratio, parse_color, parse_func_iri,
    parse_iri, parse_length, parse_length_list, parse_number, parse_number_or_percent, parse_paint, parse_points,
    parse_transform, parse_view_box,
};

/// The SVG namespace.
const SVG_NS: &str = "http://www.w3.org/2000/svg";
/// The XLink namespace (`xlink:href`).
const XLINK_NS: &str = "http://www.w3.org/1999/xlink";

/// An element's presentation attributes as given (SkSVGPresentationAttributes): each unset where it wasn't given
/// (or didn't read, or is `inherit`); the inherited ones first, then those that aren't (opacity, display,
/// clip-path, stop-color, stop-opacity).
#[derive(Clone, Default, Debug)]
pub(crate) struct Props {
    pub fill: Option<SvgPaint>,
    pub fill_opacity: Option<f32>,
    pub fill_rule: Option<PathFillType>,
    pub clip_rule: Option<PathFillType>,
    pub stroke: Option<SvgPaint>,
    pub stroke_opacity: Option<f32>,
    pub stroke_width: Option<Length>,
    pub stroke_linecap: Option<StrokeCap>,
    pub stroke_linejoin: Option<StrokeJoin>,
    pub stroke_miterlimit: Option<f32>,
    /// `Some(None)`: `none`.
    pub stroke_dasharray: Option<Option<Vec<Length>>>,
    pub stroke_dashoffset: Option<Length>,
    pub color: Option<SvgColor>,
    pub visibility_hidden: Option<bool>,
    pub opacity: Option<f32>,
    pub display_none: bool,
    pub clip_path: Option<String>,
    pub stop_color: Option<SvgColor>,
    pub stop_opacity: Option<f32>,
}

impl Props {
    /// Property `name` set to `value` (an attribute or a style declaration); a value that doesn't read leaves it as
    /// it was.
    fn set(&mut self, name: &str, value: &str) {
        let v = value.trim();
        if v == "inherit" {
            return;
        }
        let fill_rule = |v: &str| match v {
            "nonzero" => Some(PathFillType::NonZero),
            "evenodd" => Some(PathFillType::EvenOdd),
            _ => None,
        };
        let opacity = |v: &str| parse_number_or_percent(v).map(|o| o.clamp(0.0, 1.0));
        fn put<T>(slot: &mut Option<T>, v: Option<T>) {
            if v.is_some() {
                *slot = v;
            }
        }
        match name {
            "fill" => put(&mut self.fill, parse_paint(v)),
            "fill-opacity" => put(&mut self.fill_opacity, opacity(v)),
            "fill-rule" => put(&mut self.fill_rule, fill_rule(v)),
            "clip-rule" => put(&mut self.clip_rule, fill_rule(v)),
            "stroke" => put(&mut self.stroke, parse_paint(v)),
            "stroke-opacity" => put(&mut self.stroke_opacity, opacity(v)),
            "stroke-width" => put(&mut self.stroke_width, parse_length(v).filter(|l| l.value >= 0.0)),
            "stroke-linecap" => put(
                &mut self.stroke_linecap,
                match v {
                    "butt" => Some(StrokeCap::Butt),
                    "round" => Some(StrokeCap::Round),
                    "square" => Some(StrokeCap::Square),
                    _ => None,
                },
            ),
            "stroke-linejoin" => put(
                &mut self.stroke_linejoin,
                match v {
                    "miter" => Some(StrokeJoin::Miter),
                    "round" => Some(StrokeJoin::Round),
                    "bevel" => Some(StrokeJoin::Bevel),
                    _ => None,
                },
            ),
            "stroke-miterlimit" => put(&mut self.stroke_miterlimit, parse_number(v).filter(|m| *m >= 1.0)),
            "stroke-dasharray" => {
                put(&mut self.stroke_dasharray, if v == "none" { Some(None) } else { parse_length_list(v).map(Some) })
            }
            "stroke-dashoffset" => put(&mut self.stroke_dashoffset, parse_length(v)),
            "color" => put(&mut self.color, parse_color(v)),
            "visibility" => put(
                &mut self.visibility_hidden,
                match v {
                    "visible" => Some(false),
                    "hidden" | "collapse" => Some(true),
                    _ => None,
                },
            ),
            "opacity" => put(&mut self.opacity, opacity(v)),
            "display" => self.display_none = v == "none",
            "clip-path" => {
                if v == "none" {
                    self.clip_path = None;
                } else {
                    put(&mut self.clip_path, parse_func_iri(v));
                }
            }
            "stop-color" => put(&mut self.stop_color, parse_color(v)),
            "stop-opacity" => put(&mut self.stop_opacity, opacity(v)),
            _ => {}
        }
    }

    /// An element's: its attributes, then its `style` attribute's declarations (`name: value; …`) over them.
    fn of(e: &Element) -> Props {
        let mut p = Props::default();
        for a in e.attributes.iter().filter(|a| a.namespace_uri.is_empty() && a.local_name != "style") {
            p.set(&a.local_name, &a.value);
        }
        let style = e.get_attribute("style");
        for decl in strip_css_comments(style).split(';') {
            if let Some((name, value)) = decl.split_once(':') {
                let value = value.trim();
                let value = value.strip_suffix("!important").unwrap_or(value);
                p.set(name.trim(), value);
            }
        }
        p
    }
}

/// CSS text without its `/* … */` comments.
fn strip_css_comments(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find("/*") {
        out.push_str(&rest[..i]);
        match rest[i + 2..].find("*/") {
            Some(j) => rest = &rest[i + 2 + j + 2..],
            None => return out,
        }
    }
    out.push_str(rest);
    out
}

/// A viewport an element establishes (an `svg`'s or a `symbol`'s): where (x, y), its size (none: 100%), its
/// viewBox and how it goes in.
#[derive(Clone, Debug)]
pub(crate) struct Viewport {
    pub x: Length,
    pub y: Length,
    pub width: Option<Length>,
    pub height: Option<Length>,
    pub view_box: Option<Rect>,
    pub par: AspectRatio,
}

/// A `use`: what it refers to, where it puts it, the size it gives a `symbol` or an `svg`.
#[derive(Clone, Debug)]
pub(crate) struct UseRef {
    pub href: Option<String>,
    pub x: Length,
    pub y: Length,
    pub width: Option<Length>,
    pub height: Option<Length>,
}

/// A gradient's attributes as given (each unset is taken from the one its href refers to, then the default).
#[derive(Clone, Debug, Default)]
pub(crate) struct Gradient {
    pub radial: bool,
    pub href: Option<String>,
    pub units_obb: Option<bool>,
    pub transform: Option<Affine>,
    pub spread: Option<TileMode>,
    /// Linear: x1, y1, x2, y2; radial: cx, cy, r, fx, fy.
    pub coords: [Option<Length>; 5],
}

/// What an element is.
#[derive(Clone, Debug)]
pub(crate) enum Kind {
    Svg(Viewport),
    G,
    Defs,
    Switch,
    Symbol(Viewport),
    Use(UseRef),
    Path(Path),
    Rect {
        x: Length,
        y: Length,
        width: Length,
        height: Length,
        rx: Option<Length>,
        ry: Option<Length>,
    },
    Circle {
        cx: Length,
        cy: Length,
        r: Length,
    },
    Ellipse {
        cx: Length,
        cy: Length,
        rx: Option<Length>,
        ry: Option<Length>,
    },
    Line {
        x1: Length,
        y1: Length,
        x2: Length,
        y2: Length,
    },
    Polyline(Vec<Offset>),
    Polygon(Vec<Offset>),
    Gradient(Gradient),
    Stop {
        offset: f32,
    },
    ClipPath {
        obb: bool,
    },
    /// A graphics element drawn as nothing here (`text`, `image`, `foreignObject`).
    Unsupported,
}

impl Kind {
    fn is_shape(&self) -> bool {
        matches!(
            self,
            Kind::Path(_)
                | Kind::Rect { .. }
                | Kind::Circle { .. }
                | Kind::Ellipse { .. }
                | Kind::Line { .. }
                | Kind::Polyline(_)
                | Kind::Polygon(_)
        )
    }
    /// A graphics element or container that renders where it is (what a `switch` picks among).
    fn is_rendered(&self) -> bool {
        self.is_shape() || matches!(self, Kind::Svg(_) | Kind::G | Kind::Switch | Kind::Use(_) | Kind::Unsupported)
    }
}

/// An element: what it is, its presentation attributes, its transform, its children (indices), and whether its
/// conditional attributes hold (`requiredExtensions`: none is supported).
#[derive(Clone, Debug)]
pub(crate) struct Node {
    pub kind: Kind,
    pub props: Props,
    pub transform: Affine,
    pub children: Vec<usize>,
    pub conditions_hold: bool,
}

/// A parsed SVG document: its elements (the root `svg` first) and their ids.
pub(crate) struct SvgDom {
    pub(crate) nodes: Vec<Node>,
    pub(crate) ids: HashMap<String, usize>,
}

impl SvgDom {
    /// The document of `bytes` (SkSVGDOM::MakeFromStream): XML that isn't well formed, or a root that isn't an SVG
    /// `svg` (in the SVG namespace or none), is an error.
    pub(crate) fn parse(bytes: &[u8]) -> Result<SvgDom, SvgParseError> {
        let root = parse_xml_bytes(bytes).map_err(|e| SvgParseError(format!("malformed SVG: {e}")))?;
        if root.local_name != "svg" || !is_svg_ns(&root) {
            return Err(SvgParseError(format!("the root element is <{}>, not <svg>", root.node_name)));
        }
        let mut dom = SvgDom { nodes: Vec::new(), ids: HashMap::new() };
        dom.build(&root);
        Ok(dom)
    }

    /// The root `svg`'s viewport.
    fn root_viewport(&self) -> &Viewport {
        match &self.nodes[0].kind {
            Kind::Svg(vp) => vp,
            _ => unreachable!("the root is an svg"),
        }
    }

    /// The root `svg`'s width and height in px (absolute units converted as Skia's SkSVGLengthContext converts
    /// them: 90 px an inch; an em 16 px); a percentage or a missing one: none.
    pub(crate) fn root_size_px(&self) -> (Option<f32>, Option<f32>) {
        let vp = self.root_viewport();
        (vp.width.and_then(absolute_px), vp.height.and_then(absolute_px))
    }

    pub(crate) fn has_view_box(&self) -> bool {
        self.root_viewport().view_box.is_some()
    }

    pub(crate) fn set_view_box(&mut self, view_box: Rect) {
        if let Kind::Svg(vp) = &mut self.nodes[0].kind {
            vp.view_box = Some(view_box);
        }
    }

    /// Draws the document into `scope` as Skia's SVGDOM renders it after CMP set the root's width and height to
    /// `width` and `height` px and its preserveAspectRatio to `none`: its viewBox stretched onto (0, 0, width,
    /// height); without one, a user unit a px from the origin.
    pub(crate) fn render(&self, scope: &mut DrawScope<'_>, width: f32, height: f32) {
        render::render(self, scope, width, height);
    }

    /// Element `e` (and what is in it) added; none if it isn't one Skia's SVG module knows (left out with what is in
    /// it).
    fn build(&mut self, e: &Element) -> Option<usize> {
        let len = |name: &str| parse_length(attr(e, name));
        let len0 = |name: &str| len(name).unwrap_or(Length::number(0.0));
        let viewport = || Viewport {
            x: len0("x"),
            y: len0("y"),
            width: len("width"),
            height: len("height"),
            view_box: parse_view_box(attr(e, "viewBox")),
            par: parse_aspect_ratio(attr(e, "preserveAspectRatio")).unwrap_or(AspectRatio::DEFAULT),
        };
        let (kind, has_children) = match e.local_name.as_str() {
            "svg" => (Kind::Svg(viewport()), true),
            "g" => (Kind::G, true),
            "defs" => (Kind::Defs, true),
            "switch" => (Kind::Switch, true),
            "symbol" => (Kind::Symbol(viewport()), true),
            "use" => {
                let href = match e.get_attribute_ns(XLINK_NS, "href") {
                    "" => attr(e, "href"),
                    h => h,
                };
                let u = UseRef {
                    href: parse_iri(href),
                    x: len0("x"),
                    y: len0("y"),
                    width: len("width"),
                    height: len("height"),
                };
                (Kind::Use(u), false)
            }
            "path" => {
                let mut p = to_path(&add_path_nodes(attr(e, "d")));
                p.fill_type = PathFillType::NonZero;
                (Kind::Path(p), false)
            }
            "rect" => (
                Kind::Rect {
                    x: len0("x"),
                    y: len0("y"),
                    width: len0("width"),
                    height: len0("height"),
                    rx: len("rx"),
                    ry: len("ry"),
                },
                false,
            ),
            "circle" => (Kind::Circle { cx: len0("cx"), cy: len0("cy"), r: len0("r") }, false),
            "ellipse" => (Kind::Ellipse { cx: len0("cx"), cy: len0("cy"), rx: len("rx"), ry: len("ry") }, false),
            "line" => (Kind::Line { x1: len0("x1"), y1: len0("y1"), x2: len0("x2"), y2: len0("y2") }, false),
            "polyline" => (Kind::Polyline(parse_points(attr(e, "points"))), false),
            "polygon" => (Kind::Polygon(parse_points(attr(e, "points"))), false),
            name @ ("linearGradient" | "radialGradient") => {
                let radial = name == "radialGradient";
                let href = match e.get_attribute_ns(XLINK_NS, "href") {
                    "" => attr(e, "href"),
                    h => h,
                };
                let names: &[&str] = if radial { &["cx", "cy", "r", "fx", "fy"] } else { &["x1", "y1", "x2", "y2"] };
                let mut coords = [None; 5];
                for (c, n) in coords.iter_mut().zip(names) {
                    *c = len(n);
                }
                let g = Gradient {
                    radial,
                    href: parse_iri(href),
                    units_obb: units(attr(e, "gradientUnits")),
                    transform: parse_transform(attr(e, "gradientTransform")),
                    spread: match attr(e, "spreadMethod").trim() {
                        "pad" => Some(TileMode::Clamp),
                        "reflect" => Some(TileMode::Mirror),
                        "repeat" => Some(TileMode::Repeated),
                        _ => None,
                    },
                    coords,
                };
                (Kind::Gradient(g), true)
            }
            "stop" => {
                let offset = parse_number_or_percent(attr(e, "offset")).unwrap_or(0.0).clamp(0.0, 1.0);
                (Kind::Stop { offset }, false)
            }
            "clipPath" => (Kind::ClipPath { obb: units(attr(e, "clipPathUnits")).unwrap_or(false) }, true),
            "text" | "image" | "foreignObject" => (Kind::Unsupported, false),
            _ => return None,
        };
        let transform = match kind {
            Kind::Gradient(_) => Affine::IDENTITY,
            _ => parse_transform(attr(e, "transform")).unwrap_or(Affine::IDENTITY),
        };
        let index = self.nodes.len();
        self.nodes.push(Node {
            kind,
            props: Props::of(e),
            transform,
            children: Vec::new(),
            conditions_hold: !e.has_attribute("requiredExtensions"),
        });
        let id = attr(e, "id");
        if !id.is_empty() {
            self.ids.entry(id.to_string()).or_insert(index);
        }
        if has_children {
            let children: Vec<usize> =
                e.child_elements().filter(|c| is_svg_ns(c)).filter_map(|c| self.build(c)).collect();
            self.nodes[index].children = children;
        }
        Some(index)
    }
}

/// Whether an element is SVG's (in its namespace, or none).
fn is_svg_ns(e: &Element) -> bool {
    e.namespace_uri.is_empty() || e.namespace_uri == SVG_NS
}

/// Attribute `name` of no namespace (empty where it has none).
fn attr<'e>(e: &'e Element, name: &str) -> &'e str {
    e.get_attribute_ns("", name)
}

/// `objectBoundingBox` (true) or `userSpaceOnUse` (false).
fn units(v: &str) -> Option<bool> {
    match v.trim() {
        "objectBoundingBox" => Some(true),
        "userSpaceOnUse" => Some(false),
        _ => None,
    }
}
