//! Material 3 Expressive's shapes (Compose's MaterialShapes) and what they are made of, the shapes library
//! (androidx.graphics.shapes): polygons with rounded and smoothed corners ([`RoundedPolygon`]), stars, pills,
//! their features, morphs between them ([`Morph`]), SVG path data and serialized features; and the paths and
//! [`Shape`]s they make.
//!
//! ```ignore
//! Box(modifier = Modifier.size(96.dp()).clip(MaterialShapes::Cookie9Sided().to_shape(0)))
//! let morph = Arc::new(Morph::new(MaterialShapes::Circle().clone(), MaterialShapes::Square().clone()));
//! Box(modifier = Modifier.size(96.dp()).background(color, morph.to_shape(progress)))
//! ```

mod cubic;
mod features;
mod mapping;
mod morph;
mod rounded_polygon;
mod svg;
mod utils;

pub use cubic::{Cubic, MutableCubic, MutablePoint, PointTransformer, TransformResult};
pub use features::Feature;
pub use morph::Morph;
pub use rounded_polygon::{CornerRounding, RoundedPolygon};
pub use svg::{FeatureSerializer, SvgPathParser};

use crate::geometry::{Offset, Size};
use crate::matrix::Matrix;
use crate::path::{Outline, Path};
use crate::shape::Shape;
use crate::unit::{Density, LayoutDirection};
use std::fmt;
use std::sync::{Arc, OnceLock};
use utils::{FLOAT_PI, atan2, cos, sin};

// ---- Paths and shapes of polygons and morphs ---------------------------------------------------------------------

/// The cubics as a path (material3's pathFromCubics): from the first's start, each in turn, again if
/// `repeat_path`, closed if `close_path`; turned about the origin so that the first starts at `start_angle`
/// degrees from the pivot.
fn path_from_cubics(
    path: &mut Path,
    start_angle: i32,
    repeat_path: bool,
    close_path: bool,
    cubics: &[Cubic],
    rotation_pivot_x: f32,
    rotation_pivot_y: f32,
) {
    path.rewind();
    let Some(first) = cubics.first() else { return };
    path.move_to(first.anchor0_x(), first.anchor0_y());
    for c in cubics {
        path.cubic_to(c.control0_x(), c.control0_y(), c.control1_x(), c.control1_y(), c.anchor1_x(), c.anchor1_y());
    }
    if repeat_path {
        path.line_to(first.anchor0_x(), first.anchor0_y());
        for c in cubics {
            path.cubic_to(c.control0_x(), c.control0_y(), c.control1_x(), c.control1_y(), c.anchor1_x(), c.anchor1_y());
        }
    }
    if close_path {
        path.close();
    }
    if start_angle != 0 {
        let radians = atan2(first.anchor0_y() - rotation_pivot_y, first.anchor0_x() - rotation_pivot_x);
        let angle_to_first_cubic = (radians as f64 * 180.0 / std::f64::consts::PI) as f32;
        let mut m = Matrix::new();
        m.rotate_z(-angle_to_first_cubic + start_angle as f32);
        path.transform(&m);
    }
}

impl RoundedPolygon {
    /// Its outline as a path, turned to start at `start_angle` degrees (material3's RoundedPolygon.toPath; Compose's
    /// default 0, as it is).
    pub fn to_path(&self, start_angle: i32) -> Path {
        let mut path = Path::new();
        self.to_path_into(&mut path, start_angle, false, true);
        path
    }

    /// Its outline into `path` (emptied first): turned so that it starts `start_angle` degrees round its center,
    /// twice round if `repeat_path`, closed if `close_path` (material3's internal toPath, as its progress
    /// indicators draw parts of it; its default start 270, 12 o'clock).
    pub fn to_path_into(&self, path: &mut Path, start_angle: i32, repeat_path: bool, close_path: bool) {
        path_from_cubics(path, start_angle, repeat_path, close_path, self.cubics(), self.center_x(), self.center_y());
    }

    /// It as a [`Shape`] (material3's RoundedPolygon.toShape): its path stretched over the size it fills (its
    /// unit square, for a normalized one), centered there.
    ///
    /// Kotlin: `toShape(startAngle: Int = 0)`
    pub fn to_shape(&self, start_angle: i32) -> RoundedPolygonShape {
        RoundedPolygonShape { polygon: self.clone(), start_angle, path: self.to_path(start_angle) }
    }

