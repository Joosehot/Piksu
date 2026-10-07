//! Fish for an ice-fishing game: every fish sentence makes a sprite that passes the checks
//! (src/checks.rs) at the baseline 128 and at the game's 32 pixels, the same every run; what a
//! fish can't do is an honest error.

use piksu::config::Config;
use piksu::request::Subject;

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

const SENTENCES: &[&str] = &[
    "a fish, swimming",
    "a perch, swimming",
    "a roach, swimming",
    "a pike, swimming",
    "a perch, struggling",
    "a roach, struggling",
    "a pike, struggling",
    "a perch, hurt",
    "a pike, idle",
    "a realistic perch, swimming",
    "a realistic roach, swimming",
    "a realistic pike, swimming",
    "a cartoon perch, swimming",
    "a cartoon pike, swimming",
    "a cute fish with big eyes, swimming",
    "a roach with spots, swimming",
    "a silver hook",
    "a silver hook, floating",
];

#[test]
fn every_fish_sentence_passes_the_checks_at_128_and_32() {
    for s in SENTENCES {
        for px in [128, 32] {
            let sentence = format!("{s}, {px}px");
            let sp = make(&sentence);
            assert_eq!(sp.size, px, "{sentence}");
            for (anim, frames) in &sp.anims {
                assert!(!frames.is_empty(), "{sentence}: {anim}");
            }
        }
    }
}

#[test]
fn the_default_size_is_128_and_a_small_hook_is_small() {
    assert_eq!(make("a perch, swimming").size, 128);
    assert_eq!(make("a small silver hook").size, 12);
}

#[test]
fn swimming_is_a_seamless_loop_the_same_every_run() {
    let a = make("a perch, swimming, 32px");
    let b = make("a perch, swimming, 32px");
    assert_eq!(a.anims, b.anims);
    let (name, frames) = &a.anims[0];
    assert_eq!(name, "swim");
    assert_eq!(frames.len(), 8);
    // the tail moves: neighbouring frames differ, and no two neighbours are equal
    for i in 0..frames.len() {
        assert_ne!(frames[i], frames[(i + 1) % frames.len()], "frame {i}");
    }
}

#[test]
fn every_fish_struggles_in_a_short_loop_that_passes_the_checks_at_32() {
    for s in ["a perch", "a roach", "a pike", "a fish"] {
        for w in ["struggling", "thrashing", "hooked"] {
            let sp = make(&format!("{s}, {w}, 32px"));
            let (name, frames) = &sp.anims[0];
            assert_eq!(name, "struggle");
            assert!((4..=6).contains(&frames.len()));
            assert_ne!(frames[0], frames[frames.len() / 2], "{s}: bends one way, then the other");
        }
    }
}

#[test]
fn synonyms_name_the_right_fish() {
    assert_eq!(make("an ahven, swimming, 32px").request.subject.v, Subject::Perch);
    assert_eq!(make("a särki, 32px").request.subject.v, Subject::Roach);
    assert_eq!(make("a northern pike, 32px").request.subject.v, Subject::Pike);
    assert_eq!(make("a fish hook, 32px").request.subject.v, Subject::Hook);
}

#[test]
fn what_a_fish_cant_do_is_an_honest_error() {
    assert!(err("a perch, walking").contains("no legs"));
    assert!(err("a perch, blinking").contains("no eyelids"));
    assert!(err("a cat, swimming").contains("no fins"));
    assert!(err("a slime, struggling").contains("no fins"));
    assert!(err("a perch, spinning").contains("spinning is for items"));
    assert!(err("a pike with a crown").contains("doesn't fit a pike"));
    assert!(err("a perch with a coat").contains("clothes are for people"));
    // a thin J turned edge-on jumps between frames: the checks refuse it rather than write it
    assert!(err("a silver hook, spinning").contains("jumpy"));
}

#[test]
fn an_ice_fisherman_is_a_person_in_winter_clothes() {
    let sp = make("an ice fisherman, walking");
    assert_eq!(sp.request.subject.v, Subject::Human);
    use piksu::request::Feature;
    for f in [Feature::Beanie, Feature::Coat, Feature::Boots] {
        assert!(sp.request.has(f).is_some(), "{f:?}");
    }
    assert!(sp.why[0].1.contains("ice fisherman"), "the longest character name wins");
    let sp = make("a fisherman, 32px");
    assert!(sp.request.has(Feature::Beanie).is_some());
}
