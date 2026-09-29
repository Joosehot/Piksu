//! The mouth: none, a smile, open, or fangs. Only with a face.

use super::*;

pub struct Mouth;

impl Rule for Mouth {
    fn name(&self) -> &'static str {
        "mouth"
    }
    fn all_variants(&self) -> &'static [&'static str] {
        &["none", "smile", "open", "fangs"]
    }
    fn needed(&self, cx: &Ctx, chosen: &Chosen) -> Option<String> {
        let _ = (cx, chosen);
        has_face(cx.req)
    }
    fn offers(&self, cx: &Ctx, chosen: &Chosen) -> Result<Vec<Offer>, String> {
        let _ = (cx, chosen);
        let r = cx.req;
        if let Some(f) = r.has(Feature::Fangs) {
            return Ok(vec![Offer::named("fangs", f.feature.words.clone())]);
        }
        if let Some(f) = r.has(Feature::Happy) {
            return Ok(vec![Offer::named("smile", f.feature.words.clone())]);
        }
        if matches!(r.subject.v, Subject::Knight | Subject::Robot) {
            return Ok(vec![Offer::plain("none")]);
        }
        Ok(vec![Offer::plain("none"), Offer::plain("smile"), Offer::plain("open"), Offer::plain("fangs")])
    }
}

#[cfg(test)]
mod tests {
    use super::test_util::offers;
    use super::*;

    #[test]
    fn happy_smiles() {
        assert_eq!(offers(&Mouth, "a happy slime"), Some(vec!["smile"]));
    }

    #[test]
    fn no_face_no_mouth() {
        assert_eq!(offers(&Mouth, "a gem"), None);
    }

}
