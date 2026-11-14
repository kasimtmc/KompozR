//! Material Color Utilities' QuantizerWsmeans.

use super::PointProviderLab;
use std::collections::{BTreeMap, HashMap};

/// Weighted k-means in L*a*b* (Material Color Utilities' QuantizerWsmeans): the colors' clusters, starting from
/// `starting_clusters` (Wu's colors), and how many pixels each has.
pub struct QuantizerWsmeans;

impl QuantizerWsmeans {
    const MAX_ITERATIONS: usize = 10;
    /// Points don't move to a nearer cluster within this distance: an L*a*b* ΔE under 3 is not seen.
    const MIN_MOVEMENT_DISTANCE: f64 = 3.0;

    /// Each cluster's color (opaque ARGB, clipped to sRGB) and its pixels, by color.
    pub fn quantize(counts: &HashMap<u32, u32>, starting_clusters: &[u32], max_colors: usize) -> BTreeMap<u32, u32> {
        // A fixed order, whatever the hash map's.
        let mut pixels: Vec<u32> = counts.keys().copied().collect();
        pixels.sort_unstable_by_key(|&p| p & 0xffffff);
        let points: Vec<PointProviderLab> = pixels.iter().map(|&c| PointProviderLab::from_int(c)).collect();
        let weights: Vec<u32> = pixels.iter().map(|c| counts[c]).collect();
        let cluster_count = max_colors.min(points.len()).min(starting_clusters.len());
        if cluster_count == 0 {
            return BTreeMap::new();
        }
        let mut clusters: Vec<PointProviderLab> =
            starting_clusters[..cluster_count].iter().map(|&c| PointProviderLab::from_int(c)).collect();
        // Every point starts in a pseudo-random cluster (Material Color Utilities' seeded random, as a portable
        // generator).
        let mut seed: u32 = 42688;
        let mut cluster_of: Vec<usize> = points
            .iter()
            .map(|_| {
                seed = seed.wrapping_mul(1103515245).wrapping_add(12345);
                ((seed >> 16) % cluster_count as u32) as usize
            })
            .collect();
        let k = cluster_count;
        // Each cluster's distance to every cluster, nearest first.
        let mut nearest = vec![vec![(0.0f64, 0usize); k]; k];
        let mut population = vec![0u32; k];
        for iteration in 0..QuantizerWsmeans::MAX_ITERATIONS {
            for i in 0..k {
                nearest[i][i] = (0.0, i);
                for j in 0..i {
                    let d = clusters[i].distance(&clusters[j]);
                    nearest[j][i] = (d, i);
                    nearest[i][j] = (d, j);
                }
            }
            for row in &mut nearest {
                row.sort_by(|a, b| a.0.total_cmp(&b.0));
            }
            let mut moved = false;
            for (i, point) in points.iter().enumerate() {
                let previous = cluster_of[i];
                let previous_distance = point.distance(&clusters[previous]);
                let mut best = previous_distance;
                let mut next = None;
                for j in 0..k {
                    // As Material Color Utilities: clusters 4× (squared) farther from the old one than the point
                    // skipped.
                    if nearest[previous][j].0 >= 4.0 * previous_distance {
                        continue;
                    }
                    let d = point.distance(&clusters[j]);
                    if d < best {
                        best = d;
                        next = Some(j);
                    }
                }
                if let Some(next) = next
                    && (best.sqrt() - previous_distance.sqrt()).abs() > QuantizerWsmeans::MIN_MOVEMENT_DISTANCE
                {
                    moved = true;
                    cluster_of[i] = next;
                }
            }
            if !moved && iteration != 0 {
                break;
            }
            let mut sums = vec![PointProviderLab::default(); k];
            population.fill(0);
            for (i, point) in points.iter().enumerate() {
                let c = cluster_of[i];
                let w = weights[i];
                population[c] += w;
                sums[c].l += point.l * w as f64;
                sums[c].a += point.a * w as f64;
                sums[c].b += point.b * w as f64;
            }
            for c in 0..k {
                let p = population[c] as f64;
                clusters[c] = if population[c] != 0 {
                    PointProviderLab { l: sums[c].l / p, a: sums[c].a / p, b: sums[c].b / p }
                } else {
                    PointProviderLab::default()
                };
            }
        }
        let mut result = BTreeMap::new();
        for c in 0..k {
            if population[c] != 0 {
                *result.entry(clusters[c].to_int()).or_insert(0) += population[c];
            }
        }
        result
    }
}
