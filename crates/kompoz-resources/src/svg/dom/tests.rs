//! The SVG document's reading and drawing, drawing checked through a context that records what reaches it (with
//! the transforms in effect).

use super::values::{
    Affine, AspectRatio, SvgColor, SvgPaint, Unit, named_color_count, parse_aspect_ratio, parse_color, parse_length,
    parse_paint, parse_transform,
};
use super::{Kind, SvgDom};
use kompoz_graphics::{
    BlendMode, Brush, BrushKind, Color, ColorFilter, Density, FilterQuality, ImageBitmap, LayoutDirection, Matrix,
    Offset, Path, PathStroke, Rect, ShaderKind, Size, TileMode, Vertices,
};
use kompoz_ui::draw::DrawScope;
use kompoz_ui::graphics_context::{ClipOp, CornerRadii, GraphicsContext, LayerTransform, TextDraw};

// ---- A recording context ----------------------------------------------------------------------------------------

/// A drawn path: where it landed (its bounds through the transforms in effect), the transform, what painted it.
struct DrawnPath {
    bounds: Rect,
    ctm: Affine,
    brush: Brush,
    area: Rect,
    stroke: PathStroke,
}

impl DrawnPath {
    /// The color it paints at `p` where it landed.
    fn color_at(&self, p: Offset) -> Color {
        let local = self.ctm.invert().expect("invertible").map(p);
        self.brush.color_at(local, self.area)
    }
    fn shader_stops(&self) -> Vec<(f32, Color)> {
        match self.brush.kind() {
            BrushKind::Shader(s) => s.fixed_shader().unwrap().stops().iter().map(|s| (s.offset, s.color)).collect(),
            _ => Vec::new(),
        }
    }
}

#[derive(Default)]
struct Recorder {
    ctm_stack: Vec<Affine>,
    ctm: Option<Affine>,
    paths: Vec<DrawnPath>,
    /// Layers' bounds (where they land) and alphas.
    layers: Vec<(Rect, f32)>,
    /// Clips' bounds where they land.
    clips: Vec<Rect>,
    /// Pushes and pops in order: L l (layer), C c (clip), T t (transform), p (a path drawn).
    log: String,
}

fn affine_of(m: &Matrix) -> Affine {
    let v = &m.values;
    Affine {
        a: v[Matrix::ScaleX],
        b: v[Matrix::SkewY],
        c: v[Matrix::SkewX],
        d: v[Matrix::ScaleY],
        e: v[Matrix::TranslateX],
        f: v[Matrix::TranslateY],
    }
}

impl Recorder {
    fn ctm(&self) -> Affine {
        self.ctm.unwrap_or(Affine::IDENTITY)
    }
}

