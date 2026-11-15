//! Glyph outlines from skrifa at a whole pixel size, hinted as FreeType hints for the C++ Kompoz's text (light:
//! FreeType's automatic hinter moving points only vertically; the font's own PostScript hints lightly), held as
//! FreeType holds them (26.6, y up, tagged points) so the rasterizer cuts their curves as FreeType does; then drawn
//! in a style: stretched and slanted after hinting (FreeType's FT_Set_Transform), faked bold (Skia's stroke and
//! fill) or italic (Skia's slant), outlined by a stroke.

use super::raster::{Coverage, FillRule, FixedMatrix, FixedOutline, FixedPoint, FixedSink, PointTag};
use kompoz_graphics::{Contour, Offset, Path, PathEffect, PathStroke, StrokeCap, StrokeJoin};
use kompoz_graphics::{flatten_path, path_effected, stroke_outline};
use kompoz_text::draw::GlyphSynthesis;
use kompoz_text::shape::fake_bold_strength;
use skrifa::font::glyph::outline::OutlineSource;
use skrifa::raw::TableProvider;
use skrifa::raw::limits::MAX_RECURSION_DEPTH;
use skrifa::raw::tables::glyf::{Glyf, Glyph as GlyfGlyph};
use skrifa::raw::tables::loca::Loca;
use skrifa::{DrawMemory, Font, FontRef, GlyphId, HintOptions, PathSink, RasterTarget, ScalerFont};

/// How far a curve cut into lines (to be stroked or emboldened) may stray from it: a sixteenth of a pixel, the
/// flatness FreeType's rasterizer cuts quadratics to.
pub const FLATTEN_TOLERANCE: f32 = 1.0 / 16.0;

/// How an outline is fitted to the pixel grid, as FreeType's load flags ask: not at all; lightly (the C++ text's
/// FT_LOAD_TARGET_LIGHT: TrueType outlines through the automatic hinter, vertically only, since FreeType's TrueType
/// driver doesn't hint lightly; PostScript outlines through their own hints, which the CFF driver applies lightly);
/// or as FT_LOAD_DEFAULT (a COLR v0 glyph's layers: the font's instructions, or the automatic hinter for a font
/// without any).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Hinting {
    None,
    Light,
    Normal,
}

/// A scaler drawing `font`'s outlines at `ppem` px with `hinting`.
pub fn scaler(font: &Font, ppem: u32, hinting: Hinting) -> ScalerFont {
    let ppem_f = ppem as f32;
    match hinting {
        Hinting::None => ScalerFont::unhinted(font.clone(), Some(ppem_f)),
        Hinting::Light => {
            let postscript = matches!(
                ScalerFont::unhinted(font.clone(), None).outline_source(),
                Some(OutlineSource::Cff | OutlineSource::Cff2 | OutlineSource::Type1)
            );
            let options = if postscript { HintOptions::interpreter() } else { HintOptions::auto() };
            ScalerFont::hinted(font.clone(), ppem_f, options.with_target(RasterTarget::Light))
        }
        Hinting::Normal => ScalerFont::hinted(font.clone(), ppem_f, HintOptions::auto_fallback()),
    }
}

/// Glyph `glyph`'s outline from `scaler`, in 26.6 px from its origin, y up (None if the font has none for it, or it
/// can't be drawn). An empty outline for a glyph with no contours (a space). `tags`: the tags of the glyph's points
/// in the font, contour by contour (see [`glyf_tags`]), to rebuild exactly the points FreeType holds; without them,
/// the builder's guess.
pub fn load(scaler: &ScalerFont, glyph: u32, tags: Option<Vec<Vec<PointTag>>>) -> Option<FixedOutline> {
    let outline = scaler.outline(GlyphId::new(glyph))?;
    let mut builder = OutlineBuilder { tags, ..OutlineBuilder::default() };
    let metrics = outline.draw(DrawMemory::Scratch, &mut builder).ok()?;
    let mut out = builder.finish();
    out.overlap = metrics.has_overlaps;
    Some(out)
}

