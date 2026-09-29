//! Blueprints: a sprite as coloured parts in canvas fractions (0..1, y down). The rules chose
//! the variants; this file turns them into shapes. Proportions come from rules.toml [shapes];
//! what stays here are the relations ("ears sit on top of the head", "wings hang off the sides").

use crate::color::{adjust, Rgb};
use crate::config::Config;
use crate::request::*;
use crate::rules::Chosen;
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Group {
    Body,
    WingL,
    WingR,
    LegL,
    LegR,
    Tail,
}

#[derive(Clone, Debug)]
pub enum Shape {
    Ellipse { cx: f32, cy: f32, rx: f32, ry: f32 },
    /// An ellipse with a different radius above and below the line `cy`.
    Egg { cx: f32, cy: f32, rx: f32, ry_top: f32, ry_bottom: f32 },
    Rect { x0: f32, y0: f32, x1: f32, y1: f32 },
    Poly(Vec<(f32, f32)>),
}

impl Shape {
    pub fn contains(&self, x: f32, y: f32) -> bool {
        match self {
            Shape::Ellipse { cx, cy, rx, ry } => ((x - cx) / rx).powi(2) + ((y - cy) / ry).powi(2) <= 1.0,
            Shape::Egg { cx, cy, rx, ry_top, ry_bottom } => {
                let ry = if y < *cy { ry_top } else { ry_bottom };
                ((x - cx) / rx).powi(2) + ((y - cy) / ry).powi(2) <= 1.0
            }
            Shape::Rect { x0, y0, x1, y1 } => x >= *x0 && x < *x1 && y >= *y0 && y < *y1,
            Shape::Poly(p) => {
                // even-odd rule
                let mut inside = false;
                let n = p.len();
                for i in 0..n {
                    let (xi, yi) = p[i];
                    let (xj, yj) = p[(i + n - 1) % n];
                    if (yi > y) != (yj > y) && x < (xj - xi) * (y - yi) / (yj - yi) + xi {
                        inside = !inside;
                    }
                }
                inside
            }
        }
    }

    pub fn bbox(&self) -> (f32, f32, f32, f32) {
        match self {
            Shape::Ellipse { cx, cy, rx, ry } => (cx - rx, cy - ry, cx + rx, cy + ry),
            Shape::Egg { cx, cy, rx, ry_top, ry_bottom } => (cx - rx, cy - ry_top, cx + rx, cy + ry_bottom),
            Shape::Rect { x0, y0, x1, y1 } => (*x0, *y0, *x1, *y1),
            Shape::Poly(p) => p.iter().fold((f32::MAX, f32::MAX, f32::MIN, f32::MIN), |b, &(x, y)| (b.0.min(x), b.1.min(y), b.2.max(x), b.3.max(y))),
        }
    }

    fn mirror(&self) -> Shape {
        match self {
            Shape::Ellipse { cx, cy, rx, ry } => Shape::Ellipse { cx: 1.0 - cx, cy: *cy, rx: *rx, ry: *ry },
            Shape::Egg { cx, cy, rx, ry_top, ry_bottom } => Shape::Egg { cx: 1.0 - cx, cy: *cy, rx: *rx, ry_top: *ry_top, ry_bottom: *ry_bottom },
            Shape::Rect { x0, y0, x1, y1 } => Shape::Rect { x0: 1.0 - x1, y0: *y0, x1: 1.0 - x0, y1: *y1 },
            Shape::Poly(p) => Shape::Poly(p.iter().rev().map(|&(x, y)| (1.0 - x, y)).collect()),
        }
    }
}

/// When a part is drawn, relative to blinking.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Show {
    Always,
    EyesOpen,
    EyesClosed,
}

#[derive(Clone, Debug)]
pub struct Part {
    pub role: &'static str,
    pub group: Group,
    pub layer: i32,
    pub shape: Shape,
    pub color: Rgb,
    /// Takes the shading rule (light and shade from the ramp).
    pub shaded: bool,
    /// The box shading is measured in (the whole body for multi-part bodies).
    pub shade_box: Option<(f32, f32, f32, f32)>,
    /// Only paint over pixels already painted by a part with this role.
    pub clip: Option<&'static str>,
    pub show: Show,
    /// At least one pixel, even when the shape is smaller than a pixel.
    pub min_pixel: bool,
}

