//! Features from SVG path data and to and from text (androidx.graphics.shapes' SvgPathParser and
//! FeatureSerializer).

use super::cubic::Cubic;
use super::features::{Feature, detect_features};
use super::rounded_polygon::RoundedPolygon;
use super::utils::*;

/// SVG path data as features (Compose's SvgPathParser), for making a polygon from a designer's path once (then
/// kept as [`FeatureSerializer`] text).
pub struct SvgPathParser {
    cubics: Vec<Cubic>,
    start: Point,
    previous_command: Command,
}

impl SvgPathParser {
    /// The features of the first closed shape in `svg_path` (a path's `d`), oriented clockwise (Compose's
    /// parseFeatures). Panics, as Compose throws, for a path it can't read or a shape that isn't a closed polygon.
    pub fn parse_features(svg_path: &str) -> Vec<Feature> {
        let parsed = SvgPathParser::parse_cubics(svg_path);
        let continuous = |first: &Cubic, second: &Cubic| {
            (second.anchor0_x() - first.anchor1_x()).abs() < DISTANCE_EPSILON
                && (second.anchor0_y() - first.anchor1_y()).abs() < DISTANCE_EPSILON
        };
        let mut count = parsed.len();
        for index in 0..parsed.len().saturating_sub(1) {
            if !continuous(&parsed[index], &parsed[index + 1]) {
                count = index;
                break;
            }
        }
        let polygon = RoundedPolygon::from_features(detect_features(&parsed[..count]), None);
        polygon.fixed_orientation().features().to_vec()
    }

    /// Each command of `svg_path` (but its moves) as cubics (Compose's parseCubics).
    pub(crate) fn parse_cubics(svg_path: &str) -> Vec<Cubic> {
        let mut out = Vec::new();
        let mut current = Point::new(0.0, 0.0);
        // each subpath finished before the next, so relative moves start where it ended
        for path in split_before(svg_path, |c| c == 'm' || c == 'M') {
            let commands = split_before(path, |c| c.is_ascii_alphabetic());
            let move_to = Command::parse(commands[0], current);
            current = move_to.start + Point::new(move_to.parameters[0], move_to.parameters[1]);
            let mut parser = SvgPathParser {
                cubics: Vec::new(),
                start: current,
                previous_command: Command {
                    letter: 'I',
                    is_relative: false,
                    parameters: Vec::new(),
                    params_count: 0,
                    start: Point::Zero,
                },
            };
            // the pairs after a move's first are lines
            parser.parse_command(move_to.as_line(current));
            for c in &commands[1..] {
                let command = Command::parse(c, parser.position());
                parser.parse_command(command);
            }
            out.extend(parser.cubics);
        }
        out
    }

    fn position(&self) -> Point {
        self.cubics.last().map_or(self.start, |c| Point::new(c.anchor1_x(), c.anchor1_y()))
    }

    fn reflected_previous_control_point(&self) -> Point {
        let last = self.cubics[self.cubics.len() - 1];
        let position = self.position();
        position + (position - Point::new(last.control1_x(), last.control1_y()))
    }

    fn parse_command(&mut self, command: Command) {
        if command.letter == 'z' {
            let (position, start) = (self.position(), self.start);
            self.cubics.push(Cubic::straight_line(position.x, position.y, start.x, start.y));
            return;
        }
        // one command may hold several: each on its own
        assert!(command.params_count > 0, "Step must be positive, was: 0.");
        let mut i = 0;
        while i < command.parameters.len() {
            let atomic = command.chunk(i, self.position());
            self.parse_atomic_command(atomic);
            i += command.params_count;
        }
    }

    fn parse_atomic_command(&mut self, command: Command) {
        match command.letter {
            'l' | 'h' | 'v' => self.parse_line(&command),
            'c' | 's' | 'q' | 't' => self.parse_curve(&command),
            'a' => self.parse_arc(&command),
            _ => {}
        }
        self.previous_command = command;
    }