/// The tags of a TrueType glyph's points as its `glyf` entry gives them (a composite's components' in turn), contour
/// by contour: what FreeType's outline of it holds, whatever hinting moves. None for a glyph not in `glyf` (or too
/// deeply nested).
pub fn glyf_tags(font: &FontRef<'_>, glyph: u32) -> Option<Vec<Vec<PointTag>>> {
    fn add(glyf: &Glyf<'_>, loca: &Loca<'_>, glyph: GlyphId, depth: usize, out: &mut Vec<Vec<PointTag>>) -> Option<()> {
        if depth > MAX_RECURSION_DEPTH {
            return None;
        }
        match loca.get_glyf(glyph, glyf).ok()? {
            None => Some(()),
            Some(GlyfGlyph::Simple(simple)) => {
                let points: Vec<_> = simple.points().collect();
                let mut first = 0usize;
                for end in simple.end_pts_of_contours() {
                    let end = end.get() as usize;
                    let contour = points.get(first..=end)?;
                    out.push(
                        contour
                            .iter()
                            .map(|p| match (p.on_curve, p.cubic) {
                                (true, _) => PointTag::On,
                                (false, true) => PointTag::Cubic,
                                (false, false) => PointTag::Conic,
                            })
                            .collect(),
                    );
                    first = end + 1;
                }
                Some(())
            }
            Some(GlyfGlyph::Composite(composite)) => {
                for component in composite.components() {
                    add(glyf, loca, component.glyph, depth + 1, out)?;
                }
                Some(())
            }
        }
    }
    let glyf = font.glyf().ok()?;
    let loca = font.loca(None).ok()?;
    let mut out = Vec::new();
    add(&glyf, &loca, GlyphId::new(glyph), 0, &mut out)?;
    Some(out)
}

/// A FreeType outline built from path callbacks (px, y up): each contour's points with their tags, as the font has
/// them. Given the font's tags for the glyph's contours, the callbacks are matched to them as skrifa walks a contour
/// (FreeType's start: the first point if on the curve, else the last if it is, else between the two), each point
/// put back in its place and the on-curve points skrifa adds between two quadratic controls left out (skrifa works
/// them out by C's halving before the glyph is placed, FreeType from points moved onto its bitmap, after any
/// transform). Without the tags (or callbacks that don't match them), a contour is taken as it comes: a closing
/// segment back to its start left to the decomposition (which always closes), an on-curve point between two
/// quadratic controls left out where it is their midpoint (one a font gives exactly there too, which changes it by a
/// 64th of a pixel where the halving rounds differently, or where the outline is transformed).
#[derive(Default)]
pub struct OutlineBuilder {
    outline: FixedOutline,
    contour: Vec<(FixedPoint, PointTag)>,
    tags: Option<Vec<Vec<PointTag>>>,
    contour_index: usize,
}

/// Where each point a contour's callbacks give comes from, as skrifa walks a contour with these tags: an index into
/// the contour, or None for an on-curve point it adds (between two controls, or the start between the last point and
/// the first). None if skrifa wouldn't walk it.
fn walk_pattern(tags: &[PointTag]) -> Option<Vec<Option<usize>>> {
    /// Controls seen and not yet drawn.
    enum Pending {
        Empty,
        Quad(Option<usize>),
        Cubic(Option<usize>),
        TwoCubics(Option<usize>, Option<usize>),
    }
    fn emit(point: Option<usize>, tag: PointTag, state: &mut Pending, out: &mut Vec<Option<usize>>) -> bool {
        match (&*state, tag) {
            (Pending::Empty, PointTag::Conic) => *state = Pending::Quad(point),
            (Pending::Empty, PointTag::Cubic) => *state = Pending::Cubic(point),
            (Pending::Empty, PointTag::On) => out.push(point),
            (&Pending::Quad(q), PointTag::Conic) => {
                out.extend([q, None]);
                *state = Pending::Quad(point);
            }
            (&Pending::Quad(q), PointTag::On) => {
                out.extend([q, point]);
                *state = Pending::Empty;
            }
            (&Pending::Cubic(c), PointTag::Cubic) => *state = Pending::TwoCubics(c, point),
            (&Pending::TwoCubics(c0, c1), PointTag::Cubic) => {
                out.extend([c0, c1, None]);
                *state = Pending::Cubic(point);
            }
            (&Pending::TwoCubics(c0, c1), PointTag::On) => {
                out.extend([c0, c1, point]);
                *state = Pending::Empty;
            }
            _ => return false,
        }
        true
    }
    let n = tags.len();
    if n == 0 {
        return Some(Vec::new());
    }
    let (start, base, count) = match (tags[0], tags[n - 1]) {
        (PointTag::On, _) => (Some(0), 1, n - 1),
        (_, PointTag::On) => (Some(n - 1), 0, n - 1),
        (PointTag::Cubic, PointTag::Cubic) | (PointTag::Conic, PointTag::Conic) => (None, 0, n),
        _ => return None,
    };
    let mut out = vec![start];
    let mut state = Pending::Empty;
    for k in base..base + count {
        let i = k % n;
        if !emit(Some(i), tags[i], &mut state, &mut out) {
            return None;
        }
    }
    if !matches!(state, Pending::Empty) && !emit(start, PointTag::On, &mut state, &mut out) {
        return None;
    }
    Some(out)
}

