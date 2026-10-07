//! Fish and fishing gear. Finnish lake fish are drawn from the side, facing right, at every
//! realism level: one blueprint, more detail the more realistic it is. Cartoon and stylized fish
//! take their body box from rules.toml [shapes.<fish>] (round / tall / wide) and get a big eye;
//! realistic fish take it from [anatomy.<fish>] and add fin rays, the lateral line, the gill
//! cover's rim and a nostril (scales come from the Scales material when the light is lit). The
//! rest of the body is in [anatomy.<fish>] as shares of the body's length and depth, colours in
//! [fish.<fish>]. The relations stay here: "the tail fin hangs on the tail root", "the pectoral
//! fin sits just behind the gill cover", "the dorsal fin rays sweep back".
//!
//! Groups: the front of the body is `Body`, the rear (behind `fish.bend_at`) is `Rear` so the
//! body can bend when it swims, the tail fin is `Tail`, the pectoral fin `Fin`, the pelvic fin
//! `Fin2`. Pivots: where each turns, and `Body` = the mouth (where a hook holds a fish).

use crate::anatomy::{band, curve, oval};
use crate::color::{adjust, mix, Rgb};
use crate::config::Config;
use crate::figure::{material_of, Figure, Gait, Group, Palette, Part, Shape, Show};
use crate::request::*;
use crate::rules::Chosen;
use std::collections::BTreeMap;
use std::f32::consts::PI;

/// The fish subjects (all share this blueprint).
pub const FISH: &[Subject] = &[Subject::Fish, Subject::Perch, Subject::Roach, Subject::Pike];

pub fn is_fish(s: Subject) -> bool {
    FISH.contains(&s)
}

struct B {
    parts: Vec<Part>,
}

impl B {
    fn add(&mut self, role: &'static str, group: Group, layer: i32, shape: Shape, color: Rgb) -> &mut Part {
        self.parts.push(Part { role, group, layer, shape, color, shaded: false, shade_box: None, clip: None, show: Show::Always, min_pixel: false });
        self.parts.last_mut().unwrap()
    }
}

/// The body's side outline. `t` runs from the snout (0) to the tail root (1).
struct Body {
    /// snout x, length, midline y, depth
    xs: f32,
    len: f32,
    cy: f32,
    d: f32,
    /// [back peak t, belly peak t, snout taper, tail-root depth, share above the midline, snout tip depth]
    p: [f32; 6],
}

impl Body {
    fn x(&self, t: f32) -> f32 {
        self.xs - self.len * t
    }
    /// 0..1: how deep the body is at `t` on one side, peaking at `peak`.
    fn f(&self, t: f32, peak: f32) -> f32 {
        let (taper, root, tip) = (self.p[2], self.p[3], self.p[5]);
        if t <= peak {
            let u = (peak - t) / peak;
            tip + (1.0 - tip) * (1.0 - u * u).max(0.0).powf(taper)
        } else {
            let u = ((t - peak) / (1.0 - peak)).min(1.0);
            root + (1.0 - root) * 0.5 * (1.0 + (PI * u).cos())
        }
    }
    fn top(&self, t: f32) -> f32 {
        self.cy - self.d * self.p[4] * self.f(t, self.p[0])
    }
    fn bot(&self, t: f32) -> f32 {
        self.cy + self.d * (1.0 - self.p[4]) * self.f(t, self.p[1])
    }
    fn mid(&self, t: f32) -> f32 {
        (self.top(t) + self.bot(t)) / 2.0
    }
    /// A band along the body between `a` and `b`, bounded by `top(t)` above and `bot(t)` below.
    fn band(&self, a: f32, b: f32, top: impl Fn(f32) -> f32, bot: impl Fn(f32) -> f32) -> Shape {
        let n = 28;
        let mut pts: Vec<(f32, f32)> = (0..=n).map(|k| a + (b - a) * k as f32 / n as f32).map(|t| (self.x(t), top(t))).collect();
        pts.extend((0..=n).rev().map(|k| a + (b - a) * k as f32 / n as f32).map(|t| (self.x(t), bot(t))));
        Shape::Poly(pts)
    }
    fn outline(&self, a: f32, b: f32) -> Shape {
        self.band(a, b, |t| self.top(t), |t| self.bot(t))
    }
    /// The whole body's box (for shading).
    fn bbox(&self) -> (f32, f32, f32, f32) {
        (self.x(1.0), self.cy - self.d * self.p[4], self.xs, self.cy + self.d * (1.0 - self.p[4]))
    }
}

/// A fin along the back (`up`) or the belly, from t0 to t1: its base sits inside the body, its
/// rays sweep back, the front is `h` high and the rear `rear` x h. A spiny fin's edge is notched
/// between the spines. Returns the fin and its rays (base, tip).
/// A fin ray from its base to its tip.
type Ray = ((f32, f32), (f32, f32));

fn fin(body: &Body, t0: f32, t1: f32, h: f32, rear: f32, up: bool, spines: usize) -> (Shape, Vec<Ray>) {
    let dir = if up { -1.0 } else { 1.0 };
    let edge_y = |t: f32| if up { body.top(t) } else { body.bot(t) };
    let height = |s: f32| {
        if s < 0.15 {
            h * (0.55 + 0.45 * s / 0.15)
        } else {
            h * (1.0 - (1.0 - rear) * (s - 0.15) / 0.85)
        }
    };
    let sweep = h * 0.45;
    let at = |s: f32| t0 + (t1 - t0) * s;
    let tip = |s: f32, k: f32| (body.x(at(s)) - sweep * k * height(s) / h, edge_y(at(s)) + dir * height(s) * k);
    let n = 10;
    let mut pts: Vec<(f32, f32)> = (0..=n).map(|i| {
        let t = at(i as f32 / n as f32);
        (body.x(t), edge_y(t) - dir * h * 0.35)
    }).collect();
    let mut rays = Vec::new();
    if spines > 0 {
        let m = spines;
        for j in (0..=m).rev() {
            let s = j as f32 / m as f32;
            if j < m {
                let sv = (j as f32 + 0.5) / m as f32;
                pts.push(tip(sv, 0.62));
            }
            pts.push(tip(s, 1.0));
        }
        for j in 0..=m {
            let s = j as f32 / m as f32;
            rays.push(((body.x(at(s)), edge_y(at(s))), tip(s, 1.0)));
        }
    } else {
        for i in (0..=n).rev() {
            pts.push(tip(i as f32 / n as f32, 1.0));
        }
        for j in 0..7 {
            let s = (j as f32 + 0.5) / 7.0;
            rays.push(((body.x(at(s)), edge_y(at(s))), tip(s, 0.92)));
        }
    }
    (Shape::Poly(pts), rays)
}

