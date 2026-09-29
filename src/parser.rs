//! A sentence -> [`Request`].
//! - Exactly one subject ("a slime", "a ghost").
//! - A colour (with an optional "light"/"dark") before the subject colours the body; before a
//!   feature it colours that feature ("a ghost with red eyes" colours the eyes).
//! - A size word or "16x16" / "16 pixels" sets the canvas; "6 frames" sets the frame count.
//! - "no" / "without" before a style or feature removes it ("no outline").
//! - Anything left unused is an error.

use crate::lexicon::{self, Tok};
use crate::request::*;
use std::fmt;

#[derive(Clone, Debug, PartialEq)]
pub struct Diag {
    pub message: String,
    pub hint: Option<String>,
}

impl Diag {
    pub fn new(m: impl Into<String>) -> Diag {
        Diag { message: m.into(), hint: None }
    }
    pub fn hint(mut self, h: impl Into<String>) -> Diag {
        self.hint = Some(h.into());
        self
    }
}

impl fmt::Display for Diag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "error: {}", self.message)?;
        if let Some(h) = &self.hint {
            write!(f, "\n  hint: {h}")?;
        }
        Ok(())
    }
}

pub fn parse(sentence: &str, lenient: bool) -> Result<Request, Vec<Diag>> {
    let (tokens, unknown) = lexicon::tokenize(sentence);
    let mut diags = Vec::new();
    if !lenient {
        for w in &unknown {
            let d = Diag::new(format!("no rule knows the word \"{w}\""));
            diags.push(match lexicon::closest(w) {
                Some(c) => d.hint(format!("did you mean \"{c}\"? (--lenient skips unknown words)")),
                None => d.hint("rephrase with known words (`piksu vocabulary` lists them), or pass --lenient"),
            });
        }
    }
    let mut subject: Option<Val<Subject>> = None;
    let mut size: Option<Val<SizeReq>> = None;
    let mut color: Option<Val<(Color, Option<Tone>)>> = None;
    let mut features: Vec<FeatureReq> = Vec::new();
    let mut anims: Vec<Val<Anim>> = Vec::new();
    let mut frames: Option<Val<u32>> = None;
    let mut mods: Vec<Val<Mod>> = Vec::new();
    let mut styles: Vec<Val<Style>> = Vec::new();
    let mut realism: Option<Val<Realism>> = None;

    let mut pending_color: Option<(Color, Option<Tone>, String)> = None;
    let mut pending_tone: Option<(Tone, String)> = None;
    let mut pending_num: Option<(u32, String)> = None;
    let mut pending_no: Option<String> = None;

    for t in &tokens {
        let w = t.text.clone();
        match t.tok {
            Tok::Subject(s) => {
                if let Some(prev) = &subject {
                    if prev.v != s {
                        diags.push(Diag::new(format!("one sprite at a time: \"{}\" and \"{w}\"", prev.words)).hint("describe one thing per sentence"));
                        continue;
                    }
                }
                subject = Some(Val::new(s, w.clone()));
                if let Some((c, tone, cw)) = pending_color.take() {
                    color = Some(Val::new((c, tone), format!("{cw} {w}")));
                }
            }
            Tok::Tone(tone) => pending_tone = Some((tone, w)),
            Tok::Color(c) => {
                let (tone, tw) = pending_tone.take().map(|(t, tw)| (Some(t), format!("{tw} "))).unwrap_or((None, String::new()));
                pending_color = Some((c, tone, format!("{tw}{w}")));
            }
            Tok::Feature(f) => {
                if pending_no.take().is_some() {
                    features.retain(|x| x.feature.v != f);
                    continue;
                }
                let col = pending_color.take().map(|(c, tone, cw)| Val::new((c, tone), format!("{cw} {w}")));
                if let Some(x) = features.iter_mut().find(|x| x.feature.v == f) {
                    if col.is_some() {
                        x.color = col;
                    }
                } else {
                    features.push(FeatureReq { feature: Val::new(f, w), color: col });
                }
            }
            Tok::Anim(a) => {
                if !anims.iter().any(|x| x.v == a) {
                    anims.push(Val::new(a, w));
                }
            }
            Tok::Mod(m) => mods.push(Val::new(m, w)),
            Tok::Realism(r) => realism = Some(Val::new(r, w)),
            Tok::Style(s) => {
                if let Some(nw) = pending_no.take() {
                    match s {
                        Style::Outlined => styles.push(Val::new(Style::NoOutline, format!("{nw} {w}"))),
                        Style::Shaded => styles.push(Val::new(Style::Flat, format!("{nw} {w}"))),
                        _ => diags.push(Diag::new(format!("\"{nw} {w}\" doesn't mean anything"))),
                    }
                } else {
                    styles.push(Val::new(s, w));
                }
            }
            Tok::Pixels(n) => size = Some(Val::new(SizeReq::Px(n), w)),
            Tok::SizeWord(sw) => size = Some(Val::new(SizeReq::Word(sw), w)),
            Tok::Num(n) => pending_num = Some((n, w)),
            Tok::Frames => match pending_num.take() {
                Some((n, nw)) => frames = Some(Val::new(n, format!("{nw} {w}"))),
                None => diags.push(Diag::new(format!("\"{w}\": how many?")).hint("e.g. \"6 frames\"")),
            },
            Tok::No => pending_no = Some(w),
            Tok::With | Tok::And | Tok::Filler => {}
        }
    }
    if let Some((n, nw)) = pending_num {
        diags.push(Diag::new(format!("\"{nw}\": {n} what?")).hint(format!("e.g. \"{nw} frames\" or \"{nw} pixels\"")));
    }
    if let Some((_, _, cw)) = pending_color {
        diags.push(Diag::new(format!("\"{cw}\" what?")).hint(format!("put it before the thing, e.g. \"{cw} slime\" or \"{cw} eyes\"")));
    }
    if let Some((_, tw)) = pending_tone {
        diags.push(Diag::new(format!("\"{tw}\" what?")).hint("e.g. \"light green\""));
    }
    if let Some(nw) = pending_no {
        diags.push(Diag::new(format!("\"{nw}\" what?")).hint("e.g. \"no outline\""));
    }
    let Some(subject) = subject else {
        if diags.is_empty() {
            let names: Vec<&str> = Subject::ALL.iter().map(|s| s.name()).collect();
            diags.push(Diag::new("the sentence doesn't name a sprite").hint(format!("name one of: {}", names.join(", "))));
        }
        return Err(diags);
    };
    if !diags.is_empty() {
        return Err(diags);
    }
    Ok(Request { sentence: sentence.trim().to_string(), subject, size, color, features, anims, frames, mods, styles, realism })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(s: &str) -> Request {
        parse(s, false).unwrap_or_else(|d| panic!("{s}: {d:?}"))
    }

    #[test]
    fn a_full_sentence() {
        let r = p("a small light green slime with big eyes and horns, bouncing, 6 frames, no outline");
        assert_eq!(r.subject.v, Subject::Slime);
        assert_eq!(r.color.as_ref().unwrap().v, (Color::Green, Some(Tone::Light)));
        assert_eq!(r.color.as_ref().unwrap().words, "light green slime");
        assert!(r.has(Feature::BigEyes).is_some());
        assert!(r.has(Feature::Horns).is_some());
        assert_eq!(r.anims[0].v, Anim::Bounce);
        assert_eq!(r.frames.as_ref().unwrap().v, 6);
        assert!(r.style(Style::NoOutline).is_some());
        assert_eq!(r.size.unwrap().v, SizeReq::Word("small"));
    }

    #[test]
    fn colours_attach_to_features() {
        let r = p("a white ghost with red eyes and a gold crown");
        assert_eq!(r.color.as_ref().unwrap().v.0, Color::White);
        assert_eq!(r.has(Feature::Crown).unwrap().color.as_ref().unwrap().v.0, Color::Gold);
    }

    #[test]
    fn errors() {
        assert!(parse("a slime with 4", false).is_err());
        assert!(parse("a red", false).is_err());
        assert!(parse("a slime and a bat", false).is_err());
        assert!(parse("a slmie", false).unwrap_err()[0].hint.as_ref().unwrap().contains("slime"));
    }
}