    /// Its points through `matrix` (material3's internal RoundedPolygon.transformed(matrix)).
    pub fn transformed_by(&self, matrix: &Matrix) -> RoundedPolygon {
        let m = *matrix;
        self.transformed(move |x: f32, y: f32| {
            let p = m.map(Offset::new(x, y));
            (p.x, p.y)
        })
    }
}

impl Morph {
    /// The shape `progress` of the way as a path, turned to start at `start_angle` degrees about the origin
    /// (material3's Morph.toPath; Compose's default 0).
    pub fn to_path(&self, progress: f32, start_angle: i32) -> Path {
        let mut path = Path::new();
        self.to_path_into(progress, &mut path, start_angle, false, true, 0.0, 0.0);
        path
    }

    /// The shape `progress` of the way into `path` (emptied first): turned so that it starts `start_angle` degrees
    /// round the pivot, twice round if `repeat_path`, closed if `close_path` (material3's internal toPath; its
    /// defaults 270, false, true, a pivot at the origin).
    #[allow(clippy::too_many_arguments)]
    pub fn to_path_into(
        &self,
        progress: f32,
        path: &mut Path,
        start_angle: i32,
        repeat_path: bool,
        close_path: bool,
        rotation_pivot_x: f32,
        rotation_pivot_y: f32,
    ) {
        let cubics = self.as_cubics(progress);
        path_from_cubics(path, start_angle, repeat_path, close_path, &cubics, rotation_pivot_x, rotation_pivot_y);
    }

    /// The shape `progress` of the way as a [`Shape`]: a morph of normalized shapes stretched from the unit square
    /// over the size it fills (what Compose writes as an `object : Shape` over [`Morph::to_path`]).
    pub fn to_shape(self: &Arc<Morph>, progress: f32) -> MorphShape {
        MorphShape { morph: self.clone(), progress }
    }
}

/// A [`RoundedPolygon`] as a [`Shape`] ([`RoundedPolygon::to_shape`], material3's toShape): its path scaled to the
/// size it fills and centered there. Equal by its polygon and start angle.
#[derive(Clone)]
pub struct RoundedPolygonShape {
    polygon: RoundedPolygon,
    start_angle: i32,
    path: Path,
}

impl RoundedPolygonShape {
    pub fn polygon(&self) -> &RoundedPolygon {
        &self.polygon
    }
    pub fn start_angle(&self) -> i32 {
        self.start_angle
    }
}

impl PartialEq for RoundedPolygonShape {
    fn eq(&self, o: &RoundedPolygonShape) -> bool {
        self.polygon == o.polygon && self.start_angle == o.start_angle
    }
}

impl fmt::Debug for RoundedPolygonShape {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "RoundedPolygonShape(startAngle = {}, {:?})", self.start_angle, self.polygon)
    }
}

impl Shape for RoundedPolygonShape {
    fn create_outline(&self, size: Size, _: LayoutDirection, _: &Density) -> Outline {
        let mut path = Path::new();
        path.add_path(&self.path, Offset::Zero);
        let mut scale = Matrix::new();
        scale.scale(size.width, size.height, 1.0);
        path.transform(&scale);
        // its center on the size's
        let center = path.get_bounds().center();
        path.translate(size.center() - center);
        Outline::Generic(path)
    }
}

/// A [`Morph`] at a progress as a [`Shape`] ([`Morph::to_shape`]): its path stretched from the unit square over the
/// size it fills. Equal by its morph (the same one) and progress.
#[derive(Clone)]
pub struct MorphShape {
    morph: Arc<Morph>,
    progress: f32,
}

impl MorphShape {
    pub fn morph(&self) -> &Arc<Morph> {
        &self.morph
    }
    pub fn progress(&self) -> f32 {
        self.progress
    }
}

impl PartialEq for MorphShape {
    fn eq(&self, o: &MorphShape) -> bool {
        Arc::ptr_eq(&self.morph, &o.morph) && self.progress == o.progress
    }
}

impl fmt::Debug for MorphShape {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "MorphShape({:p}, progress = {})", Arc::as_ptr(&self.morph), self.progress)
    }
}

impl Shape for MorphShape {
    fn create_outline(&self, size: Size, _: LayoutDirection, _: &Density) -> Outline {
        let mut path = self.morph.to_path(self.progress, 0);
        let mut scale = Matrix::new();
        scale.scale(size.width, size.height, 1.0);
        path.transform(&scale);
        Outline::Generic(path)
    }
}