fn dims<'a>(cfg: &'a Config, s: Subject, key: &str) -> Option<&'a [f32]> {
    cfg.anatomy[s.name()].get(key).map(|v| v.as_slice())
}

pub fn build(req: &Request, cfg: &Config, chosen: &Chosen, pal: Palette, mut why: Vec<(String, String)>) -> Result<Figure, String> {
    let subj = req.subject.v;
    let realism = chosen.get("realism").copied().unwrap_or("cartoon");
    let real = realism == "realistic";
    let variant = chosen.get("body").copied().unwrap_or("round");
    let boxed = if real {
        let b = dims(cfg, subj, "body").expect("validated");
        [b[0], b[1], b[2], b[3]]
    } else {
        let s = &cfg.shapes.subjects[subj.name()];
        match variant {
            "tall" => s.tall,
            "wide" => s.wide,
            _ => s.round,
        }
    };
    // features a side-view fish (or a hook) can't carry are an honest error, not a guess
    for f in &req.features {
        let ok = matches!(f.feature.v, Feature::Stripes | Feature::Spots | Feature::Tail) || crate::rules::FACE_FEATURES.contains(&f.feature.v);
        if !ok || (matches!(subj, Subject::Hook | Subject::Lure) && matches!(f.feature.v, Feature::Stripes | Feature::Spots | Feature::Tail)) {
            return Err(format!("\"{}\" doesn't fit a {} (seen from the side); a fish takes stripes, spots and eye colours", f.feature.words, subj.name()));
        }
    }
    // one canvas pixel (before the fit, which changes it a little): fine lines stay at least
    // a pixel wide, and fine detail waits for a canvas that has room for it
    let size = cfg.size_of(req) as f32;
    let px = 1.0 / size;
    if subj == Subject::Hook {
        return hook(cfg, realism, pal, boxed, why);
    }
    if subj == Subject::Lure {
        return lure(cfg, realism, pal, boxed, px, why);
    }
    let d = |k: &str| dims(cfg, subj, k).expect("validated");
    let look = &cfg.fish.kinds[subj.name()];
    let main = pal.main;
    let [cx, cy, len, depth] = boxed;
    let pr = d("profile");
    let body = Body { xs: cx + len / 2.0, len, cy, d: depth, p: [pr[0], pr[1], pr[2], pr[3], pr[4], pr[5]] };
    let (hd, tl, df, an, fn_) = (d("head"), d("tail"), d("dorsal"), d("anal"), d("fin"));
    // light falls from above: the shading box reaches below the belly, so the belly stays pale
    let bb = body.bbox();
    let sbox = Some((bb.0, bb.1, bb.2, bb.3 + depth * cfg.fish.shade_drop));
    let detail = realism != "cartoon" && size >= cfg.fish.detail_min_size as f32;
    let line = |k: f32, min_px: f32| (len * k).max(px * min_px);
    let mut b = B { parts: vec![] };
    let mut pivots = BTreeMap::new();

    // ---- the bend: front and rear halves overlap around `bend_at`
    let split = cfg.fish.bend_at;
    let split_x = body.x(split);
    let group_at = |x: f32| if x < split_x { Group::Rear } else { Group::Body };
    let clip_of = |g: Group| if g == Group::Rear { "body_rear" } else { "body" };
    pivots.insert(Group::Rear, (split_x, body.mid(split)));

    // ---- colours: a dark back, the flank, a pale belly; fins in the kind's colours
    let back = mix(main, look.back, look.back_mix);
    let belly = mix(main, look.belly, 0.85);
    let fins_c = look.fins;
    let upper_c = mix(look.upper_fins, main, 0.2);
    let ray_c = |c: Rgb| adjust(c, 0.7, 1.05, 0.0);
    let edge_c = |c: Rgb| adjust(c, 0.55, 1.1, 0.0);
    let deep = pal.deep;

    // ---- tail fin (behind the body): forked, hung on the tail root
    let root = (body.x(1.0), body.mid(1.0));
    let (t_len, t_h, fork) = (tl[0] * len, tl[1] * depth, tl[2]);
    let (rt, rb) = (body.top(1.0), body.bot(1.0));
    let in_x = root.0 + len * 0.03;
    let up_tip = (root.0 - t_len, root.1 - t_h / 2.0);
    let lo_tip = (root.0 - t_len, root.1 + t_h / 2.0);
    let notch = (root.0 - t_len * (1.0 - fork), root.1);
    let mut tail = vec![(in_x, rt + (root.1 - rt) * 0.1)];
    tail.extend(curve((root.0, rt), (root.0 - t_len * 0.45, rt - (t_h / 2.0 - (root.1 - rt)) * 0.35), up_tip, 6));
    tail.extend(curve(up_tip, (root.0 - t_len * (1.0 - fork * 0.35), root.1 - t_h * 0.12), notch, 5).into_iter().skip(1));
    tail.extend(curve(notch, (root.0 - t_len * (1.0 - fork * 0.35), root.1 + t_h * 0.12), lo_tip, 5).into_iter().skip(1));
    tail.extend(curve(lo_tip, (root.0 - t_len * 0.45, rb + (t_h / 2.0 - (rb - root.1)) * 0.35), (root.0, rb), 6).into_iter().skip(1));
    tail.push((in_x, rb - (rb - root.1) * 0.1));
    b.add("tail_fin", Group::Tail, -2, Shape::Poly(tail), mix(fins_c, upper_c, 0.3));
    // the lower lobe is the most colourful (a perch's is the reddest)
    b.add("tail_lobe", Group::Tail, -1, Shape::Poly(vec![(in_x, root.1 + (rb - root.1) * 0.2), (root.0 - t_len * 1.1, root.1 + t_h * 0.08), (root.0 - t_len * 1.1, root.1 + t_h)]), fins_c).clip = Some("tail_fin");
    if detail {
        let n = 7;
        for k in 0..=n {
            let s = k as f32 / n as f32 - 0.5;
            let base = (root.0 + len * 0.01, root.1 + s * (rb - rt) * 0.7);
            let reach = 1.0 - fork * (1.0 - (2.0 * s).abs()).powf(0.7) * 0.9;
            let end = (root.0 - t_len * reach * 0.94, root.1 + s * t_h * 0.9);
            b.add("ray", Group::Tail, 0, band(&[base, end], line(0.008, 1.0), line(0.006, 1.0)), ray_c(fins_c)).clip = Some("tail_fin");
        }
    }
    pivots.insert(Group::Tail, root);

    // ---- dorsal and anal fins (behind the body)
    let mut fins: Vec<(&'static str, Shape, Vec<Ray>, Rgb, bool)> = Vec::new();
    let spiny = df[3] > 0.5;
    let (shape, rays) = fin(&body, df[0], df[1], df[2] * depth, df[4], true, if spiny { cfg.fish.spines } else { 0 });
    fins.push(("dorsal_fin", shape, rays, upper_c, spiny));
    if let Some(d2) = dims(cfg, subj, "dorsal2") {
        let (shape, rays) = fin(&body, d2[0], d2[1], d2[2] * depth, d2[3], true, 0);
        fins.push(("dorsal_fin", shape, rays, upper_c, false));
    }
    let (shape, rays) = fin(&body, an[0], an[1], an[2] * depth, an[3], false, 0);
    fins.push(("anal_fin", shape, rays, fins_c, false));
    for (role, shape, rays, c, spiny) in fins {
        let bx = shape.bbox();
        let g = group_at((bx.0 + bx.2) / 2.0);
        b.add(role, g, -1, shape, c);
        // spines show at every level that has room for them; soft rays only with detail
        if (spiny && size >= cfg.fish.detail_min_size as f32 / 2.0) || detail {
            for (base, tip) in rays {
                let w = if spiny { line(0.011, 1.0) } else { line(0.008, 1.0) };
                b.add("ray", g, 0, band(&[base, tip], w, w * 0.8), if spiny { edge_c(c) } else { ray_c(c) }).clip = Some(role);
            }
        }
    }
    if spiny {
        // a perch's first dorsal fin ends in a black blotch
        let t = df[0] + (df[1] - df[0]) * 0.8;
        let h = df[2] * depth * (0.3 + 0.7 * df[4]);
        b.add("fin_spot", group_at(body.x(t)), 1, oval(body.x(t) - h * 0.2, body.top(t) - h * 0.4, len * 0.04, h * 0.42, -20.0), look.mark).clip = Some("dorsal_fin");
    }

    // ---- the body: rear half first, the front over it where they overlap
    let ov = 0.06;
    let r = b.add("body_rear", Group::Rear, 0, body.outline(split - ov, 1.0), main);
    r.shaded = true;
    r.shade_box = sbox;
    let f = b.add("body", Group::Body, 0, body.outline(0.0, split + ov), main);
    f.shaded = true;
    f.shade_box = sbox;
    // a long part goes on both halves, each copy kept to its own half
    let both = |b: &mut B, role: &'static str, layer: i32, shape: Shape, c: Rgb, shaded: bool| {
        for g in [Group::Body, Group::Rear] {
            let p = b.add(role, g, layer, shape.clone(), c);
            p.clip = Some(clip_of(g));
            p.shaded = shaded;
            p.shade_box = sbox;
        }
    };
    // countershading: dark back, pale belly; with detail a soft step between each
    if detail {
        both(&mut b, "back", 1, body.band(0.0, 1.0, |t| body.top(t) - depth * 0.05, |t| body.top(t) + (body.mid(t) - body.top(t)) * 0.75), mix(main, back, 0.5), true);
        both(&mut b, "belly", 1, body.band(0.0, 1.0, |t| body.mid(t) + (body.bot(t) - body.mid(t)) * 0.05, |t| body.bot(t) + depth * 0.05), mix(main, belly, 0.5), true);
    }
    both(&mut b, "back", 2, body.band(0.0, 1.0, |t| body.top(t) - depth * 0.05, |t| body.top(t) + (body.mid(t) - body.top(t)) * 0.45), back, true);
    both(&mut b, "belly", 2, body.band(0.0, 1.0, |t| body.mid(t) + (body.bot(t) - body.mid(t)) * 0.35, |t| body.bot(t) + depth * 0.05), belly, true);

    // ---- markings: the kind's own (a perch's bars, a pike's spots) and any the sentence asks for
    let feature_c = |f: Feature, dflt: Rgb| req.has(f).and_then(|x| x.color.as_ref()).map(|c| crate::figure::color_of(cfg, c.v.0, c.v.1)).unwrap_or(dflt);
    let stripes = look.markings == "stripes" || req.has(Feature::Stripes).is_some();
    let spots = look.markings == "spots" || req.has(Feature::Spots).is_some();
    if stripes {
        let c = feature_c(Feature::Stripes, look.mark);
        let n = cfg.fish.stripes;
        for k in 0..n {
            let t = 0.27 + 0.63 * k as f32 / (n - 1).max(1) as f32;
            let x = body.x(t);
            let w = (len * 0.06 * (1.0 - 0.25 * k as f32 / n as f32)).max(px * 1.5);
            // bars alternate long and short, broad on the back and narrowing down the flank
            let reach = if k % 2 == 0 { 0.7 } else { 0.45 };
            let y_end = body.mid(t) + (body.bot(t) - body.mid(t)) * reach;
            let slant = len * 0.015;
            let bar = Shape::Poly(vec![
                (x - w / 2.0, body.top(t) - depth * 0.1), (x + w / 2.0, body.top(t) - depth * 0.1),
                (x + w * 0.45 - slant * 0.5, body.mid(t)), (x + w * 0.2 - slant, y_end),
                (x - w * 0.2 - slant, y_end), (x - w * 0.45 - slant * 0.5, body.mid(t)),
            ]);
            let g = group_at(x);
            let p = b.add("stripe", g, 3, bar, c);
            p.clip = Some(clip_of(g));
            p.shaded = true;
            p.shade_box = sbox;
        }
        why.push(("stripes".into(), match req.has(Feature::Stripes) {
            Some(f) => format!("\"{}\"", f.feature.words),
            None => format!("a {} has dark bars (rules.toml [fish.{}])", subj.name(), subj.name()),
        }));
    }
    if spots {
        let c = feature_c(Feature::Spots, look.mark);
        let n = cfg.fish.spots;
        // staggered rows of bean-shaped spots, each nudged off the grid (deterministically)
        for (row, (lift, shift)) in [(0.72, 0.0), (0.3, 0.5), (-0.12, 0.15), (-0.5, 0.65)].iter().enumerate() {
            for k in 0..n {
                let i = (row as u32) * 97 + k;
                let t = 0.27 + 0.68 * (k as f32 + shift + 0.3 * jitter(i)) / n as f32;
                if t > 0.97 {
                    continue;
                }
                let x = body.x(t);
                let y = body.mid(t) - (body.mid(t) - body.top(t)) * lift + depth * 0.06 * jitter(i + 7);
                let (rx, ry) = ((len * 0.017).max(px * 1.2), (depth * 0.06).max(px));
                let g = group_at(x);
                let p = b.add("spot", g, 3, oval(x, y, rx * (1.0 + 0.25 * jitter(i + 3)), ry, 25.0 * jitter(i + 11)), c);
                p.clip = Some(clip_of(g));
            }
        }
        why.push(("spots".into(), match req.has(Feature::Spots) {
            Some(f) => format!("\"{}\"", f.feature.words),
            None => format!("a {} has light spots (rules.toml [fish.{}])", subj.name(), subj.name()),
        }));
    }

    // ---- head: gill cover, lateral line, mouth, nostril
    let (t_head, eye_r, t_mouth, eye_at, eye_up) = (hd[0], hd[1] * depth, hd[2], hd[3], hd[4]);
    let gill = curve(
        (body.x(t_head * 0.82), body.top(t_head * 0.82) + (body.mid(t_head) - body.top(t_head)) * 0.3),
        (body.x(t_head * 1.08), body.mid(t_head)),
        (body.x(t_head * 0.85), body.bot(t_head * 0.85) - (body.bot(t_head) - body.mid(t_head)) * 0.15),
        8,
    );
    b.add("gill", Group::Body, 4, band(&gill, line(0.012, 1.0), line(0.009, 1.0)), mix(main, deep, 0.65)).clip = Some("body");
    if detail {
        let rim: Vec<(f32, f32)> = gill.iter().map(|&(x, y)| (x + line(0.012, 1.0), y)).collect();
        b.add("gill_rim", Group::Body, 4, band(&rim, line(0.006, 1.0), line(0.005, 1.0)), mix(back, belly, 0.6)).clip = Some("body");
    }
    if real && detail {
        let pts: Vec<(f32, f32)> = (0..=20).map(|k| {
            let t = t_head * 1.05 + (1.0 - t_head * 1.05) * k as f32 / 20.0;
            (body.x(t), body.top(t) + (body.mid(t) - body.top(t)) * 0.7)
        }).collect();
        both(&mut b, "lateral_line", 4, band(&pts, line(0.006, 1.0), line(0.004, 1.0)), mix(main, deep, 0.3), false);
    }
    let tip_y = body.top(0.0) + (body.bot(0.0) - body.top(0.0)) * 0.55;
    let hold = (body.x(0.0), tip_y);
    pivots.insert(Group::Body, hold);
    let mouth_end = (body.x(t_mouth), tip_y + depth * 0.035);
    let mouth = curve((body.x(0.0) + len * 0.01, tip_y), (body.x(t_mouth * 0.5), tip_y + depth * 0.045), mouth_end, 6);
    let mv = chosen.get("mouth").copied().unwrap_or("none");
    match mv {
        "open" => {
            b.add("gape", Group::Body, 5, oval(body.x(0.0) + len * 0.005, tip_y, len * 0.03, depth * 0.11, 0.0), pal.mouth).min_pixel = true;
        }
        "fangs" => {
            for k in 1..=3 {
                let (x, y) = mouth[k * 6 / 4];
                b.add("tooth", Group::Body, 6, Shape::Poly(vec![(x - len * 0.008, y), (x + len * 0.008, y), (x, y + depth * 0.09)]), pal.eye_white).min_pixel = true;
            }
        }
        _ => {}
    }
    // fighting a hook the jaws gape: a dark wedge from the snout back to the corner of the mouth
    let front = body.x(0.0) + len * 0.012;
    let gape = b.add("gape", Group::Body, 6, Shape::Poly(vec![
        (front, tip_y - depth * 0.17), (body.x(t_mouth * 0.4), tip_y - depth * 0.04),
        (mouth_end.0, mouth_end.1), (body.x(t_mouth * 0.4), tip_y + depth * 0.15), (front, tip_y + depth * 0.26),
    ]), adjust(pal.mouth, 0.55, 1.0, 0.0));
    gape.show = Show::MouthOpen;
    gape.clip = Some("body");
    let mut m = mouth.clone();
    if mv == "smile" {
        m.push((mouth_end.0 - len * 0.012, mouth_end.1 - depth * 0.06));
    }
    b.add("mouth", Group::Body, 5, band(&m, line(0.011, 1.0), line(0.009, 1.0)), adjust(deep, 0.6, 1.0, 0.0)).min_pixel = true;
    if real && detail {
        let t = t_head * eye_at * 0.45;
        b.add("nostril", Group::Body, 5, oval(body.x(t), body.top(t) + (body.mid(t) - body.top(t)) * 0.45, line(0.007, 0.6), line(0.007, 0.6), 0.0), deep).min_pixel = true;
    }
    why.push(("mouth".into(), format!("mouth/{mv} (at the snout, side view)")));

    // ---- pectoral fin behind the gill cover, pelvic fin under the belly (both paddle); each
    // gets a darker rim so it reads against the body
    let (p_len, v_len, t_pel) = (fn_[0] * len, fn_[1] * len, fn_[2]);
    let pec = (body.x(t_head * 1.1), body.mid(t_head) + (body.bot(t_head) - body.mid(t_head)) * 0.3);
    let pec_c = mix(look.fins, look.belly, 0.45);
    let pel = (body.x(t_pel), body.bot(t_pel) - depth * 0.1);
    // a fan from a narrow base: (role, group, base, length, direction in degrees on screen
    // (0 = towards the snout, 90 = down), half the spread, colour)
    for (role, g, base, l, dir, spread, c) in [("pectoral_fin", Group::Fin, pec, p_len, 160.0f32, 15.0f32, pec_c), ("pelvic_fin", Group::Fin2, pel, v_len, 125.0, 17.0, fins_c)] {
        let at = |deg: f32, r: f32| (base.0 + r * deg.to_radians().cos(), base.1 + r * deg.to_radians().sin());
        let w0 = (l * 0.12).max(px);
        let (n0, n1) = (at(dir - 90.0, w0), at(dir + 90.0, w0));
        let mut pts = vec![n1, n0];
        pts.extend((0..=8).map(|k| at(dir - spread + 2.0 * spread * k as f32 / 8.0, l * (1.0 - 0.12 * ((k as f32 - 4.0) / 4.0).powi(2)))));
        let fan = Shape::Poly(pts.clone());
        // a darker rim around the fan so it reads against the body
        let grown: Vec<(f32, f32)> = pts.iter().map(|&(x, y)| {
            let (dx, dy) = (x - base.0, y - base.1);
            let r = (dx * dx + dy * dy).sqrt().max(1e-5);
            (x + dx / r * px * 1.2, y + dy / r * px * 1.2)
        }).collect();
        b.add("fin_rim", g, 5, Shape::Poly(grown), edge_c(c));
        b.add(role, g, 6, fan, c);
        if detail {
            for j in -1..=1 {
                let end = at(dir + spread * 0.6 * j as f32, l * 0.85);
                b.add("ray", g, 7, band(&[base, end], line(0.007, 1.0), line(0.005, 1.0)), ray_c(c)).clip = Some(role);
            }
        }
    }
    pivots.insert(Group::Fin, pec);
    pivots.insert(Group::Fin2, pel);

    // ---- the eye: no lids; a coloured iris ring round a big black pupil
    let t_eye = t_head * eye_at;
    let ex = body.x(t_eye);
    let ey = body.mid(t_eye) - (body.mid(t_eye) - body.top(t_eye)) * eye_up;
    let iris_name = cfg.anatomy_iris.get(subj.name()).map(|s| s.as_str()).unwrap_or("gold");
    let iris = req.has(Feature::Eyes).and_then(|f| f.color.as_ref()).map(|c| cfg.colors[c.v.0.name()]).unwrap_or(cfg.colors[iris_name]);
    let ev = chosen.get("eyes").copied();
    let pr_ = &cfg.palette_rules;
    if real {
        let eye_r = eye_r.max(px * 1.6);
        b.add("socket", Group::Body, 6, oval(ex, ey, eye_r * 1.3, eye_r * 1.25, 0.0), mix(main, deep, 0.35)).min_pixel = true;
        b.add("iris", Group::Body, 7, Shape::Ellipse { cx: ex, cy: ey, rx: eye_r, ry: eye_r }, iris).min_pixel = true;
        b.add("iris_shade", Group::Body, 8, Shape::Rect { x0: ex - eye_r, y0: ey - eye_r, x1: ex + eye_r, y1: ey - eye_r * 0.45 }, adjust(iris, 0.6, 1.0, 0.0)).clip = Some("iris");
        b.add("pupil", Group::Body, 9, Shape::Ellipse { cx: ex + eye_r * 0.08, cy: ey + eye_r * 0.05, rx: eye_r * 0.58, ry: eye_r * 0.58 }, pr_.eye_dark).min_pixel = true;
        b.add("catchlight", Group::Body, 10, Shape::Rect { x0: ex - eye_r * 0.4, y0: ey - eye_r * 0.5, x1: ex - eye_r * 0.05, y1: ey - eye_r * 0.15 }, pr_.eye_white);
        why.push(("eyes".into(), format!("a fish's eye: {iris_name} iris, no lids")));
    } else if let Some(ev) = ev {
        // cartoon eyes are bigger, by the same share as other subjects' cartoon eyes
        let scale = cfg.realism.eye_scale[realism] / cfg.realism.eye_scale["realistic"];
        let size_mul = if req.has(Feature::BigEyes).is_some() {
            cfg.shapes.big_eyes
        } else if req.has(Feature::SmallEyes).is_some() {
            cfg.shapes.small_eyes
        } else {
            1.0
        };
        let r = (eye_r * scale * size_mul).max(px * 1.6);
        if ev == "dot" {
            b.add("eye", Group::Body, 7, Shape::Ellipse { cx: ex, cy: ey, rx: r * 0.5, ry: r * 0.55 }, pr_.eye_dark).min_pixel = true;
        } else {
            // a white eye with a coloured iris ring: a roach's red eye shows even in cartoon
            b.add("eye_white", Group::Body, 7, Shape::Ellipse { cx: ex, cy: ey, rx: r, ry: r }, pr_.eye_white).min_pixel = true;
            b.add("iris", Group::Body, 8, Shape::Ellipse { cx: ex + r * 0.15, cy: ey + r * 0.05, rx: r * 0.72, ry: r * 0.72 }, iris).clip = Some("eye_white");
            b.add("pupil", Group::Body, 9, Shape::Ellipse { cx: ex + r * 0.22, cy: ey + r * 0.08, rx: r * 0.42, ry: r * 0.42 }, pr_.eye_dark).min_pixel = true;
            if ev == "shine" {
                b.add("shine", Group::Body, 10, Shape::Rect { x0: ex - r * 0.25, y0: ey - r * 0.45, x1: ex + r * 0.1, y1: ey - r * 0.1 }, pr_.eye_white);
            }
            if ev == "angry" {
                // a brow slanting down towards the snout
                b.add("brow", Group::Body, 11, Shape::Poly(vec![(ex - r * 1.3, ey - r * 1.3), (ex + r * 1.3, ey - r * 1.3), (ex + r * 1.3, ey + r * 0.05)]), main).clip = Some("eye_white");
            }
            if ev == "sleepy" {
                b.add("lid", Group::Body, 11, Shape::Rect { x0: ex - r * 1.2, y0: ey - r * 1.3, x1: ex + r * 1.2, y1: ey - r * 0.05 }, main).clip = Some("eye_white");
            }
        }
        why.push(("eyes".into(), format!("eyes/{ev} (one eye: side view)")));
    }

    let x0 = body.x(1.0) - t_len;
    let all_box = [(x0 + body.xs) / 2.0, cy, body.xs - x0, depth];
    why.push(("anatomy".into(), format!("a {} from the side, facing right (rules.toml [anatomy.{}], [fish.{}])", subj.name(), subj.name(), subj.name())));
    let feet_y = b.parts.iter().map(|p| p.shape.bbox().3).fold(0.0f32, f32::max);
    Ok(Figure {
        parts: b.parts,
        pal,
        pivots,
        body: all_box,
        feet_y,
        symmetric: false,
        has_legs: false,
        has_wings: false,
        has_eyes: true,
        material: material_of(subj),
        realism,
        gait: Gait::Swim,
        why,
    })
}

