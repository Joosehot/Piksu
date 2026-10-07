//! Animation: each frame is a set of transforms (move, squash, turn) for the whole sprite and
//! for its groups (wings, legs, tail), plus blink and flash flags. Frames are phases of one loop,
//! so the last frame always leads back into the first.

use crate::config::Config;
use crate::figure::{Figure, Gait, Group};
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
    /// how far the frame is washed towards white (hurt, a hooked fish's hardest pulls); 0 = not
    pub flash: f32,
    /// a fish's mouth gapes (it fights the hook)
    pub mouth_open: bool,
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
        Anim::Blink if fig.gait == Gait::Swim => Err(format!("a {subject_words} has no eyelids to blink with; try swimming")),
        Anim::Blink if !fig.has_eyes => Err(format!("a {subject_words} without eyes can't blink; add \"with eyes\"")),
        Anim::Swim | Anim::Struggle if fig.gait != Gait::Swim => Err(format!("a {subject_words} has no fins to {}; try a fish", if anim == Anim::Swim { "swim with" } else { "thrash with" })),
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
                // a fish never holds still: idle is a gentle swim in place
                Anim::Idle if fig.gait == Gait::Swim => swim(&mut f, fig, p, timing, cfg, a("fish"), Anim::Swim),
                Anim::Swim => swim(&mut f, fig, p, timing, cfg, 1.0, Anim::Swim),
                Anim::Struggle => swim(&mut f, fig, p, timing, cfg, 1.0, Anim::Struggle),
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
                    let swing = a("swing");
                    let turn = |g: Group, deg: f32| Xf { rot: deg, pivot: fig.pivots.get(&g).copied().unwrap_or((cx, cy)), ..Xf::default() };
                    match fig.gait {
                        Gait::Biped => {
                            // seen from the front: the stepping leg lifts (knee up), the arm on the
                            // other side comes forward (it looks shorter), the body rises over the
                            // standing leg
                            let lift = |s: f32| (s * w).max(0.0);
                            f.groups.insert(Group::LegL, Xf { dy: -step * lift(1.0), ..Xf::default() });
                            f.groups.insert(Group::LegR, Xf { dy: -step * lift(-1.0), ..Xf::default() });
                            let arm = |g: Group, s: f32| {
                                let pv = fig.pivots.get(&g).copied().unwrap_or((cx, cy));
                                Xf { sy: 1.0 - a("arm_foreshorten") * lift(s), pivot: pv, ..Xf::default() }
                            };
                            f.groups.insert(Group::ArmL, arm(Group::ArmL, -1.0));
                            f.groups.insert(Group::ArmR, arm(Group::ArmR, 1.0));
                            f.whole.dy = -a("bob") * w.abs();
                        }
                        Gait::Quadruped => {
                            // diagonal pairs: near front with far back, far front with near back
                            for (g, s) in [(Group::LegFL, 1.0), (Group::LegBR, 1.0), (Group::LegFR, -1.0), (Group::LegBL, -1.0)] {
                                f.groups.insert(g, turn(g, -swing * s * w));
                            }
                            f.whole.dy = a("bob") * (1.0 - w.abs());
                        }
                        Gait::Hop => {
                            // crouch, then a leap with the hind leg stretched back
                            let up = hump(p, timing, cfg);
                            f.whole.dy = -a("height") * 0.5 * up;
                            f.whole.pivot = (cx, fig.feet_y);
                            f.whole.rot = -8.0 * up;
                            f.groups.insert(Group::LegBR, turn(Group::LegBR, swing * up));
                            f.groups.insert(Group::LegBL, turn(Group::LegBL, swing * up));
                            f.groups.insert(Group::LegFL, turn(Group::LegFL, -swing * 0.6 * up));
                        }
                        _ => {
                            f.groups.insert(Group::LegL, Xf { dy: -step * w.max(0.0), dx: step * 0.5 * w, ..Xf::default() });
                            f.groups.insert(Group::LegR, Xf { dy: -step * (-w).max(0.0), dx: -step * 0.5 * w, ..Xf::default() });
                            f.whole.dy = -a("bob") * w.abs();
                        }
                    }
                }
                Anim::Fly => {
                    let flap = if fig.realism == "realistic" { a("flap_realistic") } else { a("flap") } * w;
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
                    f.flash = if i % 2 == 1 { a("flash") } else { 0.0 };
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

/// Turn `p` by `deg` degrees (clockwise on screen) around `c`.
fn turned(p: (f32, f32), c: (f32, f32), deg: f32) -> (f32, f32) {
    let (s, k) = deg.to_radians().sin_cos();
    let (x, y) = (p.0 - c.0, p.1 - c.1);
    (c.0 + x * k - y * s, c.1 + x * s + y * k)
}

/// A fish's motion. The rear of the body bends around its pivot; the tail fin hangs on the rear
/// (it is carried along, then turns further, a little later); the pectoral and pelvic fins
/// paddle. Swim: the head answers the bend a little and the fish bobs. Struggle: the fish swings
/// from its mouth and bends into a C against the swing. `scale` softens it (idle).
fn swim(f: &mut FrameState, fig: &Figure, p: f32, timing: &str, cfg: &Config, scale: f32, kind: Anim) {
    let a = |k: &str| cfg.anim(kind, k) * scale;
    let struggle = kind == Anim::Struggle;
    // a struggle starts at its hardest pull (frame 0 is a peak), so the strongest frames are
    // whole frames whatever the count
    let p = if struggle { p + 0.25 } else { p };
    let w = wave(p, timing, cfg);
    // the tail whips back and forth twice per bend when fighting
    let lag = if struggle { wave(2.0 * p - cfg.anim(kind, "lag"), timing, cfg) } else { wave(p - cfg.anim(kind, "lag"), timing, cfg) };
    let [cx, cy, _, _] = fig.body;
    let pivot = |g: Group| fig.pivots.get(&g).copied().unwrap_or((cx, cy));
    let bend = if kind == Anim::Struggle { -a("body") * w } else { a("body") * w };
    let (rear, tail) = (pivot(Group::Rear), pivot(Group::Tail));
    f.groups.insert(Group::Rear, Xf { rot: bend, pivot: rear, ..Xf::default() });
    // the tail root rides on the bent rear
    let moved = turned(tail, rear, bend);
    let sweep = if kind == Anim::Struggle { -a("tail") * lag } else { a("tail") * lag };
    f.groups.insert(Group::Tail, Xf { rot: bend + sweep, pivot: tail, dx: moved.0 - tail.0, dy: moved.1 - tail.1, ..Xf::default() });
    f.groups.insert(Group::Fin, Xf { rot: a("fin") * wave(p + 0.5, timing, cfg), pivot: pivot(Group::Fin), ..Xf::default() });
    f.groups.insert(Group::Fin2, Xf { rot: a("fin") * 0.5 * wave(p + 0.25, timing, cfg), pivot: pivot(Group::Fin2), ..Xf::default() });
    if struggle {
        // held by the mouth (the body pivot is where a hook would catch), gaping, pulling; the
        // hardest pulls flash
        f.whole.pivot = pivot(Group::Body);
        f.whole.rot = a("thrash") * w;
        f.mouth_open = true;
        if w.abs() >= cfg.anim(kind, "flash_at") {
            f.flash = cfg.anim(kind, "flash");
        }
    } else {
        f.whole.rot = -a("head") * w;
        f.whole.dy = a("bob") * wave(p + 0.25, timing, cfg);
    }
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
    fn a_swimming_fish_carries_its_tail_on_the_bent_rear() {
        let cfg = Config::load(None).unwrap();
        let req = crate::parser::parse("a perch, swimming", false).unwrap();
        let mut chosen = crate::rules::Chosen::new();
        chosen.insert("realism", "stylized");
        chosen.insert("eyes", "shine");
        let fig = crate::figure::build(&req, &cfg, &chosen).unwrap();
        let fr = frames(Anim::Swim, 8, "eased", &fig, &cfg);
        let f = &fr[2];
        let (rear, tail) = (f.groups[&Group::Rear], f.groups[&Group::Tail]);
        assert!(rear.rot.abs() > 1.0 && tail.rot.abs() > rear.rot.abs(), "the tail turns further than the body bends");
        // the tail root lands where the bent rear put it
        let root = fig.pivots[&Group::Tail];
        let moved = turned(root, fig.pivots[&Group::Rear], rear.rot);
        assert!(((root.0 + tail.dx) - moved.0).abs() < 1e-5 && ((root.1 + tail.dy) - moved.1).abs() < 1e-5);
        assert!(f.groups.contains_key(&Group::Fin) && f.groups.contains_key(&Group::Fin2));
    }

    #[test]
    fn a_hooked_fish_gapes_bends_hard_and_flashes_on_its_hardest_pulls() {
        let cfg = Config::load(None).unwrap();
        for s in ["a perch", "a roach", "a pike"] {
            let req = crate::parser::parse(s, false).unwrap();
            let mut chosen = crate::rules::Chosen::new();
            chosen.insert("realism", "stylized");
            chosen.insert("eyes", "shine");
            let fig = crate::figure::build(&req, &cfg, &chosen).unwrap();
            let n = frame_count(Anim::Struggle, None, &cfg);
            assert!((4..=6).contains(&n), "a short loop");
            let fr = frames(Anim::Struggle, n, "eased", &fig, &cfg);
            assert!(fr.iter().all(|f| f.mouth_open), "{s}: the jaws gape on the hook");
            let flashed: Vec<usize> = (0..fr.len()).filter(|&i| fr[i].flash > 0.0).collect();
            assert_eq!(flashed, vec![0, n as usize / 2], "{s}: the two hardest pulls flash");
            // the hardest pull bends the body hard, one way then the other
            let bend = |i: usize| fr[i].groups[&Group::Rear].rot;
            assert!(bend(0).abs() >= cfg.anim(Anim::Struggle, "body") * 0.95 && bend(0) * bend(n as usize / 2) < 0.0);
            // swimming keeps the mouth shut
            assert!(frames(Anim::Swim, 8, "eased", &fig, &cfg).iter().all(|f| !f.mouth_open && f.flash == 0.0));
        }
    }

    #[test]
    fn waves_loop() {
        let cfg = Config::load(None).unwrap();
        for t in ["linear", "eased", "snappy"] {
            assert!((wave(0.0, t, &cfg) - wave(1.0, t, &cfg)).abs() < 1e-4, "{t}");
        }
    }
}
