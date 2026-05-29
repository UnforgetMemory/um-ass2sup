use crate::types::Rgba;

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
    fn from_pixels(pixels: &[Rgba]) -> Self {
        let mut bx = BoundingBox {
            r_min: 255, r_max: 0,
            g_min: 255, g_max: 0,
            b_min: 255, b_max: 0,
            a_min: 255, a_max: 0,
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
        bx
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
        let bb = BoundingBox::from_pixels(&cube.pixels);
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

pub fn find_nearest_index(color: &Rgba, palette: &[Rgba]) -> u8 {
    palette
        .iter()
        .enumerate()
        .min_by_key(|(_, p)| color.distance_sq(p))
        .map(|(i, _)| i as u8)
        .unwrap_or(0)
}