    fn line_to(&mut self, end: Point) {
        let position = self.position();
        self.cubics.push(Cubic::straight_line(position.x, position.y, end.x, end.y));
    }

    fn parse_line(&mut self, command: &Command) {
        match command.letter {
            'l' => self.line_to(command.xy(0, 1)),
            'h' => self.line_to(Point::new(command.x(0), command.start.y)),
            'v' => self.line_to(Point::new(command.start.x, command.y(0))),
            _ => {}
        }
    }

    fn curve_to(&mut self, c0: Point, c1: Point, a1: Point) {
        let a0 = self.position();
        self.cubics.push(Cubic::of_points(a0, c0, c1, a1));
    }

    fn parse_curve(&mut self, command: &Command) {
        match command.letter {
            'c' => self.curve_to(command.xy(0, 1), command.xy(2, 3), command.xy(4, 5)),
            's' => {
                let c0 = if matches!(self.previous_command.letter, 'c' | 's') {
                    self.reflected_previous_control_point()
                } else {
                    self.position()
                };
                self.curve_to(c0, command.xy(0, 1), command.xy(2, 3));
            }
            'q' => self.curve_to(command.xy(0, 1), command.xy(0, 1), command.xy(2, 3)),
            't' => {
                let c0 = if matches!(self.previous_command.letter, 'q' | 't') {
                    self.reflected_previous_control_point()
                } else {
                    self.position()
                };
                self.curve_to(c0, c0, command.xy(0, 1));
            }
            _ => {}
        }
    }

    fn parse_arc(&mut self, command: &Command) {
        let target = command.xy(5, 6);
        let position = self.position();
        let p = &command.parameters;
        self.cubics.extend(arc_to_cubics(
            position.x,
            position.y,
            target.x,
            target.y,
            p[0],
            p[1],
            p[2],
            p[3] != 0.0,
            p[4] != 0.0,
        ));
    }
}

/// `s` split before each char `at` matches (that char starting the next piece), blank pieces left out (Kotlin's
/// split on a lookahead).
fn split_before(s: &str, at: impl Fn(char) -> bool) -> Vec<&str> {
    let mut pieces = Vec::new();
    let mut from = 0;
    for (i, c) in s.char_indices() {
        if i > 0 && at(c) {
            pieces.push(&s[from..i]);
            from = i;
        }
    }
    pieces.push(&s[from..]);
    pieces.retain(|p| !p.trim().is_empty());
    pieces
}

/// One SVG command: its letter (lowercase), whether relative, its numbers, how many each one takes, where it is
/// relative to.
#[derive(Clone, Debug)]
struct Command {
    letter: char,
    is_relative: bool,
    parameters: Vec<f32>,
    params_count: usize,
    start: Point,
}

impl Command {
    fn parse(input: &str, current_position: Point) -> Command {
        let letter = input.chars().next().unwrap_or(' ');
        let is_relative = letter.is_lowercase();
        let parameters: Vec<f32> = input[letter.len_utf8()..]
            .split([' ', ','])
            .filter(|s| !s.trim().is_empty())
            .map(|s| java_parse_float(s.trim()))
            .collect();
        let lower = letter.to_ascii_lowercase();
        let params_count = match lower {
            'm' | 'l' | 't' => 2,
            'h' | 'v' => 1,
            'c' => 6,
            's' | 'q' => 4,
            'a' => 7,
            _ => 0,
        };
        Command {
            letter: lower,
            is_relative,
            parameters,
            params_count,
            start: if is_relative { current_position } else { Point::new(0.0, 0.0) },
        }
    }

    fn x(&self, i: usize) -> f32 {
        if self.is_relative { self.start.x + self.parameters[i] } else { self.parameters[i] }
    }
    fn y(&self, i: usize) -> f32 {
        if self.is_relative { self.start.y + self.parameters[i] } else { self.parameters[i] }
    }
    fn xy(&self, i: usize, j: usize) -> Point {
        let coordinates = Point::new(self.parameters[i], self.parameters[j]);
        if self.is_relative { self.start + coordinates } else { coordinates }
    }

