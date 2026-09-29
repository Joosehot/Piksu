//! Piksu: a sentence in, pixel art and animation frames out. A HotMotor engine:
//! lexicon -> parser -> rules with variants -> beam search with judges -> draw -> checks.
//! No model runs; the same sentence and rules.toml give the same pixels.

pub mod anim;
pub mod checks;
pub mod color;
pub mod config;
pub mod figure;
pub mod lexicon;
pub mod output;
pub mod parser;
pub mod render;
pub mod request;
pub mod rules;
pub mod search;

use config::Config;
use render::Grid;
use request::*;
use std::fmt::Write as _;

pub struct Options {
    pub lenient: bool,
}

pub struct Sprite {
    pub request: Request,
    pub size: u32,
    /// (animation name, frames); a still sprite is one animation "still" with one frame.
    pub anims: Vec<(String, Vec<Grid>)>,
    pub chosen: rules::Chosen,
    pub why: Vec<(String, String)>,
    pub explain: String,
}

/// The box every frame of every requested animation covers (canvas fractions).
fn extent(fig: &figure::Figure, req: &Request, cfg: &Config, timing: &str) -> (f32, f32, f32, f32) {
    let mut states = vec![anim::FrameState::default()];
    for a in &req.anims {
        let n = anim::frame_count(a.v, req.frames.as_ref().map(|f| f.v), cfg);
        states.extend(anim::frames(a.v, n, timing, fig, cfg));
    }
    let mut b = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for st in &states {
        for part in &fig.parts {
            let g = st.groups.get(&part.group).copied().unwrap_or_default();
            let (x0, y0, x1, y1) = part.shape.bbox();
            for c in [(x0, y0), (x1, y0), (x0, y1), (x1, y1)] {
                let p = render::forward(&st.whole, &g, c);
                b = (b.0.min(p.0), b.1.min(p.1), b.2.max(p.0), b.3.max(p.1));
            }
        }
    }
    b
}

/// Fit the figure (with all its motion) inside the canvas, leaving room for the outline.
fn fit(mut fig: figure::Figure, req: &Request, cfg: &Config, timing: &str, size: u32, outlined: bool) -> figure::Figure {
    let edge_px = if outlined { 2.0 } else { 1.0 };
    let m = cfg.shapes.margin.max((edge_px + 0.25) / size as f32);
    // twice: motion amplitudes don't scale with the figure, so the second pass settles it
    for _ in 0..2 {
        let (x0, y0, x1, y1) = extent(&fig, req, cfg, timing);
        let s = ((1.0 - 2.0 * m) / (x1 - x0)).min((1.0 - 2.0 * m) / (y1 - y0)).min(cfg.shapes.max_grow);
        fig.transform(s, ((x0 + x1) / 2.0, (y0 + y1) / 2.0), (0.5, 0.5));
    }
    fig
}

pub fn make(sentence: &str, cfg: &Config, opts: &Options) -> Result<Sprite, Vec<parser::Diag>> {
    let req = parser::parse(sentence, opts.lenient)?;
    let size = cfg.size_of(&req);
    if size < cfg.sizes.min || size > cfg.sizes.max {
        return Err(vec![parser::Diag::new(format!("a {size}px sprite is outside {}..{} pixels", cfg.sizes.min, cfg.sizes.max))]);
    }
    let cx = rules::Ctx { req: &req, cfg, profile: rules::profile(&req, cfg), size };
    let outcome = search::search(&cx).map_err(|e| vec![parser::Diag::new(e)])?;
    let mut ex = String::new();
    let toks: Vec<String> = lexicon::tokenize(sentence).0.iter().map(|t| format!("{:?}", t.tok)).collect();
    let _ = writeln!(ex, "tokens: {}", toks.join(" "));
    let _ = writeln!(ex, "size: {size}px   profile [cute, readable, detail] = {:?}", cx.profile);
    let _ = writeln!(ex, "{} candidates scored, {} finalists", outcome.explored, outcome.finalists.len());
    let mut failures = Vec::new();
    for (k, cand) in outcome.finalists.iter().enumerate() {
        let picks: Vec<String> = cand.steps.iter().map(|s| format!("{}={}", s.rule, s.variant)).collect();
        let _ = writeln!(ex, "\nfinalist #{k} score {:.2}: {}", cand.score, picks.join(" "));
        for (s, n) in &cand.judges {
            let _ = writeln!(ex, "  judge {s:+.2} {n}");
        }
        let fig = match figure::build(&req, cfg, &cand.chosen) {
            Ok(f) => fit(f, &req, cfg, cand.chosen.get("timing").copied().unwrap_or("eased"), size, cand.chosen["outline"] != "none"),
            Err(e) => {
                let _ = writeln!(ex, "  build failed: {e}");
                failures.push(e);
                continue;
            }
        };
        let item = rules::is_item(req.subject.v);
        let mut anims = Vec::new();
        let mut bad = None;
        let timing = cand.chosen.get("timing").copied().unwrap_or("eased");
        let style = render::Style { shading: cand.chosen["shading"], outline: cand.chosen["outline"] };
        if req.anims.is_empty() {
            anims.push(("still".to_string(), vec![render::render(&fig, &anim::FrameState::default(), size, &style, cfg)]));
        }
        for a in &req.anims {
            if let Err(e) = anim::check_fits(a.v, &fig, &req.subject.words, item) {
                bad = Some(e);
                break;
            }
            let n = anim::frame_count(a.v, req.frames.as_ref().map(|f| f.v), cfg);
            let frames: Vec<Grid> = anim::frames(a.v, n, timing, &fig, cfg).iter().map(|f| render::render(&fig, f, size, &style, cfg)).collect();
            anims.push((a.v.name().to_string(), frames));
        }
        if let Some(e) = bad {
            // no variant can give a slime legs: this is the request, not the finalist
            return Err(vec![parser::Diag::new(e)]);
        }
        match checks::run(&fig, &anims, cand.chosen.get("eyes").copied(), cand.chosen["outline"], cfg) {
            Ok(passed) => {
                for p in &passed {
                    let _ = writeln!(ex, "  check ok: {p}");
                }
                let mut why = fig.why.clone();
                for s in &cand.steps {
                    let by = s.evidence.as_ref().map(|w| format!("\"{w}\"")).unwrap_or_else(|| "scored best for the profile".into());
                    why.push((s.rule.to_string(), format!("{}/{} ({by}; {})", s.rule, s.variant, s.why)));
                }
                for a in &req.anims {
                    why.push((format!("animation {}", a.v.name()), format!("\"{}\"", a.words)));
                }
                return Ok(Sprite { request: req.clone(), size, anims, chosen: cand.chosen.clone(), why, explain: ex });
            }
            Err(e) => {
                let _ = writeln!(ex, "  check failed: {e}");
                failures.push(e);
            }
        }
    }
    let mut d = parser::Diag::new("no design passed the checks");
    if let Some(f) = failures.first() {
        d = d.hint(f.clone());
    }
    Err(vec![d])
}