// ---- Material's shapes -------------------------------------------------------------------------------------------

/// A point of a custom polygon and its corner's rounding (material3's PointNRound).
#[derive(Clone, Copy)]
struct PointNRound {
    o: Offset,
    r: CornerRounding,
}

const fn pnr(x: f32, y: f32, r: CornerRounding) -> PointNRound {
    PointNRound { o: Offset::new(x, y), r }
}
const fn p(x: f32, y: f32) -> PointNRound {
    pnr(x, y, CornerRounding::Unrounded)
}
const fn r(radius: f32) -> CornerRounding {
    CornerRounding::new(radius, 0.0)
}
const fn rs(radius: f32, smoothing: f32) -> CornerRounding {
    CornerRounding::new(radius, smoothing)
}

/// Degrees in radians, as material3's (`this / 360f * 2 * PI`).
fn to_radians(degrees: f32) -> f32 {
    degrees / 360.0 * 2.0 * FLOAT_PI
}

/// Its angle round the origin in degrees.
fn angle_degrees(o: Offset) -> f32 {
    atan2(o.y, o.x) * 180.0 / FLOAT_PI
}

/// `o` turned `angle` degrees round `center`.
fn rotate_degrees(o: Offset, angle: f32, center: Offset) -> Offset {
    let a = to_radians(angle);
    let off = o - center;
    Offset::new(off.x * cos(a) - off.y * sin(a), off.x * sin(a) + off.y * cos(a)) + center
}

/// The points repeated `reps` times round `center`; or (`mirroring`) twice as many times, every other one mirrored.
fn do_repeat(points: &[PointNRound], reps: usize, center: Offset, mirroring: bool) -> Vec<PointNRound> {
    let mut out = Vec::new();
    if mirroring {
        let angles: Vec<f32> = points.iter().map(|p| angle_degrees(p.o - center)).collect();
        let distances: Vec<f32> = points.iter().map(|p| (p.o - center).get_distance()).collect();
        let actual_reps = reps * 2;
        let section_angle = 360.0 / actual_reps as f32;
        for it in 0..actual_reps {
            for index in 0..points.len() {
                let i = if it % 2 == 0 { index } else { points.len() - 1 - index };
                if i > 0 || it % 2 == 0 {
                    let a = to_radians(
                        section_angle * it as f32
                            + if it % 2 == 0 { angles[i] } else { section_angle - angles[i] + 2.0 * angles[0] },
                    );
                    let final_point = Offset::new(cos(a), sin(a)) * distances[i] + center;
                    out.push(PointNRound { o: final_point, r: points[i].r });
                }
            }
        }
    } else {
        let np = points.len();
        for it in 0..np * reps {
            let point = rotate_degrees(points[it % np].o, (it / np) as f32 * 360.0 / reps as f32, center);
            out.push(PointNRound { o: point, r: points[it % np].r });
        }
    }
    out
}

/// The polygon of the points repeated round `center` (material3's customPolygon).
fn custom_polygon(pnr: &[PointNRound], reps: usize, center: Offset, mirroring: bool) -> RoundedPolygon {
    let actual = do_repeat(pnr, reps, center, mirroring);
    let vertices: Vec<f32> = actual.iter().flat_map(|p| [p.o.x, p.o.y]).collect();
    let rounding: Vec<CornerRounding> = actual.iter().map(|p| p.r).collect();
    RoundedPolygon::from_vertices(&vertices, CornerRounding::Unrounded, Some(&rounding), Some(center))
}

fn custom(pnr: &[PointNRound], reps: usize) -> RoundedPolygon {
    custom_polygon(pnr, reps, Offset::new(0.5, 0.5), false)
}

fn mirrored(pnr: &[PointNRound], reps: usize) -> RoundedPolygon {
    custom_polygon(pnr, reps, Offset::new(0.5, 0.5), true)
}

fn rotated(degrees: f32) -> Matrix {
    let mut m = Matrix::new();
    m.rotate_z(degrees);
    m
}

fn scaled(x: f32, y: f32) -> Matrix {
    let mut m = Matrix::new();
    m.scale(x, y, 1.0);
    m
}

