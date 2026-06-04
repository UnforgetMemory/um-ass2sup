use crate::types::Rgba;
use std::cell::RefCell;

#[derive(Debug)]
struct BoundingBox {
    r_min: u8,
    r_max: u8,
    g_min: u8,
    g_max: u8,
    b_min: u8,
    b_max: u8,
    a_min: u8,
    a_max: u8,
}

impl BoundingBox {
    fn from_pixels(pixels: &[Rgba]) -> Option<Self> {
        if pixels.is_empty() {
            return None;
        }
        let mut bx = BoundingBox {
            r_min: 255,
            r_max: 0,
            g_min: 255,
            g_max: 0,
            b_min: 255,
            b_max: 0,
            a_min: 255,
            a_max: 0,
        };
        for p in pixels {
            bx.r_min = bx.r_min.min(p.r);
            bx.r_max = bx.r_max.max(p.r);
            bx.g_min = bx.g_min.min(p.g);
            bx.g_max = bx.g_max.max(p.g);
            bx.b_min = bx.b_min.min(p.b);
            bx.b_max = bx.b_max.max(p.b);
            bx.a_min = bx.a_min.min(p.a);
            bx.a_max = bx.a_max.max(p.a);
        }
        Some(bx)
    }

    fn longest_axis(&self) -> usize {
        let r_range = self.r_max as u32 - self.r_min as u32;
        let g_range = self.g_max as u32 - self.g_min as u32;
        let b_range = self.b_max as u32 - self.b_min as u32;
        let a_range = self.a_max as u32 - self.a_min as u32;

        let ranges = [r_range, g_range, b_range, a_range];
        ranges
            .iter()
            .enumerate()
            .max_by_key(|(_, v)| *v)
            .map(|(i, _)| i)
            .unwrap_or(0)
    }
}

#[derive(Debug)]
struct Cube {
    pixels: Vec<Rgba>,
}

impl Cube {
    fn average(&self) -> Rgba {
        if self.pixels.is_empty() {
            return Rgba::new(0, 0, 0, 0);
        }
        let (mut r, mut g, mut b, mut a) = (0u64, 0u64, 0u64, 0u64);
        for p in &self.pixels {
            r += p.r as u64;
            g += p.g as u64;
            b += p.b as u64;
            a += p.a as u64;
        }
        let n = self.pixels.len() as u64;
        Rgba::new((r / n) as u8, (g / n) as u8, (b / n) as u8, (a / n) as u8)
    }
}

/// Reduces a slice of RGBA pixels to a palette of at most `max_colors` entries
/// using the median-cut algorithm.
///
/// Recursively splits the bounding box with the longest colour-channel range
/// at the median until the desired number of cubes is reached, then averages
/// each cube to produce the final palette. Returns an empty `Vec` if
/// `pixels` is empty or `max_colors` is zero.
pub fn median_cut(pixels: &[Rgba], max_colors: usize) -> Vec<Rgba> {
    if pixels.is_empty() || max_colors == 0 {
        return Vec::new();
    }

    let mut cubes = vec![Cube {
        pixels: pixels.to_vec(),
    }];

    while cubes.len() < max_colors {
        let idx = match cubes
            .iter()
            .enumerate()
            .max_by_key(|(_, c)| c.pixels.len())
            .map(|(i, _)| i)
        {
            Some(i) if cubes[i].pixels.len() > 1 => i,
            _ => break,
        };

        let mut cube = cubes.swap_remove(idx);
        let bb = match BoundingBox::from_pixels(&cube.pixels) {
            Some(bb) => bb,
            None => {
                cubes.push(cube);
                break;
            }
        };
        let axis = bb.longest_axis();

        cube.pixels.sort_by(|a, b| {
            let va = match axis {
                0 => a.r,
                1 => a.g,
                2 => a.b,
                _ => a.a,
            };
            let vb = match axis {
                0 => b.r,
                1 => b.g,
                2 => b.b,
                _ => b.a,
            };
            va.cmp(&vb)
        });

        let mid = cube.pixels.len() / 2;
        let (left, right) = cube.pixels.split_at(mid);
        cubes.push(Cube {
            pixels: left.to_vec(),
        });
        cubes.push(Cube {
            pixels: right.to_vec(),
        });
    }

    cubes.iter().map(|c| c.average()).collect()
}

// ---------------------------------------------------------------------------
// k-d tree accelerator for find_nearest_index
// ---------------------------------------------------------------------------

/// Minimum palette size before k-d tree is used instead of linear scan.
/// Below this threshold, linear scan is faster and guarantees tie-breaking parity.
const LINEAR_THRESHOLD: usize = 32;

/// Maximum palette indices in a k-d tree leaf before forcing another split.
const LEAF_SIZE: usize = 8;

/// A k-d tree node for nearest-neighbor color search.
enum KdNode {
    /// Leaf node containing a small batch of palette indices.
    Leaf(Vec<usize>),
    /// Internal node splitting on one color axis at a threshold value.
    Split {
        axis: usize,
        threshold: u8,
        left: Box<KdNode>,
        right: Box<KdNode>,
    },
}

/// Thread-local cache entry: remembers the k-d tree for a palette slice
/// so that the O(n log n) build cost is paid once per quantize() call.
struct KdCacheEntry {
    palette_ptr: usize,
    palette_len: usize,
    tree: KdNode,
}

thread_local! {
    static KD_TREE_CACHE: RefCell<Option<KdCacheEntry>> = const { RefCell::new(None) };
}

/// Extract a color channel value by axis index (0=R, 1=G, 2=B, 3=A).
#[inline(always)]
fn channel(c: &Rgba, axis: usize) -> u8 {
    match axis {
        0 => c.r,
        1 => c.g,
        2 => c.b,
        _ => c.a,
    }
}

