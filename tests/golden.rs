//! The proof:
//! 1. determinism: every example gives byte-identical files, run after run
//! 2. stability: the output matches the checked-in golden (sheet.png, the Droid text frames, why.txt)
//! Accept intended changes with `PIKSU_BLESS=1 cargo test`.

use piksu::config::Config;
use std::fs;
use std::path::Path;

fn files(sentence: &str, cfg: &Config) -> Vec<(String, Vec<u8>)> {
    let sp = piksu::make(sentence, cfg, &piksu::Options { lenient: false }).unwrap_or_else(|d| panic!("{sentence}: {d:?}"));
    let mut sheet = Vec::new();
    piksu::output::sheet(&sp, cfg, 1).write_to(&mut std::io::Cursor::new(&mut sheet), image::ImageFormat::Png).unwrap();
    let mut out = vec![("sheet.png".to_string(), sheet), ("why.txt".to_string(), piksu::output::why_text(&sp).into_bytes())];
    for (anim, frames) in &sp.anims {
        for (i, g) in frames.iter().enumerate() {
            out.push((format!("{anim}_{}.txt", i + 1), piksu::output::text_sprite(g).into_bytes()));
        }
    }
    out
}

#[test]
fn examples_are_deterministic_and_match_goldens() {
    let cfg = Config::load(None).unwrap();
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples");
    let bless = std::env::var_os("PIKSU_BLESS").is_some();
    let mut failures = Vec::new();
    let mut entries: Vec<_> = fs::read_dir(&root).unwrap().map(|e| e.unwrap().path()).filter(|p| p.extension().is_some_and(|x| x == "txt")).collect();
    entries.sort();
    assert!(!entries.is_empty());
    for path in entries {
        let stem = path.file_stem().unwrap().to_string_lossy().to_string();
        let sentence = fs::read_to_string(&path).unwrap();
        let first = files(&sentence, &cfg);
        for _ in 0..3 {
            assert_eq!(first, files(&sentence, &cfg), "{stem}: output changed between runs");
        }
        let dir = root.join("out").join(&stem);
        if bless {
            let _ = fs::remove_dir_all(&dir);
            fs::create_dir_all(&dir).unwrap();
            for (name, bytes) in &first {
                fs::write(dir.join(name), bytes).unwrap();
            }
            continue;
        }
        for (name, bytes) in &first {
            match fs::read(dir.join(name)) {
                Ok(g) if normalise(&g, name) == normalise(bytes, name) => {}
                Ok(_) => failures.push(format!("{stem}/{name} differs")),
                Err(_) => failures.push(format!("{stem}/{name} missing")),
            }
        }
    }
    assert!(failures.is_empty(), "{}\n(run PIKSU_BLESS=1 cargo test to accept)", failures.join("\n"));
}

/// Text goldens may come back with CRLF on Windows checkouts.
fn normalise(b: &[u8], name: &str) -> Vec<u8> {
    if name.ends_with(".txt") {
        String::from_utf8_lossy(b).replace("\r\n", "\n").into_bytes()
    } else {
        b.to_vec()
    }
}

#[test]
fn honest_failures() {
    let cfg = Config::load(None).unwrap();
    let opts = piksu::Options { lenient: false };
    let e = piksu::make("a slime, walking", &cfg, &opts).err().unwrap();
    assert!(e[0].message.contains("no legs"), "{e:?}");
    let e = piksu::make("a slmie", &cfg, &opts).err().unwrap();
    assert!(e[0].hint.as_deref().unwrap().contains("slime"));
    let e = piksu::make("a slime with 4", &cfg, &opts).err().unwrap();
    assert!(e[0].message.contains("4 what"));
}

#[test]
fn a_rules_typo_is_rejected() {
    let broken = piksu::config::DEFAULT_RULES.replace("[variants.eyes]", "[variants.eyez]");
    let dir = std::env::temp_dir().join(format!("piksu-rules-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let p = dir.join("rules.toml");
    fs::write(&p, broken).unwrap();
    assert!(Config::load(Some(&p)).is_err());
    let _ = fs::remove_dir_all(&dir);
}
