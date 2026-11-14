//! Material Color Utilities' Score: which of a picture's colors make good theme colors.

use crate::material_color::hct::Hct;
use crate::material_color::utils::math_utils::{difference_degrees, round, sanitize_degrees_int};

/// Ranks a picture's colors as theme (source) colors (Material Color Utilities' Score): by chroma (48 is best)
/// and by how much of the picture is near its hue, the greyish (chroma under 5) and the rare (1% or less of the
/// picture around their hue) left out, then picked as far apart in hue as `desired` of them can be.
pub struct Score;

impl Score {
    const TARGET_CHROMA: f64 = 48.0; // A1 Chroma
    const WEIGHT_PROPORTION: f64 = 0.7;
    const WEIGHT_CHROMA_ABOVE: f64 = 0.3;
    const WEIGHT_CHROMA_BELOW: f64 = 0.1;
    const CUTOFF_CHROMA: f64 = 5.0;
    const CUTOFF_EXCITED_PROPORTION: f64 = 0.01;

    /// Google blue: what [`Score::score`] gives when no color is fit.
    pub const FALLBACK_COLOR_ARGB: u32 = 0xff4285f4;

    /// The best 4 colors (ARGB) of `colors_to_population` (each color and how many pixels have it), best first;
    /// Google blue alone when none is fit.
    pub fn score(colors_to_population: &[(u32, u32)]) -> Vec<u32> {
        Score::score_with(colors_to_population, 4, Score::FALLBACK_COLOR_ARGB, true)
    }

    /// Up to `desired`, `fallback_color_argb` when none is fit; `filter` false keeps the greyish and the rare.
    pub fn score_with(
        colors_to_population: &[(u32, u32)],
        desired: usize,
        fallback_color_argb: u32,
        filter: bool,
    ) -> Vec<u32> {
        // The HCT color of each ARGB value, and the per hue count and total count.
        let mut colors_hct = Vec::new();
        let mut hue_population = [0i64; 360];
        let mut population_sum = 0.0;
        for &(argb, population) in colors_to_population {
            let hct = Hct::from_int(argb);
            colors_hct.push(hct);
            let hue = hct.hue().floor() as usize;
            hue_population[hue] += population as i64;
            population_sum += population as f64;
        }

        // Hues with more usage in neighboring 30 degree slice get a larger number.
        let mut hue_excited_proportions = [0.0f64; 360];
        for hue in 0i32..360 {
            let proportion = hue_population[hue as usize] as f64 / population_sum;
            for i in hue - 14..hue + 16 {
                hue_excited_proportions[sanitize_degrees_int(i) as usize] += proportion;
            }
        }

        // Scores each color by usage and chroma, leaving out those without enough of either.
        let mut scored = Vec::new();
        for hct in colors_hct {
            let hue = sanitize_degrees_int(round(hct.hue()) as i32);
            let proportion = hue_excited_proportions[hue as usize];
            if filter && (hct.chroma() < Score::CUTOFF_CHROMA || proportion <= Score::CUTOFF_EXCITED_PROPORTION) {
                continue;
            }
            let proportion_score = proportion * 100.0 * Score::WEIGHT_PROPORTION;
            let chroma_weight = if hct.chroma() < Score::TARGET_CHROMA {
                Score::WEIGHT_CHROMA_BELOW
            } else {
                Score::WEIGHT_CHROMA_ABOVE
            };
            let chroma_score = (hct.chroma() - Score::TARGET_CHROMA) * chroma_weight;
            scored.push((hct, proportion_score + chroma_score));
        }
        // Higher scores first.
        scored.sort_by(|a, b| b.1.total_cmp(&a.1));

        // As far apart in hue as `desired` of them can be: 90° apart first, down to 15°.
        let mut chosen: Vec<Hct> = Vec::new();
        for apart in (15..=90).rev() {
            chosen.clear();
            for (hct, _) in &scored {
                if !chosen.iter().any(|c| difference_degrees(hct.hue(), c.hue()) < apart as f64) {
                    chosen.push(*hct);
                }
                if chosen.len() >= desired {
                    break;
                }
            }
            if chosen.len() >= desired {
                break;
            }
        }
        if chosen.is_empty() {
            return vec![fallback_color_argb];
        }
        chosen.iter().map(Hct::to_int).collect()
    }
}
