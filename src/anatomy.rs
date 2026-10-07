//! Realistic blueprints: real anatomy and construction instead of cartoon primitives. Animals
//! are drawn from the side (the way they read best at pixel scale), people and robots from the
//! front, objects in three-quarter view. Main proportions come from rules.toml [anatomy.*]; the
//! relations here are anatomy ("the hip joint sits at the back of the body", "the ring hangs
//! under the cap").

use crate::color::{adjust, mix, Rgb};

/// Subjects drawn only by anatomy, at every realism level (no cartoon blueprint in figure.rs).
pub const ANATOMY_ONLY: &[Subject] = &[Subject::Human, Subject::Dog];
use crate::config::Config;
use crate::figure::{material_of, Figure, Gait, Group, Palette, Part, Shape, Show};
use crate::request::*;
use crate::rules::Chosen;
use std::collections::BTreeMap;
use std::f32::consts::PI;

// ---------------------------------------------------------------- shape helpers

/// An ellipse turned by `deg` degrees, as a polygon.
pub fn oval(cx: f32, cy: f32, rx: f32, ry: f32, deg: f32) -> Shape {
    let (s, c) = deg.to_radians().sin_cos();
    Shape::Poly(
        (0..24)
            .map(|k| {
                let a = 2.0 * PI * k as f32 / 24.0;
                let (x, y) = (rx * a.cos(), ry * a.sin());
                (cx + x * c - y * s, cy + x * s + y * c)
            })
            .collect(),
    )
}

/// A limb segment from (x0, y0) to (x1, y1), `w0` wide at the start and `w1` at the end, with
/// rounded ends.
pub fn limb(x0: f32, y0: f32, x1: f32, y1: f32, w0: f32, w1: f32) -> Shape {
    let (dx, dy) = (x1 - x0, y1 - y0);
    let len = (dx * dx + dy * dy).sqrt().max(1e-5);
    let (ux, uy) = (dx / len, dy / len);
    let (nx, ny) = (-uy, ux);
    let mut pts = Vec::new();
    // start cap: from +n round the back (-u) to -n
    for k in 0..=6 {
        let t = PI * k as f32 / 6.0;
        let (c, s) = (t.cos(), t.sin());
        pts.push((x0 + (nx * c - ux * s) * w0 / 2.0, y0 + (ny * c - uy * s) * w0 / 2.0));
    }
    // end cap: from -n round the front (+u) to +n
    for k in 0..=6 {
        let t = PI * k as f32 / 6.0;
        let (c, s) = (t.cos(), t.sin());
        pts.push((x1 + (-nx * c + ux * s) * w1 / 2.0, y1 + (-ny * c + uy * s) * w1 / 2.0));
    }
    Shape::Poly(pts)
}

/// A band along a polyline, `w0` wide at the first point tapering to `w1` at the last.
pub fn band(points: &[(f32, f32)], w0: f32, w1: f32) -> Shape {
    let n = points.len();
    let mut left = Vec::new();
    let mut right = Vec::new();
    for i in 0..n {
        let (a, b) = if i + 1 < n { (points[i], points[i + 1]) } else { (points[i - 1], points[i]) };
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        let len = (dx * dx + dy * dy).sqrt().max(1e-5);
        let w = w0 + (w1 - w0) * i as f32 / (n - 1).max(1) as f32;
        let (nx, ny) = (-dy / len * w / 2.0, dx / len * w / 2.0);
        left.push((points[i].0 + nx, points[i].1 + ny));
        right.push((points[i].0 - nx, points[i].1 - ny));
    }
    right.reverse();
    left.extend(right);
    Shape::Poly(left)
}

/// Points along a quadratic curve.
pub fn curve(p0: (f32, f32), c: (f32, f32), p1: (f32, f32), steps: usize) -> Vec<(f32, f32)> {
    (0..=steps)
        .map(|k| {
            let t = k as f32 / steps as f32;
            let u = 1.0 - t;
            (u * u * p0.0 + 2.0 * u * t * c.0 + t * t * p1.0, u * u * p0.1 + 2.0 * u * t * c.1 + t * t * p1.1)
        })
        .collect()
}

/// An organic outline: an ellipse whose radius wobbles by `bumps` (angle in degrees, amount).
pub fn blob(cx: f32, cy: f32, rx: f32, ry: f32, bumps: &[(f32, f32)]) -> Shape {
    Shape::Poly(
        (0..36)
            .map(|k| {
                let a = 2.0 * PI * k as f32 / 36.0;
                let mut r = 1.0;
                for &(at, amt) in bumps {
                    let d = ((a - at.to_radians() + PI).rem_euclid(2.0 * PI) - PI).abs();
                    r += amt * (-(d * d) / 0.25).exp();
                }
                (cx + rx * r * a.cos(), cy + ry * r * a.sin())
            })
            .collect(),
    )
}

struct B {
    parts: Vec<Part>,
}

impl B {
    fn add(&mut self, role: &'static str, group: Group, layer: i32, shape: Shape, color: Rgb) -> &mut Part {
        self.parts.push(Part { role, group, layer, shape, color, shaded: false, shade_box: None, clip: None, show: Show::Always, min_pixel: false });
        self.parts.last_mut().unwrap()
    }
    /// A lit, textured surface part.
    fn solid(&mut self, role: &'static str, group: Group, layer: i32, shape: Shape, color: Rgb, shade_box: Option<(f32, f32, f32, f32)>) -> &mut Part {
        let p = self.add(role, group, layer, shape, color);
        p.shaded = true;
        p.shade_box = shade_box;
        p
    }
    fn detail(&mut self, role: &'static str, group: Group, layer: i32, shape: Shape, color: Rgb) -> &mut Part {
        let p = self.add(role, group, layer, shape, color);
        p.min_pixel = true;
        p
    }
}

fn boxed(c: [f32; 4]) -> (f32, f32, f32, f32) {
    (c[0] - c[2] / 2.0, c[1] - c[3] / 2.0, c[0] + c[2] / 2.0, c[1] + c[3] / 2.0)
}

fn dims<'a>(cfg: &'a Config, s: Subject, key: &str) -> &'a [f32] {
    cfg.anatomy[s.name()].get(key).map(|v| v.as_slice()).unwrap_or_else(|| panic!("anatomy.{}.{key} missing (validated at load)", s.name()))
}

/// What realistic blueprints need from rules.toml, per subject (checked at load).
pub const REQUIRED: &[(&str, &[&str])] = &[
    ("slime", &["body"]),
    ("ghost", &["body"]),
    ("bat", &["body", "head", "wing", "ear"]),
    ("cat", &["body", "head", "leg", "tail", "ear"]),
    ("frog", &["body", "head", "leg"]),
    ("knight", &["body", "head", "leg", "arm"]),
    ("robot", &["body", "head", "leg", "arm"]),
    ("coin", &["body", "edge"]),
    ("gem", &["body"]),
    ("star", &["body"]),
    ("heart", &["body"]),
    ("potion", &["body", "neck"]),
    ("mushroom", &["cap", "stem"]),
    ("human", &["body", "head", "leg", "arm"]),
    ("dog", &["body", "head", "leg", "tail", "ear"]),
    ("fish", &["body", "profile", "head", "tail", "dorsal", "anal", "fin"]),
    ("perch", &["body", "profile", "head", "tail", "dorsal", "dorsal2", "anal", "fin"]),
    ("roach", &["body", "profile", "head", "tail", "dorsal", "anal", "fin"]),
    ("pike", &["body", "profile", "head", "tail", "dorsal", "anal", "fin"]),
    ("hook", &["body", "wire"]),
];

/// A realistic eye: almond white, coloured iris, pupil (slit, round or wide), one catch-light.
#[allow(clippy::too_many_arguments)]
fn eye(b: &mut B, x: f32, y: f32, r: f32, iris: Rgb, pupil: &str, lid: Rgb, angry: bool, cfg: &Config) {
    let pal = &cfg.palette_rules;
    b.detail("eye_white", Group::Body, 5, oval(x, y, r * 1.25, r * 0.85, 0.0), mix(iris, pal.eye_white, 0.35)).show = Show::EyesOpen;
    b.detail("iris", Group::Body, 6, Shape::Ellipse { cx: x, cy: y, rx: r * 0.8, ry: r * 0.8 }, iris).show = Show::EyesOpen;
    let p = match pupil {
        "slit" => Shape::Ellipse { cx: x, cy: y, rx: r * 0.2, ry: r * 0.75 },
        "wide" => Shape::Ellipse { cx: x, cy: y, rx: r * 0.65, ry: r * 0.35 },
        _ => Shape::Ellipse { cx: x, cy: y, rx: r * 0.42, ry: r * 0.42 },
    };
    b.detail("pupil", Group::Body, 7, p, pal.eye_dark).show = Show::EyesOpen;
    b.add("catchlight", Group::Body, 8, Shape::Rect { x0: x - r * 0.55, y0: y - r * 0.6, x1: x - r * 0.2, y1: y - r * 0.25 }, pal.eye_white).show = Show::EyesOpen;
    // the upper lid shades the top of the eye; angry: a heavy brow slanting down to the front
    let top = if angry {
        Shape::Poly(vec![(x - r * 1.5, y - r * 1.2), (x + r * 1.6, y - r * 1.2), (x + r * 1.6, y - r * 0.05), (x - r * 1.5, y - r * 0.75)])
    } else {
        Shape::Rect { x0: x - r * 1.5, y0: y - r * 1.2, x1: x + r * 1.5, y1: y - r * 0.62 }
    };
    let l = b.add("lid", Group::Body, 9, top, lid);
    l.clip = Some("eye_white");
    l.show = Show::EyesOpen;
    b.detail("eye_closed", Group::Body, 5, Shape::Rect { x0: x - r * 1.2, y0: y - r * 0.1, x1: x + r * 1.2, y1: y + r * 0.2 }, adjust(lid, 0.45, 1.0, 0.0)).show = Show::EyesClosed;
}

