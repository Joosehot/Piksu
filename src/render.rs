//! Rasterise a figure at one frame: every pixel centre is mapped back through the part's
//! transforms and tested against its shape (no anti-aliasing: crisp pixel art). Then shading,
//! the outline pass and the hurt flash.

use crate::anim::{FrameState, Xf};
use crate::color::{adjust, mix, Rgb};
use crate::config::Config;
use crate::figure::{Figure, Show};

#[derive(Clone, Debug, PartialEq)]
pub struct Grid {
    pub size: u32,
    pub px: Vec<Option<Rgb>>,
    /// Which part role painted each pixel (for clipping and checks).
    pub role: Vec<Option<&'static str>>,
}

impl Grid {
    pub fn get(&self, x: i32, y: i32) -> Option<Rgb> {
        if x < 0 || y < 0 || x >= self.size as i32 || y >= self.size as i32 {
            return None;
        }
        self.px[(y as u32 * self.size + x as u32) as usize]
    }
    pub fn filled(&self) -> usize {
        self.px.iter().filter(|p| p.is_some()).count()
    }
}

pub struct Style<'a> {
    pub shading: &'a str,
    pub outline: &'a str,
}

pub fn render(fig: &Figure, frame: &FrameState, size: u32, style: &Style, cfg: &Config) -> Grid {
    let n = (size * size) as usize;
    let mut g = Grid { size, px: vec![None; n], role: vec![None; n] };
    let mut order: Vec<usize> = (0..fig.parts.len()).collect();
    order.sort_by_key(|&i| (fig.parts[i].layer, i));
    let sh = &cfg.shading;
    for i in order {
        let part = &fig.parts[i];
        match (part.show, frame.eyes_closed) {
            (Show::EyesOpen, true) | (Show::EyesClosed, false) => continue,
            _ => {}
        }
        match (part.show, frame.mouth_open) {
            (Show::MouthOpen, false) | (Show::MouthShut, true) => continue,
            _ => {}
        }
        let gx = frame.groups.get(&part.group).copied().unwrap_or_default();
        let (bx0, by0, bx1, by1) = part.shade_box.unwrap_or_else(|| part.shape.bbox());
        let (mx, my, hx, hy) = ((bx0 + bx1) / 2.0, (by0 + by1) / 2.0, ((bx1 - bx0) / 2.0).max(1e-4), ((by1 - by0) / 2.0).max(1e-4));
        let mut painted = false;
        let paint = |g: &mut Grid, x: u32, y: u32, q: (f32, f32)| -> bool {
            let k = (y * size + x) as usize;
            if let Some(c) = part.clip {
                if g.role[k] != Some(c) {
                    return false;
                }
            }
            let mut c = part.color;
            if part.shaded && style.shading != "flat" {
                let (nx, ny) = ((q.0 - mx) / hx, (q.1 - my) / hy);
                let diag = nx + ny;
                match style.shading {
                    "lit" => c = lit(fig, part.color, nx, ny, x, y, size, cfg),
                    "rim" => {
                        let r = (nx * nx + ny * ny).sqrt();
                        if r > sh.rim_band {
                            if diag > 0.0 {
                                c = fig.pal.shade_of(part.color);
                            } else if diag < -0.2 {
                                c = fig.pal.light_of(part.color);
                            }
                        }
                    }
                    _ => {
                        if diag > sh.shade_cut {
                            c = fig.pal.shade_of(part.color);
                        } else if diag < -sh.light_cut {
                            c = fig.pal.light_of(part.color);
                        }
                    }
                }
            }
            g.px[k] = Some(c);
            // a clipped part paints over, but the region keeps its role for later clips
            if part.clip.is_none() {
                g.role[k] = Some(part.role);
            }
            true
        };
        for y in 0..size {
            for x in 0..size {
                let p = ((x as f32 + 0.5) / size as f32, (y as f32 + 0.5) / size as f32);
                let q = through(&frame.whole, &gx, p);
                if part.shape.contains(q.0, q.1) {
                    painted |= paint(&mut g, x, y, q);
                }
            }
        }
        // shapes smaller than a pixel still show up as one
        if !painted && part.min_pixel {
            let (x0, y0, x1, y1) = part.shape.bbox();
            let centre = ((x0 + x1) / 2.0, (y0 + y1) / 2.0);
            let s = forward(&frame.whole, &gx, centre);
            let (x, y) = ((s.0 * size as f32).floor() as i32, (s.1 * size as f32).floor() as i32);
            if x >= 0 && y >= 0 && (x as u32) < size && (y as u32) < size {
                paint(&mut g, x as u32, y as u32, centre);
            }
        }
    }
    outline(&mut g, fig, style.outline);
    if frame.flash > 0.0 {
        for p in g.px.iter_mut().flatten() {
            *p = mix(*p, [1.0, 1.0, 1.0], frame.flash);
        }
    }
    g
}

