//! Colour scales of the figures and their colour-blind check (plan 4.8).
//!
//! - **Support** (branches): a sequential scale from the viridis colour map
//!   (van der Walt and Smith), reversed and cut at 85% so that low support
//!   stays visible on white: light green for 0, dark purple for 1. Width
//!   also encodes support.
//! - **Halo values:** the diverging ColorBrewer PuOr scale of
//!   [`crate::tree::halo_colour`]; values below 0.6 are also marked with a
//!   dot.
//! - **Groups:** the Okabe–Ito categorical palette (Okabe and Ito, 2008);
//!   every band also carries its group name.
//!
//! The check simulates protanopia, deuteranopia and tritanopia with the
//! matrices of Machado, Oliveira and Fernandes (2009, severity 1.0) on
//! linear sRGB, and measures colour differences as CIEDE2000 in CIELAB
//! (D65). Colour is never the only carrier of meaning: widths, labels and
//! symbols carry it too.

/// Viridis at 10 evenly spaced points (0 to 1).
const VIRIDIS: [[u8; 3]; 10] = [
    [68, 1, 84],
    [72, 40, 120],
    [62, 73, 137],
    [49, 104, 142],
    [38, 130, 142],
    [31, 158, 137],
    [53, 183, 121],
    [110, 206, 88],
    [181, 222, 43],
    [253, 231, 37],
];

fn lerp(a: [u8; 3], b: [u8; 3], t: f64) -> [u8; 3] {
    std::array::from_fn(|i| (a[i] as f64 + t * (b[i] as f64 - a[i] as f64)).round() as u8)
}

fn viridis(x: f64) -> [u8; 3] {
    let x = x.clamp(0.0, 1.0) * 9.0;
    let i = (x.floor() as usize).min(8);
    lerp(VIRIDIS[i], VIRIDIS[i + 1], x - i as f64)
}

/// Branch colour for a support value in [0, 1]; grey without a value.
pub fn support_colour(value: Option<f64>) -> [u8; 3] {
    match value.filter(|v| v.is_finite()) {
        Some(v) => viridis(0.85 * (1.0 - v.clamp(0.0, 1.0))),
        None => [150, 150, 150],
    }
}

/// Branch width for a support value: 1 to 4 pixels.
pub fn support_width(value: Option<f64>) -> f64 {
    1.0 + 3.0
        * value
            .filter(|v| v.is_finite())
            .unwrap_or(0.0)
            .clamp(0.0, 1.0)
}

/// The Okabe–Ito palette without black (black is used for text).
pub const GROUPS: [[u8; 3]; 7] = [
    [230, 159, 0],   // orange
    [86, 180, 233],  // sky blue
    [0, 158, 115],   // bluish green
    [240, 228, 66],  // yellow
    [0, 114, 178],   // blue
    [213, 94, 0],    // vermillion
    [204, 121, 167], // reddish purple
];

/// Colour of group number `i` (repeats after 7; bands are also labelled).
pub fn group_colour(i: usize) -> [u8; 3] {
    GROUPS[i % GROUPS.len()]
}

pub fn hex(c: [u8; 3]) -> String {
    format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2])
}

/// Kinds of colour vision simulated by the check.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Vision {
    Normal,
    Protanopia,
    Deuteranopia,
    Tritanopia,
}

pub const VISIONS: [Vision; 4] = [
    Vision::Normal,
    Vision::Protanopia,
    Vision::Deuteranopia,
    Vision::Tritanopia,
];

impl Vision {
    pub fn name(self) -> &'static str {
        match self {
            Vision::Normal => "normal",
            Vision::Protanopia => "protanopia",
            Vision::Deuteranopia => "deuteranopia",
            Vision::Tritanopia => "tritanopia",
        }
    }

    /// Machado et al. (2009), severity 1.0, on linear RGB.
    fn matrix(self) -> [[f64; 3]; 3] {
        match self {
            Vision::Normal => [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            Vision::Protanopia => [
                [0.152286, 1.052583, -0.204868],
                [0.114503, 0.786281, 0.099216],
                [-0.003882, -0.048116, 1.051998],
            ],
            Vision::Deuteranopia => [
                [0.367322, 0.860646, -0.227968],
                [0.280085, 0.672501, 0.047413],
                [-0.011820, 0.042940, 0.968881],
            ],
            Vision::Tritanopia => [
                [1.255528, -0.076749, -0.178779],
                [-0.078411, 0.930809, 0.147602],
                [0.004733, 0.691367, 0.303900],
            ],
        }
    }
}

