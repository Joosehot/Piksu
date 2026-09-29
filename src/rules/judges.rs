//! Judges score combinations of choices. Each fires only once its rules are decided.

use super::*;

pub fn score(cx: &Ctx, chosen: &Chosen) -> Vec<(f32, String)> {
    let is = |r: &str, v: &str| chosen.get(r).copied() == Some(v);
    let j = |n: &str| cx.cfg.judge(n);
    let th = &cx.cfg.thresholds;
    let mut out = Vec::new();
    if cx.size < th.detail_min_size && (is("shading", "rim") || is("eyes", "shine")) {
        out.push((j("small_canvas_detail"), format!("fine detail on a {}px canvas", cx.size)));
    }
    if cx.size < th.outline_min_size && is("outline", "none") {
        out.push((j("no_outline_small"), format!("no outline on a {}px canvas", cx.size)));
    }
    if cx.req.has_mod(Mod::Cute) && is("eyes", "shine") && is("mouth", "smile") {
        out.push((j("cute_needs_soft_face"), "shiny eyes and a smile".into()));
    }
    if cx.req.has_mod(Mod::Spooky) && (is("eyes", "angry") || is("mouth", "fangs")) {
        out.push((j("spooky_needs_hard_face"), "an angry or fanged face".into()));
    }
    if is("outline", "colored") && is("palette", "hue_shift") {
        out.push((j("colored_outline_with_hue_shift"), "coloured outline on a hue-shifted ramp".into()));
    }
    if cx.size > th.dot_eyes_max_size && is("eyes", "dot") {
        out.push((j("dot_eyes_on_big_canvas"), format!("dot eyes on a {}px canvas", cx.size)));
    }
    let real = is("realism", "realistic");
    if real && is("shading", "lit") {
        out.push((j("realistic_needs_lit"), "directional light for realism".into()));
    }
    if real && is("shading", "flat") {
        out.push((j("realistic_not_flat"), "flat colour can't look real".into()));
    }
    if is("realism", "cartoon") && is("shading", "lit") {
        out.push((j("cartoon_avoids_lit"), "a cartoon lit like a photo".into()));
    }
    if real && cx.size < th.realism_min_size {
        out.push((j("realistic_small_canvas"), format!("realism on a {}px canvas", cx.size)));
    }
    if real && is("eyes", "shine") && cx.req.has(Feature::BigEyes).is_none() {
        out.push((j("realistic_no_shine_eyes"), "glossy cartoon eyes on a realistic sprite".into()));
    }
    if is("realism", "stylized") && is("shading", "lit") {
        out.push((j("stylized_likes_lit"), "directional light for a stylized look".into()));
    }
    if real && is("outline", "colored") {
        out.push((j("realistic_colored_outline"), "a selective outline for realism".into()));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fired(s: &str, chosen: &[(&'static str, &'static str)]) -> Vec<String> {
        let cfg = Config::load(None).unwrap();
        let req = crate::parser::parse(s, false).unwrap();
        let cx = Ctx { req: &req, cfg: &cfg, profile: profile(&req, &cfg), size: cfg.size_of(&req) };
        score(&cx, &chosen.iter().copied().collect()).into_iter().map(|x| x.1).collect()
    }

    #[test]
    fn tiny_sprites_want_outlines() {
        assert_eq!(fired("a tiny slime", &[("outline", "none")]), vec!["no outline on a 8px canvas"]);
    }

    #[test]
    fn cute_faces() {
        assert_eq!(fired("a cute slime", &[("eyes", "shine"), ("mouth", "smile")]), vec!["shiny eyes and a smile"]);
    }
}
