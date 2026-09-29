//! Beam search over the design rules (same machine as every HotMotor engine). A variant scores
//! `weight * (1 + axes . (profile - 0.5))` plus an evidence bonus when the sentence named it;
//! judges rescore whole candidates; ties break on offer order, so results are deterministic.

use crate::rules::{self, Chosen, Ctx};
use std::cmp::Ordering;

#[derive(Clone, Debug)]
pub struct Step {
    pub rule: &'static str,
    pub variant: &'static str,
    pub score: f32,
    pub why: String,
    pub evidence: Option<String>,
}

#[derive(Clone, Debug)]
pub struct Candidate {
    pub chosen: Chosen,
    pub steps: Vec<Step>,
    pub local: f32,
    pub judges: Vec<(f32, String)>,
    pub score: f32,
    key: Vec<u16>,
}

pub struct Outcome {
    pub finalists: Vec<Candidate>,
    pub explored: usize,
}

/// Rules that change the silhouette; finalists differ in these.
const SHAPE: &[&str] = &["body", "eyes", "outline"];

fn shape(c: &Candidate) -> Vec<Option<&'static str>> {
    SHAPE.iter().map(|r| c.chosen.get(r).copied()).collect()
}

pub fn variant_score(cx: &Ctx, rule: &str, v: &str, evidence: bool) -> f32 {
    let vc = cx.cfg.variant(rule, v);
    let dot: f32 = (0..3).map(|i| vc.axes[i] * (cx.profile[i] - 0.5)).sum();
    vc.weight * (1.0 + dot) + if evidence { cx.cfg.search.evidence_bonus } else { 0.0 }
}

pub fn search(cx: &Ctx) -> Result<Outcome, String> {
    let mut beam = vec![Candidate { chosen: Chosen::new(), steps: vec![], local: 0.0, judges: vec![], score: 0.0, key: vec![] }];
    let mut explored = 0;
    for rule in rules::all() {
        let mut next = Vec::new();
        let mut failure = None;
        for c in &beam {
            let Some(why) = rule.needed(cx, &c.chosen) else {
                next.push(c.clone());
                continue;
            };
            let offers = match rule.offers(cx, &c.chosen) {
                Ok(o) if !o.is_empty() => o,
                Ok(_) => continue,
                Err(e) => {
                    failure.get_or_insert(e);
                    continue;
                }
            };
            for (i, o) in offers.iter().enumerate() {
                explored += 1;
                let s = variant_score(cx, rule.name(), o.variant, o.evidence.is_some());
                let mut n = c.clone();
                n.chosen.insert(rule.name(), o.variant);
                n.steps.push(Step { rule: rule.name(), variant: o.variant, score: s, why: why.clone(), evidence: o.evidence.clone() });
                n.local += s;
                n.key.push(i as u16);
                n.judges = rules::judges::score(cx, &n.chosen);
                n.score = n.local + n.judges.iter().map(|j| j.0).sum::<f32>();
                next.push(n);
            }
        }
        if next.is_empty() {
            return Err(failure.unwrap_or_else(|| format!("{}: nothing fits", rule.name())));
        }
        next.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(Ordering::Equal).then_with(|| a.key.cmp(&b.key)));
        let mut per: std::collections::BTreeMap<Vec<Option<&str>>, usize> = Default::default();
        next.retain(|c| {
            let n = per.entry(shape(c)).or_insert(0);
            *n += 1;
            *n <= cx.cfg.search.per_shape
        });
        next.truncate(cx.cfg.search.beam_width);
        beam = next;
    }
    let mut seen = std::collections::BTreeSet::new();
    let finalists: Vec<Candidate> = beam.into_iter().filter(|c| seen.insert(shape(c))).take(cx.cfg.search.finalists).collect();
    Ok(Outcome { finalists, explored })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    fn best(s: &str) -> Chosen {
        let cfg = Config::load(None).unwrap();
        let req = crate::parser::parse(s, false).unwrap();
        let cx = Ctx { req: &req, cfg: &cfg, profile: rules::profile(&req, &cfg), size: cfg.size_of(&req) };
        search(&cx).unwrap().finalists.remove(0).chosen
    }

    #[test]
    fn deterministic() {
        let a = best("a green slime with big eyes, bouncing");
        for _ in 0..5 {
            assert_eq!(a, best("a green slime with big eyes, bouncing"));
        }
    }

    #[test]
    fn mood_changes_the_face() {
        let cute = best("a cute slime");
        let spooky = best("a spooky slime");
        assert_ne!(cute.get("eyes"), spooky.get("eyes"));
    }

    #[test]
    fn tiny_sprites_keep_an_outline() {
        assert_ne!(best("a tiny slime").get("outline").copied(), Some("none"));
    }
}
