//! Material Color Utilities' TemperatureCache: colors as warm or cool, their analogous colors and complement.

use crate::material_color::hct::Hct;
use crate::material_color::utils::color_utils::lab_from_argb;
use crate::material_color::utils::math_utils::{
    round, sanitize_degrees_double, sanitize_degrees_int, to_degrees, to_radians,
};

/// How warm a color is next to the others of its chroma and tone in every hue (Material Color Utilities'
/// TemperatureCache): the analogous colors (neighbors by temperature, not merely by hue) and the complement (the
/// color as far on the other side of warm and cool), as Content and Fidelity schemes take their tertiary.
pub struct TemperatureCache {
    input: Hct,
    /// The input's chroma and tone at every hue 0..=360.
    hcts_by_hue: Vec<Hct>,
    /// Those and the input, coldest first, each with its raw temperature.
    hcts_by_temp: Vec<(Hct, f64)>,
    input_temperature: f64,
}

impl TemperatureCache {
    pub fn new(input: Hct) -> TemperatureCache {
        let hcts_by_hue: Vec<Hct> = (0..=360).map(|hue| Hct::from(hue as f64, input.chroma(), input.tone())).collect();
        let mut hcts_by_temp: Vec<(Hct, f64)> = hcts_by_hue
            .iter()
            .chain(std::iter::once(&input))
            .map(|&h| (h, TemperatureCache::raw_temperature(&h)))
            .collect();
        hcts_by_temp.sort_by(|a, b| a.1.total_cmp(&b.1));
        TemperatureCache {
            input_temperature: TemperatureCache::raw_temperature(&input),
            input,
            hcts_by_hue,
            hcts_by_temp,
        }
    }

    /// The coolest of the input's chroma and tone.
    pub fn coldest(&self) -> Hct {
        self.hcts_by_temp[0].0
    }

    /// The warmest of the input's chroma and tone.
    pub fn warmest(&self) -> Hct {
        self.hcts_by_temp[self.hcts_by_temp.len() - 1].0
    }

    /// 5 colors of 12 divisions of the temperature wheel, the input among them.
    pub fn analogous_colors(&self) -> Vec<Hct> {
        self.analogous_colors_with(5, 12)
    }

    /// `count` colors, the input in the middle, its neighbors in `divisions` equal steps of temperature around
    /// the hues.
    pub fn analogous_colors_with(&self, count: usize, divisions: usize) -> Vec<Hct> {
        // The starting hue is the hue of the input color.
        let start_hue = round(self.input.hue()) as i32;
        let start_hct = self.hcts_by_hue[start_hue as usize];
        let mut last_temp = self.relative_temperature(&start_hct);
        let mut all_colors = vec![start_hct];

        let mut absolute_total_temp_delta = 0.0;
        for i in 0..360 {
            let hue = sanitize_degrees_int(start_hue + i);
            let temp = self.relative_temperature(&self.hcts_by_hue[hue as usize]);
            absolute_total_temp_delta += (temp - last_temp).abs();
            last_temp = temp;
        }

        let mut hue_addend = 1;
        let temp_step = absolute_total_temp_delta / divisions as f64;
        let mut total_temp_delta = 0.0;
        last_temp = self.relative_temperature(&start_hct);
        while all_colors.len() < divisions {
            let hue = sanitize_degrees_int(start_hue + hue_addend);
            let hct = self.hcts_by_hue[hue as usize];
            let temp = self.relative_temperature(&hct);
            total_temp_delta += (temp - last_temp).abs();

            let mut desired_total_temp_delta_for_index = all_colors.len() as f64 * temp_step;
            let mut index_satisfied = total_temp_delta >= desired_total_temp_delta_for_index;
            let mut index_addend = 1;
            // Keep adding this hue to the answers until its temperature is insufficient. This ensures consistent
            // behavior when there aren't `divisions` discrete steps between 0 and 360 in hue with `temp_step`
            // delta in temperature between them. For example, white and black have no analogues: double counting
            // them gives the right count.
            while index_satisfied && all_colors.len() < divisions {
                all_colors.push(hct);
                desired_total_temp_delta_for_index = (all_colors.len() + index_addend) as f64 * temp_step;
                index_satisfied = total_temp_delta >= desired_total_temp_delta_for_index;
                index_addend += 1;
            }
            last_temp = temp;
            hue_addend += 1;
            if hue_addend > 360 {
                while all_colors.len() < divisions {
                    all_colors.push(hct);
                }
                break;
            }
        }

        let mut answers = vec![self.input];
        let len = all_colors.len() as i32;
        let wrap = |index: i32| {
            let mut index = index;
            while index < 0 {
                index += len;
            }
            if index >= len {
                index %= len;
            }
            index as usize
        };
        // First, generate analogues from rotating counter-clockwise.
        let ccw_count = ((count as f64 - 1.0) / 2.0).floor() as i32;
        for i in 1..ccw_count + 1 {
            answers.insert(0, all_colors[wrap(-i)]);
        }
        // Second, generate analogues from rotating clockwise.
        let cw_count = count as i32 - ccw_count - 1;
        for i in 1..cw_count + 1 {
            answers.push(all_colors[wrap(i)]);
        }
        answers
    }