    /// Its numbers from `index`, as one command relative to `current_position`.
    fn chunk(&self, index: usize, current_position: Point) -> Command {
        Command {
            letter: self.letter,
            is_relative: self.is_relative,
            parameters: self.parameters[index..index + self.params_count].to_vec(),
            params_count: self.params_count,
            start: current_position,
        }
    }

    /// A move's pairs after its first, as lines.
    fn as_line(&self, new_start: Point) -> Command {
        Command {
            letter: 'l',
            is_relative: self.is_relative,
            parameters: self.parameters.iter().skip(self.params_count).copied().collect(),
            params_count: 2,
            start: new_start,
        }
    }
}

/// An SVG elliptical arc from (x0, y0) to (x1, y1) as cubics (Android's PathParser's, as Compose copies it).
#[allow(clippy::too_many_arguments)]
fn arc_to_cubics(
    x0: f32,
    y0: f32,
    x1: f32,
    y1: f32,
    a: f32,
    b: f32,
    theta: f32,
    is_more_than_half: bool,
    is_positive_arc: bool,
) -> Vec<Cubic> {
    use std::f64::consts::PI;
    // the rotation in radians
    let theta_d: f64 = theta as f64 / 180.0 * PI;
    let cos_theta = theta_d.cos();
    let sin_theta = theta_d.sin();
    // the ends into unit space: rotated back, then scaled back
    let x0p = (x0 as f64 * cos_theta + y0 as f64 * sin_theta) / a as f64;
    let y0p = (-x0 as f64 * sin_theta + y0 as f64 * cos_theta) / b as f64;
    let x1p = (x1 as f64 * cos_theta + y1 as f64 * sin_theta) / a as f64;
    let y1p = (-x1 as f64 * sin_theta + y1 as f64 * cos_theta) / b as f64;
    let dx = x0p - x1p;
    let dy = y0p - y1p;
    let xm = (x0p + x1p) / 2.0;
    let ym = (y0p + y1p) / 2.0;
    // where the unit circles meet
    let dsq = dx * dx + dy * dy;
    if dsq == 0.0 {
        return Vec::new(); // (the same point)
    }
    let disc = 1.0 / dsq - 1.0 / 4.0;
    if disc < 0.0 {
        // (too far apart: the radii grown)
        let adjust = (dsq.sqrt() / 1.99999) as f32;
        return arc_to_cubics(x0, y0, x1, y1, a * adjust, b * adjust, theta, is_more_than_half, is_positive_arc);
    }
    let s = disc.sqrt();
    let sdx = s * dx;
    let sdy = s * dy;
    let (mut cx, mut cy) =
        if is_more_than_half == is_positive_arc { (xm - sdy, ym + sdx) } else { (xm + sdy, ym - sdx) };
    let eta0 = (y0p - cy).atan2(x0p - cx);
    let eta1 = (y1p - cy).atan2(x1p - cx);
    let mut sweep = eta1 - eta0;
    if is_positive_arc != (sweep >= 0.0) {
        if sweep > 0.0 {
            sweep -= 2.0 * PI;
        } else {
            sweep += 2.0 * PI;
        }
    }
    cx *= a as f64;
    cy *= b as f64;
    let tcx = cx;
    cx = cx * cos_theta - cy * sin_theta;
    cy = tcx * sin_theta + cy * cos_theta;
    arc_to_bezier(cx as f32, cy as f32, a, b, x0, y0, theta_d as f32, eta0 as f32, sweep as f32)
}

