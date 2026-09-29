//! The eyes: tiny dots, big eyes with a shine, angry, sleepy, or a visor (knights and robots).
//! Items only have eyes when the sentence gives them a face.

use super::*;

pub struct Eyes;

impl Rule for Eyes {
    fn name(&self) -> &'static str {
        "eyes"
    }
    fn all_variants(&self) -> &'static [&'static str] {
        &["dot", "shine", "angry", "sleepy", "visor"]
    }
    fn needed(&self, cx: &Ctx, chosen: &Chosen) -> Option<String> {
        let _ = (cx, chosen);
        has_face(cx.req)
    }
    fn offers(&self, cx: &Ctx, chosen: &Chosen) -> Result<Vec<Offer>, String> {
        let _ = (cx, chosen);
        let r = cx.req;
        if let Some(f) = r.has(Feature::Angry) {
            return Ok(vec![Offer::named("angry", f.feature.words.clone())]);
        }
        if let Some(f) = r.has(Feature::Sleepy) {
            return Ok(vec![Offer::named("sleepy", f.feature.words.clone())]);
        }
        if let Some(f) = r.has(Feature::BigEyes) {
            return Ok(vec![Offer::named("shine", f.feature.words.clone())]);
        }
        if let Some(f) = r.has(Feature::SmallEyes) {
            return Ok(vec![Offer::named("dot", f.feature.words.clone())]);
        }
        Ok(match r.subject.v {
            Subject::Knight => vec![Offer::plain("visor")],
            Subject::Robot => vec![Offer::plain("visor"), Offer::plain("dot")],
            _ => vec![Offer::plain("dot"), Offer::plain("shine"), Offer::plain("angry"), Offer::plain("sleepy")],
        })
    }
}

#[cfg(test)]
mod tests {
    use super::test_util::offers;
    use super::*;

    #[test]
    fn big_eyes_shine() {
        assert_eq!(offers(&Eyes, "a slime with big eyes"), Some(vec!["shine"]));
    }

    #[test]
    fn plain_coins_have_no_eyes() {
        assert_eq!(offers(&Eyes, "a coin"), None);
    }

}
