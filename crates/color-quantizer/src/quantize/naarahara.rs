#![allow(missing_docs)]

//! Octree (Naarahara) quantizer.
//!
//! Builds an octree by inserting pixels one at a time, then reduces the
//! tree to the target number of leaf nodes. Each leaf holds one palette
//! entry (the average of its pixels).

/// A single octree node — uses a flat `Vec` pool indexed by handles.
struct OctreeNode {
    children: [Option<usize>; 8],
    r_sum: u64,
    g_sum: u64,
    b_sum: u64,
    a_sum: u64,
    count: u64,
}

/// Octree node pool.
struct Octree {
    nodes: Vec<OctreeNode>,
}

impl Octree {
    fn new() -> Self {
        Self {
            nodes: vec![OctreeNode {
                children: [None; 8],
                r_sum: 0,
                g_sum: 0,
                b_sum: 0,
                a_sum: 0,
                count: 0,
            }],
        }
    }

    fn insert(&mut self, pixel: &[u8; 4]) {
        let mut node_idx = 0usize;
        // Accumulate at every level along the path.
        for level in 0..=8 {
            let n = &mut self.nodes[node_idx];
            n.r_sum += pixel[0] as u64;
            n.g_sum += pixel[1] as u64;
            n.b_sum += pixel[2] as u64;
            n.a_sum += pixel[3] as u64;
            n.count += 1;

            if level == 8 {
                break;
            }

            let shift = 7 - level;
            let bit_r = ((pixel[0] >> shift) & 1) << 2;
            let bit_g = ((pixel[1] >> shift) & 1) << 1;
            let bit_b = (pixel[2] >> shift) & 1;
            let child_idx = (bit_r | bit_g | bit_b) as usize;

            if self.nodes[node_idx].children[child_idx].is_none() {
                let new_idx = self.nodes.len();
                self.nodes.push(OctreeNode {
                    children: [None; 8],
                    r_sum: 0,
                    g_sum: 0,
                    b_sum: 0,
                    a_sum: 0,
                    count: 0,
                });
                self.nodes[node_idx].children[child_idx] = Some(new_idx);
            }
            node_idx = self.nodes[node_idx].children[child_idx].unwrap();
        }
    }

    /// Collect all leaf nodes that are still "active" (not shadowed by
    /// a merged ancestor). Returns (node_index, node_ref) pairs.
    fn active_leaves(&self) -> Vec<usize> {
        // Use iterative post-order traversal to find leaves.
        let mut leaves = Vec::new();
        let mut stack = vec![(0usize, false)]; // (node, visited_children)
        while let Some((idx, visited)) = stack.pop() {
            if visited {
                let n = &self.nodes[idx];
                let is_leaf = n.children.iter().all(|c| c.is_none());
                if is_leaf && n.count > 0 {
                    leaves.push(idx);
                }
            } else {
                stack.push((idx, true));
                // Push children in reverse order for proper traversal.
                for child in self.nodes[idx].children.iter().rev().flatten() {
                    if self.nodes[*child].count > 0 {
                        stack.push((*child, false));
                    }
                }
            }
        }
        leaves
    }
}

pub fn quantize(pixels: &[[u8; 4]], max_colors: usize) -> Vec<[u8; 4]> {
    if pixels.is_empty() || max_colors == 0 {
        return Vec::new();
    }

    let mut tree = Octree::new();
    for p in pixels {
        tree.insert(p);
    }

    // Collect all leaves and sort by pixel count (descending).
    let mut leaves: Vec<usize> = tree.active_leaves();
    leaves.sort_by(|&a, &b| tree.nodes[b].count.cmp(&tree.nodes[a].count));

    // Keep top max_colors leaves; merge others upward.
    while leaves.len() > max_colors {
        // Merge the smallest leaf into its parent.
        let smallest = leaves.pop().unwrap();
        let parent_idx = tree
            .nodes
            .iter()
            .position(|n| n.children.contains(&Some(smallest)));
        if let Some(pi) = parent_idx {
            // Find the child slot index and clear it.
            for slot in &mut tree.nodes[pi].children {
                if *slot == Some(smallest) {
                    *slot = None;
                }
            }
            // Check if parent became a leaf.
            if tree.nodes[pi].children.iter().all(|c| c.is_none()) && tree.nodes[pi].count > 0 {
                leaves.push(pi);
                // Re-sort if the parent has more pixels than others.
                // Simple approach: re-sort periodically.
                if leaves.len() > max_colors * 2 {
                    leaves.sort_by(|&a, &b| tree.nodes[b].count.cmp(&tree.nodes[a].count));
                }
            }
        }
    }

    // Build palette from the remaining leaves.
    leaves
        .iter()
        .map(|&idx| {
            let n = &tree.nodes[idx];
            let cnt = n.count;
            [
                (n.r_sum / cnt) as u8,
                (n.g_sum / cnt) as u8,
                (n.b_sum / cnt) as u8,
                (n.a_sum / cnt) as u8,
            ]
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_input() {
        assert!(quantize(&[], 8).is_empty());
    }

    #[test]
    fn zero_colors() {
        let pixels = [[255, 0, 0, 255]];
        assert!(quantize(&pixels, 0).is_empty());
    }

    #[test]
    fn single_color() {
        let pixels = vec![[42, 100, 200, 255]; 100];
        let pal = quantize(&pixels, 8);
        assert_eq!(pal.len(), 1, "single colour should produce 1 palette entry");
        assert_eq!(pal[0], [42, 100, 200, 255]);
    }

    #[test]
    fn produces_at_most_max_colors() {
        let pixels: Vec<[u8; 4]> = (0..200u8).map(|i| [i, i, i, 255]).collect();
        let pal = quantize(&pixels, 32);
        assert!(!pal.is_empty());
        assert!(pal.len() <= 32, "got {} colours, expected ≤ 32", pal.len());
    }

    #[test]
    fn larger_max_colors_preserves_more_colours() {
        let pixels: Vec<[u8; 4]> = (0..100u8).map(|i| [i * 2, i, i / 2, 255]).collect();
        let small = quantize(&pixels, 8);
        let large = quantize(&pixels, 32);
        assert!(large.len() >= small.len());
    }
}
