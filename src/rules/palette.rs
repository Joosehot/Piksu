//! How the colour ramp is built: shadows shifting toward blue and highlights toward yellow, or
//! only darker/lighter. Greys have no hue to shift.

use super::*;

pub struct Palette;

impl Rule for Palette {
    fn name(&self) -> &'static str {
        "palette"
    }
    fn all_variants(&self) -> &'static [&'static str] {
        &["hue_shift", "value_only"]
    }
    fn needed(&self, cx: &Ctx, chosen: &Chosen) -> Option<String> {
        let _ = (cx, chosen);
        Some("every sprite has colours".into())
    }
    fn offers(&self, cx: &Ctx, chosen: &Chosen) -> Result<Vec<Offer>, String> {
        let _ = (cx, chosen);
        let name = cx.req.color.as_ref().map(|c| c.v.0).unwrap_or_else(|| {
            let d = &cx.cfg.default_color[cx.req.subject.v.name()];
            *Color::ALL.iter().find(|c| c.name() == d).expect("validated")
        });
        let (_, s, _) = crate::color::to_hsv(cx.cfg.colors[name.name()]);
        Ok(if s < 0.15 { vec![Offer::plain("value_only")] } else { vec![Offer::plain("hue_shift"), Offer::plain("value_only")] })
    }
}

#[cfg(test)]
mod tests {
    use super::test_util::offers;
    use super::*;

    #[test]
    fn colourful_bodies_may_shift_hue() {
        assert_eq!(offers(&Palette, "a green slime"), Some(vec!["hue_shift", "value_only"]));
    }

    #[test]
    fn greys_only_change_value() {
        assert_eq!(offers(&Palette, "a grey robot"), Some(vec!["value_only"]));
    }

}
