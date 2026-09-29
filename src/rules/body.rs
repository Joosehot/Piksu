//! The body proportions: round, tall or wide (sizes per subject in rules.toml [shapes.*]).

use super::*;

pub struct Body;

impl Rule for Body {
    fn name(&self) -> &'static str {
        "body"
    }
    fn all_variants(&self) -> &'static [&'static str] {
        &["round", "tall", "wide"]
    }
    fn needed(&self, cx: &Ctx, chosen: &Chosen) -> Option<String> {
        let _ = (cx, chosen);
        Some("every sprite has a body".into())
    }
    fn offers(&self, cx: &Ctx, chosen: &Chosen) -> Result<Vec<Offer>, String> {
        let _ = (cx, chosen);
        Ok(vec![Offer::plain("round"), Offer::plain("tall"), Offer::plain("wide")])
    }
}

#[cfg(test)]
mod tests {
    use super::test_util::offers;
    use super::*;

    #[test]
    fn always_three_shapes() {
        assert_eq!(offers(&Body, "a slime"), Some(vec!["round", "tall", "wide"]));
    }

    #[test]
    fn items_too() {
        assert!(offers(&Body, "a coin").is_some());
    }

}