impl GraphicsContext for Recorder {
    fn fill_round_rect(&mut self, _: Rect, _: CornerRadii, _: Color) {
        self.log.push('r');
    }
    fn stroke_round_rect(&mut self, _: Rect, _: CornerRadii, _: f32, _: Color) {
        self.log.push('r');
    }
    fn draw_line(&mut self, _: Offset, _: Offset, _: f32, _: Color) {}
    fn draw_text(&mut self, _: &TextDraw, _: Color) {}
    fn draw_arc(&mut self, _: Rect, _: f32, _: f32, _: f32, _: Color, _: bool) {}
    fn draw_image(&mut self, _: &ImageBitmap, _: Rect, _: Rect, _: f32, _: FilterQuality) {}
    fn push_layer(&mut self, bounds: Rect, _: CornerRadii, alpha: f32, _: LayerTransform) {
        self.log.push('L');
        self.layers.push((self.ctm().map_rect(bounds), alpha));
    }
    fn pop_layer(&mut self) {
        self.log.push('l');
    }
    fn push_clip(&mut self, rect: Rect) {
        self.log.push('C');
        self.clips.push(self.ctm().map_rect(rect));
    }
    fn push_clip_round_rect(&mut self, rect: Rect, _: CornerRadii, _: ClipOp) {
        self.log.push('C');
        self.clips.push(self.ctm().map_rect(rect));
    }
    fn push_clip_path(&mut self, path: &Path, origin: Offset, _: ClipOp) {
        self.log.push('C');
        let mut p = path.clone();
        p.translate(origin);
        p.transform(&self.ctm().to_matrix());
        self.clips.push(p.get_bounds());
    }
    fn pop_clip(&mut self) {
        self.log.push('c');
    }
    fn push_translation(&mut self, by: Offset) {
        self.log.push('T');
        self.ctm_stack.push(self.ctm());
        self.ctm = Some(self.ctm().pre(Affine::translate(by.x, by.y)));
    }
    fn pop_translation(&mut self) {
        self.log.push('t');
        self.ctm = self.ctm_stack.pop();
    }
    fn push_transform(&mut self, matrix: &Matrix) {
        self.log.push('T');
        self.ctm_stack.push(self.ctm());
        self.ctm = Some(self.ctm().pre(affine_of(matrix)));
    }
    fn pop_transform(&mut self) {
        self.log.push('t');
        self.ctm = self.ctm_stack.pop();
    }
    fn push_color_filter(&mut self, _: &ColorFilter) {}
    fn pop_color_filter(&mut self) {}
    fn draw_vertices(&mut self, _: &Vertices, _: Offset, _: BlendMode, _: &Brush, _: Rect, _: f32) {}
    fn draw_path(&mut self, path: &Path, origin: Offset, brush: &Brush, brush_area: Rect, stroke: &PathStroke) {
        self.log.push('p');
        let ctm = self.ctm().pre(Affine::translate(origin.x, origin.y));
        let mut p = path.clone();
        p.transform(&ctm.to_matrix());
        self.paths.push(DrawnPath {
            bounds: super::render::tight_bounds(&p).unwrap_or(Rect::Zero),
            ctm,
            brush: brush.clone(),
            area: brush_area,
            stroke: stroke.clone(),
        });
    }
}

/// `svg` drawn at `w` by `h`.
fn draw(svg: &str, w: f32, h: f32) -> Recorder {
    let dom = SvgDom::parse(svg.as_bytes()).expect("an SVG");
    let mut rec = Recorder::default();
    {
        let mut scope = DrawScope::new(&mut rec, Size::new(w, h), LayoutDirection::Ltr, Density::new(1.0, 1.0));
        dom.render(&mut scope, w, h);
    }
    rec
}

fn svg(body: &str) -> String {
    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="24" height="24" viewBox="0 0 24 24">{body}</svg>"##
    )
}

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.01
}

fn assert_rect(r: Rect, l: f32, t: f32, rr: f32, b: f32) {
    assert!(
        close(r.left, l) && close(r.top, t) && close(r.right, rr) && close(r.bottom, b),
        "{r:?} isn't ({l}, {t}, {rr}, {b})"
    );
}

fn argb(c: Color) -> u32 {
    c.to_argb()
}

/// Channels within 2 of each other.
fn near(c: Color, want: u32) {
    let got = c.to_argb();
    let ok = (0..4).all(|k| ((got >> (k * 8)) & 0xFF).abs_diff((want >> (k * 8)) & 0xFF) <= 2);
    assert!(ok, "{got:08x} isn't {want:08x}");
}

// ---- Reading ----------------------------------------------------------------------------------------------------

#[test]
fn malformed_xml_and_other_roots_are_errors() {
    assert!(SvgDom::parse(b"<svg><g></svg>").is_err());
    assert!(SvgDom::parse(b"not xml").is_err());
    assert!(SvgDom::parse(b"<html xmlns='http://www.w3.org/1999/xhtml'/>").is_err());
    assert!(SvgDom::parse(b"<svg xmlns='http://example.com/other'/>").is_err());
    assert!(SvgDom::parse(b"<svg/>").is_ok());
    assert!(SvgDom::parse(b"<svg xmlns='http://www.w3.org/2000/svg'/>").is_ok());
}