#[derive(Clone, Debug)]
pub struct Palette {
    pub main: Rgb,
    pub shade: Rgb,
    pub light: Rgb,
    pub dark_outline: Rgb,
    pub eye_white: Rgb,
    pub eye_dark: Rgb,
    pub mouth: Rgb,
    /// value, saturation, hue shift for shade and light (from the palette rule)
    pub ramp: [(f32, f32, f32); 2],
    pub outline_value: f32,
}

impl Palette {
    pub fn shade_of(&self, c: Rgb) -> Rgb {
        if c == self.main {
            self.shade
        } else {
            adjust(c, self.ramp[0].0, self.ramp[0].1, self.ramp[0].2)
        }
    }
    pub fn light_of(&self, c: Rgb) -> Rgb {
        if c == self.main {
            self.light
        } else {
            adjust(c, self.ramp[1].0, self.ramp[1].1, self.ramp[1].2)
        }
    }
}

pub struct Figure {
    pub parts: Vec<Part>,
    pub pal: Palette,
    /// Pivots groups turn around (wing roots, hips).
    pub pivots: BTreeMap<Group, (f32, f32)>,
    /// The body's box (centre x, centre y, w, h).
    pub body: [f32; 4],
    /// The lowest point of the sprite (bounce lands here).
    pub feet_y: f32,
    pub symmetric: bool,
    pub has_legs: bool,
    pub has_wings: bool,
    pub has_eyes: bool,
    /// (piece, how it was decided) for why.txt
    pub why: Vec<(String, String)>,
}

fn color_of(cfg: &Config, c: Color, tone: Option<Tone>) -> Rgb {
    let base = cfg.colors[c.name()];
    match tone {
        Some(Tone::Light) => adjust(base, cfg.tones.light, 0.8, 0.0),
        Some(Tone::Dark) => adjust(base, cfg.tones.dark, 1.0, 0.0),
        None => base,
    }
}

fn named(cfg: &Config, name: &str) -> Rgb {
    cfg.colors[name]
}

struct B<'a> {
    parts: Vec<Part>,
    cfg: &'a Config,
}

impl<'a> B<'a> {
    fn add(&mut self, role: &'static str, group: Group, layer: i32, shape: Shape, color: Rgb) -> &mut Part {
        self.parts.push(Part { role, group, layer, shape, color, shaded: false, shade_box: None, clip: None, show: Show::Always, min_pixel: false });
        self.parts.last_mut().unwrap()
    }
    /// A part and its mirror image across x = 0.5 (roles given for left and right).
    fn pair(&mut self, roles: (&'static str, &'static str), groups: (Group, Group), layer: i32, shape: Shape, color: Rgb) -> (usize, usize) {
        let m = shape.mirror();
        self.add(roles.0, groups.0, layer, shape, color);
        let a = self.parts.len() - 1;
        self.add(roles.1, groups.1, layer, m, color);
        (a, a + 1)
    }
}

