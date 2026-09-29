//! How motion is spaced over the loop: evenly, eased at the ends, or snapping and holding.

use super::*;

pub struct Timing;

impl Rule for Timing {
    fn name(&self) -> &'static str {
        "timing"
    }
    fn all_variants(&self) -> &'static [&'static str] {
        &["linear", "eased", "snappy"]
    }
    fn needed(&self, cx: &Ctx, chosen: &Chosen) -> Option<String> {
        let _ = (cx, chosen);
        cx.req.anims.first().map(|a| a.words.clone())
    }
    fn offers(&self, cx: &Ctx, chosen: &Chosen) -> Result<Vec<Offer>, String> {
        let _ = (cx, chosen);
        Ok(vec![Offer::plain("linear"), Offer::plain("eased"), Offer::plain("snappy")])
    }
}

#[cfg(test)]
mod tests {
    use super::test_util::offers;
    use super::*;

    #[test]
    fn with_an_animation() {
        assert!(offers(&Timing, "a bouncing slime").is_some());
    }

    #[test]
    fn not_for_a_still_sprite() {
        assert_eq!(offers(&Timing, "a slime"), None);
    }

}
