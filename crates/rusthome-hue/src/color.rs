//! Color conversions between Hue (CIE xy, mireds) and HA attributes
//! (`rgb_color`, `hs_color`, kelvin). Formulas: Philips Hue developer notes
//! (wide gamut, sRGB gamma) and Tanner Helland's kelvin approximation.

// r, g, b, x, y, h, s: the usual names in color formulas, clearer than words.
#![allow(clippy::many_single_char_names)]

/// sRGB (0-255) of a CIE xy point at full brightness.
#[must_use]
pub fn xy_to_rgb(x: f64, y: f64) -> [u8; 3] {
    let y = y.max(1e-6);
    let (cap_y, cap_x) = (1.0, x / y);
    let cap_z = (1.0 - x - y) / y;
    let r = cap_x * 1.656_492 - cap_y * 0.354_851 - cap_z * 0.255_038;
    let g = -cap_x * 0.707_196 + cap_y * 1.655_397 + cap_z * 0.036_152;
    let b = cap_x * 0.051_713 - cap_y * 0.121_364 + cap_z * 1.011_530;
    let gamma = |c: f64| {
        if c <= 0.003_130_8 {
            12.92 * c
        } else {
            1.055 * c.powf(1.0 / 2.4) - 0.055
        }
    };
    let (r, g, b) = (gamma(r).max(0.0), gamma(g).max(0.0), gamma(b).max(0.0));
    // the brightest channel is scaled to 1: brightness is a separate attribute
    let m = r.max(g).max(b).max(1e-6);
    [to_u8(r / m), to_u8(g / m), to_u8(b / m)]
}

/// CIE xy of an sRGB color.
#[must_use]
pub fn rgb_to_xy(rgb: [u8; 3]) -> (f64, f64) {
    let lin = |c: u8| {
        let c = f64::from(c) / 255.0;
        if c > 0.040_45 {
            ((c + 0.055) / 1.055).powf(2.4)
        } else {
            c / 12.92
        }
    };
    let (r, g, b) = (lin(rgb[0]), lin(rgb[1]), lin(rgb[2]));
    let x = r * 0.664_511 + g * 0.154_324 + b * 0.162_028;
    let y = r * 0.283_881 + g * 0.668_433 + b * 0.047_685;
    let z = r * 0.000_088 + g * 0.072_310 + b * 0.986_039;
    let sum = x + y + z;
    if sum <= 0.0 {
        return (0.3127, 0.3290); // white point
    }
    (round4(x / sum), round4(y / sum))
}

/// Hue (0-360) and saturation (0-100) of an sRGB color.
#[must_use]
pub fn rgb_to_hs(rgb: [u8; 3]) -> (f64, f64) {
    let [r, g, b] = rgb.map(|c| f64::from(c) / 255.0);
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let d = max - min;
    let h = if d == 0.0 {
        0.0
    } else if (max - r).abs() < f64::EPSILON {
        60.0 * ((g - b) / d).rem_euclid(6.0)
    } else if (max - g).abs() < f64::EPSILON {
        60.0 * ((b - r) / d + 2.0)
    } else {
        60.0 * ((r - g) / d + 4.0)
    };
    let s = if max == 0.0 { 0.0 } else { d / max * 100.0 };
    (round1(h), round1(s))
}

/// sRGB of a hue (0-360) / saturation (0-100) pair at full value.
#[must_use]
pub fn hs_to_rgb(h: f64, s: f64) -> [u8; 3] {
    let s = (s / 100.0).clamp(0.0, 1.0);
    let h = h.rem_euclid(360.0) / 60.0;
    let c = s;
    let x = c * (1.0 - (h % 2.0 - 1.0).abs());
    let (r, g, b) = match h {
        h if h < 1.0 => (c, x, 0.0),
        h if h < 2.0 => (x, c, 0.0),
        h if h < 3.0 => (0.0, c, x),
        h if h < 4.0 => (0.0, x, c),
        h if h < 5.0 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = 1.0 - c;
    [to_u8(r + m), to_u8(g + m), to_u8(b + m)]
}

/// Approximate sRGB of a black body at `kelvin` (1000-40000 K).
#[must_use]
pub fn kelvin_to_rgb(kelvin: f64) -> [u8; 3] {
    let t = kelvin.clamp(1000.0, 40000.0) / 100.0;
    let r = if t <= 66.0 {
        255.0
    } else {
        329.698_727_446 * (t - 60.0).powf(-0.133_204_759_2)
    };
    let g = if t <= 66.0 {
        99.470_802_586_1 * t.ln() - 161.119_568_166_1
    } else {
        288.122_169_528_3 * (t - 60.0).powf(-0.075_514_849_2)
    };
    let b = if t >= 66.0 {
        255.0
    } else if t <= 19.0 {
        0.0
    } else {
        138.517_731_223_1 * (t - 10.0).ln() - 305.044_792_730_7
    };
    [to_u8(r / 255.0), to_u8(g / 255.0), to_u8(b / 255.0)]
}

fn to_u8(c: f64) -> u8 {
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // clamped to 0..=255
    let v = (c.clamp(0.0, 1.0) * 255.0).round() as u8;
    v
}

fn round4(v: f64) -> f64 {
    (v * 10_000.0).round() / 10_000.0
}

fn round1(v: f64) -> f64 {
    (v * 10.0).round() / 10.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn primaries_and_white() {
        let white = xy_to_rgb(0.3127, 0.3290);
        assert!(white.iter().all(|&c| c >= 240), "{white:?}");
        let red = xy_to_rgb(0.6915, 0.3083);
        assert!(red[0] == 255 && red[1] < 60 && red[2] < 40, "{red:?}");
        let (x, y) = rgb_to_xy([255, 0, 0]);
        assert!((x - 0.7006).abs() < 0.01 && (y - 0.2993).abs() < 0.01);
    }

    #[test]
    fn rgb_xy_round_trip_keeps_the_hue() {
        for rgb in [[255, 0, 0], [0, 255, 0], [0, 0, 255], [255, 160, 30]] {
            let (x, y) = rgb_to_xy(rgb);
            let back = xy_to_rgb(x, y);
            let (h1, _) = rgb_to_hs(rgb);
            let (h2, _) = rgb_to_hs(back);
            let diff = (h1 - h2).abs().min(360.0 - (h1 - h2).abs());
            assert!(diff < 8.0, "{rgb:?} -> {back:?}");
        }
    }

    #[test]
    fn hue_saturation() {
        assert_eq!(rgb_to_hs([255, 0, 0]), (0.0, 100.0));
        assert_eq!(rgb_to_hs([0, 0, 255]), (240.0, 100.0));
        assert_eq!(rgb_to_hs([255, 255, 255]), (0.0, 0.0));
        assert_eq!(hs_to_rgb(120.0, 100.0), [0, 255, 0]);
        assert_eq!(hs_to_rgb(0.0, 0.0), [255, 255, 255]);
    }

    #[test]
    fn warm_and_cold_white() {
        let warm = kelvin_to_rgb(2200.0);
        let cold = kelvin_to_rgb(6500.0);
        assert!(warm[0] == 255 && warm[2] < 120, "{warm:?}");
        assert!(cold[2] > 240, "{cold:?}");
    }
}