impl OutlineBuilder {
    fn end_contour(&mut self) {
        let mut points = std::mem::take(&mut self.contour);
        if points.is_empty() {
            return;
        }
        let index = self.contour_index;
        self.contour_index += 1;
        if let Some(tags) = self.tags.as_ref().and_then(|t| t.get(index))
            && let Some(pattern) = walk_pattern(tags)
            && pattern.len() == points.len()
        {
            let mut placed: Vec<Option<FixedPoint>> = vec![None; tags.len()];
            for (from, &(p, _)) in pattern.iter().zip(&points) {
                if let Some(i) = *from {
                    placed[i] = Some(p);
                }
            }
            if placed.iter().all(|p| p.is_some()) {
                for (p, &tag) in placed.into_iter().zip(tags) {
                    self.outline.points.push(p.unwrap_or_default());
                    self.outline.tags.push(tag);
                }
                self.outline.contours.push(self.outline.points.len() - 1);
                return;
            }
        }
        // An explicit last point back on the start: the decomposition's own closing segment draws it.
        if points.len() > 1 && points[points.len() - 1] == (points[0].0, PointTag::On) {
            points.pop();
        }
        let trunc_mid =
            |a: FixedPoint, b: FixedPoint| FixedPoint::new(a.x.wrapping_add(b.x) / 2, a.y.wrapping_add(b.y) / 2);
        let n = points.len();
        let mut keep = vec![true; n];
        for i in 1..n {
            let (p, tag) = points[i];
            if tag == PointTag::On
                && i + 1 < n
                && points[i - 1].1 == PointTag::Conic
                && points[i + 1].1 == PointTag::Conic
                && p == trunc_mid(points[i - 1].0, points[i + 1].0)
            {
                keep[i] = false;
            }
        }
        // The start, between the last control and the first.
        if n > 2
            && points[1].1 == PointTag::Conic
            && points[n - 1].1 == PointTag::Conic
            && points[0].0 == trunc_mid(points[n - 1].0, points[1].0)
        {
            keep[0] = false;
        }
        let mut any = false;
        for (i, (p, tag)) in points.into_iter().enumerate() {
            if keep[i] {
                self.outline.points.push(p);
                self.outline.tags.push(tag);
                any = true;
            }
        }
        if any {
            self.outline.contours.push(self.outline.points.len() - 1);
        }
    }

    pub fn finish(mut self) -> FixedOutline {
        self.end_contour();
        self.outline
    }
}

impl PathSink for OutlineBuilder {
    fn move_to(&mut self, x: f32, y: f32) {
        self.end_contour();
        self.contour.push((FixedPoint::from_px(x, y), PointTag::On));
    }

    fn line_to(&mut self, x: f32, y: f32) {
        self.contour.push((FixedPoint::from_px(x, y), PointTag::On));
    }

    fn quad_to(&mut self, cx0: f32, cy0: f32, x: f32, y: f32) {
        self.contour.push((FixedPoint::from_px(cx0, cy0), PointTag::Conic));
        self.contour.push((FixedPoint::from_px(x, y), PointTag::On));
    }

    fn curve_to(&mut self, cx0: f32, cy0: f32, cx1: f32, cy1: f32, x: f32, y: f32) {
        self.contour.push((FixedPoint::from_px(cx0, cy0), PointTag::Cubic));
        self.contour.push((FixedPoint::from_px(cx1, cy1), PointTag::Cubic));
        self.contour.push((FixedPoint::from_px(x, y), PointTag::On));
    }