/// Deterministic jitter in -1..1 (markings sit a little off the grid, the same every run).
fn jitter(i: u32) -> f32 {
    let mut h = i.wrapping_mul(2_654_435_761) ^ 0x9e37_79b9;
    h = (h ^ (h >> 15)).wrapping_mul(2_246_822_519);
    h ^= h >> 13;
    (h & 0xffff) as f32 / 32767.5 - 1.0
}

/// A fishing hook hanging from its eye: the shank, the bend, the point with its barb.
fn hook(cfg: &Config, realism: &'static str, pal: Palette, boxed: [f32; 4], mut why: Vec<(String, String)>) -> Result<Figure, String> {
    let w = &cfg.anatomy["hook"]["wire"];
    let [cx, cy, bw, h] = boxed;
    let (thick, er, barb) = (w[0] * h, w[1] * h, w[2] * h);
    let main = pal.main;
    let top = cy - h / 2.0;
    let r = bw / 2.0 - thick / 2.0;
    let bend_y = cy + h / 2.0 - r - thick / 2.0;
    let shank_x = cx + r;
    let eye_c = (shank_x, top + er + thick * 0.4);
    let point_x = cx - r;
    let tip_y = bend_y - r * 1.25;
    let sbox = Some((cx - bw / 2.0, top, cx + bw / 2.0, cy + h / 2.0));
    let mut b = B { parts: vec![] };
    let solid = |b: &mut B, layer: i32, shape: Shape| {
        let p = b.add("body", Group::Body, layer, shape, main);
        p.shaded = true;
        p.shade_box = sbox;
    };
    // the wire: down the shank, round the bend, up the point, which narrows to a sharp tip
    let mut path: Vec<(f32, f32)> = (0..=6).map(|k| (shank_x, eye_c.1 + er + (bend_y - eye_c.1 - er) * k as f32 / 6.0)).collect();
    path.extend((1..=16).map(|k| {
        let a = PI * k as f32 / 16.0;
        (cx + r * a.cos(), bend_y + r * a.sin())
    }));
    path.extend((1..=4).map(|k| (point_x + thick * 0.08 * k as f32, bend_y - (bend_y - tip_y) * k as f32 / 4.0)));
    solid(&mut b, 0, band(&path, thick, thick * 0.7));
    solid(&mut b, 0, Shape::Poly(vec![(point_x - thick * 0.05, tip_y + thick * 0.4), (point_x + thick * 0.65, tip_y + thick * 0.4), (point_x + thick * 0.25, tip_y - thick * 1.4)]));
    // the barb: a spike on the inside of the point, pointing back down towards the bend
    let base = point_x + thick * 0.4;
    solid(&mut b, 0, Shape::Poly(vec![(base - thick * 0.2, tip_y - thick * 0.2), (base + barb * 0.55, tip_y + barb * 1.2), (base - thick * 0.2, tip_y + barb * 0.7)]));
    // the eye the line is tied to
    let ring: Vec<(f32, f32)> = (0..=24).map(|k| {
        let a = 2.0 * PI * k as f32 / 24.0;
        (eye_c.0 + er * a.cos(), eye_c.1 + er * a.sin())
    }).collect();
    solid(&mut b, 0, band(&ring, thick * 0.8, thick * 0.8));
    if realism != "realistic" {
        // a glint down the shank (realistic metal gets its streak from the light)
        b.add("glint", Group::Body, 2, band(&[(shank_x - thick * 0.15, eye_c.1 + er * 1.6), (shank_x - thick * 0.15, bend_y - r * 0.2)], thick * 0.3, thick * 0.3), pal.light).clip = Some("body");
    }
    why.push(("anatomy".into(), "a hook hanging from its eye: shank, bend, point and barb (rules.toml [anatomy.hook])".into()));
    let feet_y = b.parts.iter().map(|p| p.shape.bbox().3).fold(0.0f32, f32::max);
    Ok(Figure {
        parts: b.parts,
        pal,
        pivots: BTreeMap::from([(Group::Body, eye_c)]),
        body: [cx, cy, bw, h],
        feet_y,
        symmetric: false,
        has_legs: false,
        has_wings: false,
        has_eyes: false,
        material: material_of(Subject::Hook),
        realism,
        gait: Gait::Hang,
        why,
    })
}

