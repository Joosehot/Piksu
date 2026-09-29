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
        let gx = frame.groups.get(&part.group).copied().unwrap_or_default();
        let (bx0, by0, bx1, by1) = part.shade_box.unwrap_or_else(|| part.shape.bbox());
        let (mx, my, hx, hy) = ((bx0 + bx1) / 2.0, (by0 + by1) / 2.0, ((bx1 - bx0) / 2.0).max(1e-4), ((by1 - by0) / 2.0).max(1e-4));
        let mut painted = false;
        let paint = |g: &mut Grid, x: u32, y: u32, q: (f32, f32)| {
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
    if frame.flash {
        for p in g.px.iter_mut().flatten() {
            *p = mix(*p, [1.0, 1.0, 1.0], 0.7);
        }
    }
    g
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