pub fn build(req: &Request, cfg: &Config, chosen: &Chosen) -> Result<Figure, String> {
    let subj = req.subject.v;
    let shapes = &cfg.shapes.subjects[subj.name()];
    let variant = chosen.get("body").copied().unwrap_or("round");
    let [cx, cy, w, h] = match variant {
        "tall" => shapes.tall,
        "wide" => shapes.wide,
        _ => shapes.round,
    };
    let mut why = vec![("body".to_string(), format!("body/{variant} (the {})", req.subject.words))];

    // ---- palette
    let main = match &req.color {
        Some(c) => {
            why.push(("colour".into(), format!("\"{}\"", c.words)));
            color_of(cfg, c.v.0, c.v.1)
        }
        None => {
            let d = &cfg.default_color[subj.name()];
            why.push(("colour".into(), format!("default for a {} ({d})", subj.name())));
            named(cfg, d)
        }
    };
    let pr = &cfg.palette_rules;
    let hue = chosen.get("palette").copied() == Some("hue_shift");
    let ramp = [
        (pr.shade_value, 1.0, if hue { pr.shade_hue_shift } else { 0.0 }),
        (pr.light_value, pr.light_saturation, if hue { pr.light_hue_shift } else { 0.0 }),
    ];
    let pal = Palette {
        main,
        shade: adjust(main, ramp[0].0, ramp[0].1, ramp[0].2),
        light: adjust(main, ramp[1].0, ramp[1].1, ramp[1].2),
        dark_outline: pr.dark_outline,
        eye_white: pr.eye_white,
        eye_dark: pr.eye_dark,
        mouth: pr.mouth,
        ramp,
        outline_value: pr.outline_value,
    };
    let feature_color = |f: Feature, default: Rgb| -> Rgb { req.has(f).and_then(|x| x.color.as_ref()).map(|c| color_of(cfg, c.v.0, c.v.1)).unwrap_or(default) };

    let mut b = B { parts: vec![], cfg };
    let body_box = (cx - w / 2.0, cy - h / 2.0, cx + w / 2.0, cy + h / 2.0);
    let top = cy - h / 2.0;
    let bottom = cy + h / 2.0;
    let mut pivots = BTreeMap::new();
    let mut has_legs = false;
    let mut has_wings = false;
    let mut feet_y = bottom;
    let mut face = (cx, cy + shapes.eyes_y);
    let x = |key: &str| cfg.subject_extra(subj, key);

    macro_rules! body {
        ($shape:expr) => {{
            let p = b.add("body", Group::Body, 0, $shape, main);
            p.shaded = true;
            p.shade_box = Some(body_box);
        }};
    }

    match subj {
        Subject::Slime => body!(Shape::Egg { cx, cy: top + 0.62 * h, rx: w / 2.0, ry_top: 0.62 * h, ry_bottom: 0.38 * h }),
        Subject::Ghost => {
            let r = w / 2.0;
            let yc = top + r;
            let mut pts: Vec<(f32, f32)> = (0..=10).map(|k| {
                let a = std::f32::consts::PI * (1.0 + k as f32 / 10.0);
                (cx + r * a.cos(), yc + r * a.sin())
            }).collect();
            let tooth = h * 0.12;
            let teeth = 3;
            pts.push((cx + r, bottom));
            for k in 1..=(teeth * 2) {
                let xk = cx + r - (2.0 * r) * k as f32 / (teeth * 2) as f32;
                let yk = if k % 2 == 1 { bottom - tooth } else { bottom };
                pts.push((xk, yk));
            }
            body!(Shape::Poly(pts));
        }
        Subject::Bat => {
            body!(Shape::Ellipse { cx, cy, rx: w / 2.0, ry: h / 2.0 });
            let (span, wh) = (x("wing_span"), x("wing_h"));
            let root = (cx - w * 0.4, cy - h * 0.1);
            let wing = Shape::Poly(vec![
                root,
                (cx - span, cy - wh * 0.6),
                (cx - span * 0.82, cy + wh * 0.12),
                (cx - span * 0.6, cy - wh * 0.02),
                (cx - span * 0.42, cy + wh * 0.26),
                (cx - w * 0.36, cy + h * 0.25),
            ]);
            let wc = pal.shade;
            b.pair(("wing_l", "wing_r"), (Group::WingL, Group::WingR), -1, wing, wc);
            pivots.insert(Group::WingL, root);
            pivots.insert(Group::WingR, (1.0 - root.0, root.1));
            has_wings = true;
            let ear = Shape::Poly(vec![(cx - w * 0.38, cy - h * 0.25), (cx - w * 0.1, cy - h * 0.4), (cx - w * 0.34, cy - h * 0.82)]);
            b.pair(("ear_l", "ear_r"), (Group::Body, Group::Body), -1, ear, main);
        }
        Subject::Cat => {
            body!(Shape::Ellipse { cx, cy, rx: w / 2.0, ry: h / 2.0 });
            let eh = x("ear_h");
            let ear = Shape::Poly(vec![(cx - w * 0.44, cy - h * 0.12), (cx - w * 0.1, cy - h * 0.42), (cx - w * 0.4, cy - h * 0.46 - eh)]);
            b.pair(("ear_l", "ear_r"), (Group::Body, Group::Body), -1, ear, main);
            let paw = Shape::Ellipse { cx: cx - w * 0.22, cy: bottom, rx: w * 0.13, ry: h * 0.11 };
            b.pair(("leg_l", "leg_r"), (Group::LegL, Group::LegR), 1, paw, pal.light);
            has_legs = true;
            feet_y = bottom + h * 0.11;
        }
        Subject::Frog => {
            body!(Shape::Ellipse { cx, cy, rx: w / 2.0, ry: h / 2.0 });
            let bump = Shape::Ellipse { cx: cx - cfg.shapes.eye_gap * 0.55, cy: top + h * 0.05, rx: cfg.shapes.eye_r * 1.55, ry: cfg.shapes.eye_r * 1.45 };
            let (l, r) = b.pair(("bump_l", "bump_r"), (Group::Body, Group::Body), 0, bump, main);
            for i in [l, r] {
                b.parts[i].shaded = true;
                b.parts[i].shade_box = Some(body_box);
            }
            let belly = b.add("belly", Group::Body, 1, Shape::Ellipse { cx, cy: cy + h * 0.14, rx: w * 0.3, ry: h * 0.28 }, pal.light);
            belly.clip = Some("body");
            let lh = x("leg_h");
            let leg = Shape::Ellipse { cx: cx - w * 0.38, cy: bottom - lh * 0.4, rx: w * 0.17, ry: lh };
            b.pair(("leg_l", "leg_r"), (Group::LegL, Group::LegR), -1, leg, pal.shade);
            has_legs = true;
            feet_y = bottom + lh * 0.6;
        }
        Subject::Knight | Subject::Robot => {
            let robot = subj == Subject::Robot;
            if robot {
                body!(Shape::Rect { x0: cx - w / 2.0, y0: top, x1: cx + w / 2.0, y1: bottom });
            } else {
                body!(Shape::Egg { cx, cy: top + 0.55 * h, rx: w / 2.0, ry_top: 0.55 * h, ry_bottom: 0.45 * h });
            }
            let th = x("torso_h");
            let torso = b.add("torso", Group::Body, -1, Shape::Rect { x0: cx - w * 0.4, y0: bottom - h * 0.08, x1: cx + w * 0.4, y1: bottom + th }, pal.shade);
            torso.shaded = true;
            let lh = x("leg_h");
            let legc = named(cfg, "black");
            let leg = Shape::Rect { x0: cx - w * 0.3, y0: bottom + th - 0.01, x1: cx - w * 0.07, y1: bottom + th + lh };
            b.pair(("leg_l", "leg_r"), (Group::LegL, Group::LegR), -2, leg, legc);
            has_legs = true;
            feet_y = bottom + th + lh;
            if !robot {
                // the plume
                let plume = Shape::Poly(vec![(cx - w * 0.08, top + h * 0.05), (cx + w * 0.1, top - h * 0.28), (cx + w * 0.16, top + h * 0.08)]);
                b.add("plume", Group::Body, -1, plume, named(cfg, "red"));
            }
        }
        Subject::Coin => {
            body!(Shape::Ellipse { cx, cy, rx: w / 2.0, ry: h / 2.0 });
            b.add("rim", Group::Body, 1, Shape::Ellipse { cx, cy, rx: w * 0.36, ry: h * 0.36 }, pal.shade);
            let inner = b.add("face", Group::Body, 2, Shape::Ellipse { cx, cy, rx: w * 0.29, ry: h * 0.29 }, main);
            inner.shaded = true;
        }
        Subject::Gem => {
            let (yt, yg, yb) = (top, cy - h * 0.15, bottom);
            body!(Shape::Poly(vec![(cx - w * 0.28, yt), (cx + w * 0.28, yt), (cx + w / 2.0, yg), (cx, yb), (cx - w / 2.0, yg)]));
            b.add("facet_light", Group::Body, 1, Shape::Poly(vec![(cx - w * 0.28, yt), (cx, yt), (cx - w * 0.18, yg)]), pal.light).clip = Some("body");
            b.add("facet_shade", Group::Body, 1, Shape::Poly(vec![(cx, yb), (cx + w / 2.0, yg), (cx + w * 0.1, yg)]), pal.shade).clip = Some("body");
        }
        Subject::Star => {
            let pts: Vec<(f32, f32)> = (0..10)
                .map(|k| {
                    let a = (-90.0 + 36.0 * k as f32).to_radians();
                    let r = if k % 2 == 0 { 1.0 } else { 0.45 };
                    (cx + a.cos() * r * w / 2.0, cy + a.sin() * r * h / 2.0 + h * 0.04)
                })
                .collect();
            body!(Shape::Poly(pts));
        }
        Subject::Heart => {
            let lobe = Shape::Ellipse { cx: cx - w * 0.235, cy: top + h * 0.28, rx: w * 0.265, ry: h * 0.28 };
            let (l, r) = b.pair(("body", "body"), (Group::Body, Group::Body), 0, lobe, main);
            for i in [l, r] {
                b.parts[i].shaded = true;
                b.parts[i].shade_box = Some(body_box);
            }
            body!(Shape::Poly(vec![(cx - w / 2.0 + w * 0.005, top + h * 0.34), (cx + w / 2.0 - w * 0.005, top + h * 0.34), (cx, bottom)]));
        }
        Subject::Potion => {
            body!(Shape::Ellipse { cx, cy, rx: w / 2.0, ry: h / 2.0 });
            let (nw, nh) = (x("neck_w"), x("neck_h"));
            b.add("neck", Group::Body, -1, Shape::Rect { x0: cx - nw / 2.0, y0: top - nh * 0.85, x1: cx + nw / 2.0, y1: top + h * 0.12 }, named(cfg, "silver"));
            b.add("cork", Group::Body, 1, Shape::Rect { x0: cx - nw * 0.42, y0: top - nh * 1.15, x1: cx + nw * 0.42, y1: top - nh * 0.72 }, named(cfg, "brown"));
            b.add("glint", Group::Body, 2, Shape::Ellipse { cx: cx - w * 0.2, cy: cy - h * 0.18, rx: w * 0.07, ry: h * 0.1 }, pr.eye_white).min_pixel = true;
        }
        Subject::Mushroom => {
            let stem_top = cy + h * 0.2;
            body!(Shape::Egg { cx, cy: stem_top, rx: w / 2.0, ry_top: h * 0.7, ry_bottom: h * 0.12 });
            let (sw, sh) = (x("stem_w"), x("stem_h"));
            let stem = b.add("stem", Group::Body, -1, Shape::Rect { x0: cx - sw / 2.0, y0: stem_top, x1: cx + sw / 2.0, y1: stem_top + sh }, named(cfg, "white"));
            stem.shaded = true;
            feet_y = stem_top + sh;
            if req.has(Feature::Spots).is_none() {
                spots(&mut b, cfg, (cx, stem_top - h * 0.3, w, h * 0.6), named(cfg, "white"));
                why.push(("spots".into(), "a mushroom's cap has spots".into()));
            }
            face = (cx, stem_top + sh * 0.45);
        }
    }

    // ---- features
    let fh = |k: &str| cfg.feature(k);
    for f in &req.features {
        let fw = f.feature.words.clone();
        match f.feature.v {
            Feature::Horns => {
                let (hh, hw) = (fh("horn_h"), fh("horn_w"));
                let base_y = top + h * 0.12;
                let horn = Shape::Poly(vec![(cx - w * 0.3 - hw / 2.0, base_y), (cx - w * 0.3 + hw / 2.0, base_y), (cx - w * 0.36, base_y - hh)]);
                b.pair(("horn_l", "horn_r"), (Group::Body, Group::Body), -1, horn, feature_color(Feature::Horns, named(cfg, "white")));
            }
            Feature::Ears if subj != Subject::Cat && subj != Subject::Bat => {
                let ear = Shape::Poly(vec![(cx - w * 0.42, top + h * 0.2), (cx - w * 0.12, top + h * 0.05), (cx - w * 0.36, top - h * 0.22)]);
                b.pair(("ear_l", "ear_r"), (Group::Body, Group::Body), -1, ear, feature_color(Feature::Ears, main));
            }
            Feature::Wings if subj != Subject::Bat => {
                let span = fh("wing_span");
                let root = (cx - w * 0.42, cy - h * 0.05);
                let wing = Shape::Poly(vec![root, (cx - w * 0.42 - span, cy - h * 0.3), (cx - w * 0.42 - span * 0.7, cy + h * 0.05), (cx - w * 0.42 - span * 0.35, cy + h * 0.15)]);
                b.pair(("wing_l", "wing_r"), (Group::WingL, Group::WingR), -1, wing, feature_color(Feature::Wings, named(cfg, "white")));
                pivots.insert(Group::WingL, root);
                pivots.insert(Group::WingR, (1.0 - root.0, root.1));
                has_wings = true;
            }
            Feature::Crown => {
                let (ch, x0, x1) = (fh("crown_h"), cx - w * 0.25, cx + w * 0.25);
                let yb = top + h * 0.06;
                let crown = Shape::Poly(vec![(x0, yb), (x0, yb - ch), (x0 + (x1 - x0) * 0.25, yb - ch * 0.5), (cx, yb - ch), (x1 - (x1 - x0) * 0.25, yb - ch * 0.5), (x1, yb - ch), (x1, yb)]);
                b.add("crown", Group::Body, 2, crown, feature_color(Feature::Crown, named(cfg, "gold")));
            }
            Feature::Hat => {
                let hh = fh("hat_h");
                let c = feature_color(Feature::Hat, named(cfg, "black"));
                b.add("brim", Group::Body, 2, Shape::Rect { x0: cx - w * 0.36, y0: top + h * 0.02, x1: cx + w * 0.36, y1: top + h * 0.02 + hh * 0.22 }, c);
                b.add("hat", Group::Body, 2, Shape::Rect { x0: cx - w * 0.22, y0: top + h * 0.04 - hh, x1: cx + w * 0.22, y1: top + h * 0.04 }, c);
            }
            Feature::Antenna => {
                let ah = fh("antenna_h");
                let stalk = b.add("antenna", Group::Body, -1, Shape::Rect { x0: cx - 0.02, y0: top - ah, x1: cx + 0.02, y1: top + 0.02 }, named(cfg, "grey"));
                stalk.min_pixel = true;
                let ball = b.add("antenna_tip", Group::Body, 1, Shape::Ellipse { cx, cy: top - ah, rx: 0.05, ry: 0.05 }, feature_color(Feature::Antenna, named(cfg, "red")));
                ball.min_pixel = true;
            }
            Feature::Tail => {
                let tl = fh("tail_len");
                let x0 = cx + w * 0.4;
                let tail = Shape::Poly(vec![(x0, cy + h * 0.2), (x0 + tl, cy - h * 0.1), (x0 + tl * 1.1, cy - h * 0.28), (x0 + tl * 0.9, cy - h * 0.05), (x0 + w * 0.02, cy + h * 0.36)]);
                b.add("tail", Group::Tail, -1, tail, feature_color(Feature::Tail, main));
                pivots.insert(Group::Tail, (x0, cy + h * 0.25));
            }
            Feature::Spots => spots(&mut b, cfg, (cx, cy, w, h), feature_color(Feature::Spots, pal.light)),
            Feature::Stripes => {
                let c = feature_color(Feature::Stripes, pal.shade);
                let sh = fh("stripe_h");
                for dy in [-0.15, 0.05, 0.25] {
                    let s = b.add("stripe", Group::Body, 1, Shape::Rect { x0: cx - w, y0: cy + h * dy, x1: cx + w, y1: cy + h * dy + sh }, c);
                    s.clip = Some("body");
                }
            }
            _ => continue,
        }
        why.push((f.feature.v.name().to_string(), format!("\"{fw}\"")));
    }

    // ---- eyes and mouth
    let k = (w / 0.7).clamp(0.5, 1.1);
    let size_mul = if req.has(Feature::BigEyes).is_some() {
        cfg.shapes.big_eyes
    } else if req.has(Feature::SmallEyes).is_some() {
        cfg.shapes.small_eyes
    } else {
        1.0
    };
    let r = cfg.shapes.eye_r * size_mul * k;
    let eye_variant = chosen.get("eyes").copied();
    let one = req.has(Feature::OneEye).is_some();
    let eye_xs: Vec<f32> = if one { vec![face.0] } else { vec![face.0 - cfg.shapes.eye_gap * k / 2.0, face.0 + cfg.shapes.eye_gap * k / 2.0] };
    let r = if one { r * 1.3 } else { r };
    let eye_color = feature_color(Feature::Eyes, pal.eye_dark);
    if let Some(ev) = eye_variant {
        why.push(("eyes".into(), format!("eyes/{ev}")));
        let fy = face.1;
        for (i, &ex) in eye_xs.iter().enumerate() {
            let left = i == 0 && !one;
            match ev {
                "visor" => {}
                "dot" => {
                    let e = b.add("eye", Group::Body, 3, Shape::Ellipse { cx: ex, cy: fy, rx: r * 0.55, ry: r * 0.7 }, eye_color);
                    e.min_pixel = true;
                    e.show = Show::EyesOpen;
                }
                _ => {
                    let wht = b.add("eye_white", Group::Body, 3, Shape::Ellipse { cx: ex, cy: fy, rx: r, ry: r * 1.1 }, pal.eye_white);
                    wht.min_pixel = true;
                    wht.show = Show::EyesOpen;
                    let p = b.add("pupil", Group::Body, 4, Shape::Ellipse { cx: ex, cy: fy + r * 0.2, rx: r * cfg.shapes.pupil, ry: r * cfg.shapes.pupil * 1.2 }, eye_color);
                    p.min_pixel = true;
                    p.show = Show::EyesOpen;
                    if ev == "shine" {
                        let s = b.add("shine", Group::Body, 5, Shape::Rect { x0: ex - r * 0.45, y0: fy - r * 0.35, x1: ex - r * 0.1, y1: fy }, pal.eye_white);
                        s.show = Show::EyesOpen;
                    }
                    if ev == "angry" {
                        let brow = if left || one {
                            Shape::Poly(vec![(ex - r * 1.3, fy - r * 1.4), (ex + r * 1.35, fy - r * 1.4), (ex + r * 1.35, fy + r * 0.05)])
                        } else {
                            Shape::Poly(vec![(ex - r * 1.35, fy - r * 1.4), (ex + r * 1.3, fy - r * 1.4), (ex - r * 1.35, fy + r * 0.05)])
                        };
                        let bp = b.add("brow", Group::Body, 6, brow, main);
                        bp.show = Show::EyesOpen;
                        bp.clip = Some("eye_white");
                    }
                    if ev == "sleepy" {
                        let lid = b.add("lid", Group::Body, 6, Shape::Rect { x0: ex - r * 1.2, y0: fy - r * 1.3, x1: ex + r * 1.2, y1: fy - r * 0.05 }, main);
                        lid.show = Show::EyesOpen;
                        lid.clip = Some("eye_white");
                    }
                }
            }
            if ev != "visor" {
                let closed = b.add("eye_closed", Group::Body, 3, Shape::Rect { x0: ex - r, y0: fy, x1: ex + r, y1: fy + r * 0.45 }, eye_color);
                closed.min_pixel = true;
                closed.show = Show::EyesClosed;
            }
        }
        if ev == "visor" {
            let vw = w * 0.34;
            let v = b.add("visor", Group::Body, 3, Shape::Rect { x0: face.0 - vw, y0: fy - r * 0.55, x1: face.0 + vw, y1: fy + r * 0.55 }, pal.eye_dark);
            v.min_pixel = true;
            if subj == Subject::Robot {
                for &ex in &eye_xs {
                    let lamp = b.add("lamp", Group::Body, 4, Shape::Ellipse { cx: ex, cy: fy, rx: r * 0.45, ry: r * 0.45 }, feature_color(Feature::Eyes, named(cfg, "teal")));
                    lamp.min_pixel = true;
                    lamp.show = Show::EyesOpen;
                }
            }
        }
    }
    if let Some(mv) = chosen.get("mouth").copied() {
        why.push(("mouth".into(), format!("mouth/{mv}")));
        let (mw, mh) = (cfg.shapes.mouth_w * k, cfg.shapes.mouth_h * k);
        let (mx, my) = (face.0, face.1 + r * 1.9 + mh);
        match mv {
            "smile" => {
                let s = b.add("mouth", Group::Body, 3, Shape::Poly(vec![(mx - mw / 2.0, my - mh / 2.0), (mx - mw * 0.3, my + mh / 2.0), (mx + mw * 0.3, my + mh / 2.0), (mx + mw / 2.0, my - mh / 2.0), (mx + mw * 0.3, my), (mx - mw * 0.3, my)]), pal.mouth);
                s.min_pixel = true;
            }
            "open" => {
                b.add("mouth", Group::Body, 3, Shape::Ellipse { cx: mx, cy: my, rx: mw * 0.35, ry: mh * 0.95 }, pal.mouth).min_pixel = true;
            }
            "fangs" => {
                b.add("mouth", Group::Body, 3, Shape::Rect { x0: mx - mw / 2.0, y0: my - mh * 0.4, x1: mx + mw / 2.0, y1: my + mh * 0.4 }, pal.mouth).min_pixel = true;
                let fang = Shape::Poly(vec![(mx - mw * 0.42, my), (mx - mw * 0.12, my), (mx - mw * 0.27, my + mh * 1.6)]);
                b.pair(("fang_l", "fang_r"), (Group::Body, Group::Body), 4, fang, pal.eye_white);
            }
            _ => {}
        }
    }
    let _ = b.cfg;
    let symmetric = req.has(Feature::Tail).is_none();
    Ok(Figure {
        parts: b.parts,
        pal,
        pivots,
        body: [cx, cy, w, h],
        feet_y,
        symmetric,
        has_legs,
        has_wings,
        has_eyes: eye_variant.is_some_and(|e| e != "visor"),
        why,
    })
}

