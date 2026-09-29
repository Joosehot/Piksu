//! How close to life: cartoon (flat-ish, big eyes, bright), stylized (lit, a little texture),
//! or realistic (directional light in five tones, material texture, smaller eyes, natural
//! colours). Words like "realistic" or "cartoon" name it; otherwise the profile decides.

use super::*;

pub struct Realism_;

impl Rule for Realism_ {
    fn name(&self) -> &'static str {
        "realism"
    }
    fn all_variants(&self) -> &'static [&'static str] {
        &["cartoon", "stylized", "realistic"]
    }
    fn needed(&self, _cx: &Ctx, _chosen: &Chosen) -> Option<String> {
        Some("every sprite sits somewhere between cartoon and life".into())
    }
    fn offers(&self, cx: &Ctx, _chosen: &Chosen) -> Result<Vec<Offer>, String> {
        if let Some(r) = &cx.req.realism {
            return Ok(vec![Offer::named(r.v.name(), r.words.clone())]);
        }
        Ok(vec![Offer::plain("cartoon"), Offer::plain("stylized"), Offer::plain("realistic")])
    }
}


#[cfg(test)]
mod tests {
    use super::test_util::offers;
    use super::*;

    #[test]
    fn named_realism_wins() {
        assert_eq!(offers(&Realism_, "a realistic frog"), Some(vec!["realistic"]));
    }

    #[test]
    fn otherwise_all_three() {
        assert_eq!(offers(&Realism_, "a frog"), Some(vec!["cartoon", "stylized", "realistic"]));
    }
}