/// Deterministic noise in -1..1 for a pixel (texture must not change between runs).
fn noise(x: u32, y: u32, seed: u32) -> f32 {
    let mut h = x.wrapping_mul(374_761_393) ^ y.wrapping_mul(668_265_263) ^ seed.wrapping_mul(2_147_483_647);
    h = (h ^ (h >> 13)).wrapping_mul(1_274_126_177);
    h ^= h >> 16;
    (h & 0xffff) as f32 / 32767.5 - 1.0
}

const BAYER: [[f32; 4]; 4] = [[0.0, 8.0, 2.0, 10.0], [12.0, 4.0, 14.0, 6.0], [3.0, 11.0, 1.0, 9.0], [15.0, 7.0, 13.0, 5.0]];

/// Directional light on a rounded surface, in five tones, with material texture.
/// `nx`, `ny` are the pixel's position in the part's own box (-1..1, y down).
#[allow(clippy::too_many_arguments)]
fn lit(fig: &Figure, base: Rgb, nx: f32, ny: f32, x: u32, y: u32, size: u32, cfg: &Config) -> Rgb {
    use crate::figure::Material;
    let l = &cfg.lighting;
    let len = (l.direction[0].powi(2) + l.direction[1].powi(2) + l.direction[2].powi(2)).sqrt();
    let ld = [l.direction[0] / len, l.direction[1] / len, l.direction[2] / len];
    // treat the part as a dome: the normal leans out towards the edge
    let r2 = (nx * nx + ny * ny).min(1.0);
    let n = [nx * 0.9, ny * 0.9, (1.0 - r2 * 0.81).max(0.0).sqrt()];
    let mut i = l.ambient + (1.0 - l.ambient) * (n[0] * ld[0] + n[1] * ld[1] + n[2] * ld[2]).max(0.0);
    let m = &cfg.materials;
    let strength = match fig.realism {
        "realistic" => 1.0,
        "stylized" => 0.5,
        _ => 0.0,
    };
    match fig.material {
        Material::Fur => i += m.fur * strength * noise(x, y / 2, 1),
        Material::Skin => i += m.skin * strength * noise(x / 2, y / 2, 2),
        Material::Cloth => i += m.cloth * strength * noise(x, y / 3, 3),
        Material::Stone => i += m.stone * strength * noise(x, y, 4),
        Material::Gel => i += m.gel_core * strength * (1.0 - r2).max(0.0) + m.gel * strength * noise(x, y, 5),
        Material::Scales => {
            i += m.skin * strength * noise(x / 2, y / 2, 7);
            // a net of scale edges: two diagonal sets of lines, wide enough apart to read
            if size >= m.scales_min_size {
                let k = m.scale_px;
                if (x + y).is_multiple_of(k) || (x + size * k - y).is_multiple_of(k) {
                    i -= m.scales * strength;
                }
            }
        }
        Material::Metal => {
            if nx > m.metal_band[0] && nx < m.metal_band[1] {
                i += 0.25 * strength.max(0.5);
            }
            i += m.metal * strength * noise(x, y, 6);
        }
    }
    // realistic sprites dither across each tone boundary; others cut clean
    if fig.realism == "realistic" {
        i += (BAYER[(y % 4) as usize][(x % 4) as usize] / 16.0 - 0.5) * l.dither;
    }
    let tones = fig.pal.tones_of(base, &cfg.realism);
    let k = l.levels.iter().filter(|c| i > **c).count();
    tones[k]
}

/// Screen point -> part space: undo the whole-sprite transform, then the group's.
fn through(whole: &Xf, group: &Xf, p: (f32, f32)) -> (f32, f32) {
    group.inverse(whole.inverse(p))
}

/// Part space -> screen (for single-pixel parts and the fit step).
pub fn forward(whole: &Xf, group: &Xf, p: (f32, f32)) -> (f32, f32) {
    let apply = |x: &Xf, p: (f32, f32)| {
        let (dx, dy) = ((p.0 - x.pivot.0) * x.sx, (p.1 - x.pivot.1) * x.sy);
        let (s, c) = x.rot.to_radians().sin_cos();
        (dx * c - dy * s + x.pivot.0 + x.dx, dx * s + dy * c + x.pivot.1 + x.dy)
    };
    apply(whole, apply(group, p))
}

fn outline(g: &mut Grid, fig: &Figure, kind: &str) {
    if kind == "none" {
        return;
    }
    let size = g.size as i32;
    let mut add = Vec::new();
    for y in 0..size {
        for x in 0..size {
            if g.get(x, y).is_some() {
                continue;
            }
            let neighbour = [(0, -1), (-1, 0), (1, 0), (0, 1)].iter().find_map(|(dx, dy)| g.get(x + dx, y + dy));
            if let Some(n) = neighbour {
                let c = if kind == "colored" { adjust(n, fig.pal.outline_value, 1.0, 0.0) } else { fig.pal.dark_outline };
                add.push(((y * size + x) as usize, c));
            }
        }
    }
    for (k, c) in add {
        g.px[k] = Some(c);
        g.role[k] = Some("outline");
    }
}