impl Shape {
    fn map(&mut self, f: &impl Fn((f32, f32)) -> (f32, f32), s: f32) {
        match self {
            Shape::Ellipse { cx, cy, rx, ry } => {
                (*cx, *cy) = f((*cx, *cy));
                *rx *= s;
                *ry *= s;
            }
            Shape::Egg { cx, cy, rx, ry_top, ry_bottom } => {
                (*cx, *cy) = f((*cx, *cy));
                *rx *= s;
                *ry_top *= s;
                *ry_bottom *= s;
            }
            Shape::Rect { x0, y0, x1, y1 } => {
                (*x0, *y0) = f((*x0, *y0));
                (*x1, *y1) = f((*x1, *y1));
            }
            Shape::Poly(p) => {
                for q in p.iter_mut() {
                    *q = f(*q);
                }
            }
        }
    }
}

impl Figure {
    /// Scale by `s` about `from` and move that point to `to` (the fit step).
    pub fn transform(&mut self, s: f32, from: (f32, f32), to: (f32, f32)) {
        let f = |p: (f32, f32)| ((p.0 - from.0) * s + to.0, (p.1 - from.1) * s + to.1);
        for part in &mut self.parts {
            part.shape.map(&f, s);
            if let Some((x0, y0, x1, y1)) = part.shade_box {
                let (a, b) = (f((x0, y0)), f((x1, y1)));
                part.shade_box = Some((a.0, a.1, b.0, b.1));
            }
        }
        for p in self.pivots.values_mut() {
            *p = f(*p);
        }
        let c = f((self.body[0], self.body[1]));
        self.body = [c.0, c.1, self.body[2] * s, self.body[3] * s];
        self.feet_y = f((0.0, self.feet_y)).1;
    }
}