const ROUND15: CornerRounding = r(0.15);
const ROUND20: CornerRounding = r(0.2);
const ROUND30: CornerRounding = r(0.3);
const ROUND50: CornerRounding = r(0.5);
const ROUND100: CornerRounding = r(1.0);

fn star(n: i32, inner_radius: f32, rounding: CornerRounding) -> RoundedPolygon {
    RoundedPolygon::star(n, 1.0, inner_radius, rounding, None, None, 0.0, 0.0)
}

/// Material 3 Expressive's 35 shapes (Compose's MaterialShapes), each made once, normalized into the unit square.
pub struct MaterialShapes;

/// One of [`MaterialShapes`]' shapes, made once (normalized into the unit square) the first time it is asked for.
macro_rules! shape {
    ($make:expr) => {{
        static SHAPE: OnceLock<RoundedPolygon> = OnceLock::new();
        SHAPE.get_or_init(|| { $make }.normalized())
    }};
}

// (Written out one by one, so the API index sees them: Kotlin's `MaterialShapes.Diamond` reads one.)
impl MaterialShapes {
    pub fn Circle() -> &'static RoundedPolygon {
        shape!(RoundedPolygon::circle(10, 1.0, 0.0, 0.0))
    }

    pub fn Square() -> &'static RoundedPolygon {
        shape!(RoundedPolygon::rectangle(1.0, 1.0, ROUND30, None, 0.0, 0.0))
    }

    pub fn Slanted() -> &'static RoundedPolygon {
        shape!(custom(&[pnr(0.926, 0.970, rs(0.189, 0.811)), pnr(-0.021, 0.967, rs(0.187, 0.057))], 2))
    }

    pub fn Arch() -> &'static RoundedPolygon {
        shape!({
            let rounding = [ROUND100, ROUND100, ROUND20, ROUND20];
            let arch = RoundedPolygon::new(4, 1.0, 0.0, 0.0, CornerRounding::Unrounded, Some(&rounding));
            arch.transformed_by(&rotated(-135.0))
        })
    }

    pub fn Fan() -> &'static RoundedPolygon {
        shape!(custom(
            &[
                pnr(1.004, 1.000, rs(0.148, 0.417)),
                pnr(0.000, 1.000, r(0.151)),
                pnr(0.000, -0.003, r(0.148)),
                pnr(0.978, 0.020, r(0.803)),
            ],
            1,
        ))
    }

    pub fn Arrow() -> &'static RoundedPolygon {
        shape!(custom(
            &[
                pnr(0.500, 0.892, r(0.313)),
                pnr(-0.216, 1.050, r(0.207)),
                pnr(0.499, -0.160, rs(0.215, 1.000)),
                pnr(1.225, 1.060, r(0.211)),
            ],
            1,
        ))
    }

    pub fn SemiCircle() -> &'static RoundedPolygon {
        shape!(RoundedPolygon::rectangle(
            1.6,
            1.0,
            CornerRounding::Unrounded,
            Some(&[ROUND20, ROUND20, ROUND100, ROUND100]),
            0.0,
            0.0,
        ))
    }

    pub fn Oval() -> &'static RoundedPolygon {
        shape!(
            RoundedPolygon::circle(8, 1.0, 0.0, 0.0).transformed_by(&scaled(1.0, 0.64)).transformed_by(&rotated(-45.0))
        )
    }

    pub fn Pill() -> &'static RoundedPolygon {
        shape!(mirrored(&[pnr(0.961, 0.039, r(0.426)), p(1.001, 0.428), pnr(1.000, 0.609, r(1.000))], 2))
    }

    pub fn Triangle() -> &'static RoundedPolygon {
        shape!(RoundedPolygon::new(3, 1.0, 0.0, 0.0, ROUND20, None).transformed_by(&rotated(-90.0)))
    }

    pub fn Diamond() -> &'static RoundedPolygon {
        shape!(custom(&[pnr(0.500, 1.096, rs(0.151, 0.524)), pnr(0.040, 0.500, r(0.159))], 2))
    }

    pub fn ClamShell() -> &'static RoundedPolygon {
        shape!(custom(&[pnr(0.171, 0.841, r(0.159)), pnr(-0.020, 0.500, r(0.140)), pnr(0.170, 0.159, r(0.159))], 2))
    }

    pub fn Pentagon() -> &'static RoundedPolygon {
        shape!(mirrored(&[pnr(0.500, -0.009, r(0.172)), pnr(1.030, 0.365, r(0.164)), pnr(0.828, 0.970, r(0.169))], 1))
    }

    pub fn Gem() -> &'static RoundedPolygon {
        shape!(mirrored(
            &[
                pnr(0.499, 1.023, rs(0.241, 0.778)),
                pnr(-0.005, 0.792, r(0.208)),
                pnr(0.073, 0.258, r(0.228)),
                pnr(0.433, -0.000, r(0.491)),
            ],
            1,
        ))
    }

    pub fn Sunny() -> &'static RoundedPolygon {
        shape!(star(8, 0.8, ROUND15))
    }

    pub fn VerySunny() -> &'static RoundedPolygon {
        shape!(custom(&[pnr(0.500, 1.080, r(0.085)), pnr(0.358, 0.843, r(0.085))], 8))
    }

    pub fn Cookie4Sided() -> &'static RoundedPolygon {
        shape!(custom(&[pnr(1.237, 1.236, r(0.258)), pnr(0.500, 0.918, r(0.233))], 4))
    }

    pub fn Cookie6Sided() -> &'static RoundedPolygon {
        shape!(custom(&[pnr(0.723, 0.884, r(0.394)), pnr(0.500, 1.099, r(0.398))], 6))
    }

    pub fn Cookie7Sided() -> &'static RoundedPolygon {
        shape!(star(7, 0.75, ROUND50).transformed_by(&rotated(-90.0)))
    }

    pub fn Cookie9Sided() -> &'static RoundedPolygon {
        shape!(star(9, 0.8, ROUND50).transformed_by(&rotated(-90.0)))
    }

    pub fn Cookie12Sided() -> &'static RoundedPolygon {
        shape!(star(12, 0.8, ROUND50).transformed_by(&rotated(-90.0)))
    }

    pub fn Ghostish() -> &'static RoundedPolygon {
        shape!(mirrored(
            &[
                pnr(0.500, 0.0, r(1.000)),
                pnr(1.0, 0.0, r(1.000)),
                pnr(1.0, 1.140, rs(0.254, 0.106)),
                pnr(0.575, 0.906, r(0.253)),
            ],
            1,
        ))
    }

    pub fn Clover4Leaf() -> &'static RoundedPolygon {
        shape!(mirrored(&[p(0.500, 0.074), pnr(0.725, -0.099, r(0.476))], 4))
    }

    pub fn Clover8Leaf() -> &'static RoundedPolygon {
        shape!(custom(&[p(0.500, 0.036), pnr(0.758, -0.101, r(0.209))], 8))
    }

    pub fn Burst() -> &'static RoundedPolygon {
        shape!(custom(&[pnr(0.500, -0.006, r(0.006)), pnr(0.592, 0.158, r(0.006))], 12))
    }

    pub fn SoftBurst() -> &'static RoundedPolygon {
        shape!(custom(&[pnr(0.193, 0.277, r(0.053)), pnr(0.176, 0.055, r(0.053))], 10))
    }

    pub fn Boom() -> &'static RoundedPolygon {
        shape!(custom(&[pnr(0.457, 0.296, r(0.007)), pnr(0.500, -0.051, r(0.007))], 15))
    }

    pub fn SoftBoom() -> &'static RoundedPolygon {
        shape!(mirrored(
            &[
                p(0.733, 0.454),
                pnr(0.839, 0.437, r(0.532)),
                pnr(0.949, 0.449, rs(0.439, 1.000)),
                pnr(0.998, 0.478, r(0.174)),
            ],
            16,
        ))
    }

    pub fn Flower() -> &'static RoundedPolygon {
        shape!(mirrored(&[p(0.370, 0.187), pnr(0.416, 0.049, r(0.381)), pnr(0.479, 0.001, r(0.095))], 8))
    }

    pub fn Puffy() -> &'static RoundedPolygon {
        shape!(
            mirrored(
                &[
                    p(0.500, 0.053),
                    pnr(0.545, -0.040, r(0.405)),
                    pnr(0.670, -0.035, r(0.426)),
                    pnr(0.717, 0.066, r(0.574)),
                    p(0.722, 0.128),
                    pnr(0.777, 0.002, r(0.360)),
                    pnr(0.914, 0.149, r(0.660)),
                    pnr(0.926, 0.289, r(0.660)),
                    p(0.881, 0.346),
                    pnr(0.940, 0.344, r(0.126)),
                    pnr(1.003, 0.437, r(0.255)),
                ],
                2,
            )
            .transformed_by(&scaled(1.0, 0.742))
        )
    }

    pub fn PuffyDiamond() -> &'static RoundedPolygon {
        shape!(mirrored(&[pnr(0.870, 0.130, r(0.146)), p(0.818, 0.357), pnr(1.000, 0.332, r(0.853))], 4))
    }

    pub fn PixelCircle() -> &'static RoundedPolygon {
        shape!(mirrored(
            &[
                p(0.500, 0.000),
                p(0.704, 0.000),
                p(0.704, 0.065),
                p(0.843, 0.065),
                p(0.843, 0.148),
                p(0.926, 0.148),
                p(0.926, 0.296),
                p(1.000, 0.296),
            ],
            2,
        ))
    }

    pub fn PixelTriangle() -> &'static RoundedPolygon {
        shape!(mirrored(
            &[
                p(0.110, 0.500),
                p(0.113, 0.000),
                p(0.287, 0.000),
                p(0.287, 0.087),
                p(0.421, 0.087),
                p(0.421, 0.170),
                p(0.560, 0.170),
                p(0.560, 0.265),
                p(0.674, 0.265),
                p(0.675, 0.344),
                p(0.789, 0.344),
                p(0.789, 0.439),
                p(0.888, 0.439),
            ],
            1,
        ))
    }

    pub fn Bun() -> &'static RoundedPolygon {
        shape!(mirrored(
            &[p(0.796, 0.500), pnr(0.853, 0.518, r(1.0)), pnr(0.992, 0.631, r(1.0)), pnr(0.968, 1.000, r(1.0))],
            2,
        ))
    }

    pub fn Heart() -> &'static RoundedPolygon {
        shape!(mirrored(
            &[
                pnr(0.500, 0.268, r(0.016)),
                pnr(0.792, -0.066, r(0.958)),
                pnr(1.064, 0.276, r(1.000)),
                pnr(0.501, 0.946, r(0.129)),
            ],
            1,
        ))
    }

    /// All of them, in Material's order, with their names.
    pub fn all() -> &'static [(&'static str, fn() -> &'static RoundedPolygon)] {
        &[
            ("Circle", MaterialShapes::Circle),
            ("Square", MaterialShapes::Square),
            ("Slanted", MaterialShapes::Slanted),
            ("Arch", MaterialShapes::Arch),
            ("Fan", MaterialShapes::Fan),
            ("Arrow", MaterialShapes::Arrow),
            ("SemiCircle", MaterialShapes::SemiCircle),
            ("Oval", MaterialShapes::Oval),
            ("Pill", MaterialShapes::Pill),
            ("Triangle", MaterialShapes::Triangle),
            ("Diamond", MaterialShapes::Diamond),
            ("ClamShell", MaterialShapes::ClamShell),
            ("Pentagon", MaterialShapes::Pentagon),
            ("Gem", MaterialShapes::Gem),
            ("Sunny", MaterialShapes::Sunny),
            ("VerySunny", MaterialShapes::VerySunny),
            ("Cookie4Sided", MaterialShapes::Cookie4Sided),
            ("Cookie6Sided", MaterialShapes::Cookie6Sided),
            ("Cookie7Sided", MaterialShapes::Cookie7Sided),
            ("Cookie9Sided", MaterialShapes::Cookie9Sided),
            ("Cookie12Sided", MaterialShapes::Cookie12Sided),
            ("Ghostish", MaterialShapes::Ghostish),
            ("Clover4Leaf", MaterialShapes::Clover4Leaf),
            ("Clover8Leaf", MaterialShapes::Clover8Leaf),
            ("Burst", MaterialShapes::Burst),
            ("SoftBurst", MaterialShapes::SoftBurst),
            ("Boom", MaterialShapes::Boom),
            ("SoftBoom", MaterialShapes::SoftBoom),
            ("Flower", MaterialShapes::Flower),
            ("Puffy", MaterialShapes::Puffy),
            ("PuffyDiamond", MaterialShapes::PuffyDiamond),
            ("PixelCircle", MaterialShapes::PixelCircle),
            ("PixelTriangle", MaterialShapes::PixelTriangle),
            ("Bun", MaterialShapes::Bun),
            ("Heart", MaterialShapes::Heart),
        ]
    }
}
