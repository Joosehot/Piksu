//! A dark outline, a coloured one (darker body colour per pixel), or none.

use super::*;

pub struct Outline;

impl Rule for Outline {
    fn name(&self) -> &'static str {
        "outline"
    }
    fn all_variants(&self) -> &'static [&'static str] {
        &["dark", "colored", "none"]
    }
    fn needed(&self, cx: &Ctx, chosen: &Chosen) -> Option<String> {
        let _ = (cx, chosen);
        Some("every sprite has an edge".into())
    }
    fn offers(&self, cx: &Ctx, chosen: &Chosen) -> Result<Vec<Offer>, String> {
        let _ = (cx, chosen);
        if let Some(s) = cx.req.style(Style::NoOutline) {
            return Ok(vec![Offer::named("none", s.words.clone())]);
        }
        if let Some(s) = cx.req.style(Style::Outlined) {
            return Ok(vec![Offer::named("dark", s.words.clone()), Offer::named("colored", s.words.clone())]);
        }
        Ok(vec![Offer::plain("dark"), Offer::plain("colored"), Offer::plain("none")])
    }
}

#[cfg(test)]
mod tests {
    use super::test_util::offers;
    use super::*;

    #[test]
    fn none_when_asked() {
        assert_eq!(offers(&Outline, "a slime with no outline"), Some(vec!["none"]));
    }

    #[test]
    fn otherwise_all() {
        assert_eq!(offers(&Outline, "a slime"), Some(vec!["dark", "colored", "none"]));
    }

}
