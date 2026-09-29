//! Colour math: RGB <-> HSV, ramps (shade / light / outline) and luminance for contrast checks.

pub type Rgb = [f32; 3];

pub fn to_hsv(c: Rgb) -> (f32, f32, f32) {
    let (r, g, b) = (c[0], c[1], c[2]);
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let d = max - min;
    let h = if d <= 1e-6 {
        0.0
    } else if max == r {
        60.0 * (((g - b) / d).rem_euclid(6.0))
    } else if max == g {
        60.0 * ((b - r) / d + 2.0)
    } else {
        60.0 * ((r - g) / d + 4.0)
    };
    let s = if max <= 1e-6 { 0.0 } else { d / max };
    (h, s, max)
}

pub fn from_hsv(h: f32, s: f32, v: f32) -> Rgb {
    let h = h.rem_euclid(360.0);
    let c = v * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = v - c;
    let (r, g, b) = match (h / 60.0) as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    [(r + m).clamp(0.0, 1.0), (g + m).clamp(0.0, 1.0), (b + m).clamp(0.0, 1.0)]
}

/// Scale value (and optionally saturation), shifting hue by `hue_shift` degrees toward blue
/// (positive) for shadows or toward yellow (negative) for highlights. Blue is 240, yellow 60.
pub fn adjust(c: Rgb, value: f32, saturation: f32, hue_shift: f32) -> Rgb {
    let (h, s, v) = to_hsv(c);
    let h = if s < 0.05 || hue_shift == 0.0 {
        h
    } else {
        // move toward the target hue along the shorter way round
        let target = if hue_shift > 0.0 { 240.0 } else { 60.0 };
        let mut d = (target - h + 540.0) % 360.0 - 180.0;
        let step = hue_shift.abs().min(d.abs());
        d = d.signum() * step;
        h + d
    };
    from_hsv(h, (s * saturation).clamp(0.0, 1.0), (v * value).clamp(0.0, 1.0))
}

pub fn luminance(c: Rgb) -> f32 {
    0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2]
}

pub fn to_u8(c: Rgb) -> [u8; 3] {
    [(c[0] * 255.0).round() as u8, (c[1] * 255.0).round() as u8, (c[2] * 255.0).round() as u8]
}

pub fn mix(a: Rgb, b: Rgb, t: f32) -> Rgb {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hsv_round_trip() {
        for c in [[0.86, 0.2, 0.2], [0.35, 0.78, 0.3], [0.25, 0.48, 0.92], [0.5, 0.5, 0.5]] {
            let (h, s, v) = to_hsv(c);
            let back = from_hsv(h, s, v);
            for i in 0..3 {
                assert!((back[i] - c[i]).abs() < 1e-4, "{c:?} -> {back:?}");
            }
        }
    }

    #[test]
    fn shadows_move_toward_blue() {
        let green = [0.35, 0.78, 0.3];
        let (h0, _, _) = to_hsv(green);
        let (h1, _, _) = to_hsv(adjust(green, 0.7, 1.0, 18.0));
        assert!(h1 > h0);
    }
}