    /// The color as far on the other side of warm and cool as the input is on its side (its chroma and tone).
    pub fn complement(&self) -> Hct {
        let (coldest, warmest) = (self.hcts_by_temp[0], self.hcts_by_temp[self.hcts_by_temp.len() - 1]);
        let (coldest_hue, coldest_temp) = (coldest.0.hue(), coldest.1);
        let (warmest_hue, warmest_temp) = (warmest.0.hue(), warmest.1);
        let range = warmest_temp - coldest_temp;
        let start_hue_is_coldest_to_warmest = TemperatureCache::is_between(self.input.hue(), coldest_hue, warmest_hue);
        let start_hue = if start_hue_is_coldest_to_warmest { warmest_hue } else { coldest_hue };
        let end_hue = if start_hue_is_coldest_to_warmest { coldest_hue } else { warmest_hue };
        let direction_of_rotation = 1.0;
        let mut smallest_error = 1000.0;
        let mut answer = self.hcts_by_hue[round(self.input.hue()) as usize];
        let complement_relative_temp = 1.0 - self.input_relative_temperature();
        // Find the color in the other section, closest to the inverse percentile of the input color. This is the
        // complement.
        let mut hue_addend = 0.0;
        while hue_addend <= 360.0 {
            let hue = sanitize_degrees_double(start_hue + direction_of_rotation * hue_addend);
            hue_addend += 1.0;
            if !TemperatureCache::is_between(hue, start_hue, end_hue) {
                continue;
            }
            let possible_answer = self.hcts_by_hue[round(hue) as usize];
            let relative_temp = (TemperatureCache::raw_temperature(&possible_answer) - coldest_temp) / range;
            let error = (complement_relative_temp - relative_temp).abs();
            if error < smallest_error {
                smallest_error = error;
                answer = possible_answer;
            }
        }
        answer
    }

    /// Where a color's temperature is between the coldest and the warmest of the input's chroma and tone, 0..1
    /// (0.5 where they are as warm: at tone 100 there is only white).
    pub fn relative_temperature(&self, hct: &Hct) -> f64 {
        let coldest = self.hcts_by_temp[0].1;
        let range = self.hcts_by_temp[self.hcts_by_temp.len() - 1].1 - coldest;
        let difference_from_coldest = TemperatureCache::raw_temperature(hct) - coldest;
        if range == 0.0 {
            return 0.5;
        }
        difference_from_coldest / range
    }

    fn input_relative_temperature(&self) -> f64 {
        let coldest = self.hcts_by_temp[0].1;
        let range = self.hcts_by_temp[self.hcts_by_temp.len() - 1].1 - coldest;
        if range == 0.0 { 0.5 } else { (self.input_temperature - coldest) / range }
    }

    /// Whether `angle` is between `a` and `b` going up from `a` (both in).
    pub fn is_between(angle: f64, a: f64, b: f64) -> bool {
        if a < b { a <= angle && angle <= b } else { a <= angle || angle <= b }
    }

    /// A color's temperature, from about -0.5 (cool) to 3 (warm): its L*a*b* chroma and hue as Ou, Woodcock and
    /// Wright measured it, turned 50° (warmest at a hue of 50, the oranges).
    pub fn raw_temperature(color: &Hct) -> f64 {
        let lab = lab_from_argb(color.to_int());
        let hue = sanitize_degrees_double(to_degrees(lab[2].atan2(lab[1])));
        let chroma = lab[1].hypot(lab[2]);
        -0.5 + 0.02 * chroma.powf(1.07) * to_radians(sanitize_degrees_double(hue - 50.0)).cos()
    }
}
