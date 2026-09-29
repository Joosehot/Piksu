//! rules.toml: every number the engine uses. Validated at load, so a typo fails immediately.

use anyhow::{anyhow, bail, Context};
use serde::Deserialize;
use std::collections::BTreeMap;

pub const DEFAULT_RULES: &str = include_str!("../rules.toml");

pub type Axes = [f32; 3];

#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub search: Search,
    pub profile: Profile,
    pub sizes: Sizes,
    pub variants: BTreeMap<String, BTreeMap<String, VariantCfg>>,
    pub judges: BTreeMap<String, f32>,
    pub thresholds: Thresholds,
    pub palette_rules: PaletteRules,
    pub colors: BTreeMap<String, [f32; 3]>,
    pub tones: Tones,
    pub default_color: BTreeMap<String, String>,
    pub shapes: Shapes,
    pub features: BTreeMap<String, f32>,
    pub animations: BTreeMap<String, AnimCfg>,
    pub timing: Timing,
    pub lighting: Lighting,
    pub realism: RealismCfg,
    pub materials: Materials,
    /// realistic blueprints: per subject, named proportion arrays (canvas fractions)
    pub anatomy: BTreeMap<String, BTreeMap<String, Vec<f32>>>,
    /// realistic eye (iris) colour per subject
    pub anatomy_iris: BTreeMap<String, String>,
    /// named characters: a name in a sentence stands for this description
    pub characters: BTreeMap<String, String>,
    pub shading: Shading,
    pub checks: Checks,
    pub output: Output,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct Search {
    pub beam_width: usize,
    pub per_shape: usize,
    pub finalists: usize,
    pub evidence_bonus: f32,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    pub default: Axes,
    pub shift: BTreeMap<String, Axes>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct Sizes {
    pub default: u32,
    pub tiny: u32,
    pub small: u32,
    pub medium: u32,
    pub big: u32,
    pub large: u32,
    pub huge: u32,
    /// a realistic sprite with no size asked for (anatomy needs pixels)
    pub realistic: u32,
    /// a person with no size asked for (a full figure needs height)
    pub person: u32,
    pub min: u32,
    pub max: u32,
}

#[derive(Deserialize, Debug, Clone, Copy)]
#[serde(deny_unknown_fields)]
pub struct VariantCfg {
    pub weight: f32,
    pub axes: Axes,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct Thresholds {
    pub detail_min_size: u32,
    pub outline_min_size: u32,
    pub dot_eyes_max_size: u32,
    pub realism_min_size: u32,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct PaletteRules {
    pub shade_value: f32,
    pub light_value: f32,
    pub light_saturation: f32,
    pub shade_hue_shift: f32,
    pub light_hue_shift: f32,
    pub outline_value: f32,
    pub dark_outline: [f32; 3],
    pub eye_white: [f32; 3],
    pub eye_dark: [f32; 3],
    pub mouth: [f32; 3],
    pub accent_default: [f32; 3],
}

#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct Tones {
    pub light: f32,
    pub dark: f32,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Shapes {
    pub eye_gap: f32,
    pub eye_r: f32,
    pub big_eyes: f32,
    pub small_eyes: f32,
    pub pupil: f32,
    pub mouth_w: f32,
    pub mouth_h: f32,
    pub margin: f32,
    pub max_grow: f32,
    #[serde(flatten)]
    pub subjects: BTreeMap<String, SubjectShape>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct SubjectShape {
    pub round: [f32; 4],
    pub tall: [f32; 4],
    pub wide: [f32; 4],
    pub eyes_y: f32,
    #[serde(flatten)]
    pub extra: BTreeMap<String, f32>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct AnimCfg {
    pub frames: u32,
    #[serde(flatten)]
    pub params: BTreeMap<String, f32>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct Timing {
    pub snappy_hold: f32,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct Lighting {
    pub direction: [f32; 3],
    pub ambient: f32,
    pub levels: [f32; 4],
    pub dither: f32,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct RealismCfg {
    pub eye_scale: BTreeMap<String, f32>,
    pub saturation: BTreeMap<String, f32>,
    pub deep_value: f32,
    pub spec_value: f32,
    pub spec_saturation: f32,
    /// clothes are dyed cloth: saturation of a named clothing colour
    pub cloth_saturation: f32,
    /// and its value (dyed cloth is darker than paint)
    pub cloth_value: f32,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct Materials {
    pub fur: f32,
    pub skin: f32,
    pub metal: f32,
    pub gel: f32,
    pub cloth: f32,
    pub stone: f32,
    pub metal_band: [f32; 2],
    pub gel_core: f32,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct Shading {
    pub shade_cut: f32,
    pub light_cut: f32,
    pub rim_band: f32,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct Checks {
    pub min_fill: f32,
    pub max_fill: f32,
    pub min_contrast: f32,
    pub max_change: f32,
    pub max_step: f32,
    pub symmetry_tolerance: f32,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct Output {
    pub preview_size: u32,
    pub gif_frame_ms: u32,
    pub sheet_padding: u32,
}

/// Judges the code knows (validated both ways).
pub const JUDGES: &[&str] = &[
    "small_canvas_detail",
    "no_outline_small",
    "cute_needs_soft_face",
    "spooky_needs_hard_face",
    "colored_outline_with_hue_shift",
    "dot_eyes_on_big_canvas",
    "realistic_needs_lit",
    "realistic_not_flat",
    "cartoon_avoids_lit",
    "realistic_small_canvas",
    "realistic_no_shine_eyes",
    "realistic_colored_outline",
    "stylized_likes_lit",
];

/// Feature and animation numbers the code reads.
pub const FEATURE_KEYS: &[&str] = &["horn_h", "horn_w", "crown_h", "hat_h", "antenna_h", "spot_r", "stripe_h", "tail_len", "wing_span"];

impl Config {
    pub fn load(path: Option<&std::path::Path>) -> anyhow::Result<Config> {
        let (src, name) = match path {
            Some(p) => (std::fs::read_to_string(p).with_context(|| format!("reading {}", p.display()))?, p.display().to_string()),
            None => (DEFAULT_RULES.to_string(), "built-in rules.toml".into()),
        };
        let cfg: Config = toml::from_str(&src).map_err(|e| anyhow!("{name}: {e}"))?;
        cfg.validate().map_err(|e| anyhow!("{name}: {e}"))?;
        Ok(cfg)
    }

    fn validate(&self) -> anyhow::Result<()> {
        use crate::request::*;
        for r in crate::rules::all() {
            let t = self.variants.get(r.name()).ok_or_else(|| anyhow!("missing [variants.{}]", r.name()))?;
            for v in r.all_variants() {
                if !t.contains_key(*v) {
                    bail!("missing variants.{}.{v}", r.name());
                }
            }
            for k in t.keys() {
                if !r.all_variants().contains(&k.as_str()) {
                    bail!("variants.{}.{k} is not a variant of that rule", r.name());
                }
            }
        }
        for k in self.variants.keys() {
            if !crate::rules::all().iter().any(|r| r.name() == k) {
                bail!("[variants.{k}] matches no rule");
            }
        }
        for j in JUDGES {
            if !self.judges.contains_key(*j) {
                bail!("missing judges.{j}");
            }
        }
        for k in self.judges.keys() {
            if !JUDGES.contains(&k.as_str()) {
                bail!("judges.{k} is not a judge (known: {})", JUDGES.join(", "));
            }
        }
        for m in Mod::ALL {
            if !self.profile.shift.contains_key(m.name()) {
                bail!("missing profile.shift.{}", m.name());
            }
        }
        for c in Color::ALL {
            if !self.colors.contains_key(c.name()) {
                bail!("missing colors.{}", c.name());
            }
        }
        for s in Subject::ALL {
            if !crate::anatomy::ANATOMY_ONLY.contains(s) && !self.shapes.subjects.contains_key(s.name()) {
                bail!("missing [shapes.{}]", s.name());
            }
            let c = self.default_color.get(s.name()).ok_or_else(|| anyhow!("missing default_color.{}", s.name()))?;
            if !self.colors.contains_key(c) {
                bail!("default_color.{} = {c:?} is not a colour", s.name());
            }
        }
        for k in FEATURE_KEYS {
            if !self.features.contains_key(*k) {
                bail!("missing features.{k}");
            }
        }
        for a in Anim::ALL {
            let c = self.animations.get(a.name()).ok_or_else(|| anyhow!("missing [animations.{}]", a.name()))?;
            if c.frames < 2 {
                bail!("animations.{}.frames must be at least 2", a.name());
            }
        }
        for r in Realism::ALL {
            if !self.realism.eye_scale.contains_key(r.name()) || !self.realism.saturation.contains_key(r.name()) {
                bail!("realism.eye_scale and realism.saturation need a value for {}", r.name());
            }
        }
        for (subj, keys) in crate::anatomy::REQUIRED {
            let t = self.anatomy.get(*subj).ok_or_else(|| anyhow!("missing [anatomy.{subj}]"))?;
            for k in *keys {
                if t.get(*k).is_none_or(|v| v.is_empty()) {
                    bail!("missing anatomy.{subj}.{k}");
                }
            }
        }
        for (subj, c) in &self.anatomy_iris {
            if !self.colors.contains_key(c) {
                bail!("anatomy_iris.{subj} = \"{c}\" is not a colour");
            }
        }
        if self.sizes.min < 4 || self.sizes.max < self.sizes.min {
            bail!("sizes.min must be at least 4 and at most sizes.max");
        }
        if self.search.beam_width == 0 || self.search.finalists == 0 || self.search.per_shape == 0 {
            bail!("search numbers must be at least 1");
        }
        Ok(())
    }

    pub fn variant(&self, rule: &str, v: &str) -> VariantCfg {
        self.variants[rule][v]
    }

    pub fn judge(&self, name: &str) -> f32 {
        self.judges[name]
    }

    pub fn feature(&self, key: &str) -> f32 {
        self.features[key]
    }

    pub fn anim(&self, a: crate::request::Anim, key: &str) -> f32 {
        *self.animations[a.name()].params.get(key).unwrap_or_else(|| panic!("animations.{}.{key} missing", a.name()))
    }

    pub fn subject_extra(&self, s: crate::request::Subject, key: &str) -> f32 {
        *self.shapes.subjects[s.name()].extra.get(key).unwrap_or_else(|| panic!("shapes.{}.{key} missing", s.name()))
    }

    pub fn size_of(&self, r: &crate::request::Request) -> u32 {
        use crate::request::SizeReq;
        match r.size.as_ref().map(|s| s.v) {
            None if r.subject.v == crate::request::Subject::Human => self.sizes.person,
            None if r.realism.as_ref().map(|x| x.v) == Some(crate::request::Realism::Realistic) => self.sizes.realistic,
            None => self.sizes.default,
            Some(SizeReq::Px(n)) => n,
            Some(SizeReq::Word(w)) => match w {
                "tiny" => self.sizes.tiny,
                "small" => self.sizes.small,
                "medium" => self.sizes.medium,
                "big" => self.sizes.big,
                "large" => self.sizes.large,
                _ => self.sizes.huge,
            },
        }
    }
}