/// An arc of the ellipse round (cx, cy) of radii rx, ry turned by `theta`, from (e1x, e1y) at angle `start`
/// sweeping `sweep`, as cubics of at most 45 degrees each (spaceroots.org's equations).
#[allow(clippy::too_many_arguments)]
fn arc_to_bezier(
    cx: f32,
    cy: f32,
    rx: f32,
    ry: f32,
    e1x: f32,
    e1y: f32,
    theta: f32,
    start: f32,
    sweep: f32,
) -> Vec<Cubic> {
    let mut cubics = Vec::new();
    let mut ce1x = e1x;
    let mut ce1y = e1y;
    let num_segments = ((sweep * 4.0) as f64 / std::f64::consts::PI).abs().ceil() as i32;
    let mut eta1 = start;
    let cos_t = cos(theta);
    let sin_t = sin(theta);
    let cos_eta1 = cos(eta1);
    let sin_eta1 = sin(eta1);
    let mut ep1x = (-rx * cos_t * sin_eta1) - (ry * sin_t * cos_eta1);
    let mut ep1y = (-rx * sin_t * sin_eta1) + (ry * cos_t * cos_eta1);
    let angle_per_segment = sweep / num_segments as f32;
    for _ in 0..num_segments {
        let eta2 = eta1 + angle_per_segment;
        let sin_eta2 = sin(eta2);
        let cos_eta2 = cos(eta2);
        let e2x = cx + (rx * cos_t * cos_eta2) - (ry * sin_t * sin_eta2);
        let e2y = cy + (rx * sin_t * cos_eta2) + (ry * cos_t * sin_eta2);
        let ep2x = -rx * cos_t * sin_eta2 - ry * sin_t * cos_eta2;
        let ep2y = -rx * sin_t * sin_eta2 + ry * cos_t * cos_eta2;
        let tan_diff2 = tan((eta2 - eta1) / 2.0);
        let alpha = sin(eta2 - eta1) * ((4.0 + (3.0 * tan_diff2 * tan_diff2)).sqrt() - 1.0) / 3.0;
        let q1x = ce1x + alpha * ep1x;
        let q1y = ce1y + alpha * ep1y;
        let q2x = e2x - alpha * ep2x;
        let q2y = e2y - alpha * ep2y;
        cubics.push(Cubic(ce1x, ce1y, q1x, q1y, q2x, q2y, e2x, e2y));
        eta1 = eta2;
        ce1x = e2x;
        ce1y = e2y;
        ep1x = ep2x;
        ep1y = ep2y;
    }
    cubics
}

const SEPARATOR: char = ',';
const CONVEX_CORNER_CHAR: char = 'x';
const CONCAVE_CORNER_CHAR: char = 'o';
const EDGE_CHAR: char = 'n';

/// Features to and from text (Compose's FeatureSerializer, its version 1): "V1", then each feature's tag (`n` an
/// edge, `x` a convex corner, `o` a concave one) and its cubics' numbers, comma separated, each cubic after the
/// first without its first anchor (its predecessor's last).
pub struct FeatureSerializer;

impl FeatureSerializer {
    pub fn serialize(features: &[Feature]) -> String {
        let mut out = String::from("V1");
        for feature in features {
            out.push(if feature.is_edge() {
                EDGE_CHAR
            } else if feature.is_convex_corner() {
                CONVEX_CORNER_CHAR
            } else {
                CONCAVE_CORNER_CHAR
            });
            out.push_str(&serialize_cubics(feature.cubics()));
        }
        out
    }

    /// Features from [`serialize`](FeatureSerializer::serialize)'s text (any version read as version 1, an unknown
    /// tag as an edge). Panics, as Compose throws, for no tags, a feature without enough numbers, or what isn't a
    /// number.
    pub fn parse(serialized_features: &str) -> Vec<Feature> {
        // the version: "V" and digits after any whitespace at the start
        let trimmed = serialized_features.trim_start();
        let mut tags_search_start = 0;
        if let Some(rest) = trimmed.strip_prefix('V') {
            let digits = rest.chars().take_while(|c| c.is_ascii_digit()).count();
            if digits > 0 {
                tags_search_start = serialized_features.len() - rest.len() + digits;
            }
        }
        let bytes = serialized_features.as_bytes();
        let tags: Vec<usize> =
            (tags_search_start..bytes.len()).filter(|&i| (bytes[i] as char).is_ascii_alphabetic()).collect();
        assert!(
            !tags.is_empty(),
            "Could not find any feature tags. Please mark all cubic bezier curve points belonging to a feature with \
             one of {{n, x, o}} for V1, e.g. 'n1,1,2,2,3,3,4,4' for an edge (n) with anchor 0 (1,1), control 0 (2,2), \
             control 1 (3,3) and anchor 1 (4,4)."
        );
        tags.iter()
            .enumerate()
            .map(|(k, &start)| {
                let end = tags.get(k + 1).copied().unwrap_or(serialized_features.len());
                parse_feature(serialized_features, start, end)
            })
            .collect()
    }
}

