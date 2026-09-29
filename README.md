# Piksu

Describe a sprite in a sentence and get pixel art and animation frames back. It feels like
prompting an image model, but it behaves like a compiler: the same sentence and the same
`rules.toml` always give the same pixels, every choice can be traced to the words that asked
for it, and a sprite that fails its checks isn't written at all.

```
$ piksu "a small green slime with big eyes, bouncing"
a small green slime with big eyes, bouncing -> out/slime_bounce (12px, 6 frames)
  body               body/round (the slime)
  colour             "green slime"
  eyes               eyes/shine
  ...
```

Piksu is a [HotMotor](https://github.com/Joosehot/HotMotor) engine, like FeelRight, Senne,
Aras and Droid. No neural network runs at generation time.

## The pipeline

| step | file | what it does |
|---|---|---|
| lexicon | `src/lexicon.rs` | a closed vocabulary; an unknown word is an error with a "did you mean" hint (`--lenient` skips it) |
| parser | `src/parser.rs` | the sentence becomes a request: subject, size, colours, features, animations, frames, mood, style; each value keeps its words; an unused value is an error ("4 what?") |
| rules | `src/rules/*.rs` | one decision per file with variants: body (round / tall / wide), palette (hue-shifted / value-only ramp), shading (flat / top light / rim), outline (dark / coloured / none), eyes (dot / shine / angry / sleepy / visor), mouth (none / smile / open / fangs), timing (linear / eased / snappy) |
| search | `src/search.rs` | beam search; a variant scores `weight * (1 + axes . (profile - 0.5))` on [cute, readable, detail]; words like "cute", "spooky", "simple", "detailed" shift the profile; judges score combinations (no outline on a tiny canvas, shiny eyes and a smile for "cute", ...) |
| figure | `src/figure.rs` | the chosen design as coloured parts: body, eyes, mouth, and features (horns, ears, wings, crown, hat, antenna, tail, spots, stripes) |
| animation | `src/anim.rs` | idle, bounce (squash and stretch), walk (legs), fly (wing flaps), spin, float, blink, hurt (flash and shake), as one seamless loop |
| fit + render | `src/lib.rs`, `src/render.rs` | the figure, with all its motion, is fitted into the canvas with room for the outline, then rasterised pixel by pixel (no anti-aliasing), shaded and outlined |
| checks | `src/checks.rs` | fits the canvas, no stray pixels, readable coverage, outline and eye contrast, smooth motion between frames (shape change and step size, including last to first), symmetry |

Finalists differ in body, eyes and outline, so when one fails a check, the next is a real
alternative. If none pass, Piksu says why and writes nothing:

```
$ piksu "a tiny pink heart, spinning"
error: no design passed the checks
  hint: spin frame 4 is empty
```

Every number (proportions, palette shifts, animation amplitudes, judge weights, check limits)
lives in `rules.toml`. Point `--rules` at a copy to change the taste.

## What it knows (v0)

- **Subjects:** slime, ghost, bat, cat, frog, knight, robot, coin, gem, star, heart, potion, mushroom.
- **Colours:** red, orange, yellow, green, blue, purple, pink, black, white, grey, brown, gold,
  silver and teal, each optionally light or dark. A colour before a feature colours that feature
  ("red eyes", "a gold crown").
- **Features:** big / small / one eye, angry, sleepy, happy, fangs, horns, ears, wings, crown,
  hat, antenna, tail, spots and stripes.
- **Animations:** idle, bouncing, walking, flying, spinning, floating, blinking and hurt.
  "6 frames" sets the count. A slime can't walk and a coin doesn't fly; you get an error instead
  of a guess.
- **Sizes:** tiny, small, medium, big, large and huge, or "24x24" / "24 pixels".
- **Style:** outlined, "no outline", flat or shaded. Mood words: cute, spooky, simple and detailed.

`piksu --vocabulary` lists every word.

## Output

`out/<subject>_<animation>/` (or `-o DIR`):

- `sheet.png` has one row per animation.
- `preview.png` is the same sheet enlarged.
- `frames/*.png` holds each frame, and `<animation>.gif` is an animated preview.
- `droid/*.txt` holds Droid text sprites (a palette, `---`, then pixel rows), ready for a Droid game's `sprites/`.
- `why.txt` maps every piece to the rule and variant that made it and the words that asked for it.
- `sentence.txt` holds the input.

## Proof

`cargo test`:
- unit tests: lexicon, parser, colour math, shapes, animation loops, and one test where each
  rule applies and one where it doesn't, plus the judges
- `tests/golden.rs`: every sentence in `examples/*.txt` is made several times and must be
  byte-identical and match `examples/out/` (`PIKSU_BLESS=1 cargo test` accepts intended changes);
  honest failures and a rules.toml typo are tested too

## License

MIT.