pub fn build(req: &Request, cfg: &Config, chosen: &Chosen, pal: Palette, mut why: Vec<(String, String)>) -> Result<Figure, String> {
    let subj = req.subject.v;
    let main = pal.main;
    let mut b = B { parts: vec![] };
    let mut pivots: BTreeMap<Group, (f32, f32)> = BTreeMap::new();
    let mut gait = Gait::None;
    let mut has_wings = false;
    let mut has_eyes = false;
    let mut symmetric = false;
    let d = |k: &str| dims(cfg, subj, k);
    let color = |name: &str| cfg.colors[name];
    let iris = cfg.colors[cfg.anatomy_iris.get(subj.name()).map(|s| s.as_str()).unwrap_or("brown")];
    let eye_col = req.has(Feature::Eyes).and_then(|f| f.color.as_ref()).map(|c| cfg.colors[c.v.0.name()]).unwrap_or(iris);
    let angry = chosen.get("eyes").copied() == Some("angry") || req.has(Feature::Angry).is_some();
    let far = |c: Rgb| adjust(c, 0.72, 1.0, 0.0);
    let body_c: [f32; 4];
    let head_c: [f32; 4];

    match subj {
        Subject::Fish | Subject::Perch | Subject::Roach | Subject::Pike | Subject::Hook => unreachable!("drawn by fish.rs"),
        Subject::Cat => {
            // side view, facing right
            let bd = d("body");
            let hd = d("head");
            let (lw, ll) = (d("leg")[0], d("leg")[1]);
            let (tl, tw) = (d("tail")[0], d("tail")[1]);
            let (ew, eh) = (d("ear")[0], d("ear")[1]);
            let [bx, by, bl, bh] = [bd[0], bd[1], bd[2], bd[3]];
            let [hx, hy, hw, hh] = [hd[0], hd[1], hd[2], hd[3]];
            body_c = [bx, by, bl, bh];
            head_c = [hx, hy, hw, hh];
            let ground = by + bh / 2.0 + ll * 0.72;
            let (front_hip, back_hip) = ((bx + bl * 0.3, by + bh * 0.1), (bx - bl * 0.33, by + bh * 0.05));
            // tail: rises from the rump in an S
            let root = (bx - bl * 0.47, by - bh * 0.15);
            let tail = curve(root, (root.0 - tl * 0.7, root.1 + tl * 0.1), (root.0 - tl * 0.55, root.1 - tl * 0.95), 10);
            b.solid("tail", Group::Tail, -3, band(&tail, tw * 1.2, tw * 0.8), main, None);
            pivots.insert(Group::Tail, root);
            // far legs, darker
            let leg = |hip: (f32, f32), fwd: f32| -> Vec<(f32, f32)> {
                vec![hip, (hip.0 + fwd * lw * 0.5, hip.1 + ll * 0.45), (hip.0 + fwd * lw * 0.15, ground - lw * 0.3)]
            };
            let draw_leg = |b: &mut B, g: Group, hip: (f32, f32), fwd: f32, c: Rgb, layer: i32, back: bool| {
                let p = leg(hip, fwd);
                let thigh_w = if back { lw * 1.9 } else { lw * 1.3 };
                b.solid("leg", g, layer, limb(p[0].0, p[0].1, p[1].0, p[1].1, thigh_w, lw * 0.95), c, None);
                b.solid("leg", g, layer, limb(p[1].0, p[1].1, p[2].0, p[2].1, lw * 0.95, lw * 0.8), c, None);
                b.solid("paw", g, layer, oval(p[2].0 + lw * 0.35, ground - lw * 0.35, lw * 0.75, lw * 0.4, 0.0), c, None);
            };
            draw_leg(&mut b, Group::LegFR, (front_hip.0 + lw * 0.4, front_hip.1), 0.4, far(main), -2, false);
            draw_leg(&mut b, Group::LegBL, (back_hip.0 + lw * 0.4, back_hip.1), -0.6, far(main), -2, true);
            // body: long barrel with a deeper chest
            b.solid("body", Group::Body, 0, blob(bx, by, bl / 2.0, bh / 2.0, &[(20.0, 0.12), (160.0, 0.08), (270.0, -0.06)]), main, Some(boxed(body_c)));
            b.add("belly", Group::Body, 1, oval(bx + bl * 0.05, by + bh * 0.36, bl * 0.34, bh * 0.2, 0.0), pal.light).clip = Some("body");
            // neck joins body and head
            b.solid("neck", Group::Body, 0, limb(bx + bl * 0.38, by - bh * 0.05, hx - hw * 0.15, hy + hh * 0.1, bh * 0.75, hh * 0.7), main, Some(boxed(body_c)));
            // near legs
            draw_leg(&mut b, Group::LegFL, front_hip, 0.4, main, 2, false);
            draw_leg(&mut b, Group::LegBR, back_hip, -0.6, main, 2, true);
            for (g, hip) in [(Group::LegFL, front_hip), (Group::LegFR, (front_hip.0 + lw * 0.4, front_hip.1)), (Group::LegBL, (back_hip.0 + lw * 0.4, back_hip.1)), (Group::LegBR, back_hip)] {
                pivots.insert(g, hip);
            }
            // head: skull, cheek, muzzle, nose, ears
            b.solid("head", Group::Body, 3, oval(hx, hy, hw / 2.0, hh / 2.0, 0.0), main, Some(boxed(head_c)));
            b.solid("cheek", Group::Body, 3, oval(hx + hw * 0.08, hy + hh * 0.18, hw * 0.4, hh * 0.3, 0.0), main, Some(boxed(head_c)));
            b.add("muzzle", Group::Body, 4, oval(hx + hw * 0.36, hy + hh * 0.2, hw * 0.2, hh * 0.17, 0.0), pal.light);
            b.detail("nose", Group::Body, 5, Shape::Poly(vec![(hx + hw * 0.46, hy + hh * 0.05), (hx + hw * 0.56, hy + hh * 0.06), (hx + hw * 0.5, hy + hh * 0.15)]), color("pink"));
            b.detail("mouth", Group::Body, 5, band(&[(hx + hw * 0.5, hy + hh * 0.22), (hx + hw * 0.42, hy + hh * 0.3), (hx + hw * 0.32, hy + hh * 0.26)], 0.012, 0.012), pal.deep);
            for (ex, layer, c) in [(hx - hw * 0.28, 2, far(main)), (hx - hw * 0.02, 4, main)] {
                b.solid("ear", Group::Body, layer, Shape::Poly(vec![(ex - ew / 2.0, hy - hh * 0.3), (ex + ew / 2.0, hy - hh * 0.38), (ex - ew * 0.1, hy - hh * 0.4 - eh)]), c, None);
            }
            b.add("inner_ear", Group::Body, 5, Shape::Poly(vec![(hx - hw * 0.02 - ew * 0.25, hy - hh * 0.34), (hx - hw * 0.02 + ew * 0.25, hy - hh * 0.38), (hx - hw * 0.04, hy - hh * 0.36 - eh * 0.7)]), mix(color("pink"), main, 0.4));
            eye(&mut b, hx + hw * 0.18, hy - hh * 0.05, hh * 0.13, eye_col, "slit", main, angry, cfg);
            has_eyes = true;
            gait = Gait::Quadruped;
        }
        Subject::Dog => {
            // side view, facing right: a deep chest and a tucked belly, a long muzzle with a black
            // nose, drop ears, hind legs with a hock, a tail carried up
            let bd = d("body");
            let hd = d("head");
            let (lw, ll) = (d("leg")[0], d("leg")[1]);
            let (tl, tw) = (d("tail")[0], d("tail")[1]);
            let (ew, eh) = (d("ear")[0], d("ear")[1]);
            let [bx, by, bl, bh] = [bd[0], bd[1], bd[2], bd[3]];
            let [hx, hy, hw, hh] = [hd[0], hd[1], hd[2], hd[3]];
            body_c = [bx, by, bl, bh];
            head_c = [hx, hy, hw, hh];
            let blaze = mix(color("white"), main, 0.15);
            let ground = by + bh / 2.0 + ll * 0.8;
            let front_hip = (bx + bl * 0.3, by + bh * 0.15);
            let back_hip = (bx - bl * 0.34, by + bh * 0.02);
            // tail: thick at the root, carried up in a gentle curve
            let root = (bx - bl * 0.48, by - bh * 0.25);
            let tail = curve(root, (root.0 - tl * 0.55, root.1 - tl * 0.1), (root.0 - tl * 0.45, root.1 - tl * 0.8), 10);
            b.solid("tail", Group::Tail, -3, band(&tail, tw * 1.5, tw * 0.5), main, None);
            b.add("tail_tip", Group::Tail, -2, band(&tail[7..], tw * 0.8, tw * 0.45), blaze).clip = Some("tail");
            pivots.insert(Group::Tail, root);
            // front leg: straight, a pastern, a paw; hind leg: thigh back, hock, cannon down
            let front = |b: &mut B, g: Group, hip: (f32, f32), c: Rgb, layer: i32| {
                let elbow = (hip.0 + lw * 0.1, hip.1 + ll * 0.35);
                let wrist = (hip.0 + lw * 0.15, ground - ll * 0.14);
                b.solid("leg", g, layer, limb(hip.0, hip.1 - bh * 0.1, elbow.0, elbow.1, lw * 1.6, lw * 1.0), c, None);
                b.solid("leg", g, layer, limb(elbow.0, elbow.1, wrist.0, wrist.1, lw * 0.95, lw * 0.8), c, None);
                b.solid("leg", g, layer, limb(wrist.0, wrist.1, wrist.0 + lw * 0.25, ground - lw * 0.35, lw * 0.8, lw * 0.8), c, None);
                b.solid("paw", g, layer, oval(wrist.0 + lw * 0.5, ground - lw * 0.32, lw * 0.75, lw * 0.36, 0.0), c, None);
                b.add("toes", g, layer + 1, Shape::Rect { x0: wrist.0 + lw * 0.45, y0: ground - lw * 0.5, x1: wrist.0 + lw * 0.55, y1: ground }, mix(c, color("black"), 0.4)).clip = Some("paw");
            };
            let hind = |b: &mut B, g: Group, hip: (f32, f32), c: Rgb, layer: i32| {
                let stifle = (hip.0 + lw * 0.6, hip.1 + ll * 0.38);
                let hock = (hip.0 - lw * 0.7, ground - ll * 0.3);
                b.solid("leg", g, layer, limb(hip.0, hip.1 - bh * 0.1, stifle.0, stifle.1, lw * 2.3, lw * 1.1), c, None);
                b.solid("leg", g, layer, limb(stifle.0, stifle.1, hock.0, hock.1, lw * 1.05, lw * 0.8), c, None);
                b.solid("leg", g, layer, limb(hock.0, hock.1, hock.0 + lw * 0.2, ground - lw * 0.35, lw * 0.8, lw * 0.75), c, None);
                b.solid("paw", g, layer, oval(hock.0 + lw * 0.55, ground - lw * 0.32, lw * 0.75, lw * 0.36, 0.0), c, None);
            };
            front(&mut b, Group::LegFR, (front_hip.0 + lw * 0.5, front_hip.1), far(main), -2);
            hind(&mut b, Group::LegBL, (back_hip.0 + lw * 0.5, back_hip.1), far(main), -2);
            // body: deep chest in front, belly tucked up towards the hind legs, a level back
            b.solid("body", Group::Body, 0, Shape::Poly(vec![
                (bx - bl * 0.5, by - bh * 0.35), (bx - bl * 0.2, by - bh * 0.48), (bx + bl * 0.2, by - bh * 0.5),
                (bx + bl * 0.45, by - bh * 0.42), (bx + bl * 0.52, by - bh * 0.05), (bx + bl * 0.42, by + bh * 0.5),
                (bx + bl * 0.15, by + bh * 0.52), (bx - bl * 0.2, by + bh * 0.25), (bx - bl * 0.42, by + bh * 0.3),
                (bx - bl * 0.54, by + bh * 0.05),
            ]), main, Some(boxed(body_c)));
            b.add("chest", Group::Body, 1, oval(bx + bl * 0.38, by + bh * 0.18, bl * 0.12, bh * 0.35, -10.0), blaze).clip = Some("body");
            // fur: short strokes along the back and the ribs
            for k in 0..6 {
                let x = bx - bl * 0.35 + bl * 0.13 * k as f32;
                b.add("fur", Group::Body, 1, band(&[(x, by - bh * 0.1), (x - bl * 0.03, by + bh * 0.08)], 0.005, 0.004), pal.shade).clip = Some("body");
            }
            // neck: thick, rising to the head
            b.solid("neck", Group::Body, 0, limb(bx + bl * 0.36, by - bh * 0.1, hx - hw * 0.25, hy + hh * 0.1, bh * 0.85, hh * 0.75), main, Some(boxed(body_c)));
            b.add("throat", Group::Body, 1, limb(bx + bl * 0.45, by + bh * 0.05, hx - hw * 0.05, hy + hh * 0.35, bh * 0.25, hh * 0.25), blaze).clip = Some("neck");
            // near legs
            front(&mut b, Group::LegFL, front_hip, main, 2);
            hind(&mut b, Group::LegBR, back_hip, main, 2);
            for (g, hip) in [(Group::LegFL, front_hip), (Group::LegFR, (front_hip.0 + lw * 0.5, front_hip.1)), (Group::LegBL, (back_hip.0 + lw * 0.5, back_hip.1)), (Group::LegBR, back_hip)] {
                pivots.insert(g, hip);
            }
            // head: skull, stop, long muzzle, black nose, lip line, jowl
            b.solid("head", Group::Body, 3, oval(hx - hw * 0.1, hy, hw * 0.42, hh * 0.48, 0.0), main, Some(boxed(head_c)));
            b.solid("muzzle", Group::Body, 3, Shape::Poly(vec![
                (hx + hw * 0.05, hy - hh * 0.18), (hx + hw * 0.55, hy - hh * 0.05), (hx + hw * 0.62, hy + hh * 0.12),
                (hx + hw * 0.55, hy + hh * 0.3), (hx + hw * 0.1, hy + hh * 0.42), (hx - hw * 0.05, hy + hh * 0.3),
            ]), main, Some(boxed(head_c)));
            b.add("muzzle_light", Group::Body, 4, Shape::Poly(vec![(hx + hw * 0.1, hy + hh * 0.12), (hx + hw * 0.6, hy + hh * 0.18), (hx + hw * 0.52, hy + hh * 0.32), (hx + hw * 0.1, hy + hh * 0.42)]), blaze).clip = Some("muzzle");
            b.solid("nose", Group::Body, 5, oval(hx + hw * 0.58, hy + hh * 0.02, hw * 0.08, hh * 0.08, 0.0), color("black"), None);
            b.detail("nose_shine", Group::Body, 6, Shape::Rect { x0: hx + hw * 0.54, y0: hy - hh * 0.03, x1: hx + hw * 0.58, y1: hy + hh * 0.0 }, color("grey"));
            b.detail("mouth", Group::Body, 5, band(&curve((hx + hw * 0.56, hy + hh * 0.2), (hx + hw * 0.35, hy + hh * 0.3), (hx + hw * 0.12, hy + hh * 0.26), 5), 0.008, 0.008), pal.deep);
            if req.has(Feature::Happy).is_some() || chosen.get("mouth").copied() == Some("open") {
                b.detail("tongue", Group::Body, 6, oval(hx + hw * 0.3, hy + hh * 0.36, hw * 0.08, hh * 0.08, 0.0), color("pink"));
            }
            b.add("stop", Group::Body, 4, band(&[(hx + hw * 0.02, hy - hh * 0.3), (hx + hw * 0.08, hy - hh * 0.1)], 0.006, 0.006), pal.shade).clip = Some("head");
            eye(&mut b, hx + hw * 0.1, hy - hh * 0.12, hh * 0.13, eye_col, "round", main, angry, cfg);
            // drop ears: the far one peeks behind the skull, the near one hangs over the cheek
            b.solid("ear", Group::Body, 2, oval(hx - hw * 0.25, hy - hh * 0.05, ew * 0.45, eh * 0.55, 15.0), far(pal.shade), None);
            b.solid("ear", Group::Body, 5, Shape::Poly(vec![
                (hx - hw * 0.35, hy - hh * 0.38), (hx - hw * 0.1, hy - hh * 0.42), (hx - hw * 0.05, hy - hh * 0.1),
                (hx - hw * 0.12, hy - hh * 0.4 + eh), (hx - hw * 0.3, hy - hh * 0.4 + eh * 1.05), (hx - hw * 0.42, hy - hh * 0.1),
            ]), mix(pal.shade, pal.deep, 0.6), None);
            b.add("ear_fold", Group::Body, 6, band(&[(hx - hw * 0.28, hy - hh * 0.3), (hx - hw * 0.24, hy - hh * 0.4 + eh * 0.9)], 0.006, 0.005), pal.deep).clip = Some("ear");
            has_eyes = true;
            gait = Gait::Quadruped;
        }
        Subject::Frog => {
            // side view, crouching, facing right
            let bd = d("body");
            let hd = d("head");
            let (lw, ll) = (d("leg")[0], d("leg")[1]);
            let [bx, by, bl, bh] = [bd[0], bd[1], bd[2], bd[3]];
            let [hx, hy, hw, hh] = [hd[0], hd[1], hd[2], hd[3]];
            body_c = [bx, by, bl, bh];
            head_c = [hx, hy, hw, hh];
            let ground = by + bh / 2.0 + lw * 0.3;
            // far hind leg peeks out behind
            b.solid("thigh", Group::LegBL, -2, oval(bx - bl * 0.18, by + bh * 0.12, ll * 0.38, lw * 0.95, -25.0), far(main), None);
            // body: pear shaped, heavier at the back, head merged at the front
            b.solid("body", Group::Body, 0, blob(bx, by, bl / 2.0, bh / 2.0, &[(200.0, 0.1), (160.0, 0.15), (300.0, -0.1)]), main, Some(boxed(body_c)));
            b.solid("head", Group::Body, 0, oval(hx, hy, hw / 2.0, hh / 2.0, 12.0), main, Some(boxed(body_c)));
            b.add("belly", Group::Body, 1, oval(bx + bl * 0.1, by + bh * 0.3, bl * 0.38, bh * 0.22, 8.0), mix(pal.light, color("yellow"), 0.3)).clip = Some("body");
            // skin spots
            for (sx, sy, r) in [(-0.22, -0.18, 0.07), (0.02, -0.28, 0.05), (-0.36, 0.0, 0.05), (0.14, -0.1, 0.04)] {
                b.add("spot", Group::Body, 1, oval(bx + bl * sx, by + bh * sy * 2.0, r, r * 0.8, 20.0), pal.shade).clip = Some("body");
            }
            // near hind leg, folded: thigh forward, shin back, long webbed foot along the ground
            let knee = (bx + bl * 0.02, by + bh * 0.28);
            b.solid("thigh", Group::LegBR, 2, oval(bx - bl * 0.12, by + bh * 0.2, ll * 0.36, lw * 0.9, -18.0), main, None);
            b.solid("shin", Group::LegBR, 2, limb(knee.0, knee.1, bx - bl * 0.3, ground - lw * 0.35, lw * 0.9, lw * 0.6), main, None);
            b.solid("foot", Group::LegBR, 2, Shape::Poly(vec![(bx - bl * 0.34, ground - lw * 0.5), (bx - bl * 0.02, ground - lw * 0.35), (bx + bl * 0.08, ground), (bx - bl * 0.36, ground)]), pal.shade, None);
            pivots.insert(Group::LegBR, (bx - bl * 0.2, by + bh * 0.05));
            // front leg under the chin
            let sh = (bx + bl * 0.32, by + bh * 0.12);
            b.solid("arm", Group::LegFL, 2, limb(sh.0, sh.1, sh.0 + lw * 0.3, ground - lw * 0.25, lw * 0.7, lw * 0.5), main, None);
            b.solid("hand", Group::LegFL, 2, oval(sh.0 + lw * 0.7, ground - lw * 0.15, lw * 0.6, lw * 0.25, 0.0), pal.shade, None);
            pivots.insert(Group::LegFL, sh);
            // eye bulge on top, eardrum behind, mouth line
            let (ex, ey, er) = (hx + hw * 0.05, hy - hh * 0.42, hh * 0.28);
            b.solid("bulge", Group::Body, 3, Shape::Ellipse { cx: ex, cy: ey, rx: er * 1.35, ry: er * 1.15 }, main, None);
            eye(&mut b, ex + er * 0.1, ey, er * 0.85, eye_col, "wide", main, angry, cfg);
            b.add("eardrum", Group::Body, 2, Shape::Ellipse { cx: hx - hw * 0.35, cy: hy - hh * 0.05, rx: hh * 0.16, ry: hh * 0.16 }, pal.shade).clip = Some("head");
            b.detail("mouth", Group::Body, 4, band(&curve((hx - hw * 0.3, hy + hh * 0.15), (hx + hw * 0.1, hy + hh * 0.28), (hx + hw * 0.48, hy + hh * 0.05), 6), 0.014, 0.014), pal.deep);
            b.detail("nostril", Group::Body, 4, Shape::Rect { x0: hx + hw * 0.36, y0: hy - hh * 0.2, x1: hx + hw * 0.42, y1: hy - hh * 0.14 }, pal.deep);
            has_eyes = true;
            gait = Gait::Hop;
        }
        Subject::Bat => {
            let bd = d("body");
            let hd = d("head");
            let (span, wh) = (d("wing")[0], d("wing")[1]);
            let (ew, eh) = (d("ear")[0], d("ear")[1]);
            let [bx, by, bw, bh] = [bd[0], bd[1], bd[2], bd[3]];
            let [hx, hy, hw, hh] = [hd[0], hd[1], hd[2], hd[3]];
            body_c = [bx, by, bw, bh];
            head_c = [hx, hy, hw, hh];
            // smooth skin, not fur: a flat dark membrane with the bones showing through
            let membrane = mix(pal.shade, pal.deep, 0.6);
            let bone = mix(pal.shade, pal.light, 0.35);
            // one wing (left), mirrored for the right: arm to the wrist, three fingers, scalloped membrane
            // the forearm rises from the shoulder to a high wrist; three long fingers fan out from
            // the wrist; the membrane stretches between them and sags back towards the wrist
            let sh = (bx - bw * 0.4, by - bh * 0.2);
            let wrist = (bx - span * 0.45, by - wh * 0.8);
            let tips = [(bx - span, by - wh * 0.45), (bx - span * 0.95, by + wh * 0.12), (bx - span * 0.62, by + wh * 0.38)];
            let foot = (bx - bw * 0.3, by + bh * 0.4);
            let mut mem = vec![sh, wrist, tips[0]];
            let edge = [tips[0], tips[1], tips[2], foot];
            for w in edge.windows(2) {
                let m = ((w[0].0 + w[1].0) / 2.0, (w[0].1 + w[1].1) / 2.0);
                let mid = (m.0 + (wrist.0 - m.0) * 0.4, m.1 + (wrist.1 - m.1) * 0.4);
                mem.extend(curve(w[0], mid, w[1], 4).into_iter().skip(1));
            }
            let wing = Shape::Poly(mem);
            let mirror = |s: &Shape| -> Shape {
                match s {
                    Shape::Poly(p) => Shape::Poly(p.iter().map(|&(x, y)| (2.0 * bx - x, y)).rev().collect()),
                    o => o.clone(),
                }
            };
            b.add("membrane", Group::WingL, -1, wing.clone(), membrane);
            b.add("membrane", Group::WingR, -1, mirror(&wing), membrane);
            let arm = limb(sh.0, sh.1, wrist.0, wrist.1, 0.04, 0.03);
            b.detail("arm", Group::WingL, 0, arm.clone(), bone);
            b.detail("arm", Group::WingR, 0, mirror(&arm), bone);
            for t in tips {
                let f = limb(wrist.0, wrist.1, t.0 + (wrist.0 - t.0) * 0.06, t.1 + (wrist.1 - t.1) * 0.06, 0.022, 0.014);
                b.detail("finger", Group::WingL, 0, f.clone(), bone);
                b.detail("finger", Group::WingR, 0, mirror(&f), bone);
            }
            b.detail("thumb", Group::WingL, 0, oval(wrist.0 + 0.01, wrist.1 - 0.025, 0.02, 0.014, 0.0), bone);
            b.detail("thumb", Group::WingR, 0, oval(2.0 * bx - wrist.0 - 0.01, wrist.1 - 0.025, 0.02, 0.014, 0.0), bone);
            pivots.insert(Group::WingL, sh);
            pivots.insert(Group::WingR, (2.0 * bx - sh.0, sh.1));
            has_wings = true;
            // furry body and head, big ears, small snout, bead eyes, feet
            b.solid("body", Group::Body, 1, blob(bx, by, bw / 2.0, bh / 2.0, &[(90.0, -0.05), (0.0, 0.05), (180.0, 0.05)]), main, Some(boxed(body_c)));
            for s in [-1.0, 1.0] {
                b.solid("ear", Group::Body, 1, Shape::Poly(vec![(hx + s * hw * 0.1, hy - hh * 0.3), (hx + s * (hw * 0.1 + ew), hy - hh * 0.2), (hx + s * (hw * 0.25 + ew * 0.4), hy - hh * 0.35 - eh)]), main, None);
                b.add("inner_ear", Group::Body, 2, Shape::Poly(vec![(hx + s * hw * 0.2, hy - hh * 0.32), (hx + s * (hw * 0.1 + ew * 0.7), hy - hh * 0.26), (hx + s * (hw * 0.25 + ew * 0.35), hy - hh * 0.3 - eh * 0.7)]), mix(color("pink"), main, 0.5));
                b.detail("foot", Group::Body, 0, oval(bx + s * bw * 0.22, by + bh * 0.55, 0.018, 0.022, 0.0), bone);
            }
            b.solid("head", Group::Body, 2, oval(hx, hy, hw / 2.0, hh / 2.0, 0.0), main, Some(boxed(head_c)));
            b.add("snout", Group::Body, 3, oval(hx, hy + hh * 0.22, hw * 0.24, hh * 0.18, 0.0), pal.light);
            b.detail("nose", Group::Body, 4, Shape::Rect { x0: hx - hw * 0.08, y0: hy + hh * 0.14, x1: hx + hw * 0.08, y1: hy + hh * 0.22 }, pal.deep);
            for s in [-1.0, 1.0] {
                b.detail("eye", Group::Body, 4, Shape::Ellipse { cx: hx + s * hw * 0.22, cy: hy - hh * 0.04, rx: hw * 0.1, ry: hw * 0.1 }, cfg.palette_rules.eye_dark).show = Show::EyesOpen;
                b.add("catchlight", Group::Body, 5, Shape::Rect { x0: hx + s * hw * 0.22 - 0.012, y0: hy - hh * 0.1, x1: hx + s * hw * 0.22, y1: hy - hh * 0.06 }, cfg.palette_rules.eye_white).show = Show::EyesOpen;
            }
            if req.has(Feature::Fangs).is_some() || chosen.get("mouth").copied() == Some("fangs") {
                for s in [-1.0, 1.0] {
                    b.detail("fang", Group::Body, 5, Shape::Poly(vec![(hx + s * hw * 0.04, hy + hh * 0.3), (hx + s * hw * 0.12, hy + hh * 0.3), (hx + s * hw * 0.08, hy + hh * 0.42)]), cfg.palette_rules.eye_white);
                }
            }
            has_eyes = true;
            symmetric = true;
        }
        Subject::Knight | Subject::Robot => {
            let robot = subj == Subject::Robot;
            let bd = d("body");
            let hd = d("head");
            let (lw, ll) = (d("leg")[0], d("leg")[1]);
            let (aw, al) = (d("arm")[0], d("arm")[1]);
            let [bx, by, bw, bh] = [bd[0], bd[1], bd[2], bd[3]];
            let [hx, hy, hw, hh] = [hd[0], hd[1], hd[2], hd[3]];
            body_c = [bx, by, bw, bh];
            head_c = [hx, hy, hw, hh];
            let dark = if robot { mix(main, color("black"), 0.55) } else { color("black") };
            let hip_y = by + bh * 0.48;
            // legs: thigh, knee, shin, foot
            for (g, s) in [(Group::LegL, -1.0), (Group::LegR, 1.0)] {
                let hip = (bx + s * bw * 0.22, hip_y);
                let knee = (hip.0, hip.1 + ll * 0.5);
                let ankle = (hip.0, hip.1 + ll);
                b.solid("thigh", g, 0, limb(hip.0, hip.1, knee.0, knee.1, lw * 1.1, lw), main, None);
                b.solid("shin", g, 0, limb(knee.0, knee.1, ankle.0, ankle.1, lw, lw * 0.85), main, None);
                b.solid("knee", g, 1, Shape::Ellipse { cx: knee.0, cy: knee.1, rx: lw * 0.62, ry: lw * 0.5 }, if robot { dark } else { pal.light }, None);
                b.solid("foot", g, 1, Shape::Poly(vec![(ankle.0 - lw * 0.6, ankle.1 - lw * 0.2), (ankle.0 + lw * 0.6, ankle.1 - lw * 0.2), (ankle.0 + lw * 0.8, ankle.1 + lw * 0.45), (ankle.0 - lw * 0.7, ankle.1 + lw * 0.45)]), dark, None);
                pivots.insert(g, hip);
            }
            // torso: tapered breastplate / chassis
            let torso = Shape::Poly(vec![(bx - bw * 0.5, by - bh * 0.45), (bx + bw * 0.5, by - bh * 0.45), (bx + bw * 0.38, by + bh * 0.38), (bx + bw * 0.3, by + bh * 0.55), (bx - bw * 0.3, by + bh * 0.55), (bx - bw * 0.38, by + bh * 0.38)]);
            b.solid("body", Group::Body, 1, torso, main, Some(boxed(body_c)));
            if robot {
                b.add("panel", Group::Body, 2, Shape::Rect { x0: bx - bw * 0.28, y0: by - bh * 0.3, x1: bx + bw * 0.28, y1: by + bh * 0.2 }, pal.shade).clip = Some("body");
                let core = cfg.colors[cfg.anatomy_iris.get("robot").map(|s| s.as_str()).unwrap_or("teal")];
                b.detail("core", Group::Body, 3, Shape::Ellipse { cx: bx, cy: by - bh * 0.05, rx: bw * 0.13, ry: bw * 0.13 }, core);
                b.detail("core_glow", Group::Body, 4, Shape::Ellipse { cx: bx - bw * 0.04, cy: by - bh * 0.09, rx: bw * 0.05, ry: bw * 0.05 }, mix(core, [1.0; 3], 0.6));
                for k in 0..3 {
                    let yv = by + bh * (0.28 + 0.07 * k as f32);
                    b.detail("vent", Group::Body, 2, Shape::Rect { x0: bx - bw * 0.2, y0: yv, x1: bx + bw * 0.2, y1: yv + 0.012 }, dark);
                }
            } else {
                // breastplate ridge, plackart line, belt with buckle, tassets
                b.add("ridge", Group::Body, 2, Shape::Rect { x0: bx - 0.008, y0: by - bh * 0.4, x1: bx + 0.012, y1: by + bh * 0.25 }, pal.light).clip = Some("body");
                b.add("plackart", Group::Body, 2, band(&curve((bx - bw * 0.36, by + bh * 0.05), (bx, by + bh * 0.2), (bx + bw * 0.36, by + bh * 0.05), 6), 0.012, 0.012), pal.shade).clip = Some("body");
                b.solid("belt", Group::Body, 3, Shape::Rect { x0: bx - bw * 0.36, y0: by + bh * 0.34, x1: bx + bw * 0.36, y1: by + bh * 0.44 }, color("brown"), None);
                b.detail("buckle", Group::Body, 4, Shape::Rect { x0: bx - bw * 0.06, y0: by + bh * 0.34, x1: bx + bw * 0.06, y1: by + bh * 0.44 }, color("gold"));
                for s in [-1.0, 1.0] {
                    b.solid("tasset", Group::Body, 2, Shape::Poly(vec![(bx + s * bw * 0.05, by + bh * 0.44), (bx + s * bw * 0.36, by + bh * 0.44), (bx + s * bw * 0.34, by + bh * 0.66), (bx + s * bw * 0.08, by + bh * 0.62)]), main, None);
                }
            }
            // arms: shoulder, upper arm, elbow, forearm, hand
            for (g, s) in [(Group::ArmL, -1.0), (Group::ArmR, 1.0)] {
                let sh = (bx + s * bw * 0.56, by - bh * 0.38);
                let elbow = (sh.0 + s * aw * 0.3, sh.1 + al * 0.5);
                let hand = (sh.0 + s * aw * 0.2, sh.1 + al);
                b.solid("upper_arm", g, 2, limb(sh.0, sh.1, elbow.0, elbow.1, aw, aw * 0.9), main, None);
                b.solid("forearm", g, 2, limb(elbow.0, elbow.1, hand.0, hand.1, aw * 0.9, aw * 0.85), main, None);
                b.solid("elbow", g, 3, Shape::Ellipse { cx: elbow.0, cy: elbow.1, rx: aw * 0.55, ry: aw * 0.5 }, if robot { dark } else { pal.light }, None);
                b.solid("hand", g, 3, Shape::Ellipse { cx: hand.0, cy: hand.1 + aw * 0.2, rx: aw * 0.6, ry: aw * 0.6 }, dark, None);
                // pauldron / shoulder joint
                b.solid("shoulder", g, 4, Shape::Ellipse { cx: sh.0, cy: sh.1 + aw * 0.1, rx: aw * 1.0, ry: aw * 0.8 }, if robot { dark } else { main }, None);
                pivots.insert(g, sh);
                if !robot && s > 0.0 {
                    // sword in the right hand, pointing down and out
                    let grip = (hand.0, hand.1 + aw * 0.2);
                    b.solid("blade", g, 5, limb(grip.0 + aw * 0.1, grip.1 + aw * 0.6, grip.0 + aw * 0.9, grip.1 + al * 1.3, aw * 0.5, aw * 0.25), color("silver"), None);
                    b.detail("guard", g, 6, limb(grip.0 - aw * 0.6, grip.1 + aw * 0.45, grip.0 + aw * 0.8, grip.1 + aw * 0.65, aw * 0.3, aw * 0.3), color("gold"));
                    b.detail("pommel", g, 6, Shape::Ellipse { cx: grip.0 - aw * 0.1, cy: grip.1 - aw * 0.45, rx: aw * 0.25, ry: aw * 0.25 }, color("gold"));
                }
                if !robot && s < 0.0 {
                    // kite shield on the left forearm with a painted emblem
                    let (sx, sy) = (elbow.0 - aw * 0.4, elbow.1 + al * 0.1);
                    let (sw2, sh2) = (bw * 0.42, bh * 0.75);
                    let shield = Shape::Poly(vec![(sx - sw2, sy - sh2 * 0.45), (sx + sw2, sy - sh2 * 0.45), (sx + sw2 * 0.9, sy + sh2 * 0.1), (sx, sy + sh2 * 0.55), (sx - sw2 * 0.9, sy + sh2 * 0.1)]);
                    let sc = req.has(Feature::Crown).map(|_| color("gold")).unwrap_or(color("red"));
                    b.solid("shield", g, 6, shield, sc, None);
                    b.add("emblem", g, 7, Shape::Rect { x0: sx - sw2 * 0.12, y0: sy - sh2 * 0.35, x1: sx + sw2 * 0.12, y1: sy + sh2 * 0.35 }, color("gold")).clip = Some("shield");
                    b.add("emblem", g, 7, Shape::Rect { x0: sx - sw2 * 0.7, y0: sy - sh2 * 0.12, x1: sx + sw2 * 0.7, y1: sy + sh2 * 0.02 }, color("gold")).clip = Some("shield");
                }
            }
            // neck and head
            b.solid("neck", Group::Body, 0, Shape::Rect { x0: hx - hw * 0.25, y0: hy + hh * 0.3, x1: hx + hw * 0.25, y1: by - bh * 0.4 }, dark, None);
            if robot {
                b.solid("head", Group::Body, 3, Shape::Rect { x0: hx - hw / 2.0, y0: hy - hh / 2.0, x1: hx + hw / 2.0, y1: hy + hh / 2.0 }, main, Some(boxed(head_c)));
                let glow = cfg.colors[cfg.anatomy_iris.get("robot").map(|s| s.as_str()).unwrap_or("teal")];
                b.detail("visor", Group::Body, 4, Shape::Rect { x0: hx - hw * 0.38, y0: hy - hh * 0.16, x1: hx + hw * 0.38, y1: hy + hh * 0.12 }, dark);
                for s in [-1.0, 1.0] {
                    b.detail("eye", Group::Body, 5, Shape::Rect { x0: hx + s * hw * 0.2 - hw * 0.08, y0: hy - hh * 0.08, x1: hx + s * hw * 0.2 + hw * 0.08, y1: hy + hh * 0.04 }, glow).show = Show::EyesOpen;
                }
                b.detail("jaw", Group::Body, 4, Shape::Rect { x0: hx - hw * 0.25, y0: hy + hh * 0.26, x1: hx + hw * 0.25, y1: hy + hh * 0.32 }, dark);
            } else {
                // great helm: flat top, rounded, eye slit, breathing holes, plume
                b.solid("head", Group::Body, 3, Shape::Egg { cx: hx, cy: hy - hh * 0.05, rx: hw / 2.0, ry_top: hh * 0.45, ry_bottom: hh * 0.55 }, main, Some(boxed(head_c)));
                b.detail("slit", Group::Body, 4, Shape::Rect { x0: hx - hw * 0.4, y0: hy - hh * 0.12, x1: hx + hw * 0.4, y1: hy - hh * 0.03 }, dark);
                b.add("nasal", Group::Body, 5, Shape::Rect { x0: hx - hw * 0.05, y0: hy - hh * 0.2, x1: hx + hw * 0.05, y1: hy + hh * 0.3 }, pal.light).clip = Some("head");
                for k in 0..3 {
                    b.detail("breath", Group::Body, 4, Shape::Rect { x0: hx + hw * (0.12 + 0.1 * k as f32), y0: hy + hh * 0.18, x1: hx + hw * (0.16 + 0.1 * k as f32), y1: hy + hh * 0.22 }, dark);
                }
                let plume = curve((hx - hw * 0.1, hy - hh * 0.48), (hx + hw * 0.1, hy - hh * 1.0), (hx - hw * 0.55, hy - hh * 0.75), 8);
                b.solid("plume", Group::Body, 2, band(&plume, hh * 0.22, hh * 0.1), color("red"), None);
            }
            has_eyes = robot;
            gait = Gait::Biped;
        }
        Subject::Slime => {
            // a gelatinous mound: sagging base, lopsided top, a clear core, bubbles, a drip
            let bd = d("body");
            let [bx, by, bw, bh] = [bd[0], bd[1], bd[2], bd[3]];
            body_c = [bx, by, bw, bh];
            head_c = body_c;
            let body = blob(bx, by, bw / 2.0, bh / 2.0, &[(90.0, 0.14), (270.0, -0.1), (235.0, 0.1), (20.0, 0.12), (160.0, 0.1)]);
            b.solid("body", Group::Body, 0, body, main, Some(boxed(body_c)));
            b.add("core", Group::Body, 1, oval(bx - bw * 0.05, by + bh * 0.05, bw * 0.28, bh * 0.25, -10.0), mix(main, pal.light, 0.55)).clip = Some("body");
            b.add("puddle", Group::Body, -1, oval(bx, by + bh * 0.46, bw * 0.55, bh * 0.07, 0.0), pal.shade);
            b.solid("drip", Group::Body, 0, limb(bx + bw * 0.3, by + bh * 0.2, bx + bw * 0.33, by + bh * 0.5, bw * 0.09, bw * 0.06), main, Some(boxed(body_c)));
            for (dx, dy, r) in [(-0.2, -0.05, 0.05), (0.12, 0.12, 0.035), (0.05, -0.22, 0.03)] {
                b.add("bubble", Group::Body, 2, Shape::Ellipse { cx: bx + bw * dx, cy: by + bh * dy, rx: r, ry: r }, mix(pal.light, [1.0; 3], 0.4)).clip = Some("body");
                b.add("bubble_rim", Group::Body, 3, Shape::Rect { x0: bx + bw * dx - r * 0.2, y0: by + bh * dy + r * 0.4, x1: bx + bw * dx + r, y1: by + bh * dy + r }, pal.shade).clip = Some("body");
            }
            b.add("gleam", Group::Body, 3, band(&curve((bx - bw * 0.3, by - bh * 0.2), (bx - bw * 0.2, by - bh * 0.4), (bx + bw * 0.02, by - bh * 0.42), 6), 0.03, 0.015), mix(pal.spec, [1.0; 3], 0.5)).clip = Some("body");
            if req.features.iter().any(|f| crate::rules::FACE_FEATURES.contains(&f.feature.v)) {
                for s in [-1.0, 1.0] {
                    eye(&mut b, bx + s * bw * 0.14, by - bh * 0.12, bh * 0.08, eye_col, "round", main, angry, cfg);
                }
                has_eyes = true;
            }
            symmetric = false;
        }
        Subject::Ghost => {
            // a draped sheet: domed head, widening folds, a ragged hem; hollow eye sockets
            let bd = d("body");
            let [bx, by, bw, bh] = [bd[0], bd[1], bd[2], bd[3]];
            body_c = [bx, by, bw, bh];
            head_c = [bx, by - bh * 0.28, bw * 0.6, bh * 0.4];
            let top = by - bh / 2.0;
            let r = bw * 0.3;
            let mut pts: Vec<(f32, f32)> = (0..=12).map(|k| {
                let a = PI * (1.0 + k as f32 / 12.0);
                (bx + r * a.cos(), top + r + r * a.sin())
            }).collect();
            // right side flares out
            pts.extend(curve((bx + r, top + r), (bx + bw * 0.42, by), (bx + bw * 0.5, by + bh * 0.42), 6).into_iter().skip(1));
            // ragged hem, right to left
            let hem = [(0.5, 0.42), (0.34, 0.5), (0.2, 0.4), (0.04, 0.5), (-0.12, 0.38), (-0.28, 0.48), (-0.44, 0.36), (-0.5, 0.44)];
            for (hx, hy) in hem {
                pts.push((bx + bw * hx, by + bh * hy));
            }
            pts.extend(curve((bx - bw * 0.5, by + bh * 0.44), (bx - bw * 0.42, by), (bx - r, top + r), 6).into_iter().skip(1));
            b.solid("body", Group::Body, 0, Shape::Poly(pts), main, Some(boxed(body_c)));
            for (x0, x1) in [(-0.1, -0.28), (0.12, 0.3), (0.0, 0.02)] {
                b.add("fold", Group::Body, 1, band(&curve((bx + bw * x0, by - bh * 0.05), (bx + bw * (x0 + x1) / 2.0, by + bh * 0.2), (bx + bw * x1, by + bh * 0.46), 6), 0.02, 0.035), pal.shade).clip = Some("body");
            }
            for s in [-1.0, 1.0] {
                b.detail("socket", Group::Body, 2, oval(bx + s * bw * 0.12, top + r * 0.95, bw * 0.07, bh * 0.07, s * -12.0), pal.deep).show = Show::EyesOpen;
                b.detail("eye_closed", Group::Body, 2, Shape::Rect { x0: bx + s * bw * 0.12 - bw * 0.06, y0: top + r * 0.95, x1: bx + s * bw * 0.12 + bw * 0.06, y1: top + r * 0.95 + 0.02 }, pal.deep).show = Show::EyesClosed;
                if let Some(c) = req.has(Feature::Eyes).and_then(|f| f.color.as_ref()) {
                    b.detail("glow", Group::Body, 3, Shape::Ellipse { cx: bx + s * bw * 0.12, cy: top + r * 0.98, rx: bw * 0.025, ry: bw * 0.025 }, cfg.colors[c.v.0.name()]).show = Show::EyesOpen;
                }
            }
            b.detail("mouth", Group::Body, 2, oval(bx, top + r * 1.55, bw * 0.06, bh * 0.05, 0.0), pal.deep);
            has_eyes = true;
        }
        Subject::Mushroom => {
            // a fly agaric: cap with warts, gills underneath, a ring, a bulb at the foot
            let cp = d("cap");
            let st = d("stem");
            let [cx, cy, cw, ch] = [cp[0], cp[1], cp[2], cp[3]];
            let (sw, sh) = (st[0], st[1]);
            body_c = [cx, cy, cw, ch];
            head_c = body_c;
            let rim = cy + ch * 0.35;
            let stem_c = mix(color("white"), color("yellow"), 0.12);
            b.solid("stem", Group::Body, -1, Shape::Poly(vec![(cx - sw * 0.42, rim), (cx + sw * 0.42, rim), (cx + sw * 0.5, rim + sh * 0.85), (cx - sw * 0.5, rim + sh * 0.85)]), stem_c, None);
            b.solid("bulb", Group::Body, -1, oval(cx, rim + sh * 0.88, sw * 0.62, sh * 0.14, 0.0), stem_c, None);
            b.solid("ring", Group::Body, 0, Shape::Poly(vec![(cx - sw * 0.5, rim + sh * 0.2), (cx + sw * 0.5, rim + sh * 0.2), (cx + sw * 0.66, rim + sh * 0.34), (cx - sw * 0.66, rim + sh * 0.34)]), stem_c, None);
            b.solid("gills", Group::Body, 0, oval(cx, rim, cw * 0.46, ch * 0.12, 0.0), mix(stem_c, color("brown"), 0.25), None);
            for k in -4..=4 {
                b.add("gill_line", Group::Body, 1, Shape::Rect { x0: cx + cw * 0.05 * k as f32, y0: rim - ch * 0.05, x1: cx + cw * 0.05 * k as f32 + 0.012, y1: rim + ch * 0.12 }, mix(stem_c, color("brown"), 0.5)).clip = Some("gills");
            }
            b.solid("body", Group::Body, 1, Shape::Egg { cx, cy: rim, rx: cw / 2.0, ry_top: ch * 0.95, ry_bottom: ch * 0.06 }, main, Some(boxed(body_c)));
            for (dx, dy, r) in [(-0.28, -0.45, 0.05), (0.02, -0.72, 0.045), (0.28, -0.4, 0.055), (-0.08, -0.25, 0.035), (0.16, -0.62, 0.03), (-0.38, -0.12, 0.03), (0.4, -0.1, 0.035)] {
                b.add("wart", Group::Body, 2, oval(cx + cw * dx, rim + ch * dy, r, r * 0.75, 15.0), stem_c).clip = Some("body");
            }
            symmetric = false;
        }
        Subject::Coin => {
            // three-quarter view: the edge shows below the face, a raised rim and an embossed star
            let bd = d("body");
            let [bx, by, bw, bh] = [bd[0], bd[1], bd[2], bd[3]];
            body_c = [bx, by, bw, bh];
            head_c = body_c;
            let edge = d("edge")[0];
            // turned a little: the milled edge shows as a band on the right
            b.solid("edge", Group::Body, 0, Shape::Ellipse { cx: bx + edge, cy: by, rx: bw / 2.0, ry: bh / 2.0 }, pal.shade, None);
            for k in 0..9 {
                let a = PI * (-0.4 + 0.8 * k as f32 / 8.0);
                let (ex, ey) = (bx + edge + bw / 2.0 * a.cos(), by + bh / 2.0 * a.sin());
                b.add("reed", Group::Body, 1, Shape::Rect { x0: ex - edge, y0: ey - 0.008, x1: ex, y1: ey + 0.008 }, pal.deep).clip = Some("edge");
            }
            b.solid("body", Group::Body, 2, Shape::Ellipse { cx: bx, cy: by, rx: bw / 2.0, ry: bh / 2.0 }, main, Some(boxed(body_c)));
            b.add("rim_in", Group::Body, 3, band(&(0..=24).map(|k| { let a = 2.0 * PI * k as f32 / 24.0; (bx + bw * 0.4 * a.cos(), by + bh * 0.4 * a.sin()) }).collect::<Vec<_>>(), 0.02, 0.02), pal.shade).clip = Some("body");
            let star: Vec<(f32, f32)> = (0..10).map(|k| {
                let a = (-90.0 + 36.0 * k as f32).to_radians();
                let r = if k % 2 == 0 { 0.26 } else { 0.11 };
                (bx + a.cos() * r * bw, by + a.sin() * r * bh)
            }).collect();
            // relief: the raised star catches the light up-left and throws a shadow down-right
            b.add("emboss_shadow", Group::Body, 4, Shape::Poly(star.iter().map(|&(x, y)| (x + 0.02, y + 0.025)).collect()), pal.deep).clip = Some("body");
            b.add("emboss", Group::Body, 5, Shape::Poly(star), pal.light).clip = Some("body");
            symmetric = false;
        }
        Subject::Gem => {
            // a brilliant cut: flat table, crown facets, girdle, pavilion facets to the culet
            let bd = d("body");
            let [bx, by, bw, bh] = [bd[0], bd[1], bd[2], bd[3]];
            body_c = [bx, by, bw, bh];
            head_c = body_c;
            let (top, girdle, culet) = (by - bh * 0.45, by - bh * 0.12, by + bh * 0.5);
            let (tw, gw) = (bw * 0.26, bw * 0.5);
            b.solid("body", Group::Body, 0, Shape::Poly(vec![(bx - tw, top), (bx + tw, top), (bx + gw, girdle), (bx, culet), (bx - gw, girdle)]), main, Some(boxed(body_c)));
            // crown: 4 facets, alternating light
            let crown = [(bx - gw, girdle), (bx - tw, top), (bx - tw * 0.2, girdle), (bx + tw, top), (bx + gw, girdle)];
            for (i, w) in crown.windows(2).enumerate() {
                let tri = vec![w[0], w[1], if i % 2 == 0 { (w[1].0 + 0.001, girdle) } else { (w[0].0, girdle) }];
                b.add("facet", Group::Body, 1, Shape::Poly(tri), if i % 2 == 0 { pal.light } else { main }).clip = Some("body");
            }
            b.add("table", Group::Body, 2, Shape::Poly(vec![(bx - tw * 0.85, top + bh * 0.02), (bx + tw * 0.85, top + bh * 0.02), (bx + tw * 0.6, top + bh * 0.12), (bx - tw * 0.6, top + bh * 0.12)]), pal.spec).clip = Some("body");
            // pavilion: fan of facets to the culet
            let xs = [-gw, -gw * 0.5, 0.0, gw * 0.5, gw];
            for (i, w) in xs.windows(2).enumerate() {
                let c = [pal.shade, main, pal.deep, pal.shade][i];
                b.add("facet", Group::Body, 1, Shape::Poly(vec![(bx + w[0], girdle), (bx + w[1], girdle), (bx, culet)]), c).clip = Some("body");
            }
            b.detail("girdle", Group::Body, 3, Shape::Rect { x0: bx - gw, y0: girdle - 0.008, x1: bx + gw, y1: girdle + 0.008 }, pal.light);
            b.detail("sparkle", Group::Body, 4, Shape::Rect { x0: bx - tw * 0.5, y0: top + bh * 0.04, x1: bx - tw * 0.3, y1: top + bh * 0.08 }, [1.0; 3]);
            symmetric = true;
        }
        Subject::Potion => {
            // a round-bottomed glass flask: glass, liquid with a meniscus, bubbles, cork, reflections
            let bd = d("body");
            let nk = d("neck");
            let [bx, by, bw, bh] = [bd[0], bd[1], bd[2], bd[3]];
            let (nw, nh) = (nk[0], nk[1]);
            body_c = [bx, by, bw, bh];
            head_c = body_c;
            let glass = mix(color("silver"), color("white"), 0.3);
            b.solid("neck", Group::Body, -1, Shape::Rect { x0: bx - nw / 2.0, y0: by - bh / 2.0 - nh, x1: bx + nw / 2.0, y1: by - bh * 0.4 }, glass, None);
            b.solid("lip", Group::Body, 0, Shape::Rect { x0: bx - nw * 0.62, y0: by - bh / 2.0 - nh, x1: bx + nw * 0.62, y1: by - bh / 2.0 - nh + nh * 0.18 }, glass, None);
            b.solid("cork", Group::Body, 1, Shape::Rect { x0: bx - nw * 0.4, y0: by - bh / 2.0 - nh * 1.4, x1: bx + nw * 0.4, y1: by - bh / 2.0 - nh * 0.9 }, color("brown"), None);
            b.solid("glass", Group::Body, 0, Shape::Ellipse { cx: bx, cy: by, rx: bw / 2.0, ry: bh / 2.0 }, glass, Some(boxed(body_c)));
            let level = by - bh * 0.08;
            let liquid = b.solid("body", Group::Body, 1, Shape::Rect { x0: bx - bw / 2.0, y0: level, x1: bx + bw / 2.0, y1: by + bh / 2.0 }, main, Some((bx - bw / 2.0, level, bx + bw / 2.0, by + bh / 2.0)));
            liquid.clip = Some("glass");
            b.add("meniscus", Group::Body, 2, Shape::Rect { x0: bx - bw / 2.0, y0: level, x1: bx + bw / 2.0, y1: level + 0.018 }, pal.light).clip = Some("body");
            for (dx, dy, r) in [(-0.12, 0.2, 0.025), (0.1, 0.1, 0.018), (0.02, 0.3, 0.02)] {
                b.add("bubble", Group::Body, 3, Shape::Ellipse { cx: bx + bw * dx, cy: level + bh * dy, rx: r, ry: r }, pal.spec).clip = Some("body");
            }
            b.add("reflection", Group::Body, 4, band(&curve((bx - bw * 0.36, by + bh * 0.1), (bx - bw * 0.38, by - bh * 0.25), (bx - bw * 0.12, by - bh * 0.4), 6), 0.035, 0.02), [1.0; 3]).clip = Some("glass");
            symmetric = false;
        }
        Subject::Human => {
            // A standing person seen from the front, about 7.5 heads tall, drawn for 128 pixels:
            // at that size a face, the fold of a coat and the scales on a claw are several pixels
            // each. The character's right side is on the viewer's left (s = -1). Clothes come
            // from the sentence; what isn't named gets plain everyday clothes.
            let bd = d("body");
            let hd = d("head");
            let (lw, ll) = (d("leg")[0], d("leg")[1]);
            let (aw, al) = (d("arm")[0], d("arm")[1]);
            let [bx, by, bw, bh] = [bd[0], bd[1], bd[2], bd[3]];
            let [hx, hy, hw, hh] = [hd[0], hd[1], hd[2], hd[3]];
            body_c = [bx, by, bw, bh];
            head_c = [hx, hy, hw, hh];
            let skin = main;
            let wear = |f: Feature, default: &str| -> Option<Rgb> {
                req.has(f).map(|x| {
                    let c = x.color.as_ref().map(|c| crate::figure::color_of(cfg, c.v.0, c.v.1)).unwrap_or(cfg.colors[default]);
                    // "dark green trousers" are nearly black: a dark tone goes further on cloth
                    let deep = x.color.as_ref().is_some_and(|c| c.v.1 == Some(Tone::Dark));
                    adjust(c, cfg.realism.cloth_value * if deep { cfg.tones.dark } else { 1.0 }, cfg.realism.cloth_saturation, 0.0)
                })
            };
            let dflt = |f: Feature, default: &str| wear(f, default).unwrap_or(cfg.colors[default]);
            let shirt = dflt(Feature::Shirt, "cream");
            let trousers = dflt(Feature::Trousers, "brown");
            let hair = req.has(Feature::Hair).and_then(|x| x.color.as_ref()).map(|c| crate::figure::color_of(cfg, c.v.0, c.v.1)).unwrap_or(cfg.colors["brown"]);
            let coat = wear(Feature::Coat, "brown");
            let boots = wear(Feature::Boots, "black");
            let belt = wear(Feature::Belt, "brown");
            let claw = req.has(Feature::Claw).map(|x| x.color.as_ref().map(|c| crate::figure::color_of(cfg, c.v.0, c.v.1)).unwrap_or(cfg.colors["black"]));
            let dark = |c: Rgb, v: f32| adjust(c, v, 1.0, 0.0);
            let (top, hip_y) = (by - bh / 2.0, by + bh / 2.0);
            let waist = top + bh * 0.82;
            let belt_h = bh * 0.08;
            let knee_y = hip_y + ll * 0.52;
            let ankle_y = hip_y + ll;
            let hem_y = hip_y + ll * 0.8;
            let coat_box = Some((bx - bw, top, bx + bw, hem_y));

            // ---- the coat's back and lining, behind the legs
            if let Some(c) = coat {
                b.solid("coat_back", Group::Body, -3, Shape::Poly(vec![(bx - bw * 0.6, top), (bx + bw * 0.6, top), (bx + bw * 0.95, hem_y + ll * 0.01), (bx - bw * 0.95, hem_y + ll * 0.01)]), dark(c, 0.62), coat_box);
                b.add("lining", Group::Body, -2, Shape::Poly(vec![(bx - bw * 0.42, waist), (bx + bw * 0.42, waist), (bx + bw * 0.55, hem_y), (bx - bw * 0.55, hem_y)]), dark(c, 0.42));
            }
            // ---- legs: trousers with a crease and a knee fold; boots or shoes
            for (g, s) in [(Group::LegL, -1.0), (Group::LegR, 1.0)] {
                let x = |f: f32| bx + s * f;
                let hip = (x(bw * 0.23), hip_y);
                let knee = (x(bw * 0.25), knee_y);
                let ankle = (x(bw * 0.29), ankle_y);
                b.solid("trouser", g, 0, limb(hip.0, hip.1, knee.0, knee.1, lw * 1.15, lw * 0.95), trousers, Some((hip.0 - lw, hip.1, hip.0 + lw, ankle.1)));
                b.solid("trouser", g, 0, limb(knee.0, knee.1, ankle.0, ankle.1, lw * 0.95, lw * 0.82), trousers, Some((hip.0 - lw, hip.1, hip.0 + lw, ankle.1)));
                b.add("crease", g, 1, band(&[(hip.0 + s * lw * 0.05, hip.1 + ll * 0.05), (knee.0, knee.1), (ankle.0, ankle.1)], 0.006, 0.006), dark(trousers, 0.72)).clip = Some("trouser");
                b.add("knee_fold", g, 1, band(&curve((knee.0 - lw * 0.35, knee.1 - ll * 0.02), (knee.0, knee.1 + ll * 0.015), (knee.0 + lw * 0.35, knee.1 - ll * 0.02), 5), 0.006, 0.006), dark(trousers, 0.7)).clip = Some("trouser");
                let (shaft_top, bc) = match boots {
                    Some(c) => (knee.1 + ll * 0.2, c),
                    None => (ankle.1 - lw * 0.25, cfg.colors["black"]),
                };
                let sole = ankle.1 + lw * 0.55;
                let toe = ankle.0 + s * lw * 0.55;
                b.solid("boot", g, 2, Shape::Poly(vec![
                    (ankle.0 - lw * 0.52, shaft_top), (ankle.0 + lw * 0.52, shaft_top),
                    (ankle.0 + lw * 0.47, ankle.1 - lw * 0.1),
                    (toe + lw * 0.55, ankle.1 + lw * 0.2), (toe + lw * 0.5, sole), (toe - lw * 0.75, sole),
                    (ankle.0 - lw * 0.5, ankle.1 - lw * 0.05),
                ]), bc, Some((ankle.0 - lw, shaft_top, ankle.0 + lw, sole)));
                b.add("sole", g, 3, Shape::Rect { x0: toe - lw * 1.0, y0: sole - lw * 0.14, x1: toe + lw * 0.7, y1: sole + 0.004 }, dark(bc, 0.45)).clip = Some("boot");
                if boots.is_some() {
                    b.add("boot_rim", g, 3, Shape::Rect { x0: ankle.0 - lw, y0: shaft_top, x1: ankle.0 + lw, y1: shaft_top + 0.008 }, adjust(bc, 1.7, 0.8, 0.0)).clip = Some("boot");
                    for k in 0..2 {
                        let y = ankle.1 - lw * (0.2 + 0.45 * k as f32);
                        b.add("boot_crease", g, 3, band(&curve((ankle.0 - lw * 0.45, y), (ankle.0, y + lw * 0.18), (ankle.0 + lw * 0.45, y), 4), 0.005, 0.005), adjust(bc, 1.5, 0.8, 0.0)).clip = Some("boot");
                    }
                    b.add("strap", g, 3, Shape::Rect { x0: ankle.0 - lw, y0: ankle.1 - lw * 0.05, x1: ankle.0 + lw, y1: ankle.1 + lw * 0.12 }, dark(bc, 0.6)).clip = Some("boot");
                    b.detail("strap_buckle", g, 4, Shape::Rect { x0: ankle.0 + s * lw * 0.3 - 0.006, y0: ankle.1 - lw * 0.08, x1: ankle.0 + s * lw * 0.3 + 0.006, y1: ankle.1 + lw * 0.15 }, cfg.colors["silver"]);
                    b.add("boot_shine", g, 3, Shape::Rect { x0: ankle.0 - lw * 0.35, y0: shaft_top + ll * 0.03, x1: ankle.0 - lw * 0.22, y1: ankle.1 - lw * 0.9 }, adjust(bc, 1.9, 0.7, 0.0)).clip = Some("boot");
                }
                pivots.insert(g, hip);
            }
            // ---- torso: the shirt, trousers from the belt down, open collar, placket, stripes
            b.solid("trouser_top", Group::Body, 1, Shape::Poly(vec![(bx - bw * 0.42, waist), (bx + bw * 0.42, waist), (bx + bw * 0.44, hip_y + ll * 0.04), (bx - bw * 0.44, hip_y + ll * 0.04)]), trousers, None);
            b.add("fly", Group::Body, 2, Shape::Rect { x0: bx - 0.003, y0: waist + belt_h, x1: bx + 0.004, y1: hip_y + ll * 0.03 }, dark(trousers, 0.7)).clip = Some("trouser_top");
            b.solid("body", Group::Body, 2, Shape::Poly(vec![(bx - bw * 0.5, top), (bx + bw * 0.5, top), (bx + bw * 0.4, waist + belt_h * 0.5), (bx - bw * 0.4, waist + belt_h * 0.5)]), shirt, Some(boxed(body_c)));
            if let Some(st) = req.has(Feature::Stripes) {
                let c = st.color.as_ref().map(|c| crate::figure::color_of(cfg, c.v.0, c.v.1)).unwrap_or(dark(shirt, 0.7));
                let c = mix(adjust(c, 0.9, cfg.realism.cloth_saturation * 0.8, 0.0), shirt, 0.5);
                for k in -5..=5 {
                    let x = bx + bw * 0.1 * k as f32 + bw * 0.03;
                    b.add("shirt_stripe", Group::Body, 3, Shape::Rect { x0: x - 0.0035, y0: top, x1: x + 0.0035, y1: waist + belt_h }, c).clip = Some("body");
                }
            }
            b.add("placket", Group::Body, 3, Shape::Rect { x0: bx - 0.004, y0: top + bh * 0.28, x1: bx + 0.005, y1: waist }, dark(shirt, 0.82)).clip = Some("body");
            for k in 0..3 {
                b.add("shirt_button", Group::Body, 4, Shape::Rect { x0: bx - 0.004, y0: top + bh * (0.36 + 0.17 * k as f32), x1: bx + 0.005, y1: top + bh * (0.36 + 0.17 * k as f32) + 0.007 }, adjust(shirt, 1.12, 0.6, 0.0)).clip = Some("body");
            }
            b.solid("collar_skin", Group::Body, 4, Shape::Poly(vec![(bx - bw * 0.15, top - 0.006), (bx + bw * 0.15, top - 0.006), (bx, top + bh * 0.27)]), skin, None);
            b.add("chest_shadow", Group::Body, 5, Shape::Poly(vec![(bx - bw * 0.06, top + bh * 0.12), (bx + bw * 0.08, top + bh * 0.12), (bx, top + bh * 0.26)]), dark(skin, 0.85)).clip = Some("collar_skin");
            for s in [-1.0, 1.0] {
                let x = |f: f32| bx + s * f;
                b.solid("shirt_collar", Group::Body, 7, Shape::Poly(vec![(x(bw * 0.12), top - 0.012), (x(bw * 0.3), top - 0.006), (x(bw * 0.2), top + bh * 0.16), (x(bw * 0.05), top + bh * 0.2)]), adjust(shirt, 1.06, 1.0, 0.0), None);
            }
            if let Some(c) = belt {
                b.solid("belt", Group::Body, 4, Shape::Rect { x0: bx - bw * 0.43, y0: waist, x1: bx + bw * 0.43, y1: waist + belt_h }, c, None);
                b.add("belt_edge", Group::Body, 5, Shape::Rect { x0: bx - bw, y0: waist, x1: bx + bw, y1: waist + 0.004 }, adjust(c, 1.3, 1.0, 0.0)).clip = Some("belt");
                for s in [-1.0, 1.0] {
                    b.add("belt_loop", Group::Body, 5, Shape::Rect { x0: bx + s * bw * 0.3 - 0.005, y0: waist - 0.002, x1: bx + s * bw * 0.3 + 0.005, y1: waist + belt_h + 0.002 }, dark(c, 0.7));
                }
                let gold = cfg.colors["gold"];
                b.solid("buckle", Group::Body, 5, Shape::Rect { x0: bx - bw * 0.1, y0: waist - belt_h * 0.15, x1: bx + bw * 0.1, y1: waist + belt_h * 1.15 }, gold, None);
                b.add("buckle_hole", Group::Body, 6, Shape::Rect { x0: bx - bw * 0.06, y0: waist + belt_h * 0.15, x1: bx + bw * 0.06, y1: waist + belt_h * 0.85 }, dark(c, 0.8)).clip = Some("buckle");
                b.add("prong", Group::Body, 7, Shape::Rect { x0: bx - bw * 0.07, y0: waist + belt_h * 0.4, x1: bx + bw * 0.03, y1: waist + belt_h * 0.62 }, gold).clip = Some("buckle");
            }
            // ---- the coat hangs open: collar, front panels, notched lapels, pocket flaps, folds
            if let Some(c) = coat {
                b.solid("coat_collar", Group::Body, 0, Shape::Poly(vec![(hx - hw * 0.62, top - bh * 0.18), (hx + hw * 0.62, top - bh * 0.18), (hx + hw * 0.75, top + 0.01), (hx - hw * 0.75, top + 0.01)]), dark(c, 0.8), None);
                for s in [-1.0, 1.0] {
                    let x = |f: f32| bx + s * f;
                    b.solid("coat", Group::Body, 5, Shape::Poly(vec![
                        (x(bw * 0.6), top), (x(bw * 0.18), top - 0.008), (x(bw * 0.36), top + bh * 0.5),
                        (x(bw * 0.43), waist), (x(bw * 0.5), hem_y), (x(bw * 0.95), hem_y + ll * 0.015),
                        (x(bw * 0.72), hip_y), (x(bw * 0.62), waist), (x(bw * 0.64), top + bh * 0.3),
                    ]), c, coat_box);
                    // notched lapel: upper part, the notch, the long lower part
                    b.solid("lapel", Group::Body, 6, Shape::Poly(vec![
                        (x(bw * 0.18), top - 0.008), (x(bw * 0.42), top - 0.004), (x(bw * 0.38), top + bh * 0.12),
                        (x(bw * 0.5), top + bh * 0.15), (x(bw * 0.4), top + bh * 0.5), (x(bw * 0.3), top + bh * 0.36),
                    ]), adjust(c, 1.12, 1.0, 0.0), None);
                    b.add("lapel_edge", Group::Body, 7, band(&[(x(bw * 0.19), top - 0.004), (x(bw * 0.37), top + bh * 0.48)], 0.005, 0.005), dark(c, 0.62)).clip = Some("lapel");
                    // pocket flap
                    let py = hip_y + ll * 0.06;
                    b.solid("flap", Group::Body, 7, Shape::Poly(vec![(x(bw * 0.52), py), (x(bw * 0.82), py + 0.004), (x(bw * 0.83), py + ll * 0.045), (x(bw * 0.53), py + ll * 0.04)]), dark(c, 0.92), None);
                    b.add("flap_shadow", Group::Body, 6, Shape::Rect { x0: x(bw * 0.5).min(x(bw * 0.85)), y0: py + ll * 0.045, x1: x(bw * 0.5).max(x(bw * 0.85)), y1: py + ll * 0.045 + 0.006 }, dark(c, 0.6)).clip = Some("coat");
                    // folds hanging from the waist to the hem, a highlight beside each
                    for (k, f) in [0.62, 0.78].iter().enumerate() {
                        let x0 = x(bw * (f - 0.02 * k as f32));
                        b.add("fold", Group::Body, 6, band(&curve((x0, waist + bh * 0.1), (x0 + s * bw * 0.02, hip_y + ll * 0.2), (x0 + s * bw * 0.08, hem_y), 6), 0.005, 0.009), dark(c, 0.68)).clip = Some("coat");
                        b.add("fold_light", Group::Body, 6, band(&curve((x0 - s * 0.008, waist + bh * 0.2), (x0 + s * bw * 0.01, hip_y + ll * 0.2), (x0 + s * bw * 0.06, hem_y - ll * 0.02), 6), 0.004, 0.006), adjust(c, 1.18, 1.0, 0.0)).clip = Some("coat");
                    }
                    b.add("hem", Group::Body, 6, Shape::Rect { x0: bx - bw, y0: hem_y - 0.006, x1: bx + bw, y1: hem_y + ll * 0.02 }, dark(c, 0.72)).clip = Some("coat");
                    if s > 0.0 {
                        for k in 0..3 {
                            let y = top + bh * (0.62 + 0.25 * k as f32);
                            b.detail("coat_button", Group::Body, 7, Shape::Ellipse { cx: x(bw * 0.47), cy: y, rx: 0.007, ry: 0.007 }, dark(c, 0.45));
                        }
                    }
                }
            }
            // ---- arms: coat (or shirt) sleeves with a seam, elbow folds and a cuff; a claw
            // replaces the right forearm and hand; with a coat the left hand is in the pocket
            let sleeve = coat.unwrap_or(shirt);
            for (g, s) in [(Group::ArmL, -1.0), (Group::ArmR, 1.0)] {
                let sh = (bx + s * bw * 0.55, top + bh * 0.06);
                let elbow = (sh.0 + s * aw * 0.45, sh.1 + al * 0.5);
                let is_claw = s < 0.0 && claw.is_some();
                let pocket = s > 0.0 && coat.is_some();
                let wrist = if pocket { (bx + s * bw * 0.42, hip_y + ll * 0.04) } else { (sh.0 + s * aw * 0.3, sh.1 + al) };
                let arm_box = Some((sh.0 - aw, sh.1, sh.0 + aw, wrist.1));
                b.solid("sleeve", g, 8, limb(sh.0, sh.1, elbow.0, elbow.1, aw * 1.15, aw), sleeve, arm_box);
                b.add("seam", g, 9, band(&curve((sh.0 - aw * 0.55, sh.1 + aw * 0.1), (sh.0, sh.1 + aw * 0.55), (sh.0 + aw * 0.55, sh.1 + aw * 0.1), 5), 0.005, 0.005), dark(sleeve, 0.7)).clip = Some("sleeve");
                if let Some(cc) = claw.filter(|_| is_claw) {
                    // the sleeve is pushed up to the elbow, the shirt cuff rolled under it
                    b.solid("cuff_roll", g, 9, oval(elbow.0, elbow.1, aw * 0.62, aw * 0.3, s * 8.0), shirt, None);
                    let w2 = (elbow.0 - s * aw * 0.05, elbow.1 + al * 0.42);
                    b.solid("claw_arm", g, 8, limb(elbow.0, elbow.1 + aw * 0.2, w2.0, w2.1, aw * 0.9, aw * 0.78), cc, Some((w2.0 - aw, elbow.1, w2.0 + aw, w2.1)));
                    // scales in staggered rows
                    for row in 0..9 {
                        let y = elbow.1 + aw * 0.35 + (w2.1 - elbow.1) * row as f32 / 9.0;
                        for col in -2..=2 {
                            let xs = elbow.0 - s * aw * 0.05 + aw * (0.22 * col as f32 + if row % 2 == 0 { 0.0 } else { 0.11 });
                            let arc = curve((xs - aw * 0.1, y), (xs, y + aw * 0.1), (xs + aw * 0.1, y), 3);
                            b.add("scale", g, 9, band(&arc, 0.004, 0.004), adjust(cc, 1.45, 0.8, 0.0)).clip = Some("claw_arm");
                        }
                    }
                    // hand: palm, knuckles, four jointed fingers ending in talons, a thumb
                    let palm = (w2.0, w2.1 + aw * 0.45);
                    b.solid("claw_hand", g, 9, oval(palm.0, palm.1, aw * 0.72, aw * 0.62, 0.0), cc, None);
                    for k in 0..4 {
                        let off = k as f32 - 1.5;
                        let base = (palm.0 + aw * 0.36 * off, palm.1 + aw * 0.45);
                        let mid = (base.0 + aw * 0.22 * off, base.1 + aw * 0.9);
                        let tip = (mid.0 + aw * 0.05 * off - s * aw * 0.12, mid.1 + aw * 0.95 - off.abs() * aw * 0.25);
                        b.solid("claw_finger", g, 9, limb(base.0, base.1, mid.0, mid.1, aw * 0.3, aw * 0.26), cc, None);
                        b.solid("claw_finger", g, 9, limb(mid.0, mid.1, tip.0, tip.1, aw * 0.26, aw * 0.18), cc, None);
                        b.detail("knuckle", g, 10, oval(base.0, base.1, aw * 0.1, aw * 0.07, 0.0), adjust(cc, 2.0, 0.6, 0.0));
                        let talon = curve(tip, (tip.0 - s * aw * 0.05, tip.1 + aw * 0.35), (tip.0 - s * aw * 0.25, tip.1 + aw * 0.55), 4);
                        b.solid("talon", g, 10, band(&talon, aw * 0.16, aw * 0.03), mix(cc, cfg.colors["grey"], 0.55), None);
                    }
                    let tb = (palm.0 - s * aw * 0.55, palm.1 - aw * 0.1);
                    b.solid("claw_finger", g, 9, limb(tb.0, tb.1, tb.0 - s * aw * 0.4, tb.1 + aw * 0.9, aw * 0.28, aw * 0.2), cc, None);
                } else {
                    b.solid("sleeve", g, 8, limb(elbow.0, elbow.1, wrist.0, wrist.1, aw, aw * 0.92), sleeve, arm_box);
                    for k in 0..2 {
                        let y = elbow.1 + aw * (0.25 * k as f32 - 0.1);
                        b.add("elbow_fold", g, 9, band(&curve((elbow.0 - aw * 0.4, y), (elbow.0, y + aw * 0.2), (elbow.0 + aw * 0.4, y), 4), 0.005, 0.005), dark(sleeve, 0.68)).clip = Some("sleeve");
                    }
                    b.solid("cuff", g, 9, limb(wrist.0 - s * aw * 0.02, wrist.1 - aw * 0.3, wrist.0, wrist.1, aw * 0.98, aw * 0.95), dark(sleeve, 0.85), None);
                    if !pocket {
                        let hc = (wrist.0, wrist.1 + aw * 0.6);
                        b.solid("hand", g, 9, oval(hc.0, hc.1, aw * 0.45, aw * 0.6, 0.0), skin, None);
                        b.add("finger_line", g, 10, Shape::Rect { x0: hc.0 - aw * 0.3, y0: hc.1 + aw * 0.1, x1: hc.0 + aw * 0.3, y1: hc.1 + aw * 0.1 + 0.005 }, dark(skin, 0.75)).clip = Some("hand");
                    }
                }
                pivots.insert(g, sh);
            }
            // ---- neck and head: a jaw, cheekbones, ears; eyes with brows; nose; lips; stubble
            b.solid("neck", Group::Body, 1, Shape::Rect { x0: hx - hw * 0.28, y0: hy + hh * 0.3, x1: hx + hw * 0.28, y1: top + 0.01 }, skin, None);
            b.add("neck_shadow", Group::Body, 2, Shape::Rect { x0: hx - hw, y0: hy + hh * 0.3, x1: hx + hw, y1: hy + hh * 0.55 }, dark(skin, 0.72)).clip = Some("neck");
            for s in [-1.0, 1.0] {
                let ex = hx + s * hw * 0.5;
                b.solid("ear", Group::Body, 8, oval(ex, hy + hh * 0.02, hw * 0.1, hh * 0.14, s * 10.0), skin, None);
                b.add("ear_inner", Group::Body, 9, oval(ex, hy + hh * 0.03, hw * 0.04, hh * 0.07, 0.0), dark(skin, 0.72)).clip = Some("ear");
            }
            let face = Shape::Poly(vec![
                (hx - hw * 0.5, hy - hh * 0.45), (hx + hw * 0.5, hy - hh * 0.45),
                (hx + hw * 0.5, hy + hh * 0.1), (hx + hw * 0.42, hy + hh * 0.3), (hx + hw * 0.18, hy + hh * 0.47),
                (hx, hy + hh * 0.5), (hx - hw * 0.18, hy + hh * 0.47), (hx - hw * 0.42, hy + hh * 0.3), (hx - hw * 0.5, hy + hh * 0.1),
            ]);
            b.solid("head", Group::Body, 9, face, skin, Some(boxed(head_c)));
            let hair_dark = dark(hair, 0.8);
            b.add("stubble", Group::Body, 10, Shape::Poly(vec![(hx - hw * 0.46, hy + hh * 0.18), (hx + hw * 0.46, hy + hh * 0.18), (hx + hw * 0.2, hy + hh * 0.5), (hx - hw * 0.2, hy + hh * 0.5)]), mix(skin, hair, 0.14)).clip = Some("head");
            b.add("cheek_shadow", Group::Body, 10, Shape::Poly(vec![(hx + hw * 0.3, hy + hh * 0.02), (hx + hw * 0.5, hy - hh * 0.02), (hx + hw * 0.5, hy + hh * 0.2), (hx + hw * 0.34, hy + hh * 0.2)]), dark(skin, 0.8)).clip = Some("head");
            let iris = eye_col;
            for s in [-1.0, 1.0] {
                let ex = hx + s * hw * 0.22;
                let ey = hy - hh * 0.06;
                b.detail("eye_white", Group::Body, 11, oval(ex, ey, hw * 0.13, hh * 0.035, 0.0), mix(cfg.palette_rules.eye_white, skin, 0.25)).show = Show::EyesOpen;
                b.detail("iris", Group::Body, 12, Shape::Rect { x0: ex - hw * 0.05 + s * hw * 0.02, y0: ey - hh * 0.035, x1: ex + hw * 0.05 + s * hw * 0.02, y1: ey + hh * 0.035 }, iris).show = Show::EyesOpen;
                b.detail("pupil", Group::Body, 13, Shape::Rect { x0: ex - hw * 0.022 + s * hw * 0.02, y0: ey - hh * 0.02, x1: ex + hw * 0.022 + s * hw * 0.02, y1: ey + hh * 0.02 }, cfg.palette_rules.eye_dark).show = Show::EyesOpen;
                b.detail("upper_lid", Group::Body, 13, band(&curve((ex - hw * 0.15, ey), (ex, ey - hh * 0.06), (ex + hw * 0.15, ey - hh * 0.01), 4), 0.006, 0.005), dark(skin, 0.45)).show = Show::EyesOpen;
                b.detail("eye_closed", Group::Body, 11, band(&[(ex - hw * 0.14, ey), (ex + hw * 0.14, ey)], 0.006, 0.006), dark(skin, 0.5)).show = Show::EyesClosed;
                // brows: thick, low and level (a serious look)
                b.detail("brow", Group::Body, 12, band(&[(ex - s * hw * 0.04, ey - hh * 0.1), (ex + s * hw * 0.17, ey - hh * 0.12)], 0.011, 0.008), hair_dark);
            }
            // nose: the lit side merges with the face; the shadow side and the tip carry it
            b.add("nose_shadow", Group::Body, 11, band(&[(hx + hw * 0.05, hy - hh * 0.04), (hx + hw * 0.08, hy + hh * 0.15)], 0.007, 0.01), dark(skin, 0.74)).clip = Some("head");
            b.add("nose_tip", Group::Body, 11, oval(hx + hw * 0.02, hy + hh * 0.17, hw * 0.09, hh * 0.03, 0.0), dark(skin, 0.8)).clip = Some("head");
            b.add("nostril", Group::Body, 12, Shape::Rect { x0: hx - hw * 0.06, y0: hy + hh * 0.17, x1: hx + hw * 0.1, y1: hy + hh * 0.19 }, dark(skin, 0.55)).clip = Some("head");
            b.add("lips", Group::Body, 11, band(&curve((hx - hw * 0.17, hy + hh * 0.29), (hx, hy + hh * 0.27), (hx + hw * 0.17, hy + hh * 0.29), 4), 0.006, 0.006), mix(dark(skin, 0.6), cfg.colors["red"], 0.15)).clip = Some("head");
            b.add("lower_lip", Group::Body, 11, oval(hx, hy + hh * 0.325, hw * 0.1, hh * 0.018, 0.0), adjust(skin, 1.06, 1.1, 0.0)).clip = Some("head");
            b.add("chin_shadow", Group::Body, 11, band(&curve((hx - hw * 0.16, hy + hh * 0.45), (hx, hy + hh * 0.52), (hx + hw * 0.16, hy + hh * 0.45), 4), 0.006, 0.006), dark(skin, 0.7)).clip = Some("head");
            // hair: volume over the crown, locks falling on the forehead, sides over the ears
            let htop = hy - hh * 0.5;
            b.solid("hair", Group::Body, 14, blob(hx + hw * 0.03, htop + hh * 0.08, hw * 0.6, hh * 0.28, &[(195.0, 0.18), (230.0, 0.28), (265.0, 0.1), (300.0, 0.3), (335.0, 0.12), (160.0, 0.1), (15.0, 0.06)]), hair, Some((hx - hw * 0.6, htop - hh * 0.15, hx + hw * 0.6, hy)));
            let locks = [(-0.42, 0.22, -0.35, 0.42), (-0.2, 0.2, -0.12, 0.48), (0.02, 0.2, 0.14, 0.44), (0.22, 0.18, 0.4, 0.36), (0.4, 0.15, 0.52, 0.34)];
            for (x0, y0, x1, y1) in locks {
                b.solid("hair", Group::Body, 14, Shape::Poly(vec![(hx + hw * (x0 - 0.1), htop + hh * y0), (hx + hw * (x0 + 0.14), htop + hh * (y0 - 0.02)), (hx + hw * x1, htop + hh * y1)]), hair, Some((hx - hw * 0.6, htop - hh * 0.15, hx + hw * 0.6, hy)));
            }
            for s in [-1.0, 1.0] {
                b.solid("hair", Group::Body, 14, Shape::Poly(vec![(hx + s * hw * 0.42, htop + hh * 0.15), (hx + s * hw * 0.6, htop + hh * 0.2), (hx + s * hw * 0.56, hy + hh * 0.02), (hx + s * hw * 0.47, hy - hh * 0.05)]), hair, None);
            }
            let tufts = [(-0.55, 0.25, -0.75, 0.05), (-0.4, 0.05, -0.5, -0.2), (-0.15, -0.05, -0.2, -0.3), (0.1, -0.06, 0.18, -0.32), (0.32, 0.0, 0.5, -0.2), (0.5, 0.18, 0.72, 0.02), (0.55, 0.35, 0.68, 0.45)];
            for (x0, y0, tx, ty) in tufts {
                b.solid("hair", Group::Body, 14, Shape::Poly(vec![(hx + hw * (x0 - 0.13), htop + hh * (y0 + 0.08)), (hx + hw * (x0 + 0.13), htop + hh * (y0 + 0.08)), (hx + hw * tx, htop + hh * ty)]), hair, Some((hx - hw * 0.6, htop - hh * 0.15, hx + hw * 0.6, hy)));
            }
            // strands: light where the hair turns to the light, dark in the partings, all sweeping
            // back and to the side
            for (x0, y0, x1, y1) in [(-0.35, 0.05, -0.05, -0.12), (-0.1, 0.12, 0.2, -0.1), (0.15, 0.2, 0.42, 0.0), (-0.45, 0.25, -0.3, 0.02)] {
                b.add("hair_light", Group::Body, 15, band(&curve((hx + hw * x0, htop + hh * y0), (hx + hw * (x0 + x1) / 2.0 + hw * 0.05, htop + hh * (y0 + y1) / 2.0 - hh * 0.05), (hx + hw * x1, htop + hh * y1), 4), 0.006, 0.003), adjust(hair, 2.1, 0.6, 0.0)).clip = Some("hair");
            }
            for (x0, y0, x1, y1) in [(-0.22, 0.18, 0.02, -0.05), (0.05, 0.25, 0.3, 0.05), (0.3, 0.3, 0.5, 0.12)] {
                b.add("hair_dark", Group::Body, 15, band(&curve((hx + hw * x0, htop + hh * y0), (hx + hw * (x0 + x1) / 2.0, htop + hh * (y0 + y1) / 2.0 - hh * 0.04), (hx + hw * x1, htop + hh * y1), 4), 0.006, 0.003), dark(hair, 0.55)).clip = Some("hair");
            }
            has_eyes = true;
            gait = Gait::Biped;
        }
        Subject::Heart | Subject::Star => {
            // symbols stay symbols, built as solid bevelled objects
            let bd = d("body");
            let [bx, by, bw, bh] = [bd[0], bd[1], bd[2], bd[3]];
            body_c = [bx, by, bw, bh];
            head_c = body_c;
            if subj == Subject::Heart {
                let lobe = |s: f32| Shape::Ellipse { cx: bx + s * bw * 0.235, cy: by - bh * 0.22, rx: bw * 0.265, ry: bh * 0.28 };
                b.solid("body", Group::Body, 0, lobe(-1.0), main, Some(boxed(body_c)));
                b.solid("body", Group::Body, 0, lobe(1.0), main, Some(boxed(body_c)));
                b.solid("body", Group::Body, 0, Shape::Poly(vec![(bx - bw * 0.495, by - bh * 0.16), (bx + bw * 0.495, by - bh * 0.16), (bx, by + bh / 2.0)]), main, Some(boxed(body_c)));
                b.add("gloss", Group::Body, 2, oval(bx - bw * 0.25, by - bh * 0.28, bw * 0.08, bh * 0.05, -35.0), [1.0; 3]).clip = Some("body");
            } else {
                let pts: Vec<(f32, f32)> = (0..10).map(|k| {
                    let a = (-90.0 + 36.0 * k as f32).to_radians();
                    let r = if k % 2 == 0 { 0.5 } else { 0.22 };
                    (bx + a.cos() * r * bw, by + a.sin() * r * bh)
                }).collect();
                b.solid("body", Group::Body, 0, Shape::Poly(pts.clone()), main, Some(boxed(body_c)));
                // bevel: each arm split along its ridge, lit side and shadow side
                for k in 0..5 {
                    let tip = pts[2 * k];
                    let left = pts[(2 * k + 9) % 10];
                    let right = pts[2 * k + 1];
                    let lit_side = tip.0 + tip.1 < bx + by;
                    b.add("bevel", Group::Body, 1, Shape::Poly(vec![(bx, by), left, tip]), if lit_side { pal.light } else { main }).clip = Some("body");
                    b.add("bevel", Group::Body, 1, Shape::Poly(vec![(bx, by), tip, right]), if lit_side { main } else { pal.shade }).clip = Some("body");
                }
            }
            symmetric = true;
        }
    }

    // features that make sense on a realistic body, placed on the head
    let [hx, hy, hw, hh] = head_c;
    let htop = hy - hh / 2.0;
    for f in &req.features {
        let fc = |d: Rgb| f.color.as_ref().map(|c| cfg.colors[c.v.0.name()]).unwrap_or(d);
        match f.feature.v {
            Feature::Horns => {
                for s in [-1.0, 1.0] {
                    let base = (hx + s * hw * 0.25, htop + hh * 0.12);
                    let horn = curve(base, (base.0 + s * hw * 0.35, base.1 - hh * 0.3), (base.0 + s * hw * 0.15, base.1 - hh * 0.7), 6);
                    b.solid("horn", Group::Body, 6, band(&horn, hw * 0.16, hw * 0.03), fc(mix(color("white"), color("brown"), 0.25)), None);
                }
            }
            Feature::Crown => {
                let (x0, x1, yb, c) = (hx - hw * 0.28, hx + hw * 0.28, htop + hh * 0.08, hh * 0.35);
                b.solid("crown", Group::Body, 7, Shape::Poly(vec![(x0, yb), (x0, yb - c), (x0 + (x1 - x0) * 0.25, yb - c * 0.5), (hx, yb - c), (x1 - (x1 - x0) * 0.25, yb - c * 0.5), (x1, yb - c), (x1, yb)]), fc(color("gold")), None);
            }
            Feature::Wings if !has_wings => {
                let root = (body_c[0] - body_c[2] * 0.2, body_c[1] - body_c[3] * 0.3);
                let wing = Shape::Poly(vec![root, (root.0 - body_c[2] * 0.5, root.1 - body_c[3] * 0.9), (root.0 - body_c[2] * 0.2, root.1 - body_c[3] * 0.4), (root.0 - body_c[2] * 0.45, root.1 - body_c[3] * 0.5), (root.0 + body_c[2] * 0.05, root.1 + body_c[3] * 0.1)]);
                b.solid("wing", Group::WingL, -4, wing, fc(color("white")), None);
                pivots.insert(Group::WingL, root);
                has_wings = true;
            }
            Feature::Hat => {
                let c = fc(color("black"));
                // a person's hat sits over the hair
                let layer = if subj == Subject::Human { 16 } else { 7 };
                b.solid("brim", Group::Body, layer, oval(hx, htop + hh * 0.05, hw * 0.55, hh * 0.08, 0.0), c, None);
                b.solid("hat", Group::Body, layer, Shape::Rect { x0: hx - hw * 0.3, y0: htop - hh * 0.55, x1: hx + hw * 0.3, y1: htop + hh * 0.05 }, c, None);
            }
            Feature::Beanie => {
                // a knitted winter hat pulled down to the brows: crown, ribbed cuff, a bobble
                // knitted wool keeps more of its colour than woven cloth
                let c = adjust(fc(color("red")), cfg.realism.cloth_value.sqrt(), cfg.realism.cloth_saturation.sqrt(), 0.0);
                let cuff_y = hy - hh * 0.12;
                b.solid("beanie", Group::Body, 16, Shape::Egg { cx: hx, cy: cuff_y, rx: hw * 0.7, ry_top: hh * 0.8, ry_bottom: hh * 0.02 }, c, Some((hx - hw * 0.7, cuff_y - hh * 0.8, hx + hw * 0.7, cuff_y)));
                for k in -2..=2 {
                    let x = hx + hw * 0.2 * k as f32;
                    b.add("knit", Group::Body, 17, band(&curve((x, cuff_y - hh * 0.7), (x + hw * 0.03 * k as f32, cuff_y - hh * 0.3), (x + hw * 0.06 * k as f32, cuff_y), 4), 0.005, 0.006), adjust(c, 0.75, 1.0, 0.0)).clip = Some("beanie");
                }
                b.solid("beanie_cuff", Group::Body, 17, Shape::Rect { x0: hx - hw * 0.72, y0: cuff_y - hh * 0.2, x1: hx + hw * 0.72, y1: cuff_y + hh * 0.05 }, adjust(c, 0.88, 1.0, 0.0), None);
                for k in -6..=6 {
                    let x = hx + hw * 0.095 * k as f32;
                    b.add("rib", Group::Body, 18, Shape::Rect { x0: x - 0.003, y0: cuff_y - hh * 0.2, x1: x + 0.003, y1: cuff_y + hh * 0.05 }, adjust(c, 0.65, 1.0, 0.0)).clip = Some("beanie_cuff");
                }
                b.solid("bobble", Group::Body, 17, blob(hx, cuff_y - hh * 0.82, hh * 0.13, hh * 0.11, &[(30.0, 0.1), (150.0, 0.12), (260.0, 0.08)]), mix(color("white"), c, 0.15), None);
            }
            Feature::Antenna => {
                b.detail("antenna", Group::Body, -1, limb(hx, htop + hh * 0.05, hx + hw * 0.1, htop - hh * 0.5, 0.02, 0.015), color("grey"));
                b.detail("antenna_tip", Group::Body, 1, Shape::Ellipse { cx: hx + hw * 0.1, cy: htop - hh * 0.55, rx: 0.03, ry: 0.03 }, fc(color("red")));
            }
            Feature::Spots if subj != Subject::Human => {
                for (dx, dy) in [(-0.2, -0.1), (0.15, 0.0), (0.0, 0.2)] {
                    b.add("spot", Group::Body, 2, oval(body_c[0] + body_c[2] * dx, body_c[1] + body_c[3] * dy, 0.04, 0.03, 15.0), fc(pal.shade)).clip = Some("body");
                }
            }
            Feature::Stripes if subj != Subject::Human => {
                for k in 0..4 {
                    let x = body_c[0] - body_c[2] * 0.3 + body_c[2] * 0.2 * k as f32;
                    b.add("stripe", Group::Body, 2, limb(x, body_c[1] - body_c[3] * 0.5, x - 0.03, body_c[1] + body_c[3] * 0.3, 0.035, 0.02), fc(pal.shade)).clip = Some("body");
                }
            }
            _ => continue,
        }
        why.push((f.feature.v.name().into(), format!("\"{}\" (placed on the realistic anatomy)", f.feature.words)));
    }

    why.push(("anatomy".into(), format!("realistic {} (side view for animals, front for figures, three-quarter for objects)", subj.name())));
    let feet_y = b.parts.iter().map(|p| p.shape.bbox().3).fold(0.0f32, f32::max);
    Ok(Figure {
        parts: b.parts,
        pal,
        pivots,
        body: body_c,
        feet_y,
        symmetric,
        has_legs: !matches!(gait, Gait::None | Gait::Swim),
        has_wings,
        has_eyes,
        material: material_of(subj),
        realism: "realistic",
        gait,
        why,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::figure::Group;

    fn fig(sentence: &str) -> Figure {
        let cfg = Config::load(None).unwrap();
        let req = crate::parser::parse(sentence, false).unwrap();
        let mut chosen = Chosen::new();
        chosen.insert("realism", "realistic");
        crate::figure::build(&req, &cfg, &chosen).unwrap()
    }

    #[test]
    fn limb_covers_both_joints() {
        let s = limb(0.2, 0.2, 0.6, 0.5, 0.1, 0.06);
        assert!(s.contains(0.2, 0.2) && s.contains(0.6, 0.5) && s.contains(0.4, 0.35));
        assert!(!s.contains(0.2, 0.5));
    }

    #[test]
    fn a_realistic_cat_stands_on_four_jointed_legs_in_side_view() {
        let f = fig("a realistic cat");
        assert_eq!(f.gait, Gait::Quadruped);
        assert!(!f.symmetric);
        for g in [Group::LegFL, Group::LegFR, Group::LegBL, Group::LegBR] {
            assert!(f.parts.iter().filter(|p| p.group == g && p.role == "leg").count() == 2, "{g:?} has thigh and shin");
            assert!(f.pivots.contains_key(&g));
        }
        assert!(f.parts.iter().any(|p| p.role == "pupil"), "slit pupils");
    }

    #[test]
    fn a_realistic_knight_is_a_biped_with_arms_sword_and_shield() {
        let f = fig("a realistic knight");
        assert_eq!(f.gait, Gait::Biped);
        for role in ["blade", "shield", "knee", "elbow", "slit"] {
            assert!(f.parts.iter().any(|p| p.role == role), "{role}");
        }
        // about six heads tall: the head is a small share of the height
        let head = f.parts.iter().find(|p| p.role == "head").unwrap().shape.bbox();
        let feet = f.feet_y;
        assert!((head.3 - head.1) / (feet - head.1) < 0.2);
    }

    #[test]
    fn a_character_name_stands_for_its_description() {
        let cfg = Config::load(None).unwrap();
        let (s, who) = crate::expand_characters("Winston Black, walking", &cfg);
        assert_eq!(who.as_deref(), Some("winston black"));
        assert!(s.contains("claw arm") && s.ends_with(", walking"));
        let f = fig(&s);
        assert_eq!(f.gait, Gait::Biped);
        for role in ["coat", "lapel", "shirt_stripe", "belt", "buckle", "boot", "claw_hand", "talon", "hair"] {
            assert!(f.parts.iter().any(|p| p.role == role), "{role}");
        }
        // the left hand is in the pocket, the right is a claw: no human hands drawn
        assert!(!f.parts.iter().any(|p| p.role == "hand"));
    }

    #[test]
    fn a_dog_is_a_quadruped_with_a_muzzle_and_drop_ears() {
        let f = fig("a dog");
        assert_eq!(f.gait, Gait::Quadruped);
        for role in ["muzzle", "nose", "ear", "tail", "paw"] {
            assert!(f.parts.iter().any(|p| p.role == role), "{role}");
        }
        for g in [Group::LegFL, Group::LegFR, Group::LegBL, Group::LegBR] {
            assert!(f.pivots.contains_key(&g));
        }
    }

    #[test]
    fn a_plain_person_gets_everyday_clothes() {
        let f = fig("a man");
        assert!(f.parts.iter().filter(|p| p.role == "hand").count() == 2);
        assert!(!f.parts.iter().any(|p| p.role == "coat" || p.role == "talon"));
    }

    #[test]
    fn clothes_are_for_people() {
        let cfg = Config::load(None).unwrap();
        let req = crate::parser::parse("a slime with a coat", false).unwrap();
        let mut chosen = Chosen::new();
        chosen.insert("realism", "cartoon");
        assert!(crate::figure::build(&req, &cfg, &chosen).err().unwrap().contains("clothes are for people"));
    }

    #[test]
    fn cartoon_keeps_its_own_blueprints() {
        let cfg = Config::load(None).unwrap();
        let req = crate::parser::parse("a cat", false).unwrap();
        let mut chosen = Chosen::new();
        chosen.insert("realism", "cartoon");
        let f = crate::figure::build(&req, &cfg, &chosen).unwrap();
        assert_eq!(f.realism, "cartoon");
        assert!(f.parts.iter().all(|p| p.group != Group::LegFL));
    }
}
