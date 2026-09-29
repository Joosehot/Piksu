//! Checks a finalist must pass before anything is written: the sprite fits the canvas with room
//! for its outline, covers a readable share of it, keeps contrast, moves smoothly between
//! frames (including last -> first), has no stray pixels, and is symmetric when it should be.

use crate::color::luminance;
use crate::config::Config;
use crate::figure::Figure;
use crate::render::Grid;

fn centroid(g: &Grid) -> (f32, f32) {
    let (mut x, mut y, mut n) = (0.0, 0.0, 0.0);
    for yy in 0..g.size as i32 {
        for xx in 0..g.size as i32 {
            if g.get(xx, yy).is_some() {
                x += xx as f32;
                y += yy as f32;
                n += 1.0;
            }
        }
    }
    if n == 0.0 { (0.0, 0.0) } else { (x / n, y / n) }
}

/// Pixels whose filled/empty state differs, at the best small shift of `b` onto `a`.
fn shape_change(a: &Grid, b: &Grid) -> usize {
    let s = a.size as i32;
    let mut best = usize::MAX;
    for dy in -3..=3 {
        for dx in -3..=3 {
            let mut d = 0;
            for y in 0..s {
                for x in 0..s {
                    if a.get(x, y).is_some() != b.get(x + dx, y + dy).is_some() {
                        d += 1;
                    }
                }
            }
            best = best.min(d);
        }
    }
    best
}

/// All animations' frames, in order.
pub fn run(fig: &Figure, anims: &[(String, Vec<Grid>)], chosen_eyes: Option<&str>, outline: &str, cfg: &Config) -> Result<Vec<String>, String> {
    let c = &cfg.checks;
    let mut passed = Vec::new();
    for (name, frames) in anims {
        for (i, g) in frames.iter().enumerate() {
            let s = g.size as i32;
            for k in 0..s {
                for (x, y) in [(k, 0), (k, s - 1), (0, k), (s - 1, k)] {
                    if g.get(x, y).is_some() {
                        return Err(format!("{name} frame {i}: the sprite touches the edge of the {s}px canvas"));
                    }
                }
            }
            if g.filled() == 0 {
                return Err(format!("{name} frame {i} is empty"));
            }
            for y in 0..s {
                for x in 0..s {
                    if g.get(x, y).is_some() {
                        let alone = (-1..=1).all(|dy| (-1..=1).all(|dx| (dx == 0 && dy == 0) || g.get(x + dx, y + dy).is_none()));
                        if alone {
                            return Err(format!("{name} frame {i}: a stray pixel at {x},{y}"));
                        }
                    }
                }
            }
        }
        // smooth motion between neighbouring frames (and last -> first): the shape changes little
        // once the move is taken out, and the move itself is small
        let n = frames.len();
        if n > 1 {
            let limit = cfg.animations.get(name.as_str()).and_then(|a| a.params.get("max_change")).copied().unwrap_or(c.max_change);
            for i in 0..n {
                let (a, b) = (&frames[i], &frames[(i + 1) % n]);
                let base = a.filled().max(b.filled()).max(1);
                let changed = shape_change(a, b);
                if changed as f32 / base as f32 > limit {
                    return Err(format!("{name}: frame {i} -> {} reshapes {changed} of {base} pixels (jumpy)", (i + 1) % n));
                }
                let (ca, cb) = (centroid(a), centroid(b));
                let step = ((ca.0 - cb.0).powi(2) + (ca.1 - cb.1).powi(2)).sqrt() / a.size as f32;
                if step > c.max_step {
                    return Err(format!("{name}: frame {i} -> {} jumps {:.0}% of the canvas", (i + 1) % n, step * 100.0));
                }
            }
        }
    }
    passed.push("fits the canvas with room for the outline, no stray pixels".to_string());
    passed.push("motion between frames stays smooth".to_string());

    let first = &anims[0].1[0];
    let fill = first.filled() as f32 / (first.size * first.size) as f32;
    if fill < c.min_fill || fill > c.max_fill {
        return Err(format!("the silhouette covers {:.0}% of the canvas (readable is {:.0}-{:.0}%)", fill * 100.0, c.min_fill * 100.0, c.max_fill * 100.0));
    }
    passed.push(format!("covers {:.0}% of the canvas", fill * 100.0));

    let main = luminance(fig.pal.main);
    if outline != "none" {
        let o = if outline == "colored" { crate::color::adjust(fig.pal.main, fig.pal.outline_value, 1.0, 0.0) } else { fig.pal.dark_outline };
        let d = (main - luminance(o)).abs();
        if d < c.min_contrast {
            return Err(format!("the outline is too close to the body colour (contrast {d:.2})"));
        }
        passed.push(format!("outline contrast {d:.2}"));
    }
    if let Some(e) = chosen_eyes {
        let dark = (main - luminance(fig.pal.eye_dark)).abs();
        let white = (main - luminance(fig.pal.eye_white)).abs();
        let d = if e == "dot" || e == "visor" { dark } else { dark.max(white) };
        if d < c.min_contrast {
            return Err(format!("{e} eyes don't stand out on this body colour (contrast {d:.2})"));
        }
        passed.push(format!("eye contrast {d:.2}"));
    }
    if fig.symmetric {
        let s = first.size as i32;
        let mut diff = 0;
        for y in 0..s {
            for x in 0..s / 2 {
                if first.get(x, y).is_some() != first.get(s - 1 - x, y).is_some() {
                    diff += 1;
                }
            }
        }
        let r = diff as f32 / first.filled().max(1) as f32;
        if r > c.symmetry_tolerance {
            return Err(format!("the first frame is lopsided ({:.0}% of pixels unmatched)", r * 100.0));
        }
        passed.push("symmetric".into());
    }
    Ok(passed)
}