    fn close(&mut self) {
        self.end_contour();
    }
}

/// A stroke outlining glyphs (a text's DrawStyle::Stroke at the size drawn): `width` px (whole pixels, as C++'s
/// FT_Stroker radius was half a whole number), its ends (only a path effect opens a glyph's contours), joins and
/// miter limit (over half the width), its path effect first.
#[derive(Clone, PartialEq, Debug)]
pub struct GlyphStroke {
    pub width: u32,
    pub cap: StrokeCap,
    pub join: StrokeJoin,
    pub miter: f32,
    pub path_effect: Option<PathEffect>,
}

/// How an outline glyph is drawn besides its face and size (C++'s GlyphStyle with the synthesis): stretched across
/// by `scale_x` and slanted by `skew_x` as Android's TextPaint.textSkewX slants it (a point y above the baseline
/// moves y · -skew_x to the right: -0.25 leans right, as a synthetic italic), faked bold or italic, outlined by a
/// stroke.
#[derive(Clone, PartialEq, Debug)]
pub struct GlyphStyle {
    pub scale_x: f32,
    pub skew_x: f32,
    pub synthesis: GlyphSynthesis,
    pub stroke: Option<GlyphStroke>,
}

impl GlyphStyle {
    /// As the font draws it.
    pub const PLAIN: GlyphStyle =
        GlyphStyle { scale_x: 1.0, skew_x: 0.0, synthesis: GlyphSynthesis::None, stroke: None };

    /// Whether it draws glyphs as their font does.
    pub fn is_plain(&self) -> bool {
        *self == GlyphStyle::PLAIN
    }
}

impl Default for GlyphStyle {
    fn default() -> GlyphStyle {
        GlyphStyle::PLAIN
    }
}

/// `outline` (a hinted glyph at `px` px, text laid out `scale` times bigger than drawn) drawn in `style`, `shift`
/// (26.6) to the right, as coverage in its pixel box: stretched and slanted by the style's transform as FreeType
/// transforms a hinted outline (16.16 fixed point), slanted by a fake italic (Skia's: x moves by -skew · y), moved,
/// then filled, filled and stroked by a fake bold's width (Skia's: the font size times 1/24 down to 1/32), or stroked
/// (a fake bold's width added to the stroke's, as Skia adds it).
pub fn styled_coverage(mut outline: FixedOutline, px: u32, style: &GlyphStyle, scale: f32, shift: i32) -> Coverage {
    if style.scale_x != 1.0 || style.skew_x != 0.0 {
        // (Android's textSkewX is Skia's, whose y is down: x moves by skew_x · y there, by -skew_x · y here.)
        outline.transform(&FixedMatrix::from_f32(style.scale_x, -style.skew_x, 0.0, 1.0));
    }
    if style.synthesis.skew != 0.0 {
        outline.transform(&FixedMatrix::from_f32(1.0, -style.synthesis.skew, 0.0, 1.0));
    }
    if shift != 0 {
        outline.translate(shift, 0);
    }
    let extra = if style.synthesis.embolden { fake_bold_strength(px as f32) } else { 0.0 };
    if let Some(stroke) = &style.stroke {
        return stroked(&outline, stroke.width as f32 + extra, stroke, scale).render();
    }
    if extra > 0.0 {
        return emboldened(&outline, extra).render();
    }
    outline.render()
}

/// `outline` as a path in px times `scale`, y down (as kompoz-graphics draws paths).
pub fn to_path(outline: &FixedOutline, scale: f32) -> Path {
    struct ToPath(Path, f32);
    impl ToPath {
        fn px(&self, p: FixedPoint) -> (f32, f32) {
            (p.x as f32 / 64.0 * self.1, -(p.y as f32) / 64.0 * self.1)
        }
    }
    impl FixedSink for ToPath {
        fn move_to(&mut self, to: FixedPoint) {
            if !self.0.is_empty() {
                self.0.close();
            }
            let (x, y) = self.px(to);
            self.0.move_to(x, y);
        }
        fn line_to(&mut self, to: FixedPoint) {
            let (x, y) = self.px(to);
            self.0.line_to(x, y);
        }
        fn conic_to(&mut self, c: FixedPoint, to: FixedPoint) {
            let ((cx, cy), (x, y)) = (self.px(c), self.px(to));
            self.0.quadratic_to(cx, cy, x, y);
        }
        fn cubic_to(&mut self, c1: FixedPoint, c2: FixedPoint, to: FixedPoint) {
            let ((x1, y1), (x2, y2), (x, y)) = (self.px(c1), self.px(c2), self.px(to));
            self.0.cubic_to(x1, y1, x2, y2, x, y);
        }
    }
    let mut sink = ToPath(Path::new(), scale);
    outline.decompose(&mut sink);
    if !sink.0.is_empty() {
        sink.0.close();
    }
    sink.0
}