#[test]
fn root_size_in_units() {
    let size =
        |w: &str, h: &str| SvgDom::parse(format!("<svg width='{w}' height='{h}'/>").as_bytes()).unwrap().root_size_px();
    let (w, h) = size("100", "24px");
    assert_eq!((w, h), (Some(100.0), Some(24.0)));
    let (w, h) = size("1in", "72.272pt");
    assert!(close(w.unwrap(), 90.0) && close(h.unwrap(), 90.0));
    let (w, h) = size("2.54cm", "25.4mm");
    assert!(close(w.unwrap(), 90.0) && close(h.unwrap(), 90.0));
    let (w, h) = size("1pc", "2em");
    // (Skia: a pica is twelve of its 1/72.272 in points, at 90 dpi)
    assert!(close(w.unwrap(), 90.0 * 12.0 / 72.272) && close(h.unwrap(), 32.0));
    assert_eq!(size("50%", "3ex"), (None, Some(24.0)));
    assert_eq!(SvgDom::parse(b"<svg/>").unwrap().root_size_px(), (None, None));
}

#[test]
fn view_box_read_and_set() {
    let mut dom = SvgDom::parse(b"<svg viewBox='0,0 24 12'/>").unwrap();
    assert!(dom.has_view_box());
    let Kind::Svg(vp) = &dom.nodes[0].kind else { panic!() };
    assert_eq!(vp.view_box, Some(Rect::new(0.0, 0.0, 24.0, 12.0)));
    assert!(!SvgDom::parse(b"<svg viewBox='0 0 -1 1'/>").unwrap().has_view_box());
    let mut plain = SvgDom::parse(b"<svg/>").unwrap();
    assert!(!plain.has_view_box());
    plain.set_view_box(Rect::new(0.0, 0.0, 5.0, 5.0));
    assert!(plain.has_view_box());
    dom.set_view_box(Rect::new(1.0, 1.0, 2.0, 2.0));
    let Kind::Svg(vp) = &dom.nodes[0].kind else { panic!() };
    assert_eq!(vp.view_box, Some(Rect::new(1.0, 1.0, 2.0, 2.0)));
}

#[test]
fn colors() {
    assert_eq!(named_color_count(), (147, true));
    assert_eq!(parse_color("red"), Some(SvgColor::Argb(0xFFFF0000)));
    assert_eq!(parse_color(" CornflowerBlue "), Some(SvgColor::Argb(0xFF6495ED)));
    assert_eq!(parse_color("lightgoldenrodyellow"), Some(SvgColor::Argb(0xFFFAFAD2)));
    assert_eq!(parse_color("#abc"), Some(SvgColor::Argb(0xFFAABBCC)));
    assert_eq!(parse_color("#12aBcD"), Some(SvgColor::Argb(0xFF12ABCD)));
    assert_eq!(parse_color("rgb(255, 0, 10)"), Some(SvgColor::Argb(0xFFFF000A)));
    assert_eq!(parse_color("rgb(100%,50%,0%)"), Some(SvgColor::Argb(0xFFFF8000)));
    assert_eq!(parse_color("rgba(0, 0, 255, 0.5)"), Some(SvgColor::Argb(0x800000FF)));
    assert_eq!(parse_color("rgb(300 -5 0)"), Some(SvgColor::Argb(0xFFFF0000)));
    assert_eq!(parse_color("currentColor"), Some(SvgColor::Current));
    // (Skia takes neither 4- nor 8-digit hex)
    assert_eq!(parse_color("#abcd"), None);
    assert_eq!(parse_color("#11223344"), None);
    assert_eq!(parse_color("notacolor"), None);
    assert_eq!(parse_color("rgb(1,2)"), None);
}

