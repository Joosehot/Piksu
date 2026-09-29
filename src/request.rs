//! What a sentence asks for. Every value keeps the words that produced it.

macro_rules! kinds {
    ($name:ident { $($var:ident = $s:literal),* $(,)? }) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub enum $name { $($var),* }
        impl $name {
            pub const ALL: &'static [$name] = &[$($name::$var),*];
            pub fn name(self) -> &'static str { match self { $($name::$var => $s),* } }
        }
    };
}

kinds!(Subject {
    Slime = "slime", Ghost = "ghost", Bat = "bat", Cat = "cat", Frog = "frog", Knight = "knight",
    Robot = "robot", Coin = "coin", Gem = "gem", Star = "star", Heart = "heart", Potion = "potion",
    Mushroom = "mushroom",
});
kinds!(Color {
    Red = "red", Orange = "orange", Yellow = "yellow", Green = "green", Blue = "blue", Purple = "purple",
    Pink = "pink", Black = "black", White = "white", Grey = "grey", Brown = "brown", Gold = "gold",
    Silver = "silver", Teal = "teal",
});
kinds!(Tone { Light = "light", Dark = "dark" });
kinds!(Feature {
    Eyes = "eyes", BigEyes = "big eyes", SmallEyes = "small eyes", OneEye = "one eye", Angry = "angry", Sleepy = "sleepy",
    Happy = "happy", Fangs = "fangs", Horns = "horns", Ears = "ears", Wings = "wings", Crown = "crown",
    Hat = "hat", Antenna = "antenna", Tail = "tail", Spots = "spots", Stripes = "stripes",
});
kinds!(Anim { Idle = "idle", Bounce = "bounce", Walk = "walk", Fly = "fly", Spin = "spin", Float = "float", Blink = "blink", Hurt = "hurt" });
kinds!(Mod { Cute = "cute", Spooky = "spooky", Simple = "simple", Detailed = "detailed" });
kinds!(Style { Outlined = "outlined", NoOutline = "no outline", Flat = "flat", Shaded = "shaded" });

#[derive(Clone, Debug, PartialEq)]
pub struct Val<T> {
    pub v: T,
    pub words: String,
}

impl<T> Val<T> {
    pub fn new(v: T, words: impl Into<String>) -> Self {
        Val { v, words: words.into() }
    }
}

/// A canvas size: exact pixels, or a word rules.toml turns into pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SizeReq {
    Px(u32),
    Word(&'static str),
}

#[derive(Clone, Debug, PartialEq)]
pub struct FeatureReq {
    pub feature: Val<Feature>,
    pub color: Option<Val<(Color, Option<Tone>)>>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Request {
    pub sentence: String,
    pub subject: Val<Subject>,
    pub size: Option<Val<SizeReq>>,
    pub color: Option<Val<(Color, Option<Tone>)>>,
    pub features: Vec<FeatureReq>,
    pub anims: Vec<Val<Anim>>,
    pub frames: Option<Val<u32>>,
    pub mods: Vec<Val<Mod>>,
    pub styles: Vec<Val<Style>>,
}

impl Request {
    pub fn has(&self, f: Feature) -> Option<&FeatureReq> {
        self.features.iter().find(|x| x.feature.v == f)
    }
    pub fn style(&self, s: Style) -> Option<&Val<Style>> {
        self.styles.iter().rev().find(|x| x.v == s)
    }
    pub fn has_mod(&self, m: Mod) -> bool {
        self.mods.iter().any(|x| x.v == m)
    }
}
