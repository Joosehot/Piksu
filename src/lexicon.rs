//! The closed vocabulary. Unknown words are errors with a closest-match hint.

use crate::request::*;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Tok {
    Subject(Subject),
    Color(Color),
    Tone(Tone),
    Feature(Feature),
    Anim(Anim),
    Mod(Mod),
    Style(Style),
    Realism(Realism),
    /// "8x8", "16 pixels"
    Pixels(u32),
    /// a size word: tiny / small / medium / big / large / huge
    SizeWord(&'static str),
    Num(u32),
    Frames,
    No,
    With,
    And,
    Filler,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Token {
    pub tok: Tok,
    pub text: String,
}

fn table() -> Vec<(&'static str, Tok)> {
    use Tok as T;
    let mut t: Vec<(&'static str, Tok)> = vec![
        ("blob", T::Subject(Subject::Slime)),
        ("blobs", T::Subject(Subject::Slime)),
        ("jelly", T::Subject(Subject::Slime)),
        ("spirit", T::Subject(Subject::Ghost)),
        ("kitten", T::Subject(Subject::Cat)),
        ("kitty", T::Subject(Subject::Cat)),
        ("toad", T::Subject(Subject::Frog)),
        ("warrior", T::Subject(Subject::Knight)),
        ("soldier", T::Subject(Subject::Knight)),
        ("droid", T::Subject(Subject::Robot)),
        ("bot", T::Subject(Subject::Robot)),
        ("jewel", T::Subject(Subject::Gem)),
        ("diamond", T::Subject(Subject::Gem)),
        ("crystal", T::Subject(Subject::Gem)),
        ("bottle", T::Subject(Subject::Potion)),
        ("flask", T::Subject(Subject::Potion)),
        ("toadstool", T::Subject(Subject::Mushroom)),
        ("gray", T::Color(Color::Grey)),
        ("golden", T::Color(Color::Gold)),
        ("violet", T::Color(Color::Purple)),
        ("cyan", T::Color(Color::Teal)),
        ("turquoise", T::Color(Color::Teal)),
        ("light", T::Tone(Tone::Light)),
        ("pale", T::Tone(Tone::Light)),
        ("bright", T::Tone(Tone::Light)),
        ("dark", T::Tone(Tone::Dark)),
        ("deep", T::Tone(Tone::Dark)),
        // features
        ("eyes", T::Feature(Feature::Eyes)),
        ("eye", T::Feature(Feature::Eyes)),
        ("big eyes", T::Feature(Feature::BigEyes)),
        ("large eyes", T::Feature(Feature::BigEyes)),
        ("huge eyes", T::Feature(Feature::BigEyes)),
        ("googly eyes", T::Feature(Feature::BigEyes)),
        ("small eyes", T::Feature(Feature::SmallEyes)),
        ("tiny eyes", T::Feature(Feature::SmallEyes)),
        ("beady eyes", T::Feature(Feature::SmallEyes)),
        ("one eye", T::Feature(Feature::OneEye)),
        ("a single eye", T::Feature(Feature::OneEye)),
        ("cyclops", T::Feature(Feature::OneEye)),
        ("angry", T::Feature(Feature::Angry)),
        ("mad", T::Feature(Feature::Angry)),
        ("grumpy", T::Feature(Feature::Angry)),
        ("sleepy", T::Feature(Feature::Sleepy)),
        ("tired", T::Feature(Feature::Sleepy)),
        ("happy", T::Feature(Feature::Happy)),
        ("smiling", T::Feature(Feature::Happy)),
        ("a smile", T::Feature(Feature::Happy)),
        ("fangs", T::Feature(Feature::Fangs)),
        ("teeth", T::Feature(Feature::Fangs)),
        ("horns", T::Feature(Feature::Horns)),
        ("horned", T::Feature(Feature::Horns)),
        ("ears", T::Feature(Feature::Ears)),
        ("wings", T::Feature(Feature::Wings)),
        ("winged", T::Feature(Feature::Wings)),
        ("crown", T::Feature(Feature::Crown)),
        ("hat", T::Feature(Feature::Hat)),
        ("top hat", T::Feature(Feature::Hat)),
        ("antenna", T::Feature(Feature::Antenna)),
        ("antennas", T::Feature(Feature::Antenna)),
        ("tail", T::Feature(Feature::Tail)),
        ("spots", T::Feature(Feature::Spots)),
        ("spotted", T::Feature(Feature::Spots)),
        ("dots", T::Feature(Feature::Spots)),
        ("stripes", T::Feature(Feature::Stripes)),
        ("striped", T::Feature(Feature::Stripes)),
        // animations
        ("idle", T::Anim(Anim::Idle)),
        ("idling", T::Anim(Anim::Idle)),
        ("breathing", T::Anim(Anim::Idle)),
        ("bounce", T::Anim(Anim::Bounce)),
        ("bounces", T::Anim(Anim::Bounce)),
        ("bouncing", T::Anim(Anim::Bounce)),
        ("bouncy", T::Anim(Anim::Bounce)),
        ("hop", T::Anim(Anim::Bounce)),
        ("hops", T::Anim(Anim::Bounce)),
        ("hopping", T::Anim(Anim::Bounce)),
        ("jumping", T::Anim(Anim::Bounce)),
        ("walk", T::Anim(Anim::Walk)),
        ("walks", T::Anim(Anim::Walk)),
        ("walking", T::Anim(Anim::Walk)),
        ("running", T::Anim(Anim::Walk)),
        ("fly", T::Anim(Anim::Fly)),
        ("flies", T::Anim(Anim::Fly)),
        ("flying", T::Anim(Anim::Fly)),
        ("flapping", T::Anim(Anim::Fly)),
        ("flaps", T::Anim(Anim::Fly)),
        ("spin", T::Anim(Anim::Spin)),
        ("spins", T::Anim(Anim::Spin)),
        ("spinning", T::Anim(Anim::Spin)),
        ("rotating", T::Anim(Anim::Spin)),
        ("float", T::Anim(Anim::Float)),
        ("floats", T::Anim(Anim::Float)),
        ("floating", T::Anim(Anim::Float)),
        ("hovering", T::Anim(Anim::Float)),
        ("blink", T::Anim(Anim::Blink)),
        ("blinks", T::Anim(Anim::Blink)),
        ("blinking", T::Anim(Anim::Blink)),
        ("hurt", T::Anim(Anim::Hurt)),
        ("getting hit", T::Anim(Anim::Hurt)),
        ("damaged", T::Anim(Anim::Hurt)),
        // mood
        ("cute", T::Mod(Mod::Cute)),
        ("adorable", T::Mod(Mod::Cute)),
        ("kawaii", T::Mod(Mod::Cute)),
        ("friendly", T::Mod(Mod::Cute)),
        ("spooky", T::Mod(Mod::Spooky)),
        ("scary", T::Mod(Mod::Spooky)),
        ("creepy", T::Mod(Mod::Spooky)),
        ("evil", T::Mod(Mod::Spooky)),
        ("menacing", T::Mod(Mod::Spooky)),
        ("simple", T::Mod(Mod::Simple)),
        ("minimal", T::Mod(Mod::Simple)),
        ("clean", T::Mod(Mod::Simple)),
        ("detailed", T::Mod(Mod::Detailed)),
        ("fancy", T::Mod(Mod::Detailed)),
        ("intricate", T::Mod(Mod::Detailed)),
        // realism
        ("realistic", T::Realism(Realism::Realistic)),
        ("lifelike", T::Realism(Realism::Realistic)),
        ("natural", T::Realism(Realism::Realistic)),
        ("photorealistic", T::Realism(Realism::Realistic)),
        ("semi realistic", T::Realism(Realism::Stylized)),
        ("stylized", T::Realism(Realism::Stylized)),
        ("stylised", T::Realism(Realism::Stylized)),
        ("cartoon", T::Realism(Realism::Cartoon)),
        ("cartoony", T::Realism(Realism::Cartoon)),
        ("toon", T::Realism(Realism::Cartoon)),
        ("chibi", T::Realism(Realism::Cartoon)),
        // style
        ("outlined", T::Style(Style::Outlined)),
        ("outline", T::Style(Style::Outlined)),
        ("with an outline", T::Style(Style::Outlined)),
        ("flat", T::Style(Style::Flat)),
        ("unshaded", T::Style(Style::Flat)),
        ("shaded", T::Style(Style::Shaded)),
        ("shading", T::Style(Style::Shaded)),
        // sizes
        ("tiny", T::SizeWord("tiny")),
        ("small", T::SizeWord("small")),
        ("little", T::SizeWord("small")),
        ("medium", T::SizeWord("medium")),
        ("big", T::SizeWord("big")),
        ("large", T::SizeWord("large")),
        ("huge", T::SizeWord("huge")),
        ("giant", T::SizeWord("huge")),
        ("pixels", T::Filler),
        ("pixel", T::Filler),
        ("px", T::Filler),
        ("frames", T::Frames),
        ("frame", T::Frames),
        ("no", T::No),
        ("without", T::No),
        ("with", T::With),
        ("and", T::And),
        (",", T::And),
    ];
    for s in Subject::ALL {
        t.push((s.name(), Tok::Subject(*s)));
        t.push((Box::leak(format!("{}s", s.name()).into_boxed_str()), Tok::Subject(*s)));
    }
    for c in Color::ALL {
        t.push((c.name(), Tok::Color(*c)));
    }
    for f in [
        "a", "an", "the", "of", "that", "which", "who", "is", "are", "it", "its", "in", "on", "at", "to", "as", "for", "some",
        "sprite", "sprites", "character", "monster", "creature", "enemy", "item", "pickup", "icon", "animation", "animations",
        "animated", "pixel art", "art", "style", "looking", "very", "really", "super", "little", "make", "draw", "give", "me",
        "i", "want", "please", "its", "own", "while", "then", "also", "one",
    ] {
        if !t.iter().any(|(p, _)| *p == f) {
            t.push((f, Tok::Filler));
        }
    }
    t
}

fn phrases() -> &'static Vec<(Vec<&'static str>, Tok, &'static str)> {
    use std::sync::OnceLock;
    static P: OnceLock<Vec<(Vec<&'static str>, Tok, &'static str)>> = OnceLock::new();
    P.get_or_init(|| {
        let mut v: Vec<(Vec<&'static str>, Tok, &'static str)> = table().into_iter().map(|(p, t)| (p.split(' ').collect(), t, p)).collect();
        v.sort_by(|a, b| b.0.len().cmp(&a.0.len()));
        v
    })
}

fn words(text: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let flush = |cur: &mut String, out: &mut Vec<(String, String)>| {
        if !cur.is_empty() {
            out.push((cur.to_lowercase(), cur.clone()));
            cur.clear();
        }
    };
    for c in text.chars() {
        match c {
            ',' => {
                flush(&mut cur, &mut out);
                out.push((",".into(), ",".into()));
            }
            c if c.is_whitespace() || c == '-' => flush(&mut cur, &mut out),
            '.' | '!' | '?' | ';' | ':' | '"' | '\'' | '(' | ')' => flush(&mut cur, &mut out),
            c => cur.push(c),
        }
    }
    flush(&mut cur, &mut out);
    out
}

const NUMBERS: &[&str] = &["zero", "one", "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten", "eleven", "twelve"];

/// Tokens and the words nobody knows.
pub fn tokenize(text: &str) -> (Vec<Token>, Vec<String>) {
    let ws = words(text);
    let mut out = Vec::new();
    let mut unknown = Vec::new();
    let mut i = 0;
    'outer: while i < ws.len() {
        let (lw, orig) = &ws[i];
        // "16x16"
        if let Some((a, b)) = lw.split_once('x') {
            if let (Ok(a), Ok(b)) = (a.parse::<u32>(), b.parse::<u32>()) {
                if a == b {
                    out.push(Token { tok: Tok::Pixels(a), text: orig.clone() });
                } else {
                    unknown.push(format!("{orig} (sprites are square)"));
                }
                i += 1;
                continue;
            }
        }
        if let Ok(n) = lw.parse::<u32>() {
            // "16 pixels" / "16px"
            let px = ws.get(i + 1).is_some_and(|w| matches!(w.0.as_str(), "pixels" | "pixel" | "px"));
            out.push(Token { tok: if px { Tok::Pixels(n) } else { Tok::Num(n) }, text: orig.clone() });
            i += if px { 2 } else { 1 };
            continue;
        }
        if let Some(n) = lw.strip_suffix("px").and_then(|n| n.parse::<u32>().ok()) {
            out.push(Token { tok: Tok::Pixels(n), text: orig.clone() });
            i += 1;
            continue;
        }
        for (p, tok, _) in phrases() {
            if i + p.len() <= ws.len() && p.iter().enumerate().all(|(k, w)| ws[i + k].0 == *w) {
                // a number word before "frames" is a count, not filler
                let text = ws[i..i + p.len()].iter().map(|w| w.1.as_str()).collect::<Vec<_>>().join(" ");
                out.push(Token { tok: *tok, text });
                i += p.len();
                continue 'outer;
            }
        }
        if let Some(n) = NUMBERS.iter().position(|w| w == lw) {
            out.push(Token { tok: Tok::Num(n as u32), text: orig.clone() });
            i += 1;
            continue;
        }
        unknown.push(orig.clone());
        i += 1;
    }
    (out, unknown)
}