/// Find which color axis has the largest spread across the given palette indices.
fn longest_axis_of_indices(indices: &[usize], palette: &[Rgba]) -> usize {
    let (mut min_r, mut max_r) = (255u8, 0u8);
    let (mut min_g, mut max_g) = (255u8, 0u8);
    let (mut min_b, mut max_b) = (255u8, 0u8);
    let (mut min_a, mut max_a) = (255u8, 0u8);

    for &i in indices {
        let p = &palette[i];
        min_r = min_r.min(p.r);
        max_r = max_r.max(p.r);
        min_g = min_g.min(p.g);
        max_g = max_g.max(p.g);
        min_b = min_b.min(p.b);
        max_b = max_b.max(p.b);
        min_a = min_a.min(p.a);
        max_a = max_a.max(p.a);
    }

    let ranges = [
        (max_r - min_r) as u32,
        (max_g - min_g) as u32,
        (max_b - min_b) as u32,
        (max_a - min_a) as u32,
    ];

    ranges
        .iter()
        .enumerate()
        .max_by_key(|(_, v)| *v)
        .map(|(i, _)| i)
        .unwrap_or(0)
}

/// Build a k-d tree from palette indices, recursively splitting on the
/// longest axis at the median.
fn build_kdtree(indices: &[usize], palette: &[Rgba]) -> KdNode {
    if indices.len() <= LEAF_SIZE {
        return KdNode::Leaf(indices.to_vec());
    }

    let axis = longest_axis_of_indices(indices, palette);

    let mut sorted = indices.to_vec();
    sorted.sort_by(|&a, &b| channel(&palette[a], axis).cmp(&channel(&palette[b], axis)));

    let mid = sorted.len() / 2;
    let threshold = channel(&palette[sorted[mid]], axis);

    // Find the true split point: first index with value >= threshold.
    // This ensures left child contains ONLY entries < threshold.
    let split = sorted.partition_point(|&i| channel(&palette[i], axis) < threshold);

    // If we can't split (all values are the same / one distinct value), make leaf.
    if split == 0 || split >= sorted.len() {
        return KdNode::Leaf(sorted);
    }

    KdNode::Split {
        axis,
        threshold,
        left: Box::new(build_kdtree(&sorted[..split], palette)),
        right: Box::new(build_kdtree(&sorted[split..], palette)),
    }
}

impl KdNode {
    /// Search for the palette index nearest to `color`, updating `best` in place.
    ///
    /// Tie-breaking: when distances are equal, the lower index is preferred,
    /// matching the stable behavior of `min_by_key`.
    fn nearest(&self, color: &Rgba, palette: &[Rgba], best: &mut (usize, u32)) {
        match self {
            KdNode::Leaf(indices) => {
                for &i in indices {
                    let d = color.distance_sq(&palette[i]);
                    if d < best.1 || (d == best.1 && i < best.0) {
                        *best = (i, d);
                    }
                }
            }
            KdNode::Split {
                axis,
                threshold,
                left,
                right,
            } => {
                let val = channel(color, *axis);
                // Descend the near side first
                let (near, far) = if val < *threshold {
                    (left.as_ref(), right.as_ref())
                } else {
                    (right.as_ref(), left.as_ref())
                };
                near.nearest(color, palette, best);

                // Only search the far side if the plane distance might yield
                // a better match (branch-and-bound pruning).
                let diff = val as i32 - *threshold as i32;
                let plane_dist = (diff * diff) as u32;
                if plane_dist < best.1 {
                    far.nearest(color, palette, best);
                }
            }
        }
    }
}

/// Linear scan fallback — exact match for the original implementation.
#[inline(always)]
fn find_nearest_index_linear(color: &Rgba, palette: &[Rgba]) -> u8 {
    palette
        .iter()
        .enumerate()
        .min_by_key(|(_, p)| color.distance_sq(p))
        .map(|(i, _)| i as u8)
        .unwrap_or(0)
}

/// Find the palette index closest to `color`.
///
/// For small palettes (< 32 entries) uses linear scan (O(n) per query).
/// For larger palettes uses a k-d tree with branch-and-bound search
/// (O(log n) per query). The k-d tree is cached per palette slice pointer
/// so that the O(n log n) build cost is paid once per quantize() call.
///
/// Tie-breaking: when two palette entries are equidistant, the lower index
/// is preferred (matching the stable behavior of `min_by_key`).
#[inline]
pub fn find_nearest_index(color: &Rgba, palette: &[Rgba]) -> u8 {
    // Empty or tiny palette → linear scan guarantees parity
    if palette.len() < LINEAR_THRESHOLD {
        return find_nearest_index_linear(color, palette);
    }

    KD_TREE_CACHE.with_borrow_mut(|cache| {
        let ptr = palette.as_ptr() as usize;
        let len = palette.len();

        // Rebuild the k-d tree if the cache doesn't match the current palette.
        let needs_rebuild = match cache {
            Some(entry) => entry.palette_ptr != ptr || entry.palette_len != len,
            None => true,
        };

        if needs_rebuild {
            let indices: Vec<usize> = (0..len).collect();
            let tree = build_kdtree(&indices, palette);
            *cache = Some(KdCacheEntry {
                palette_ptr: ptr,
                palette_len: len,
                tree,
            });
        }

        // Safety: cache was just populated if it was None or stale.
        let entry = cache.as_ref().unwrap();
        let mut best = (0usize, color.distance_sq(&palette[0]));
        entry.tree.nearest(color, palette, &mut best);
        best.0 as u8
    })
}