/// A jig (pilkki): a small metal fish on a hook that hangs from its eye. The hook's shank runs
/// down from the eye behind the lure's body, so the lure sits threaded on it; the bend and the
/// point (turned up, as a hanging hook's is) come out below. The lure is a slim lens of polished
/// metal facing right: a darker back, a bright belly, a forked tail, an eye and a red gill
/// mark, the way jigs are painted. `boxed` = [centre x, centre y, lure length, height from the
/// eye to the bend]; the rest is in [anatomy.lure].
fn lure(cfg: &Config, realism: &'static str, pal: Palette, boxed: [f32; 4], px: f32, mut why: Vec<(String, String)>) -> Result<Figure, String> {
    let a = &cfg.anatomy["lure"];
    let (w, l) = (&a["wire"], &a["lure"]);
    let [cx, cy, len, h] = boxed;
    // the fit scales a thing hung from its eye by about half the canvas over its height, so the
    // wire stays at least `wire[3]` pixels wide in the end
    let min_wire = w[3] * px * h / 0.5;
    let (thick, er, barb) = ((w[0] * h).max(min_wire), (w[1] * h).max(min_wire * 1.2), w[2] * h);
    let (depth, at, bend_w, tail) = (l[0] * len, l[1], l[2] * len, l[3] * len);
    let main = pal.main;
    let top = cy - h / 2.0;
    let shank_x = cx;
    let eye_c = (shank_x, top + er + thick * 0.5);
    let r = bend_w / 2.0;
    let bend_y = cy + h / 2.0 - r - thick / 2.0;
    let point_x = shank_x - 2.0 * r;
    let tip_y = bend_y - r * 1.4;
    let ly = top + h * at;
    let (head_x, tail_x) = (shank_x + len * 0.55, shank_x - len * 0.45);
    let sbox = Some((tail_x - tail, ly - depth / 2.0, head_x, ly + depth / 2.0));
    let wire_box = Some((point_x - r, top, shank_x + r, cy + h / 2.0));
    let mut b = B { parts: vec![] };
    let metal = |b: &mut B, role: &'static str, layer: i32, shape: Shape, c: Rgb, sb: Option<(f32, f32, f32, f32)>| {
        let p = b.add(role, Group::Body, layer, shape, c);
        p.shaded = true;
        p.shade_box = sb;
    };
    // ---- the hook: eye, shank (behind the lure), bend, point and barb
    let wire_c = adjust(main, 0.82, 0.8, 0.0);
    let mut path: Vec<(f32, f32)> = (0..=8).map(|k| (shank_x, eye_c.1 + er + (bend_y - eye_c.1 - er) * k as f32 / 8.0)).collect();
    path.extend((1..=16).map(|k| {
        let t = PI * k as f32 / 16.0;
        (shank_x - r + r * t.cos(), bend_y + r * t.sin())
    }));
    path.extend((1..=4).map(|k| (point_x, bend_y - (bend_y - tip_y) * k as f32 / 4.0)));
    metal(&mut b, "wire", -1, band(&path, thick, thick * 0.8), wire_c, wire_box);
    metal(&mut b, "wire", -1, Shape::Poly(vec![(point_x - thick * 0.5, tip_y + thick * 0.3), (point_x + thick * 0.5, tip_y + thick * 0.3), (point_x, tip_y - thick * 1.5)]), wire_c, wire_box);
    let base = point_x + thick * 0.4;
    metal(&mut b, "barb", -1, Shape::Poly(vec![(base - thick * 0.2, tip_y - thick * 0.1), (base + barb * 0.5, tip_y + barb * 1.1), (base - thick * 0.2, tip_y + barb * 0.6)]), wire_c, wire_box);
    let ring: Vec<(f32, f32)> = (0..=24).map(|k| {
        let t = 2.0 * PI * k as f32 / 24.0;
        (eye_c.0 + er * t.cos(), eye_c.1 + er * t.sin())
    }).collect();
    metal(&mut b, "eye_ring", 0, band(&ring, thick * 0.8, thick * 0.8), wire_c, wire_box);
    // ---- the lure: a lens pointed at the head and the tail root, deepest a little before the middle
    let n = 20;
    let lens = |side: f32| -> Vec<(f32, f32)> {
        (0..=n).map(|k| {
            let t = k as f32 / n as f32;
            let x = head_x - (head_x - tail_x) * t;
            let f = (PI * t.powf(0.85)).sin().max(0.0);
            (x, ly + side * depth / 2.0 * f.max(0.12))
        }).collect()
    };
    let mut outline = lens(-1.0);
    outline.extend(lens(1.0).into_iter().rev());
    metal(&mut b, "body", 1, Shape::Poly(outline.clone()), main, sbox);
    // the forked tail
    metal(&mut b, "tail_fin", 0, Shape::Poly(vec![
        (tail_x + px * 0.5, ly), (tail_x - tail, ly - depth * 0.5), (tail_x - tail * 0.55, ly), (tail_x - tail, ly + depth * 0.5),
    ]), main, sbox);
    // a darker back, a bright belly; a stripe of light down the flank (polished metal)
    let back: Vec<(f32, f32)> = lens(-1.0).into_iter().chain(lens(-0.15).into_iter().rev()).collect();
    b.add("back", Group::Body, 2, Shape::Poly(back), adjust(main, 0.62, 1.2, -0.02)).clip = Some("body");
    b.add("flank_light", Group::Body, 3, band(&[(head_x - len * 0.18, ly + depth * 0.05), (tail_x + len * 0.12, ly + depth * 0.05)], (depth * 0.16).max(px), (depth * 0.1).max(px * 0.6)), pal.spec).clip = Some("body");
    // the painted face: a red gill mark and an eye
    b.add("gill", Group::Body, 4, band(&[(head_x - len * 0.2, ly - depth * 0.3), (head_x - len * 0.24, ly + depth * 0.3)], (len * 0.05).max(px), (len * 0.04).max(px)), cfg.colors["red"]).clip = Some("body");
    let er2 = (depth * 0.17).max(px * 0.7);
    b.add("eye", Group::Body, 5, Shape::Ellipse { cx: head_x - len * 0.11, cy: ly - depth * 0.06, rx: er2, ry: er2 }, cfg.palette_rules.eye_dark).min_pixel = true;
    if realism != "realistic" {
        b.add("glint", Group::Body, 6, Shape::Rect { x0: head_x - len * 0.42, y0: ly - depth * 0.3, x1: head_x - len * 0.32, y1: ly - depth * 0.18 }, [1.0; 3]).clip = Some("body");
    }
    why.push(("anatomy".into(), "a jig: a metal fish threaded on a hook that hangs from its eye, the point up (rules.toml [anatomy.lure])".into()));
    let feet_y = b.parts.iter().map(|p| p.shape.bbox().3).fold(0.0f32, f32::max);
    Ok(Figure {
        parts: b.parts,
        pal,
        pivots: BTreeMap::from([(Group::Body, eye_c)]),
        body: [cx, ly, len, depth],
        feet_y,
        symmetric: false,
        has_legs: false,
        has_wings: false,
        has_eyes: false,
        material: material_of(Subject::Lure),
        realism,
        gait: Gait::Hang,
        why,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fig(sentence: &str, realism: &'static str) -> Figure {
        let cfg = Config::load(None).unwrap();
        let req = crate::parser::parse(sentence, false).unwrap();
        let mut chosen = Chosen::new();
        chosen.insert("realism", realism);
        chosen.insert("eyes", "shine");
        crate::figure::build(&req, &cfg, &chosen).unwrap()
    }

    fn has(f: &Figure, role: &str) -> bool {
        f.parts.iter().any(|p| p.role == role)
    }

    #[test]
    fn every_fish_has_fins_an_eye_and_a_swimming_body() {
        for s in ["a fish", "a perch", "a roach", "a pike"] {
            for r in ["cartoon", "stylized", "realistic"] {
                let f = fig(s, r);
                assert_eq!(f.gait, Gait::Swim, "{s} {r}");
                assert!(!f.symmetric && !f.has_legs);
                for role in ["body", "body_rear", "tail_fin", "dorsal_fin", "anal_fin", "pectoral_fin", "pelvic_fin", "mouth", "gill"] {
                    assert!(has(&f, role), "{s} {r}: {role}");
                }
                for g in [Group::Rear, Group::Tail, Group::Fin, Group::Fin2, Group::Body] {
                    assert!(f.pivots.contains_key(&g), "{s} {r}: {g:?}");
                }
            }
        }
    }

    #[test]
    fn a_perch_has_bars_red_fins_and_a_spiny_dorsal() {
        let cfg = Config::load(None).unwrap();
        let f = fig("a perch", "realistic");
        assert!(f.parts.iter().filter(|p| p.role == "stripe").count() as u32 == cfg.fish.stripes);
        assert!(has(&f, "fin_spot"), "the black blotch on the spiny dorsal");
        assert_eq!(f.parts.iter().filter(|p| p.role == "dorsal_fin").count(), 2, "spiny and soft dorsal fins");
        let pelvic = f.parts.iter().find(|p| p.role == "pelvic_fin").unwrap().color;
        assert!(pelvic[0] > 0.8 && pelvic[1] < 0.5, "red-orange fins: {pelvic:?}");
        assert!(!has(&f, "spot"));
    }

    #[test]
    fn a_roach_has_red_eyes_and_no_markings() {
        let f = fig("a roach", "realistic");
        let iris = f.parts.iter().find(|p| p.role == "iris").unwrap().color;
        assert!(iris[0] > 0.7 && iris[1] < 0.4, "{iris:?}");
        assert!(!has(&f, "stripe") && !has(&f, "spot"));
        // even a cartoon roach shows its red iris
        let c = fig("a roach", "cartoon");
        assert!(c.parts.iter().any(|p| p.role == "iris" && p.color[0] > 0.7));
    }

    #[test]
    fn a_pike_is_long_and_spotted_with_its_fins_far_back() {
        let f = fig("a pike", "realistic");
        let p = fig("a perch", "realistic");
        let ratio = |f: &Figure| f.body[3] / f.body[2];
        assert!(ratio(&f) < ratio(&p) * 0.6, "a pike is much slimmer than a perch");
        assert!(has(&f, "spot") && !has(&f, "stripe"));
        let dorsal = f.parts.iter().find(|p| p.role == "dorsal_fin").unwrap();
        assert_eq!(dorsal.group, Group::Rear, "the dorsal fin sits far back");
    }

    #[test]
    fn markings_follow_the_sentence() {
        let f = fig("a fish", "cartoon");
        assert!(!has(&f, "stripe") && !has(&f, "spot"));
        let f = fig("a fish with black stripes", "cartoon");
        assert!(has(&f, "stripe"));
        let f = fig("a roach with spots", "stylized");
        assert!(has(&f, "spot"));
    }

    #[test]
    fn a_fish_cant_wear_a_crown() {
        let cfg = Config::load(None).unwrap();
        let req = crate::parser::parse("a perch with a crown", false).unwrap();
        let mut chosen = Chosen::new();
        chosen.insert("realism", "cartoon");
        assert!(crate::figure::build(&req, &cfg, &chosen).err().unwrap().contains("doesn't fit a perch"));
    }

    #[test]
    fn a_hook_has_a_shank_bend_point_and_barb() {
        for r in ["cartoon", "realistic"] {
            let f = fig("a hook", r);
            assert_eq!(f.material, crate::figure::Material::Metal);
            assert!(f.parts.iter().filter(|p| p.role == "body").count() >= 3, "wire, eye, barb");
            assert_eq!(f.gait, Gait::Hang);
        }
    }
}