pub fn vocabulary() -> Vec<&'static str> {
    let mut v: Vec<&'static str> = phrases().iter().filter(|(_, t, _)| *t != Tok::Filler).map(|(_, _, p)| *p).collect();
    v.sort_unstable();
    v.dedup();
    v
}

fn levenshtein(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for i in 1..=a.len() {
        let mut cur = vec![i; b.len() + 1];
        for j in 1..=b.len() {
            cur[j] = (prev[j] + 1).min(cur[j - 1] + 1).min(prev[j - 1] + usize::from(a[i - 1] != b[j - 1]));
        }
        prev = cur;
    }
    prev[b.len()]
}

pub fn closest(word: &str) -> Option<&'static str> {
    let w = word.to_lowercase();
    let limit = (w.chars().count() / 3).max(1) + 1;
    vocabulary().into_iter().map(|v| (levenshtein(&w, v), v)).filter(|(d, _)| *d <= limit).min().map(|(_, v)| v)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn toks(s: &str) -> Vec<Tok> {
        let (t, u) = tokenize(s);
        assert!(u.is_empty(), "{u:?}");
        t.into_iter().map(|t| t.tok).collect()
    }

    #[test]
    fn phrases_are_unique() {
        let mut v: Vec<&str> = phrases().iter().map(|p| p.2).collect();
        v.sort_unstable();
        let n = v.len();
        v.dedup();
        assert_eq!(n, v.len());
    }

    #[test]
    fn sizes() {
        assert_eq!(toks("16x16"), vec![Tok::Pixels(16)]);
        assert_eq!(toks("24 pixels"), vec![Tok::Pixels(24)]);
        assert_eq!(toks("32px"), vec![Tok::Pixels(32)]);
    }

    #[test]
    fn longest_phrase_wins() {
        assert_eq!(toks("big eyes"), vec![Tok::Feature(Feature::BigEyes)]);
        assert_eq!(toks("big slime"), vec![Tok::SizeWord("big"), Tok::Subject(Subject::Slime)]);
    }

    #[test]
    fn hints() {
        assert_eq!(closest("slmie"), Some("slime"));
    }
}
