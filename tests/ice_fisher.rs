//! The rest of an ice-fishing game: a jig (a small metal lure fish on a hook) that flutters
//! around its eye, and an Inuit ice fisher sitting at the hole in a parka with the hood up, a
//! fur ruff, mittens and kamik. Every sentence passes the checks; what doesn't fit is an
//! honest error.

use piksu::config::Config;
use piksu::request::{Feature, Subject};

fn make(s: &str) -> piksu::Sprite {
    let cfg = Config::load(None).unwrap();
    piksu::make(s, &cfg, &piksu::Options { lenient: false }).unwrap_or_else(|d| panic!("{s}: {d:?}"))
}

fn err(s: &str) -> String {
    let cfg = Config::load(None).unwrap();
    match piksu::make(s, &cfg, &piksu::Options { lenient: false }) {
        Ok(_) => panic!("{s}: expected an error"),
        Err(d) => format!("{} {}", d[0].message, d[0].hint.clone().unwrap_or_default()),
    }
}

/// The filled pixel nearest the canvas centre, and its distance.
fn nearest_to_centre(g: &piksu::render::Grid) -> f32 {
    let c = g.size as f32 / 2.0;
    let mut best = f32::MAX;
    for y in 0..g.size as i32 {
        for x in 0..g.size as i32 {
            if g.get(x, y).is_some() {
                best = best.min(((x as f32 + 0.5 - c).powi(2) + (y as f32 + 0.5 - c).powi(2)).sqrt());
            }
        }
    }
    best
}

#[test]
fn a_jig_is_a_lure_fish_on_a_hook_in_every_wording() {
    for s in ["a tiny silver lure fish", "a silver jig", "a pilkki", "a jigging lure", "a fish lure"] {
        assert_eq!(make(&format!("{s}, 48px")).request.subject.v, Subject::Lure, "{s}");
    }
}

#[test]
fn a_jig_flutters_around_its_eye_at_the_canvas_centre() {
    for px in [128, 48] {
        let sp = make(&format!("a silver lure fish, fluttering, {px}px"));
        let (name, frames) = &sp.anims[0];
        assert_eq!(name, "flutter");
        assert_eq!(frames.len(), 8);
        // the eye (the point it hangs from) stays at the centre while the rest swings
        for (i, f) in frames.iter().enumerate() {
            assert!(nearest_to_centre(f) < 2.0, "{px}px frame {i}: the eye left the centre");
        }
        assert_ne!(frames[2], frames[6], "it tilts one way, then the other");
    }
}

#[test]
fn a_still_hook_hangs_from_its_eye_at_the_canvas_centre_too() {
    let sp = make("a silver hook, 32px");
    assert!(nearest_to_centre(&sp.anims[0].1[0]) < 2.0);
}

#[test]
fn an_inuit_ice_fisher_sits_in_a_parka_with_hood_ruff_mittens_and_kamik() {
    for px in [128, 44] {
        let sp = make(&format!("an Inuit ice fisher in a fur parka, sitting, idle, {px}px"));
        assert_eq!(sp.request.subject.v, Subject::Human);
        for f in [Feature::Sitting, Feature::Parka, Feature::Hood, Feature::Fur, Feature::Mittens, Feature::Kamik] {
            assert!(sp.request.has(f).is_some(), "{px}px: {f:?}");
        }
        let (name, frames) = &sp.anims[0];
        assert_eq!(name, "idle");
        // breathing moves the upper body only: the seat and the feet (the bottom rows) stay put
        let s = frames[0].size as i32;
        for f in frames {
            for y in s * 7 / 8..s {
                for x in 0..s {
                    assert_eq!(f.get(x, y).is_some(), frames[0].get(x, y).is_some(), "{px}px: the seat moved at {x},{y}");
                }
            }
        }
    }
    assert!(make("an inuk ice fisher, sitting, 44px").why[0].1.contains("inuk ice fisher"));
}

#[test]
fn the_sitting_figure_has_its_parts() {
    let cfg = Config::load(None).unwrap();
    let (expanded, _) = piksu::expand_characters("an inuit ice fisher", &cfg);
    let req = piksu::parser::parse(&expanded, false).unwrap();
    let mut chosen = piksu::rules::Chosen::new();
    chosen.insert("realism", "realistic");
    chosen.insert("eyes", "dot");
    let fig = piksu::figure::build(&req, &cfg, &chosen).unwrap();
    for role in ["seat", "parka", "trim", "hood", "ruff", "fur_tip", "head", "nose", "eye", "hand", "thumb", "cuff", "boot", "kamik_band"] {
        assert!(fig.parts.iter().any(|p| p.role == role), "{role}");
    }
    assert_eq!(fig.gait, piksu::figure::Gait::Sit);
    // without a hood the head is bare; without mittens the hand is skin
    let req = piksu::parser::parse("a realistic man sitting with a coat", false).unwrap();
    let fig = piksu::figure::build(&req, &cfg, &chosen).unwrap();
    assert!(!fig.parts.iter().any(|p| p.role == "ruff" || p.role == "hood" || p.role == "thumb"));
    assert!(fig.parts.iter().any(|p| p.role == "hair"));
}

#[test]
fn what_doesnt_fit_is_an_honest_error() {
    assert!(err("a perch, fluttering").contains("doesn't hang from an eye"));
    assert!(err("a man sitting, walking").contains("doesn't walk"));
    assert!(err("a man with a parka").contains("add \"sitting\""));
    assert!(err("a slime with mittens").contains("clothes are for people"));
}

#[test]
fn every_colour_of_a_droid_text_sprite_has_its_own_key() {
    // a realistic person has far more than 62 colours
    let sp = make("an Inuit ice fisher, sitting, idle, 96px");
    let text = piksu::output::text_sprite(&sp.anims[0].1[0]);
    let keys: Vec<&str> = text.split("---").next().unwrap().lines().filter_map(|l| l.split(" = ").next()).collect();
    assert!(keys.len() > 62, "{} colours", keys.len());
    let mut uniq = keys.clone();
    uniq.sort_unstable();
    uniq.dedup();
    assert_eq!(uniq.len(), keys.len(), "a key used twice");
}
