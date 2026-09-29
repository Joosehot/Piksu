//! Files: a spritesheet (one row per animation), each frame as a PNG, an enlarged preview PNG,
//! an animated GIF per animation, Droid text sprites (palette + rows), why.txt and sentence.txt.

use crate::config::Config;
use crate::render::Grid;
use crate::Sprite;
use anyhow::Context;
use image::{codecs::gif::GifEncoder, Delay, Frame, Rgba, RgbaImage};
use std::path::Path;

fn to_image(g: &Grid, scale: u32) -> RgbaImage {
    let s = g.size * scale;
    let mut img = RgbaImage::new(s, s);
    for y in 0..s {
        for x in 0..s {
            if let Some(c) = g.px[((y / scale) * g.size + x / scale) as usize] {
                let [r, gg, b] = crate::color::to_u8(c);
                img.put_pixel(x, y, Rgba([r, gg, b, 255]));
            }
        }
    }
    img
}

pub fn sheet(sp: &Sprite, cfg: &Config, scale: u32) -> RgbaImage {
    let pad = cfg.output.sheet_padding * scale;
    let cell = sp.size * scale + pad;
    let cols = sp.anims.iter().map(|a| a.1.len()).max().unwrap_or(1) as u32;
    let rows = sp.anims.len() as u32;
    let mut img = RgbaImage::new(cols * cell + pad, rows * cell + pad);
    for (r, (_, frames)) in sp.anims.iter().enumerate() {
        for (c, g) in frames.iter().enumerate() {
            let f = to_image(g, scale);
            image::imageops::overlay(&mut img, &f, (pad + c as u32 * cell) as i64, (pad + r as u32 * cell) as i64);
        }
    }
    img
}

/// A Droid text sprite: palette letters in first-seen order, then rows ('.' = empty).
pub fn text_sprite(g: &Grid) -> String {
    const LETTERS: &str = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
    let mut pal: Vec<[u8; 3]> = Vec::new();
    let mut rows = Vec::new();
    for y in 0..g.size {
        let mut row = String::new();
        for x in 0..g.size {
            match g.px[(y * g.size + x) as usize] {
                None => row.push('.'),
                Some(c) => {
                    let u = crate::color::to_u8(c);
                    let i = pal.iter().position(|p| *p == u).unwrap_or_else(|| {
                        pal.push(u);
                        pal.len() - 1
                    });
                    row.push(LETTERS.chars().nth(i).unwrap_or('#'));
                }
            }
        }
        rows.push(row);
    }
    let mut out = String::new();
    for (i, p) in pal.iter().enumerate() {
        out.push_str(&format!("{} = #{:02x}{:02x}{:02x}\n", LETTERS.chars().nth(i).unwrap_or('#'), p[0], p[1], p[2]));
    }
    out.push_str("---\n");
    for r in rows {
        out.push_str(&r);
        out.push('\n');
    }
    out
}

pub fn slug(sp: &Sprite) -> String {
    let mut s = sp.request.subject.v.name().to_string();
    if let Some(a) = sp.request.anims.first() {
        s.push('_');
        s.push_str(a.v.name());
    }
    s
}

pub fn why_text(sp: &Sprite) -> String {
    let mut out = format!("{}\n\n", sp.request.sentence);
    for (piece, how) in &sp.why {
        out.push_str(&format!("{piece:<18} {how}\n"));
    }
    out
}

pub fn write(sp: &Sprite, cfg: &Config, dir: &Path) -> anyhow::Result<()> {
    std::fs::create_dir_all(dir.join("frames"))?;
    std::fs::create_dir_all(dir.join("droid"))?;
    let scale = cfg.output.preview_scale;
    sheet(sp, cfg, 1).save(dir.join("sheet.png")).context("sheet.png")?;
    sheet(sp, cfg, scale).save(dir.join("preview.png")).context("preview.png")?;
    let name = slug(sp);
    for (anim, frames) in &sp.anims {
        for (i, g) in frames.iter().enumerate() {
            to_image(g, 1).save(dir.join("frames").join(format!("{anim}_{i}.png")))?;
            std::fs::write(dir.join("droid").join(format!("{name}_{anim}_{}.txt", i + 1)), text_sprite(g))?;
        }
        if frames.len() > 1 {
            let file = std::fs::File::create(dir.join(format!("{anim}.gif")))?;
            let mut enc = GifEncoder::new(file);
            enc.set_repeat(image::codecs::gif::Repeat::Infinite)?;
            for g in frames {
                let mut img = to_image(g, scale);
                // GIF has no partial alpha; give empty pixels a flat backdrop
                for p in img.pixels_mut() {
                    if p[3] == 0 {
                        *p = Rgba([236, 240, 245, 255]);
                    }
                }
                enc.encode_frame(Frame::from_parts(img, 0, 0, Delay::from_numer_denom_ms(cfg.output.gif_frame_ms, 1)))?;
            }
        }
    }
    std::fs::write(dir.join("why.txt"), why_text(sp))?;
    std::fs::write(dir.join("sentence.txt"), format!("{}\n", sp.request.sentence))?;
    Ok(())
}