#[test]
fn paints() {
    assert_eq!(parse_paint("none"), Some(SvgPaint::None));
    assert_eq!(parse_paint("blue"), Some(SvgPaint::Color(SvgColor::Argb(0xFF0000FF))));
    assert_eq!(parse_paint("url(#g)"), Some(SvgPaint::Iri("g".into(), Some(SvgColor::Argb(0xFF000000)))));
    assert_eq!(parse_paint("url( '#g' ) red"), Some(SvgPaint::Iri("g".into(), Some(SvgColor::Argb(0xFFFF0000)))));
    assert_eq!(parse_paint("url(#g) none"), Some(SvgPaint::Iri("g".into(), None)));
    assert_eq!(parse_paint("url(g)"), None);
}

#[test]
fn lengths() {
    let l = parse_length(" 1.5e1px ").unwrap();
    assert_eq!((l.value, l.unit), (15.0, Unit::Px));
    let l = parse_length("2em").unwrap();
    assert_eq!((l.value, l.unit), (2.0, Unit::Em));
    let l = parse_length("-.5%").unwrap();
    assert_eq!((l.value, l.unit), (-0.5, Unit::Percentage));
    let l = parse_length("3").unwrap();
    assert_eq!((l.value, l.unit), (3.0, Unit::Number));
    assert!(parse_length("3 px").is_none());
    assert!(parse_length("px").is_none());
}

#[test]
fn transforms() {
    let t = parse_transform("translate(10) scale(2, 3)").unwrap();
    assert_eq!(t.map(Offset::new(1.0, 1.0)), Offset::new(12.0, 3.0));
    let t = parse_transform("rotate(90 5 5)").unwrap();
    let p = t.map(Offset::new(10.0, 5.0));
    assert!(close(p.x, 5.0) && close(p.y, 10.0), "{p:?}");
    let t = parse_transform("matrix(1 0 0 1 3 4),skewX(45)").unwrap();
    let p = t.map(Offset::new(0.0, 2.0));
    assert!(close(p.x, 5.0) && close(p.y, 6.0), "{p:?}");
    let t = parse_transform("skewY(45)").unwrap();
    let p = t.map(Offset::new(2.0, 0.0));
    assert!(close(p.x, 2.0) && close(p.y, 2.0), "{p:?}");
    assert_eq!(parse_transform(""), Some(Affine::IDENTITY));
    assert_eq!(parse_transform("translate(1,2,3)"), None);
    assert_eq!(parse_transform("wobble(1)"), None);
    assert_eq!(parse_transform("scale(2"), None);
}

#[test]
fn aspect_ratios() {
    assert_eq!(parse_aspect_ratio("none"), Some(AspectRatio::NONE));
    assert_eq!(parse_aspect_ratio("xMinYMax slice"), Some(AspectRatio { align: Some((0, 2)), slice: true }));
    assert_eq!(parse_aspect_ratio("defer xMidYMid meet"), Some(AspectRatio::DEFAULT));
    assert_eq!(parse_aspect_ratio("xMidYMid fit"), None);
    assert_eq!(parse_aspect_ratio("xLeftYMid"), None);
}

#[test]
fn style_declarations_win_over_attributes() {
    let dom = SvgDom::parse(
        br##"<svg><rect fill="red" stroke="blue" style="fill: #00ff00 ; stroke-width:3px; /* c */ opacity: 50%"/></svg>"##,
    )
    .unwrap();
    let p = &dom.nodes[1].props;
    assert_eq!(p.fill, Some(SvgPaint::Color(SvgColor::Argb(0xFF00FF00))));
    assert_eq!(p.stroke, Some(SvgPaint::Color(SvgColor::Argb(0xFF0000FF))));
    assert_eq!(p.stroke_width.map(|l| (l.value, l.unit)), Some((3.0, Unit::Px)));
    assert_eq!(p.opacity, Some(0.5));
}