fn spots(b: &mut B, cfg: &Config, (cx, cy, w, h): (f32, f32, f32, f32), c: Rgb) {
    let r = cfg.feature("spot_r");
    for (dx, dy) in [(-0.22, -0.12), (0.2, 0.02), (-0.02, 0.22)] {
        let s = b.add("spot", Group::Body, 1, Shape::Ellipse { cx: cx + w * dx, cy: cy + h * dy, rx: r, ry: r }, c);
        s.clip = Some("body");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shapes_contain_their_centre() {
        for s in [
            Shape::Ellipse { cx: 0.5, cy: 0.5, rx: 0.2, ry: 0.1 },
            Shape::Egg { cx: 0.5, cy: 0.5, rx: 0.2, ry_top: 0.2, ry_bottom: 0.1 },
            Shape::Rect { x0: 0.4, y0: 0.4, x1: 0.6, y1: 0.6 },
            Shape::Poly(vec![(0.4, 0.4), (0.6, 0.4), (0.5, 0.6)]),
        ] {
            assert!(s.contains(0.5, 0.45), "{s:?}");
            assert!(!s.contains(0.05, 0.05), "{s:?}");
        }
    }

    #[test]
    fn mirror_is_across_the_middle() {
        let s = Shape::Rect { x0: 0.1, y0: 0.2, x1: 0.3, y1: 0.4 };
        assert!(s.mirror().contains(0.8, 0.3));
    }
}