/// Closed polygons in px, y down, as an outline (26.6, y up).
pub fn from_contours<'a>(contours: impl IntoIterator<Item = &'a Contour>) -> FixedOutline {
    let mut out = FixedOutline::new();
    for c in contours {
        if c.points.len() > 1 {
            out.add_polygon(c.points.iter().map(|p| FixedPoint::from_px(p.x, -p.y)));
        }
    }
    out
}

/// Twice the signed area of a polygon (px, y down: positive clockwise on the screen).
fn signed_area(points: &[Offset]) -> f32 {
    let n = points.len();
    (0..n).map(|i| points[i].x * points[(i + 1) % n].y - points[(i + 1) % n].x * points[i].y).sum()
}

/// `outline` filled and stroked `width` px wide (Skia's stroke-and-fill, its fake bold), mitered (Skia's default
/// joins, limit 4): its fill and each contour's stroke, the two loops a stroke makes of a closed contour turned so
/// that the outer one winds the way the glyph's ink does, so that by the non-zero rule the stroke adds to the fill
/// wherever they meet (a fill and a stroke winding opposite ways would cancel).
pub fn emboldened(outline: &FixedOutline, width: f32) -> FixedOutline {
    let contours = flatten_path(&to_path(outline, 1.0), FLATTEN_TOLERANCE, |p| p);
    let ink: f32 = contours.iter().map(|c| signed_area(&c.points)).sum();
    let mut out = outline.clone();
    out.fill_rule = FillRule::NonZero;
    for c in &contours {
        let mut loops = stroke_outline(
            std::slice::from_ref(c),
            width,
            StrokeCap::Butt,
            StrokeJoin::Miter,
            PathStroke::DefaultMiter,
            FLATTEN_TOLERANCE,
        );
        if loops.len() == 2 {
            let (a, b) = (signed_area(&loops[0].points), signed_area(&loops[1].points));
            let outer = if a.abs() >= b.abs() { a } else { b };
            if (outer > 0.0) != (ink > 0.0) {
                for l in &mut loops {
                    l.points.reverse();
                }
            }
        }
        out.append(&from_contours(&loops));
    }
    out
}

/// `outline` stroked `width` px wide as `stroke` says, its path effect first on the outline at `1 / scale` of its
/// size (text laid out `scale` times bigger than it is drawn: the effect's lengths are px where drawn): the outline
/// the stroke covers, filled by the non-zero rule (what a path effect leaves to be filled, filled).
pub fn stroked(outline: &FixedOutline, width: f32, stroke: &GlyphStroke, scale: f32) -> FixedOutline {
    let scale = if stroke.path_effect.is_some() && scale > 0.0 { scale } else { 1.0 };
    let path = to_path(outline, 1.0 / scale);
    let mut path_stroke = PathStroke {
        width,
        cap: stroke.cap,
        join: stroke.join,
        miter: stroke.miter,
        hairline: false,
        path_effect: stroke.path_effect.clone(),
    };
    let effected = path_effected(&path, &mut path_stroke, 1.0);
    let contours = flatten_path(&effected, FLATTEN_TOLERANCE, |p| p * scale);
    if !path_stroke.strokes() {
        let mut out = from_contours(&contours);
        out.fill_rule = FillRule::NonZero;
        return out;
    }
    let outline = stroke_outline(
        &contours,
        path_stroke.width,
        path_stroke.cap,
        path_stroke.join,
        path_stroke.miter,
        FLATTEN_TOLERANCE,
    );
    from_contours(&outline)
}