fn to_linear(c: u8) -> f64 {
    let c = c as f64 / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// CIELAB (D65) of an sRGB colour as seen with `vision`.
pub fn lab(c: [u8; 3], vision: Vision) -> [f64; 3] {
    let rgb = c.map(to_linear);
    let m = vision.matrix();
    let s: [f64; 3] = std::array::from_fn(|i| {
        (m[i][0] * rgb[0] + m[i][1] * rgb[1] + m[i][2] * rgb[2]).clamp(0.0, 1.0)
    });
    let x = 0.4124564 * s[0] + 0.3575761 * s[1] + 0.1804375 * s[2];
    let y = 0.2126729 * s[0] + 0.7151522 * s[1] + 0.0721750 * s[2];
    let z = 0.0193339 * s[0] + 0.1191920 * s[1] + 0.9503041 * s[2];
    let f = |t: f64| {
        if t > 216.0 / 24389.0 {
            t.cbrt()
        } else {
            (24389.0 / 27.0 * t + 16.0) / 116.0
        }
    };
    let (fx, fy, fz) = (f(x / 0.95047), f(y), f(z / 1.08883));
    [116.0 * fy - 16.0, 500.0 * (fx - fy), 200.0 * (fy - fz)]
}

/// CIEDE2000 colour difference.
pub fn delta_e2000(a: [f64; 3], b: [f64; 3]) -> f64 {
    use std::f64::consts::PI;
    let (l1, a1, b1) = (a[0], a[1], a[2]);
    let (l2, a2, b2) = (b[0], b[1], b[2]);
    let c1 = (a1 * a1 + b1 * b1).sqrt();
    let c2 = (a2 * a2 + b2 * b2).sqrt();
    let cbar = (c1 + c2) / 2.0;
    let g = 0.5 * (1.0 - (cbar.powi(7) / (cbar.powi(7) + 25f64.powi(7))).sqrt());
    let (a1p, a2p) = ((1.0 + g) * a1, (1.0 + g) * a2);
    let (c1p, c2p) = ((a1p * a1p + b1 * b1).sqrt(), (a2p * a2p + b2 * b2).sqrt());
    let hue = |bb: f64, aa: f64| {
        if bb == 0.0 && aa == 0.0 {
            0.0
        } else {
            let h = bb.atan2(aa).to_degrees();
            if h < 0.0 {
                h + 360.0
            } else {
                h
            }
        }
    };
    let (h1p, h2p) = (hue(b1, a1p), hue(b2, a2p));
    let dlp = l2 - l1;
    let dcp = c2p - c1p;
    let dhp = if c1p * c2p == 0.0 {
        0.0
    } else if (h2p - h1p).abs() <= 180.0 {
        h2p - h1p
    } else if h2p - h1p > 180.0 {
        h2p - h1p - 360.0
    } else {
        h2p - h1p + 360.0
    };
    let dhp_big = 2.0 * (c1p * c2p).sqrt() * (dhp.to_radians() / 2.0).sin();
    let lbp = (l1 + l2) / 2.0;
    let cbp = (c1p + c2p) / 2.0;
    let hbp = if c1p * c2p == 0.0 {
        h1p + h2p
    } else if (h1p - h2p).abs() <= 180.0 {
        (h1p + h2p) / 2.0
    } else if h1p + h2p < 360.0 {
        (h1p + h2p + 360.0) / 2.0
    } else {
        (h1p + h2p - 360.0) / 2.0
    };
    let t = 1.0 - 0.17 * ((hbp - 30.0) * PI / 180.0).cos()
        + 0.24 * ((2.0 * hbp) * PI / 180.0).cos()
        + 0.32 * ((3.0 * hbp + 6.0) * PI / 180.0).cos()
        - 0.20 * ((4.0 * hbp - 63.0) * PI / 180.0).cos();
    let dtheta = 30.0 * (-((hbp - 275.0) / 25.0).powi(2)).exp();
    let rc = 2.0 * (cbp.powi(7) / (cbp.powi(7) + 25f64.powi(7))).sqrt();
    let sl = 1.0 + 0.015 * (lbp - 50.0).powi(2) / (20.0 + (lbp - 50.0).powi(2)).sqrt();
    let sc = 1.0 + 0.045 * cbp;
    let sh = 1.0 + 0.015 * cbp * t;
    let rt = -(2.0 * dtheta * PI / 180.0).sin() * rc;
    ((dlp / sl).powi(2)
        + (dcp / sc).powi(2)
        + (dhp_big / sh).powi(2)
        + rt * (dcp / sc) * (dhp_big / sh))
        .sqrt()
}

/// Smallest CIEDE2000 difference between any two colours as seen with
/// `vision`.
pub fn min_difference(colours: &[[u8; 3]], vision: Vision) -> f64 {
    let labs: Vec<[f64; 3]> = colours.iter().map(|&c| lab(c, vision)).collect();
    let mut min = f64::INFINITY;
    for i in 0..labs.len() {
        for j in i + 1..labs.len() {
            min = min.min(delta_e2000(labs[i], labs[j]));
        }
    }
    min
}

/// Smallest difference required between the colours of a categorical
/// scale, and between the two ends of a scale, in every simulated vision.
pub const MIN_DIFFERENCE: f64 = 10.0;

/// The colour-blind check: one line per scale and vision, and whether all
/// pass. Sequential scales must keep a monotonic lightness, so their order
/// survives every simulated vision.
pub fn check() -> (Vec<String>, bool) {
    let mut lines = vec![format!(
        "Colour check: Machado et al. (2009) simulation at severity 1.0, CIEDE2000; required difference {MIN_DIFFERENCE}"
    )];
    let mut ok = true;
    let samples: Vec<f64> = (0..=20).map(|i| i as f64 / 20.0).collect();
    for v in VISIONS {
        // Groups: every pair.
        let g = min_difference(&GROUPS, v);
        let pass_g = g >= MIN_DIFFERENCE;
        // Support: monotonic lightness and distinct ends.
        let support: Vec<[u8; 3]> = samples.iter().map(|&x| support_colour(Some(x))).collect();
        let l: Vec<f64> = support.iter().map(|&c| lab(c, v)[0]).collect();
        let monotonic = l.windows(2).all(|w| w[1] < w[0]);
        let ends = delta_e2000(lab(support[0], v), lab(support[20], v));
        let pass_s = monotonic && ends >= MIN_DIFFERENCE;
        // Halo: lightness rises to the middle and falls after it; ends differ.
        let halo: Vec<[u8; 3]> = samples
            .iter()
            .map(|&x| crate::tree::halo_colour(Some(x)))
            .collect();
        let hl: Vec<f64> = halo.iter().map(|&c| lab(c, v)[0]).collect();
        let up = hl[..=10].windows(2).all(|w| w[1] > w[0]);
        let down = hl[10..].windows(2).all(|w| w[1] < w[0]);
        let hends = delta_e2000(lab(halo[0], v), lab(halo[20], v));
        let pass_h = up && down && hends >= MIN_DIFFERENCE;
        lines.push(format!(
            "{:<13} groups: smallest difference {g:.1} ({}); support: lightness {}, ends differ by {ends:.1} ({}); halo: lightness {} to the middle and {} after it, ends differ by {hends:.1} ({})",
            v.name(),
            if pass_g { "pass" } else { "FAIL" },
            if monotonic { "monotonic" } else { "NOT monotonic" },
            if pass_s { "pass" } else { "FAIL" },
            if up { "rises" } else { "does NOT rise" },
            if down { "falls" } else { "does NOT fall" },
            if pass_h { "pass" } else { "FAIL" },
        ));
        ok &= pass_g && pass_s && pass_h;
    }
    (lines, ok)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ciede2000_matches_the_published_test_pairs() {
        // Sharma, Wu and Dalal (2005), pairs 1, 7 and 17.
        let cases = [
            ([50.0, 2.6772, -79.7751], [50.0, 0.0, -82.7485], 2.0425),
            ([50.0, 0.0, 0.0], [50.0, -1.0, 2.0], 2.3669),
            ([50.0, 2.5, 0.0], [73.0, 25.0, -18.0], 27.1492),
        ];
        for (a, b, want) in cases {
            assert!((delta_e2000(a, b) - want).abs() < 1e-4, "{a:?} {b:?}");
        }
    }

    #[test]
    fn lab_of_white_and_black() {
        let w = lab([255, 255, 255], Vision::Normal);
        assert!((w[0] - 100.0).abs() < 0.01 && w[1].abs() < 0.01 && w[2].abs() < 0.01);
        assert!(lab([0, 0, 0], Vision::Normal)[0].abs() < 1e-9);
        // Simulations keep grey grey.
        for v in VISIONS {
            let g = lab([128, 128, 128], v);
            assert!(g[1].abs() < 1.0 && g[2].abs() < 1.0, "{v:?} {g:?}");
        }
    }

    #[test]
    fn the_palettes_pass_the_colour_blind_check() {
        let (lines, ok) = check();
        for l in &lines {
            eprintln!("{l}");
        }
        assert!(ok, "{lines:#?}");
    }

    #[test]
    fn support_runs_from_light_to_dark_and_widens() {
        assert!(
            lab(support_colour(Some(0.0)), Vision::Normal)[0]
                > lab(support_colour(Some(1.0)), Vision::Normal)[0]
        );
        assert_eq!(support_width(Some(1.0)), 4.0);
        assert_eq!(support_width(None), 1.0);
        assert_eq!(hex(group_colour(7)), hex(group_colour(0)));
    }
}