#[test]
fn unknown_elements_are_left_out_with_what_is_in_them() {
    let dom = SvgDom::parse(br##"<svg><foo><rect id="r" width="1" height="1"/></foo><metadata/><g/></svg>"##).unwrap();
    assert_eq!(dom.nodes.len(), 2);
    assert!(!dom.ids.contains_key("r"));
    assert!(matches!(dom.nodes[1].kind, Kind::G));
}

#[test]
fn use_href_and_xlink_href() {
    let dom = SvgDom::parse(
        br##"<svg xmlns:xlink="http://www.w3.org/1999/xlink"><rect id="r"/><use xlink:href="#r"/><use href="#r" x="3"/></svg>"##,
    )
    .unwrap();
    let hrefs: Vec<_> = dom
        .nodes
        .iter()
        .filter_map(|n| match &n.kind {
            Kind::Use(u) => Some(u.href.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(hrefs, [Some("r".to_string()), Some("r".to_string())]);
}

// ---- Drawing ----------------------------------------------------------------------------------------------------

#[test]
fn a_rect_through_the_view_box() {
    let rec = draw(&svg(r##"<rect x="2" y="4" width="10" height="6" fill="#ff0000"/>"##), 48.0, 48.0);
    assert_eq!(rec.paths.len(), 1);
    let p = &rec.paths[0];
    assert_rect(p.bounds, 4.0, 8.0, 24.0, 20.0);
    assert_eq!(argb(p.color_at(Offset::new(10.0, 10.0))), 0xFFFF0000);
    assert_eq!(p.stroke.width, 0.0);
}

#[test]
fn the_view_box_is_stretched_not_kept() {
    // (CMP sets the root's preserveAspectRatio to none)
    let rec = draw(&svg(r##"<rect width="24" height="24"/>"##), 48.0, 24.0);
    assert_rect(rec.paths[0].bounds, 0.0, 0.0, 48.0, 24.0);
    // Without a viewBox: a user unit a px.
    let rec = draw(r##"<svg width="10" height="10"><rect x="1" y="1" width="4" height="4"/></svg>"##, 40.0, 40.0);
    assert_rect(rec.paths[0].bounds, 1.0, 1.0, 5.0, 5.0);
    // An empty one: nothing.
    let rec = draw(r##"<svg viewBox="0 0 0 10"><rect width="4" height="4"/></svg>"##, 40.0, 40.0);
    assert!(rec.paths.is_empty());
}

#[test]
fn inherited_fill_and_stroke() {
    let rec = draw(
        &svg(
            r##"<g fill="green" stroke="blue" stroke-width="2" stroke-linecap="round"><circle cx="12" cy="12" r="5"/></g>"##,
        ),
        24.0,
        24.0,
    );
    assert_eq!(rec.paths.len(), 2);
    let (fill, stroke) = (&rec.paths[0], &rec.paths[1]);
    assert_rect(fill.bounds, 7.0, 7.0, 17.0, 17.0);
    assert_eq!(argb(fill.color_at(Offset::new(12.0, 12.0))), 0xFF008000);
    assert_eq!(stroke.stroke.width, 2.0);
    assert_eq!(stroke.stroke.cap, kompoz_graphics::StrokeCap::Round);
    assert_eq!(argb(stroke.color_at(Offset::new(12.0, 7.0))), 0xFF0000FF);
}

#[test]
fn current_color_and_opacities() {
    let rec = draw(
        &svg(r##"<g color="#00ff00"><rect width="4" height="4" fill="currentColor" fill-opacity="0.5"/></g>"##),
        24.0,
        24.0,
    );
    near(rec.paths[0].color_at(Offset::new(2.0, 2.0)), 0x8000FF00);
}

#[test]
fn group_opacity_is_a_layer_and_a_shapes_its_paint() {
    let rec = draw(
        &svg(r##"<g opacity="0.5"><rect width="4" height="4"/><rect y="5" width="4" height="4"/></g>"##),
        24.0,
        24.0,
    );
    assert_eq!(rec.layers.len(), 1);
    assert!(close(rec.layers[0].1, 0.5));
    assert_rect(rec.layers[0].0, 0.0, 0.0, 24.0, 24.0);
    assert!(rec.log.contains("Lpp"), "{}", rec.log);
    near(rec.paths[0].color_at(Offset::new(1.0, 1.0)), 0xFF000000);

    // A shape filled (only): its paint faded, no layer.
    let rec = draw(&svg(r##"<rect width="4" height="4" opacity="0.5" fill="red"/>"##), 24.0, 24.0);
    assert!(rec.layers.is_empty());
    near(rec.paths[0].color_at(Offset::new(1.0, 1.0)), 0x80FF0000);

    // Filled and stroked: a layer.
    let rec = draw(&svg(r##"<rect width="4" height="4" opacity="0.25" fill="red" stroke="blue"/>"##), 24.0, 24.0);
    assert_eq!(rec.layers.len(), 1);
    assert!(close(rec.layers[0].1, 0.25));
}

#[test]
fn hidden_and_undisplayed_draw_nothing() {
    let rec = draw(
        &svg(
            r##"<rect width="4" height="4" display="none"/><g visibility="hidden"><rect width="4" height="4"/></g><rect width="4" height="4" style="fill:none"/><defs><rect width="4" height="4"/></defs>"##,
        ),
        24.0,
        24.0,
    );
    assert!(rec.paths.is_empty());
}

#[test]
fn transforms_go_round_what_is_drawn() {
    let rec = draw(
        &svg(r##"<g transform="translate(5 5)"><rect transform="scale(2)" width="2" height="3"/></g>"##),
        24.0,
        24.0,
    );
    assert_rect(rec.paths[0].bounds, 5.0, 5.0, 9.0, 11.0);
}

#[test]
fn shapes() {
    let rec = draw(
        &svg(
            r##"<rect x="1" y="1" width="10" height="4" rx="1"/><ellipse cx="10" cy="10" rx="4"/><line x1="0" y1="0" x2="5" y2="5" stroke="red"/><polygon points="0,0 4,0 4,4 9"/><polyline points="1 1 2 3" fill="none" stroke="red"/><path d="M1 1 L 3 9 Z"/>"##,
        ),
        24.0,
        24.0,
    );
    let b: Vec<Rect> = rec.paths.iter().map(|p| p.bounds).collect();
    assert_eq!(b.len(), 6, "{b:?}");
    assert_rect(b[0], 1.0, 1.0, 11.0, 5.0);
    assert_rect(b[1], 6.0, 6.0, 14.0, 14.0); // (ry auto: rx)
    assert_rect(b[2], 0.0, 0.0, 5.0, 5.0); // (a line's stroke only)
    assert!(rec.paths[2].stroke.width > 0.0);
    assert_rect(b[3], 0.0, 0.0, 4.0, 4.0);
    assert_rect(b[4], 1.0, 1.0, 2.0, 3.0);
    assert_rect(b[5], 1.0, 1.0, 3.0, 9.0);
}

#[test]
fn percentages_of_the_viewport() {
    let rec = draw(&svg(r##"<rect x="50%" width="25%" height="50%"/>"##), 24.0, 24.0);
    assert_rect(rec.paths[0].bounds, 12.0, 0.0, 18.0, 12.0);
}

#[test]
fn use_moves_its_target_and_inherits_from_itself() {
    let rec = draw(
        &svg(
            r##"<defs><rect id="r" width="2" height="2"/></defs><use xlink:href="#r" x="10" y="5" fill="blue"/><use href="#r" transform="translate(1 1)"/>"##,
        ),
        24.0,
        24.0,
    );
    assert_eq!(rec.paths.len(), 2);
    assert_rect(rec.paths[0].bounds, 10.0, 5.0, 12.0, 7.0);
    assert_eq!(argb(rec.paths[0].color_at(Offset::new(11.0, 6.0))), 0xFF0000FF);
    assert_rect(rec.paths[1].bounds, 1.0, 1.0, 3.0, 3.0);
}

#[test]
fn a_symbol_through_use_gets_a_viewport() {
    let rec = draw(
        &svg(
            r##"<symbol id="s" viewBox="0 0 10 10"><rect width="10" height="5"/></symbol><rect width="1" height="1" fill="url(#s) red"/><use href="#s" x="2" y="2" width="20" height="20"/>"##,
        ),
        24.0,
        24.0,
    );
    // (a symbol isn't drawn where it is; a paint IRI that isn't a gradient takes its fallback)
    assert_eq!(rec.paths.len(), 2);
    assert_eq!(argb(rec.paths[0].color_at(Offset::new(0.5, 0.5))), 0xFFFF0000);
    assert_rect(rec.paths[1].bounds, 2.0, 2.0, 22.0, 12.0);
    assert_rect(*rec.clips.last().unwrap(), 2.0, 2.0, 22.0, 22.0);
}

#[test]
fn use_cycles_end() {
    let rec = draw(&svg(r##"<g id="a"><rect width="1" height="1"/><use href="#a" x="2"/></g>"##), 24.0, 24.0);
    assert!(rec.paths.len() <= 3 && !rec.paths.is_empty());
}

#[test]
fn nested_svg_has_its_viewport() {
    let rec = draw(
        &svg(r##"<svg x="4" y="4" width="8" height="8" viewBox="0 0 2 2"><rect width="1" height="1"/></svg>"##),
        24.0,
        24.0,
    );
    assert_rect(rec.paths[0].bounds, 4.0, 4.0, 8.0, 8.0);
    assert_rect(rec.clips[0], 4.0, 4.0, 12.0, 12.0);
}

#[test]
fn switch_draws_its_first_child_that_holds() {
    let rec = draw(
        &svg(
            r##"<switch><rect requiredExtensions="http://example.com/x" width="1" height="1" fill="red"/><rect width="2" height="2" fill="blue"/><rect width="3" height="3"/></switch>"##,
        ),
        24.0,
        24.0,
    );
    assert_eq!(rec.paths.len(), 1);
    assert_eq!(argb(rec.paths[0].color_at(Offset::new(1.0, 1.0))), 0xFF0000FF);
}

#[test]
fn a_linear_gradient_over_the_bounding_box() {
    let rec = draw(
        &svg(
            r##"<linearGradient id="g"><stop offset="0" stop-color="#ff0000"/><stop offset="100%" style="stop-color:#0000ff; stop-opacity:.5"/></linearGradient><rect x="4" y="4" width="10" height="10" fill="url(#g)"/>"##,
        ),
        24.0,
        24.0,
    );
    let p = &rec.paths[0];
    let stops = p.shader_stops();
    assert_eq!(stops.len(), 2);
    assert_eq!((stops[0].0, argb(stops[0].1)), (0.0, 0xFFFF0000));
    assert_eq!(stops[1].0, 1.0);
    near(stops[1].1, 0x800000FF);
    near(p.color_at(Offset::new(4.0, 9.0)), 0xFFFF0000);
    near(p.color_at(Offset::new(14.0, 9.0)), 0x800000FF);
    // (Clamp past its ends)
    near(p.color_at(Offset::new(20.0, 9.0)), 0x800000FF);
}

#[test]
fn a_gradient_takes_attributes_and_stops_from_its_href() {
    let rec = draw(
        &svg(
            r##"<linearGradient id="base" spreadMethod="repeat"><stop stop-color="white"/><stop offset="1" stop-color="black"/></linearGradient><linearGradient id="g" xlink:href="#base" gradientUnits="userSpaceOnUse" x1="0" y1="0" x2="0" y2="10"/><rect width="20" height="20" fill="url(#g)"/>"##,
        ),
        24.0,
        24.0,
    );
    let p = &rec.paths[0];
    assert_eq!(p.shader_stops().len(), 2);
    let BrushKind::Shader(s) = p.brush.kind() else { panic!("{:?}", p.brush) };
    let shader = s.fixed_shader().unwrap();
    assert_eq!(shader.kind(), ShaderKind::Linear);
    assert_eq!(shader.tile_mode(), TileMode::Repeated);
    near(p.color_at(Offset::new(5.0, 0.0)), 0xFFFFFFFF);
    near(p.color_at(Offset::new(5.0, 5.0)), 0xFF808080);
    near(p.color_at(Offset::new(15.0, 15.0)), 0xFF808080); // (repeated)
}

#[test]
fn a_radial_gradient_and_its_transform() {
    let rec = draw(
        &svg(
            r##"<radialGradient id="g" cx="0" cy="0" r="10" gradientUnits="userSpaceOnUse" gradientTransform="translate(12 12)"><stop stop-color="red"/><stop offset="1" stop-color="blue"/></radialGradient><rect width="24" height="24" fill="url(#g)"/>"##,
        ),
        24.0,
        24.0,
    );
    let p = &rec.paths[0];
    near(p.color_at(Offset::new(12.0, 12.0)), 0xFFFF0000);
    near(p.color_at(Offset::new(22.0, 12.0)), 0xFF0000FF);
}

#[test]
fn gradients_of_one_stop_or_none() {
    let rec = draw(
        &svg(
            r##"<linearGradient id="one"><stop stop-color="lime"/></linearGradient><linearGradient id="none"/><rect width="2" height="2" fill="url(#one)"/><rect width="2" height="2" fill="url(#none)"/><rect width="2" height="2" fill="url(#missing)"/>"##,
        ),
        24.0,
        24.0,
    );
    // (one stop: its color; none: nothing; a missing one: the fallback, black)
    assert_eq!(rec.paths.len(), 2);
    assert_eq!(argb(rec.paths[0].color_at(Offset::new(1.0, 1.0))), 0xFF00FF00);
    assert_eq!(argb(rec.paths[1].color_at(Offset::new(1.0, 1.0))), 0xFF000000);
}

#[test]
fn clip_paths() {
    let rec = draw(
        &svg(
            r##"<clipPath id="c"><rect x="2" y="2" width="5" height="5"/></clipPath><clipPath id="o" clipPathUnits="objectBoundingBox"><rect width="0.5" height="0.5"/></clipPath><rect width="20" height="20" clip-path="url(#c)"/><rect x="10" y="10" width="10" height="10" clip-path="url(#o)"/><rect width="1" height="1" clip-path="url(#nothing)"/>"##,
        ),
        48.0,
        48.0,
    );
    assert_eq!(rec.paths.len(), 3);
    assert_eq!(rec.clips.len(), 2);
    assert_rect(rec.clips[0], 4.0, 4.0, 14.0, 14.0);
    assert_rect(rec.clips[1], 20.0, 20.0, 30.0, 30.0);
    assert!(rec.log.starts_with("TCpcCpcp") || rec.log.contains("Cpc"), "{}", rec.log);
}

#[test]
fn dashes() {
    let rec =
        draw(&svg(r##"<line x2="20" stroke="black" stroke-dasharray="2 1 3" stroke-dashoffset="1"/>"##), 24.0, 24.0);
    assert_eq!(rec.paths.len(), 1);
    assert!(rec.paths[0].stroke.path_effect.is_some());
    let rec = draw(&svg(r##"<line x2="20" stroke="black" stroke-dasharray="0 0"/>"##), 24.0, 24.0);
    assert!(rec.paths[0].stroke.path_effect.is_none());
}