/// A number as Compose writes it: Kotlin's Float.toString, its trailing zeros and point taken off.
fn remove_trailing_zeroes(v: f32) -> String {
    java_float_string(v).trim_end_matches('0').trim_end_matches('.').to_string()
}

fn serialize_cubics(cubics: &[Cubic]) -> String {
    let mut out = String::new();
    // (the first six numbers of each, each followed by a comma: Kotlin's joinToString with a limit)
    for cubic in cubics {
        for v in &cubic.points()[..6] {
            out.push_str(&remove_trailing_zeroes(*v));
            out.push(SEPARATOR);
        }
    }
    let last = &cubics[cubics.len() - 1];
    out.push_str(&remove_trailing_zeroes(last.anchor1_x()));
    out.push(SEPARATOR);
    out.push_str(&remove_trailing_zeroes(last.anchor1_y()));
    out
}

fn parse_feature(serialized: &str, start: usize, end: usize) -> Feature {
    let cubics = parse_cubics(serialized, start + 1, end);
    match serialized.as_bytes()[start] as char {
        CONVEX_CORNER_CHAR => Feature::corner(cubics, true),
        CONCAVE_CORNER_CHAR => Feature::corner(cubics, false),
        _ => Feature::edge(cubics),
    }
}

/// The numbers from `start` to `end`, eight a cubic, each after the first starting at its predecessor's end.
fn parse_cubics(serialized: &str, start: usize, end: usize) -> Vec<Cubic> {
    const WINDOW_SIZE: usize = 8;
    const WINDOW_STEP: usize = 6;
    let bytes = serialized.as_bytes();
    let mut point_start = start;
    let mut point_end = start;
    let mut point_count = 0;
    let mut points = [0.0f32; WINDOW_SIZE];
    let mut out = Vec::new();
    while point_end < end {
        if bytes[point_end] as char != SEPARATOR {
            point_end += 1;
            continue;
        }
        points[point_count] = java_parse_float(&serialized[point_start..point_end]);
        point_count += 1;
        point_start = point_end + 1;
        if point_count == WINDOW_SIZE {
            out.push(Cubic::from_points(points));
            let (next_x, next_y) = (points[WINDOW_SIZE - 2], points[WINDOW_SIZE - 1]);
            points = [0.0; WINDOW_SIZE];
            points[0] = next_x;
            points[1] = next_y;
            point_count -= WINDOW_STEP;
        }
        point_end += 1;
    }
    if point_count + 1 != WINDOW_SIZE {
        let last = serialized[point_start..point_end].trim();
        let is_number = !last.is_empty() && last.parse::<f32>().is_ok();
        let needed = if is_number { WINDOW_SIZE - (point_count + 1) } else { WINDOW_SIZE - point_count };
        panic!(
            "Received a feature with an insufficient amount of numbers for substring '{}'. Wanted to create {} \
             continuous cubic bezier curves for this feature, but the last one is missing {} more numbers separated \
             by '{}'.",
            &serialized[start.saturating_sub(1)..end],
            out.len() + 1,
            needed,
            SEPARATOR
        );
    }
    points[WINDOW_SIZE - 1] = java_parse_float(&serialized[point_start..point_end]);
    out.push(Cubic::from_points(points));
    out
}
