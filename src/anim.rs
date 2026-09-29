//! Animation: each frame is a set of transforms (move, squash, turn) for the whole sprite and
//! for its groups (wings, legs, tail), plus blink and flash flags. Frames are phases of one loop,
//! so the last frame always leads back into the first.

use crate::config::Config;
use crate::figure::{Figure, Group};
use crate::request::Anim;
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Xf {
    pub dx: f32,
    pub dy: f32,
    pub sx: f32,
    pub sy: f32,
    /// degrees, clockwise on screen
    pub rot: f32,
    pub pivot: (f32, f32),
}

impl Default for Xf {
    fn default() -> Self {
        Xf { dx: 0.0, dy: 0.0, sx: 1.0, sy: 1.0, rot: 0.0, pivot: (0.5, 0.5) }
    }
}

impl Xf {
    /// Screen point -> the point in the part's own space that lands there.
    pub fn inverse(&self, p: (f32, f32)) -> (f32, f32) {
        let (x, y) = (p.0 - self.dx - self.pivot.0, p.1 - self.dy - self.pivot.1);
        let (s, c) = (-self.rot.to_radians()).sin_cos();
        let (rx, ry) = (x * c - y * s, x * s + y * c);
        (rx / self.sx + self.pivot.0, ry / self.sy + self.pivot.1)
    }
}

#[derive(Clone, Debug, Default)]
pub struct FrameState {
    pub whole: Xf,
    pub groups: BTreeMap<Group, Xf>,
    pub eyes_closed: bool,
    pub flash: bool,
}

/// A loop signal in -1..1 at phase `p` (0..1), shaped by the timing rule.
pub fn wave(p: f32, timing: &str, cfg: &Config) -> f32 {
    let s = (std::f32::consts::TAU * p).sin();
    match timing {
        // a triangle: constant speed, sharp turns
        "linear" => {
            let q = (p + 0.25).rem_euclid(1.0);
            1.0 - 4.0 * (q - 0.5).abs()
        }
        // hold at the extremes, move fast between them
        "snappy" => (s / (1.0 - cfg.timing.snappy_hold).max(0.05)).clamp(-1.0, 1.0),
        _ => s,
    }
}

/// A 0..1..0 hump over the loop (for hops).
fn hump(p: f32, timing: &str, cfg: &Config) -> f32 {
    let s = (std::f32::consts::PI * p).sin();
    match timing {
        "linear" => 1.0 - (2.0 * p - 1.0).abs(),
        "snappy" => (s / (1.0 - cfg.timing.snappy_hold).max(0.05)).min(1.0),
        _ => s,
    }
}

pub fn frame_count(anim: Anim, requested: Option<u32>, cfg: &Config) -> u32 {
    requested.unwrap_or(cfg.animations[anim.name()].frames).max(2)
}

/// Can this figure do this animation? Err explains why not.
pub fn check_fits(anim: Anim, fig: &Figure, subject_words: &str, item: bool) -> Result<(), String> {
    match anim {
        Anim::Walk if !fig.has_legs => Err(format!("a {subject_words} has no legs to walk with; try bouncing or floating")),
        Anim::Fly if !fig.has_wings => Err(format!("a {subject_words} has no wings; add \"with wings\" or try floating")),
        Anim::Spin if !item => Err(format!("spinning is for items (coins, gems, stars, hearts, potions), not a {subject_words}")),
        Anim::Blink if !fig.has_eyes => Err(format!("a {subject_words} without eyes can't blink; add \"with eyes\"")),
        _ => Ok(()),
    }
}

pub fn frames(anim: Anim, n: u32, timing: &str, fig: &Figure, cfg: &Config) -> Vec<FrameState> {
    let a = |k: &str| cfg.anim(anim, k);
    let [cx, cy, _, _] = fig.body;
    (0..n)
        .map(|i| {
            let p = i as f32 / n as f32;
            let w = wave(p, timing, cfg);
            let mut f = FrameState::default();
            f.whole.pivot = (cx, cy);
            match anim {
                Anim::Idle => f.whole.dy = a("bob") * w,
                Anim::Float => f.whole.dy = a("drift") * w,
                Anim::Bounce => {
                    let up = hump(p, timing, cfg);
                    f.whole.pivot = (cx, fig.feet_y);
                    f.whole.dy = -a("height") * up;
                    // squash on the ground, stretch in the air
                    let squash = a("squash") * (1.0 - (up * 3.0).min(1.0));
                    let stretch = a("squash") * 0.5 * (up * 2.0).min(1.0) * (1.0 - up);
                    f.whole.sy = 1.0 - squash + stretch;
                    f.whole.sx = 1.0 + squash * 0.8 - stretch * 0.6;
                }
                Anim::Walk => {
                    let step = a("step");
                    f.groups.insert(Group::LegL, Xf { dy: -step * w.max(0.0), dx: step * 0.5 * w, ..Xf::default() });
                    f.groups.insert(Group::LegR, Xf { dy: -step * (-w).max(0.0), dx: -step * 0.5 * w, ..Xf::default() });
                    f.whole.dy = -a("bob") * w.abs();
                }
                Anim::Fly => {
                    let flap = a("flap") * w;
                    let pl = fig.pivots.get(&Group::WingL).copied().unwrap_or((cx, cy));
                    let pr = fig.pivots.get(&Group::WingR).copied().unwrap_or((cx, cy));
                    f.groups.insert(Group::WingL, Xf { rot: flap, pivot: pl, ..Xf::default() });
                    f.groups.insert(Group::WingR, Xf { rot: -flap, pivot: pr, ..Xf::default() });
                    f.whole.dy = a("bob") * wave(p + 0.25, timing, cfg);
                }
                Anim::Spin => {
                    let c = (std::f32::consts::PI * p).cos().abs();
                    f.whole.sx = c.max(a("min_width"));
                }
                Anim::Blink => {
                    f.eyes_closed = i == (a("closed_frame") as u32).min(n - 1);
                }
                Anim::Hurt => {
                    f.flash = i % 2 == 1;
                    if i > 0 {
                        f.whole.dx = a("shake") * if i % 2 == 1 { 1.0 } else { -1.0 };
                    }
                }
            }
            // a tail sways with any motion
            if let Some(&tp) = fig.pivots.get(&Group::Tail) {
                f.groups.entry(Group::Tail).or_insert(Xf { rot: 12.0 * w, pivot: tp, ..Xf::default() });
            }
            f
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inverse_undoes_a_move() {
        let x = Xf { dx: 0.1, dy: -0.2, ..Xf::default() };
        let q = x.inverse((0.6, 0.3));
        assert!((q.0 - 0.5).abs() < 1e-6 && (q.1 - 0.5).abs() < 1e-6);
    }

    #[test]
    fn waves_loop() {
        let cfg = Config::load(None).unwrap();
        for t in ["linear", "eased", "snappy"] {
            assert!((wave(0.0, t, &cfg) - wave(1.0, t, &cfg)).abs() < 1e-4, "{t}");
        }
    }
}
