//! Flat colour, light from the top-left, or a rim of light and shadow.

use super::*;

pub struct Shading;

impl Rule for Shading {
    fn name(&self) -> &'static str {
        "shading"
    }
    fn all_variants(&self) -> &'static [&'static str] {
        &["flat", "top_light", "rim", "lit"]
    }
    fn needed(&self, cx: &Ctx, chosen: &Chosen) -> Option<String> {
        let _ = (cx, chosen);
        Some("every sprite is shaded somehow".into())
    }
    fn offers(&self, cx: &Ctx, chosen: &Chosen) -> Result<Vec<Offer>, String> {
        let _ = (cx, chosen);
        if let Some(s) = cx.req.style(Style::Flat) {
            return Ok(vec![Offer::named("flat", s.words.clone())]);
        }
        if let Some(s) = cx.req.style(Style::Shaded) {
            return Ok(vec![Offer::named("top_light", s.words.clone()), Offer::named("rim", s.words.clone()), Offer::named("lit", s.words.clone())]);
        }
        Ok(vec![Offer::plain("flat"), Offer::plain("top_light"), Offer::plain("rim"), Offer::plain("lit")])
    }
}

#[cfg(test)]
mod tests {
    use super::test_util::offers;
    use super::*;

    #[test]
    fn flat_when_asked() {
        assert_eq!(offers(&Shading, "a flat slime"), Some(vec!["flat"]));
    }

    #[test]
    fn otherwise_all() {
        assert_eq!(offers(&Shading, "a slime"), Some(vec!["flat", "top_light", "rim", "lit"]));
    }

}
