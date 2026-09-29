//! Design rules, one decision per file. A rule says whether the sprite needs the decision and
//! which variants are allowed (and which the sentence named). Drawing is `figure.rs`'s job.

use crate::config::Config;
use crate::request::*;
use std::collections::BTreeMap;

pub mod body;
pub mod eyes;
pub mod judges;
pub mod mouth;
pub mod outline;
pub mod palette;
pub mod shading;
pub mod timing;

pub struct Ctx<'a> {
    pub req: &'a Request,
    pub cfg: &'a Config,
    /// [cute, readable, detail], each 0..1
    pub profile: [f32; 3],
    /// canvas size in pixels
    pub size: u32,
}

pub type Chosen = BTreeMap<&'static str, &'static str>;

#[derive(Clone, Debug, PartialEq)]
pub struct Offer {
    pub variant: &'static str,
    pub evidence: Option<String>,
}

impl Offer {
    pub fn plain(v: &'static str) -> Offer {
        Offer { variant: v, evidence: None }
    }
    pub fn named(v: &'static str, w: impl Into<String>) -> Offer {
        Offer { variant: v, evidence: Some(w.into()) }
    }
}

pub trait Rule: Sync {
    fn name(&self) -> &'static str;
    fn all_variants(&self) -> &'static [&'static str];
    fn needed(&self, cx: &Ctx, chosen: &Chosen) -> Option<String>;
    fn offers(&self, cx: &Ctx, chosen: &Chosen) -> Result<Vec<Offer>, String>;
}

pub fn all() -> Vec<&'static dyn Rule> {
    vec![&body::Body, &palette::Palette, &shading::Shading, &outline::Outline, &eyes::Eyes, &mouth::Mouth, &timing::Timing]
}

pub fn profile(req: &Request, cfg: &Config) -> [f32; 3] {
    let mut p = cfg.profile.default;
    for m in &req.mods {
        let s = cfg.profile.shift[m.v.name()];
        for i in 0..3 {
            p[i] += s[i];
        }
    }
    p.map(|x| x.clamp(0.0, 1.0))
}

pub const FACE_FEATURES: &[Feature] =
    &[Feature::Eyes, Feature::BigEyes, Feature::SmallEyes, Feature::OneEye, Feature::Angry, Feature::Sleepy, Feature::Happy, Feature::Fangs];

pub fn is_item(s: Subject) -> bool {
    matches!(s, Subject::Coin | Subject::Gem | Subject::Star | Subject::Heart | Subject::Potion)
}

/// Creatures always have a face; items only when the sentence gives them one (or asks for cute).
pub fn has_face(req: &Request) -> Option<String> {
    if !is_item(req.subject.v) {
        return Some(format!("a {} has a face", req.subject.v.name()));
    }
    if let Some(f) = req.features.iter().find(|f| FACE_FEATURES.contains(&f.feature.v)) {
        return Some(f.feature.words.clone());
    }
    req.mods.iter().find(|m| m.v == Mod::Cute).map(|m| m.words.clone())
}

#[cfg(test)]
pub mod test_util {
    use super::*;

    pub fn offers(rule: &dyn Rule, sentence: &str) -> Option<Vec<&'static str>> {
        let cfg = Config::load(None).unwrap();
        let req = crate::parser::parse(sentence, false).unwrap_or_else(|d| panic!("{d:?}"));
        let cx = Ctx { req: &req, cfg: &cfg, profile: profile(&req, &cfg), size: cfg.size_of(&req) };
        let ch = Chosen::new();
        rule.needed(&cx, &ch)?;
        Some(rule.offers(&cx, &ch).ok()?.into_iter().map(|o| o.variant).collect())
    }
}
