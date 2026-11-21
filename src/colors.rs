//! Color math, palette extraction and .attheme generation.
//!
//! Ported 1:1 from the Python `colors.py`. Every color still comes from the
//! user's picture: hues are never invented, only lightness is adjusted.

use image::{DynamicImage, GenericImage, GenericImageView, Rgba};

pub const AV: [&str; 7] = ["Blue", "Cyan", "Green", "Orange", "Pink", "Red", "Violet"];
pub const BLACK: Rgb = Rgb { r: 0, g: 0, b: 0 };
/// Used ONLY when a file can't be decoded at all (achromatic — no invented hue).
pub const ACHROMATIC: [&str; 6] = [
    "#101010", "#2c2c2c", "#4d4d4d", "#707070", "#969696", "#bcbcbc",
];

/// RGB color in 0-255 channels (no alpha).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Rgb {
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }
    pub fn tuple(self) -> (i32, i32, i32) {
        (self.r as i32, self.g as i32, self.b as i32)
    }
}

/// Theme-wide resolved colors (see `resolve_theme`).
#[derive(Clone, Debug)]
pub struct Resolved {
    pub bg: Rgb,
    pub bar: Rgb,
    pub r#in: Rgb,
    pub out: Rgb,
    pub text: Rgb,
    pub accent: Rgb,
    pub reply: Rgb,
    pub link: Rgb,
    pub error: Rgb,
    pub success: Rgb,
    pub flame: Rgb,
    pub shadow: Rgb,
}

impl Resolved {
    pub fn dark(&self) -> bool {
        luminance(self.bg) < 0.5
    }
}

/// Semantic colors derived from the picture palette only.
#[derive(Clone, Debug, Default)]
pub struct Semantics {
    pub link: Rgb,
    pub error: Rgb,
    pub success: Rgb,
    pub flame: Rgb,
    pub shadow: Rgb,
}

// ---------- Color math (WCAG) ----------

pub fn rgb_to_hex(c: Rgb) -> String {
    format!("#{:02x}{:02x}{:02x}", c.r, c.g, c.b)
}

pub fn hex_to_rgb(s: &str) -> Option<Rgb> {
    let t = s.trim_start_matches('#').trim();
    let t = if t.len() == 3 {
        t.chars()
            .flat_map(|c| std::iter::repeat(c).take(2))
            .collect::<String>()
    } else {
        t.to_string()
    };
    if t.len() != 6 {
        return None;
    }
    let h = u32::from_str_radix(&t, 16).ok()?;
    Some(Rgb::new(
        ((h >> 16) & 0xff) as u8,
        ((h >> 8) & 0xff) as u8,
        (h & 0xff) as u8,
    ))
}

fn srgb_to_lin(c: u8) -> f64 {
    let v = f64::from(c) / 255.0;
    if v <= 0.04045 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}

pub fn luminance(c: Rgb) -> f64 {
    0.2126 * srgb_to_lin(c.r) + 0.7152 * srgb_to_lin(c.g) + 0.0722 * srgb_to_lin(c.b)
}

/// `weight = 1.0` → c1, `weight = 0.0` → c2.
pub fn mix(c1: Rgb, c2: Rgb, weight: f64) -> Rgb {
    let w = weight.clamp(0.0, 1.0);
    let ch = |a: u8, b: u8| {
        ((f64::from(a) * w + f64::from(b) * (1.0 - w)).round() as i32).clamp(0, 255) as u8
    };
    Rgb::new(ch(c1.r, c2.r), ch(c1.g, c2.g), ch(c1.b, c2.b))
}

pub fn contrast_ratio(c1: Rgb, c2: Rgb) -> f64 {
    let (l1, l2) = (luminance(c1), luminance(c2));
    (l1.max(l2) + 0.05) / (l1.min(l2) + 0.05)
}

pub fn readable_on(bg: Rgb) -> Rgb {
    if luminance(bg) < 0.45 {
        Rgb::new(255, 255, 255)
    } else {
        Rgb::new(0, 0, 0)
    }
}

/// Gentle: steps only as far as needed, keeps the pic's tone.
pub fn ensure_contrast(fg: Rgb, bg: Rgb, min_ratio: f64) -> Rgb {
    if contrast_ratio(fg, bg) >= min_ratio {
        return fg;
    }
    let target = readable_on(bg);
    let mut best = fg;
    let mut best_ratio = contrast_ratio(fg, bg);
    for step in 1..10 {
        let step_fg = mix(target, fg, f64::from(step) / 10.0);
        let r = contrast_ratio(step_fg, bg);
        if r >= min_ratio {
            return step_fg;
        }
        if r > best_ratio {
            best = step_fg;
            best_ratio = r;
        }
    }
    best
}

// ---------- HLS (Python colorsys semantics) ----------

#[derive(Clone, Copy, Debug)]
pub struct Hls {
    pub h: f64,
    pub l: f64,
    pub s: f64,
}

fn hls(c: Rgb) -> Hls {
    // Mirrors Python colorsys.rgb_to_hls (H ∈ [0,1), hue order preserved).
    let (r, g, b) = (
        f64::from(c.r) / 255.0,
        f64::from(c.g) / 255.0,
        f64::from(c.b) / 255.0,
    );
    let (mx, mn) = (r.max(g).max(b), r.min(g).min(b));
    let l = (mx + mn) / 2.0;
    if (mx - mn).abs() < f64::EPSILON {
        return Hls { h: 0.0, l, s: 0.0 };
    }
    let d = mx - mn;
    let s = if l > 0.5 {
        d / (2.0 - mx - mn)
    } else {
        d / (mx + mn)
    };
    let rc = (mx - r) / d;
    let gc = (mx - g) / d;
    let bc = (mx - b) / d;
    let h = match mx {
        x if (x - r).abs() < f64::EPSILON => bc - gc,
        x if (x - g).abs() < f64::EPSILON => 2.0 + rc - bc,
        _ => 4.0 + gc - rc,
    };
    let h = (h / 6.0).rem_euclid(1.0);
    Hls { h, l, s }
}

fn hls_to_rgb(h: f64, l: f64, s: f64) -> Rgb {
    // Mirrors Python colorsys.hls_to_rgb.
    let m2 = if l <= 0.5 {
        l * (1.0 + s)
    } else {
        l + s - l * s
    };
    let m1 = 2.0 * l - m2;
    let cv = |hue: f64| -> f64 {
        let hue = hue.rem_euclid(1.0);
        if hue < 1.0 / 6.0 {
            m1 + (m2 - m1) * hue * 6.0
        } else if hue < 0.5 {
            m2
        } else if hue < 2.0 / 3.0 {
            m1 + (m2 - m1) * (2.0 / 3.0 - hue) * 6.0
        } else {
            m1
        }
    };
    let ch = |v: f64| ((v * 255.0).round() as i32).clamp(0, 255) as u8;
    Rgb::new(ch(cv(h + 1.0 / 3.0)), ch(cv(h)), ch(cv(h - 1.0 / 3.0)))
}

pub fn saturation(c: Rgb) -> f64 {
    hls(c).s
}

pub fn set_lightness(c: Rgb, l: f64) -> Rgb {
    let Hls { h, s, .. } = hls(c);
    hls_to_rgb(h, l, s)
}

pub fn clamp_lightness(c: Rgb, min_l: Option<f64>, max_l: Option<f64>) -> Rgb {
    let Hls { h, mut l, s } = hls(c);
    if let Some(mn) = min_l {
        if l < mn {
            l = mn;
        }
    }
    if let Some(mx) = max_l {
        if l > mx {
            l = mx;
        }
    }
    hls_to_rgb(h, l, s)
}

fn hue_deg(c: Rgb) -> f64 {
    hls(c).h * 360.0
}

fn in_hue_deg(c: Rgb, lo: f64, hi: f64) -> bool {
    let hd = hue_deg(c);
    if lo <= hi {
        lo <= hd && hd <= hi
    } else {
        hd >= lo || hd <= hi
    }
}

fn pick_hue(pal: &[Rgb], lo: f64, hi: f64) -> Option<Rgb> {
    // Most saturated PIC color in the hue range — selection only, no invention.
    pal.iter()
        .copied()
        .filter(|&c| saturation(c) >= 0.15 && in_hue_deg(c, lo, hi))
        .max_by(|&a, &b| saturation(a).partial_cmp(&saturation(b)).unwrap())
}

// ---------- Palette extraction (colors ONLY from the pic) ----------

fn tone_variants(rgb: Rgb, count: usize) -> Vec<Rgb> {
    // Lighter/darker tones of one pic color — hue & saturation untouched.
    let Hls { h, l, s } = hls(rgb);
    let mut out = vec![rgb];
    let offsets = [0.16, -0.14, 0.30, -0.26, 0.42, -0.38, 0.54, -0.50];
    let mut i = 0;
    while out.len() < count && i < offsets.len() * 3 {
        let off = offsets[i % offsets.len()] + (i / offsets.len()) as f64 * 0.06;
        let ll = (l + off).clamp(0.05, 0.95);
        let cand = hls_to_rgb(h, ll, s);
        if out.iter().all(|o| dist(cand, *o) >= 45.0) {
            out.push(cand);
        }
        i += 1;
    }
    while out.len() < count {
        let ll = 0.05 + 0.90 * (out.len() - 1) as f64 / (count.max(1) - 1) as f64;
        out.push(hls_to_rgb(h, ll.clamp(0.05, 0.95), s));
    }
    out.truncate(count);
    out
}

fn dist(a: Rgb, b: Rgb) -> f64 {
    let (ar, ag, ab) = a.tuple();
    let (br, bg, bb) = b.tuple();
    let d2 = (ar - br).pow(2) + (ag - bg).pow(2) + (ab - bb).pow(2);
    (d2 as f64).sqrt()
}

fn thumb(img: &DynamicImage, max_side: u32) -> DynamicImage {
    // LANCZOS downscale keeping aspect, like PIL's thumbnail().
    let (w, h) = img.dimensions();
    if w <= max_side && h <= max_side {
        return img.clone();
    }
    let ratio = f64::from(w) / f64::from(h);
    let (nw, nh) = if w > h {
        (max_side, ((max_side as f64 / ratio) as u32).max(1))
    } else {
        (((max_side as f64 * ratio) as u32).max(1), max_side)
    };
    img.resize_exact(nw, nh, image::imageops::FilterType::Lanczos3)
}

/// LANCZOS 1×1 resize ≈ average color (PIL used resize((1,1), LANCZOS)).
fn average_color(img: &DynamicImage) -> Option<Rgb> {
    let p = img.resize_exact(1, 1, image::imageops::FilterType::Lanczos3);
    let Rgba([r, g, b, _]) = p.get_pixel(0, 0);
    Some(Rgb::new(r, g, b))
}

/// Median-cut quantization to `colors` palettes, backfilled from histogram.
fn median_cut_quantize(img: &DynamicImage, colors: u32) -> Option<Vec<(u64, Rgb)>> {
    let (w, h) = img.dimensions();
    let rgba = img.to_rgba8();
    let mut pixels: Vec<[u8; 3]> = Vec::with_capacity((w * h) as usize);
    for (_, _, px) in rgba.enumerate_pixels() {
        let Rgba([r, g, b, a]) = *px;
        if a >= 128 {
            pixels.push([r, g, b]);
        } // skip transparent pixels
    }
    if pixels.is_empty() {
        return None;
    }
    // Sort by the channel with the widest range; median split; recurse.
    fn widest(pixels: &[[u8; 3]]) -> usize {
        let mut mn = [255u8; 3];
        let mut mx = [0u8; 3];
        for p in pixels {
            for c in 0..3 {
                mn[c] = mn[c].min(p[c]);
                mx[c] = mx[c].max(p[c]);
            }
        }
        let mut best = 0;
        let mut best_range = 0u8;
        for c in 0..3 {
            let range = mx[c].saturating_sub(mn[c]);
            if range > best_range {
                best_range = range;
                best = c;
            }
        }
        best
    }
    let mut boxes: Vec<Vec<[u8; 3]>> = vec![pixels];
    while boxes.len() < colors as usize {
        // Split the box with the largest population × volume.
        let idx = boxes
            .iter()
            .filter(|b| b.len() > 1)
            .enumerate()
            .max_by_key(|(_, b)| b.len() * widest_range(b))
            .map(|(i, _)| i);
        let Some(idx) = idx else { break };
        let mut b = boxes.swap_remove(idx);
        let c = widest(&b);
        b.sort_by_key(|p| p[c]);
        let mid = b.len() / 2;
        let (a, bb) = b.split_at(mid);
        boxes.push(a.to_vec());
        boxes.push(bb.to_vec());
    } // Aggregate each box → palette color; weight = pixel count.
    let mut out: Vec<(u64, Rgb)> = boxes
        .into_iter()
        .filter(|b| !b.is_empty())
        .map(|b| {
            let n = b.len() as u64;
            let s: [u64; 3] = b.iter().fold([0u64; 3], |mut acc, p| {
                for c in 0..3 {
                    acc[c] += u64::from(p[c]);
                }
                acc
            });
            (
                n,
                Rgb::new((s[0] / n) as u8, (s[1] / n) as u8, (s[2] / n) as u8),
            )
        })
        .collect();
    out.sort_by_key(|a| std::cmp::Reverse(a.0));
    Some(out)
}

fn widest_range(b: &[[u8; 3]]) -> usize {
    let mut mn = [255u8; 3];
    let mut mx = [0u8; 3];
    for p in b {
        for c in 0..3 {
            mn[c] = mn[c].min(p[c]);
            mx[c] = mx[c].max(p[c]);
        }
    }
    let mut best = 0u8;
    for c in 0..3 {
        best = best.max(mx[c].saturating_sub(mn[c]));
    }
    best as usize
}

/// 6 distinct colors from the image. Never fails, never invents hues.
pub fn extract_palette(bytes: &[u8], count: usize) -> Vec<String> {
    let achromatic = || {
        ACHROMATIC
            .iter()
            .take(count)
            .map(|s| s.to_string())
            .collect::<Vec<_>>()
    };
    let Ok(img) = image::load_from_memory(bytes) else {
        return achromatic();
    };
    // Flatten alpha onto mid-gray like PIL did.
    let flat = if img.color().has_alpha() {
        let mut c = DynamicImage::new_rgb8(img.width(), img.height());
        for (x, y, px) in img.to_rgba8().enumerate_pixels() {
            let Rgba([r, g, b, a]) = *px;
            let al = f64::from(a) / 255.0;
            let ch = |v: u8| (f64::from(v) * al + 128.0 * (1.0 - al)).round() as u8;
            c.put_pixel(x, y, image::Rgba([ch(r), ch(g), ch(b), 255]));
        }
        c
    } else {
        img.clone()
    };
    let avg = average_color(&flat);

    let small = thumb(&flat, 256);
    let Some(quant) = median_cut_quantize(&small, 24) else {
        return match avg {
            Some(a) => tone_variants(a, count)
                .iter()
                .map(|c| rgb_to_hex(*c))
                .collect(),
            None => achromatic(),
        };
    };

    let mut candidates: Vec<Rgb> = quant
        .iter()
        .map(|(_, c)| *c)
        .filter(|&c| {
            let l = luminance(c);
            (0.03..=0.97).contains(&l)
        })
        .collect();
    if candidates.is_empty() {
        return match avg {
            Some(a) => tone_variants(a, count)
                .iter()
                .map(|c| rgb_to_hex(*c))
                .collect(),
            None => achromatic(),
        };
    }

    let mut selected: Vec<Rgb> = vec![candidates[0]];
    'outer: for cand in candidates.drain(..) {
        if selected.len() >= count {
            break 'outer;
        }
        if selected.iter().all(|s| dist(cand, *s) >= 45.0) {
            selected.push(cand);
        }
    }

    if selected.len() < count {
        let base = selected
            .iter()
            .copied()
            .max_by(|a, b| saturation(*a).partial_cmp(&saturation(*b)).unwrap())
            .unwrap();
        let variants = tone_variants(base, count * 2);
        for cand in variants.clone() {
            if selected.len() >= count {
                break;
            }
            if !selected.contains(&cand) && selected.iter().all(|s| dist(cand, *s) >= 45.0) {
                selected.push(cand);
            }
        }
        let mut j = 0;
        while selected.len() < count && j < variants.len() {
            if !selected.contains(&variants[j]) {
                selected.push(variants[j]);
            }
            j += 1;
        }
    }
    selected.truncate(count);
    selected.iter().map(|c| rgb_to_hex(*c)).collect()
}

// ---------- Wallpaper ----------

/// PNG — matches the format of the theme export. NO tint — original.
pub fn prepare_wallpaper(data: &[u8], max_side: u32) -> Option<Vec<u8>> {
    let img = image::load_from_memory(data).ok()?;
    let t = thumb(&img, max_side);
    let mut buf = std::io::Cursor::new(Vec::new());
    t.to_rgb8()
        .write_to(&mut buf, image::ImageFormat::Png)
        .ok()?;
    Some(buf.into_inner())
}

/// User-selectable blur — a transformation of their own image, no new color.
pub fn blur_wallpaper(wall: &[u8], radius: u32) -> Option<Vec<u8>> {
    let img = image::load_from_memory(wall).ok()?;
    let blurred = img.blur(radius as f32);
    let mut buf = std::io::Cursor::new(Vec::new());
    blurred
        .to_rgb8()
        .write_to(&mut buf, image::ImageFormat::Png)
        .ok()?;
    Some(buf.into_inner())
}

// ---------- Semantic colors — pic colors & their tones ONLY ----------

pub fn derive_semantics(pal: &[Rgb], bg: Rgb, accent: Rgb, dark: bool) -> Semantics {
    let mut pal = pal.to_vec();
    if pal.is_empty() {
        pal = vec![bg];
    }
    let tone = |base: Rgb, mn: Option<f64>, mx: Option<f64>| {
        ensure_contrast(clamp_lightness(base, mn, mx), bg, 3.0)
    };
    let (link, error, success, flame) = if dark {
        (
            tone(
                pick_hue(&pal, 195.0, 265.0).unwrap_or(accent),
                Some(0.42),
                None,
            ),
            tone(
                pick_hue(&pal, 330.0, 25.0).unwrap_or_else(|| mix(accent, BLACK, 0.22)),
                Some(0.45),
                None,
            ),
            tone(
                pick_hue(&pal, 90.0, 150.0).unwrap_or_else(|| mix(accent, BLACK, 0.12)),
                Some(0.42),
                None,
            ),
            tone(
                pick_hue(&pal, 10.0, 50.0).unwrap_or(accent),
                Some(0.50),
                None,
            ),
        )
    } else {
        (
            tone(
                pick_hue(&pal, 195.0, 265.0).unwrap_or(accent),
                None,
                Some(0.45),
            ),
            tone(
                pick_hue(&pal, 330.0, 25.0).unwrap_or_else(|| mix(accent, BLACK, 0.35)),
                None,
                Some(0.42),
            ),
            tone(
                pick_hue(&pal, 90.0, 150.0).unwrap_or_else(|| mix(accent, BLACK, 0.25)),
                None,
                Some(0.42),
            ),
            tone(
                pick_hue(&pal, 10.0, 50.0).unwrap_or(accent),
                None,
                Some(0.55),
            ),
        )
    };
    Semantics {
        link,
        error,
        success,
        flame,
        shadow: mix(bg, BLACK, if dark { 0.5 } else { 0.15 }),
    }
}

// ---------- Section resolution ----------

/// One editable color section: palette index, custom hex or auto (-1).
/// NOTE: `idx = -1` means "auto" — `Default::default()` must NOT be 0.
#[derive(Clone, Debug)]
pub struct SectionChoice {
    pub idx: i32,
    pub alpha: u32,
    pub custom: Option<Rgb>,
}

impl Default for SectionChoice {
    fn default() -> Self {
        Self {
            idx: -1,
            alpha: 0,
            custom: None,
        }
    }
}

impl SectionChoice {
    pub fn chosen(&self, pal: &[Rgb]) -> Option<Rgb> {
        if let Some(c) = self.custom {
            return Some(c);
        }
        if self.idx >= 0 && (self.idx as usize) < pal.len() {
            Some(pal[self.idx as usize])
        } else {
            None
        }
    }
}

pub fn resolve_theme(palette: &[String], sections: &HashMapSections, mode: &str) -> Resolved {
    let dark = mode == "dark";
    let pal: Vec<Rgb> = if palette.is_empty() {
        ACHROMATIC.iter().filter_map(|s| hex_to_rgb(s)).collect()
    } else {
        palette.iter().filter_map(|s| hex_to_rgb(s)).collect()
    };

    let chosen = |k: &str| sections.get(k).and_then(|s| s.chosen(&pal));

    let darkest = pal
        .iter()
        .copied()
        .min_by(|a, b| luminance(*a).partial_cmp(&luminance(*b)).unwrap())
        .unwrap_or(BLACK);
    let lightest = pal
        .iter()
        .copied()
        .max_by(|a, b| luminance(*a).partial_cmp(&luminance(*b)).unwrap())
        .unwrap_or(BLACK);
    let most_sat = pal
        .iter()
        .copied()
        .max_by(|a, b| saturation(*a).partial_cmp(&saturation(*b)).unwrap())
        .unwrap_or(BLACK);

    let auto_bg = if dark {
        set_lightness(darkest, 0.07)
    } else {
        set_lightness(lightest, 0.96)
    };
    let auto_accent = if dark {
        clamp_lightness(most_sat, Some(0.55), None)
    } else {
        clamp_lightness(most_sat, None, Some(0.50))
    };

    let bg = chosen("bg").unwrap_or(auto_bg);
    // Text: ALWAYS pure white in Dark mode (Light keeps near-black).
    let text = chosen("text").unwrap_or(if dark {
        Rgb::new(255, 255, 255)
    } else {
        Rgb::new(20, 20, 20)
    });
    let accent = chosen("accent").unwrap_or(auto_accent);
    let bar = chosen("bar").unwrap_or(bg);
    let inb = chosen("in").unwrap_or_else(|| mix(bg, BLACK, if dark { 0.12 } else { 0.05 }));
    let out = chosen("out").unwrap_or(accent);
    let reply = chosen("reply").unwrap_or(text);
    let sem = derive_semantics(&pal, bg, accent, dark);
    Resolved {
        bg,
        bar,
        r#in: inb,
        out,
        text,
        accent,
        reply,
        link: sem.link,
        error: sem.error,
        success: sem.success,
        flame: sem.flame,
        shadow: sem.shadow,
    }
}

/// HashMap of section name → choice (kept for parity with the Python dict).
pub type HashMapSections = std::collections::BTreeMap<String, SectionChoice>;

pub fn resolve_wall(
    palette: &[String],
    wall_idx: i32,
    wall_custom: Option<Rgb>,
    mode: &str,
) -> Rgb {
    if let Some(c) = wall_custom {
        return c;
    }
    let pal: Vec<Rgb> = if palette.is_empty() {
        ACHROMATIC.iter().filter_map(|s| hex_to_rgb(s)).collect()
    } else {
        palette.iter().filter_map(|s| hex_to_rgb(s)).collect()
    };
    if wall_idx >= 0 && (wall_idx as usize) < pal.len() {
        return pal[wall_idx as usize];
    }
    if mode == "dark" {
        let d = *pal
            .iter()
            .min_by_key(|&&c| luminance(c).total_cmp(&luminance(*pal.first().unwrap_or(&BLACK))))
            .unwrap_or(&BLACK);
        set_lightness(d, 0.06)
    } else {
        let l = *pal
            .iter()
            .max_by_key(|&&c| luminance(c).total_cmp(&luminance(*pal.first().unwrap_or(&BLACK))))
            .unwrap_or(&Rgb::new(255, 255, 255));
        set_lightness(l, 0.97)
    }
}

// ---------- .attheme serialization ----------

fn attheme_int(c: Rgb, alpha: i64) -> i64 {
    let a = alpha.clamp(0, 255);
    let val = (a << 24) | (i64::from(c.r) << 16) | (i64::from(c.g) << 8) | i64::from(c.b);
    val - 0x1_0000_0000 * (val >= 0x8000_0000) as i64
}

/// Build the .attheme bytes from resolved colors, alphas (0-100) and wallpaper.
pub fn build_attheme(
    c: &Resolved,
    alphas: &std::collections::BTreeMap<String, u32>,
    wallpaper: Option<&[u8]>,
    wall_flat: Option<Rgb>,
    blur: bool,
) -> Vec<u8> {
    let (bg, bar, inb, outb) = (c.bg, c.bar, c.r#in, c.out);
    let (text, accent, reply) = (c.text, c.accent, c.reply);
    let link = c.link;
    let error = c.error;
    let success = c.success;
    let shadow = c.shadow;

    let get_alpha = |k: &str| -> i64 {
        let pct = alphas.get(k).copied().unwrap_or(0).min(100);
        (255.0 * (1.0 - f64::from(pct) / 100.0)).round() as i64
    };
    let (a_bar, a_in, a_out) = (get_alpha("bar"), get_alpha("in"), get_alpha("out"));
    let (a_text, a_acc, a_reply) = (get_alpha("text"), get_alpha("accent"), get_alpha("reply"));

    let dark = luminance(bg) < 0.5;

    // ---- tones: darker/lighter versions of the pic's colors only ----
    let deep1 = if dark {
        mix(bg, BLACK, 0.18)
    } else {
        mix(bg, BLACK, 0.04)
    };
    let deep2 = if dark {
        mix(bg, BLACK, 0.32)
    } else {
        mix(bg, BLACK, 0.09)
    };
    let divider = if dark {
        mix(bg, BLACK, 0.45)
    } else {
        mix(bg, BLACK, 0.14)
    };

    let bar_text = ensure_contrast(text, bar, 3.0);
    let bar_sub = mix(bar_text, bar, 0.65);
    let bar_icon = mix(bar_text, bar, 0.82);

    let in_text = ensure_contrast(text, inb, 3.0);
    let out_text = ensure_contrast(text, outb, 3.0);
    let in_time = mix(in_text, inb, 0.72);
    let out_time = mix(out_text, outb, 0.75);

    let acc_text = ensure_contrast(accent, bg, 3.0);
    let on_acc = readable_on(accent);
    let acc_in = ensure_contrast(accent, inb, 3.0);
    let acc_out = ensure_contrast(accent, outb, 3.0);
    let reply_in = ensure_contrast(reply, inb, 3.0);
    let reply_out = ensure_contrast(reply, outb, 3.0);

    let gray1 = mix(text, bg, 0.80);
    let gray2 = mix(text, bg, 0.68);
    let gray3 = mix(text, bg, 0.55);

    let thumb = readable_on(bg);
    let in_sel = mix(inb, BLACK, 0.12);
    let out_sel = mix(outb, BLACK, 0.12);
    let in_deep = mix(inb, BLACK, 0.18);
    let out_deep = mix(outb, BLACK, 0.18);
    let fab = mix(accent, BLACK, if dark { 0.32 } else { 0.18 });
    let fab_pressed = mix(fab, BLACK, 0.15);
    let on_fab = readable_on(fab);
    let counter_bg = mix(accent, BLACK, 0.42);
    let on_counter = readable_on(counter_bg);

    let mut m: Vec<(String, Rgb, i64)> = Vec::new();
    let mut put = |keys: &[&str], rgb: Rgb, alpha: i64| {
        for k in keys {
            m.push(((*k).to_string(), rgb, alpha));
        }
    };
    let av_bar: Vec<String> = AV
        .iter()
        .map(|c| format!("avatar_backgroundActionBar{}", c))
        .collect();
    let _av_av: Vec<String> = AV
        .iter()
        .map(|c| format!("avatar_background{}", c))
        .collect();
    let av_name: Vec<String> = AV
        .iter()
        .map(|c| format!("avatar_nameInMessage{}", c))
        .collect();
    let av_sub: Vec<String> = AV
        .iter()
        .map(|c| format!("avatar_subtitleInProfile{}", c))
        .collect();
    let av_prof: Vec<String> = AV
        .iter()
        .map(|c| format!("avatar_backgroundInProfile{}", c))
        .collect();
    let av_abi: Vec<String> = AV
        .iter()
        .map(|c| format!("avatar_actionBarIcon{}", c))
        .collect();
    let av_abs: Vec<String> = AV
        .iter()
        .map(|c| format!("avatar_actionBarSelector{}", c))
        .collect();

    // ===== Links =====
    put(
        &[
            "windowBackgroundWhiteLinkText",
            "dialogTextLink",
            "chat_messageLinkIn",
            "chat_messageLinkOut",
            "chat_serviceLink",
        ],
        link,
        255,
    );
    // ===== Surfaces = bg (opaque) =====
    put(
        &[
            "windowBackgroundWhite",
            "windowBackgroundGray",
            "windowBackgroundBlack",
            "dialogBackground",
            "dialogBackgroundGray",
            "graySection",
            "actionBarDefaultSubmenuBackground",
            "chats_menuBackground",
            "chats_pinnedOverlay",
            "chats_archiveBackground",
            "chat_emojiPanelBackground",
            "chat_stickersHintPanel",
            "chat_topPanelBackground",
            "chat_messagePanelBackground",
            "chat_messagePanelVoiceBackground",
            "chat_recordedVoiceBackground",
            "contacts_inviteBackground",
            "musicPicker_buttonBackground",
            "chat_unreadMessagesStartBackground",
            "player_background",
            "inappPlayerBackground",
            "profile_actionBackground",
            "sharedMedia_actionMode",
            "returnToCallBackground",
            "player_actionBar",
            "player_actionBarTop",
            "key_chat_messagePanelVoiceLockBackground",
            "windowBackgroundGrayShadow",
        ],
        bg,
        255,
    );
    // ===== Elevation = darker, never lighter =====
    put(
        &["files_folderIconBackground", "chat_secretTimerBackground"],
        deep2,
        220,
    );
    put(
        &["chat_goDownButton", "chat_botKeyboardButtonBackground"],
        deep2,
        230,
    );
    put(
        &["chat_botKeyboardButtonBackgroundPressed"],
        mix(deep2, BLACK, 0.20),
        255,
    );
    put(&["chat_goDownButtonShadow"], shadow, 80);
    put(&["chat_messagePanelShadow"], deep2, 120);
    put(
        &["key_chat_messagePanelVoiceLockShadow", "ChatShadow"],
        shadow,
        80,
    );
    put(&["chats_menuTopShadow"], deep1, 60);
    // ===== Top bar =====
    put(
        &[
            "actionBarDefault",
            "actionBarDefaultArchived",
            "actionBarActionModeDefaultTop",
        ],
        bar,
        a_bar,
    );
    for k in &av_bar {
        put(&[k], bar, 255);
    }
    put(
        &["actionBarActionModeDefault"],
        mix(bar, accent, 0.20),
        a_bar,
    );
    put(
        &[
            "actionBarDefaultTitle",
            "actionBarDefaultArchivedTitle",
            "actionBarDefaultSearch",
            "actionBarDefaultArchivedSearch",
            "actionBarActionModeDefaultTitle",
            "player_actionBarTitle",
        ],
        bar_text,
        255,
    );
    put(
        &[
            "actionBarDefaultSubtitle",
            "actionBarDefaultSearchPlaceholder",
            "actionBarDefaultSearchArchivedPlaceholder",
            "actionBarTabText",
            "player_actionBarSubtitle",
        ],
        bar_sub,
        255,
    );
    let mut top_icons = vec![
        "actionBarDefaultIcon",
        "actionBarDefaultArchivedIcon",
        "actionBarActionModeDefaultIcon",
        "player_actionBarItems",
    ];
    for k in &av_abi {
        top_icons.push(k.as_str());
    }
    put(&top_icons, bar_icon, 255);
    let mut top_sel = vec![
        "actionBarDefaultSelector",
        "actionBarActionModeDefaultSelector",
        "actionBarWhiteSelector",
        "player_actionBarSelector",
    ];
    for k in &av_abs {
        top_sel.push(k.as_str());
    }
    put(&top_sel, text, if dark { 30 } else { 25 });
    put(&["actionBarTabLine"], accent, 255);
    put(
        &["actionBarTabActiveText"],
        ensure_contrast(accent, bar, 3.0),
        255,
    );
    put(
        &["actionBarDefaultSubmenuItem", "chats_menuItemText"],
        text,
        255,
    );
    put(
        &["actionBarDefaultSubmenuItemIcon", "chats_menuItemIcon"],
        gray2,
        255,
    );
    // ===== Avatars =====
    let mut avatar_bg: Vec<&str> = AV.to_vec();
    avatar_bg.extend([
        "avatar_backgroundSaved",
        "avatar_backgroundArchived",
        "avatar_backgroundGroupCreateSpanBlue",
    ]);
    put(&avatar_bg, accent, 255);
    for k in &av_name {
        put(&[k], acc_text, 255);
    }
    for k in &av_sub {
        put(&[k], gray2, 255);
    }
    for k in &av_prof {
        put(&[k], mix(accent, BLACK, 0.15), 255);
    }
    put(&["avatar_text"], on_acc, 255);
    // ===== Incoming bubble =====
    put(&["chat_inBubble"], inb, a_in);
    put(&["chat_inBubbleSelected"], in_sel, a_in.max(180));
    put(&["chat_inBubbleShadow"], shadow, 255);
    put(&["chat_messageTextIn"], in_text, a_text);
    put(
        &["chat_inTimeText", "chat_inTimeSelectedText"],
        in_time,
        255,
    );
    put(&["chat_inReplyLine"], acc_in, 255);
    put(
        &[
            "chat_inAudioDurationText",
            "chat_inAudioDurationSelectedText",
            "chat_inAudioTitleText",
            "chat_inFileNameText",
            "chat_inFileInfoText",
            "chat_inFileInfoSelectedText",
            "chat_inContactNameText",
            "chat_inContactPhoneText",
            "chat_inForwardedNameText",
            "chat_inSiteNameText",
            "chat_inViaBotNameText",
            "chat_inPreviewInstantText",
            "chat_inPreviewInstantSelectedText",
            "chat_inViews",
            "chat_inViewsSelected",
            "chat_inMenu",
            "chat_inMenuSelected",
            "chat_inInstant",
            "chat_inInstantSelected",
            "chat_inFileIcon",
            "chat_inFileSelectedIcon",
            "chat_inContactIcon",
            "chat_inPreviewLine",
            "chat_inLoader",
            "chat_inSentClock",
            "chat_inSentClockSelected",
            "chat_inVenueNameText",
            "chat_inLocationIcon",
            "chat_inAudioPerfomerText",
            "chat_inAudioPerformerSelectedText",
        ],
        in_text,
        a_text,
    );
    put(
        &["chat_inVenueInfoText", "chat_inVenueInfoSelectedText"],
        in_time,
        255,
    );
    put(
        &[
            "chat_inAudioSeekbar",
            "chat_inVoiceSeekbar",
            "chat_inAudioSeekbarSelected",
            "chat_inVoiceSeekbarSelected",
        ],
        in_deep,
        255,
    );
    put(
        &["chat_inAudioSeekbarFill", "chat_inVoiceSeekbarFill"],
        in_text,
        255,
    );
    put(
        &[
            "chat_inAudioProgress",
            "chat_inAudioSelectedProgress",
            "chat_inFileProgress",
            "chat_inFileProgressSelected",
        ],
        in_text,
        255,
    );
    put(
        &[
            "chat_inFileBackground",
            "chat_inFileBackgroundSelected",
            "chat_inLocationBackground",
            "chat_inLoaderPhoto",
            "chat_inLoaderPhotoSelected",
            "chat_inContactBackground",
        ],
        in_deep,
        255,
    );
    put(
        &["chat_inLoaderPhotoIcon", "chat_inLoaderPhotoIconSelected"],
        in_text,
        255,
    );
    put(&["chat_inLoaderSelected"], mix(in_text, inb, 0.45), 255);
    // ===== Outgoing bubble =====
    put(&["chat_outBubble"], outb, a_out);
    put(&["chat_outBubbleSelected"], out_sel, a_out.max(180));
    put(
        &[
            "chat_outBubbleGradient",
            "chat_outBubbleGradient2",
            "chat_outBubbleGradient3",
        ],
        outb,
        a_out,
    );
    put(&["chat_outBubbleShadow"], shadow, 255);
    put(&["chat_messageTextOut"], out_text, a_text);
    put(
        &["chat_outTimeText", "chat_outTimeSelectedText"],
        out_time,
        255,
    );
    put(&["chat_outReplyLine"], acc_out, 255);
    put(
        &[
            "chat_outAudioDurationText",
            "chat_outAudioDurationSelectedText",
            "chat_outAudioTitleText",
            "chat_outFileNameText",
            "chat_outFileInfoText",
            "chat_outFileInfoSelectedText",
            "chat_outContactNameText",
            "chat_outContactPhoneText",
            "chat_outForwardedNameText",
            "chat_outSiteNameText",
            "chat_outViaBotNameText",
            "chat_outPreviewInstantText",
            "chat_outPreviewInstantSelectedText",
            "chat_outViews",
            "chat_outViewsSelected",
            "chat_outMenu",
            "chat_outMenuSelected",
            "chat_outInstant",
            "chat_outInstantSelected",
            "chat_outFileIcon",
            "chat_outFileSelectedIcon",
            "chat_outContactIcon",
            "chat_outPreviewLine",
            "chat_outLoader",
            "chat_outSentClock",
            "chat_outSentClockSelected",
            "chat_outVenueNameText",
            "chat_outAudioPerfomerText",
            "chat_outAudioPerformerSelectedText",
            "chat_outLocationIcon",
        ],
        out_text,
        a_text,
    );
    put(
        &["chat_outVenueInfoText", "chat_outVenueInfoSelectedText"],
        out_time,
        255,
    );
    put(
        &[
            "chat_outSentCheck",
            "chat_outSentCheckSelected",
            "chat_outSentCheckRead",
            "chat_outSentCheckReadSelected",
        ],
        out_text,
        255,
    );
    put(&["chat_mediaSentCheck"], out_text, 255);
    put(
        &[
            "chat_outAudioSeekbar",
            "chat_outVoiceSeekbar",
            "chat_outAudioSeekbarSelected",
            "chat_outVoiceSeekbarSelected",
        ],
        out_deep,
        255,
    );
    put(
        &["chat_outAudioSeekbarFill", "chat_outVoiceSeekbarFill"],
        out_text,
        255,
    );
    put(
        &[
            "chat_outAudioProgress",
            "chat_outAudioSelectedProgress",
            "chat_outFileProgress",
            "chat_outFileProgressSelected",
        ],
        out_text,
        255,
    );
    put(
        &[
            "chat_outFileBackground",
            "chat_outFileBackgroundSelected",
            "chat_outLocationBackground",
            "chat_outLoaderPhoto",
            "chat_outLoaderPhotoSelected",
            "chat_outContactBackground",
        ],
        out_deep,
        255,
    );
    put(
        &["chat_outLoaderPhotoIcon", "chat_outLoaderPhotoIconSelected"],
        out_text,
        255,
    );
    put(&["chat_outLoaderSelected"], mix(out_text, outb, 0.45), 255);
    // ===== Reply (tag) block =====
    put(
        &[
            "chat_inReplyNameText",
            "chat_inReplyMessageText",
            "chat_inReplyMediaMessageText",
            "chat_inReplyMediaMessageSelectedText",
        ],
        reply_in,
        a_reply,
    );
    put(
        &[
            "chat_outReplyNameText",
            "chat_outReplyMessageText",
            "chat_outReplyMediaMessageText",
            "chat_outReplyMediaMessageSelectedText",
        ],
        reply_out,
        a_reply,
    );
    put(
        &["chat_stickerReplyNameText", "chat_stickerReplyMessageText"],
        ensure_contrast(reply, bg, 3.0),
        a_reply,
    );
    // ===== Text / UI =====
    put(
        &[
            "windowBackgroundWhiteBlackText",
            "chats_name",
            "chats_nameMessage",
            "chats_nameArchived",
            "chats_nameMessageArchived",
            "chats_secretName",
            "chats_menuName",
            "dialogTextBlack",
            "emptyListPlaceholder",
            "fastScrollText",
            "chat_messagePanelText",
            "profile_title",
            "chats_attachMessage",
            "dialogSearchText",
            "chat_fieldOverlayText",
            "chat_topPanelMessage",
            "chat_topPanelTitle",
            "chat_secretTimerText",
            "groupcreate_sectionText",
            "chats_message_threeLines",
            "chats_nameMessage_threeLines",
            "chats_nameMessageArchived_threeLines",
        ],
        text,
        a_text,
    );
    put(
        &[
            "chats_message",
            "chats_actionMessage",
            "dialogTextGray",
            "player_time",
            "inappPlayerPerformer",
            "chats_menuPhone",
            "windowBackgroundWhiteGrayText1",
        ],
        gray1,
        255,
    );
    put(
        &[
            "chats_date",
            "chats_muteIcon",
            "chats_pinnedIcon",
            "chats_secretIcon",
            "chats_mentionIcon",
            "chats_archiveIcon",
            "chat_muteIcon",
            "chat_lockIcon",
            "chat_messagePanelHint",
            "windowBackgroundWhiteGrayText2",
            "windowBackgroundWhiteGrayText3",
            "windowBackgroundWhiteGrayText4",
            "windowBackgroundWhiteGrayText8",
            "dialogTextGray2",
            "chat_emojiPanelEmptyText",
            "chat_emojiPanelBackspace",
            "chat_emojiPanelIcon",
            "chat_emojiPanelTrendingTitle",
            "fastScrollInactive",
            "inappPlayerClose",
            "chats_menuCloud",
            "chats_menuPhoneCats",
            "chats_menuCloudBackgroundCats",
            "groupcreate_hintText",
            "groupcreate_offlineText",
            "chat_previewDurationText",
            "chat_previewGameText",
            "sessions_devicesImage",
            "changephoneinfo_image",
            "key_sheet_other",
            "key_sheet_scrollUp",
            "chat_unreadMessagesStartArrowIcon",
        ],
        gray2,
        255,
    );
    put(
        &[
            "windowBackgroundWhiteGrayText",
            "windowBackgroundWhiteGrayIcon",
            "windowBackgroundWhiteIcon",
            "stickers_menu",
            "chat_emojiPanelStickerSetName",
            "dialogIcon",
            "dialogSearchIcon",
            "dialogSearchHint",
            "chat_searchPanelIcons",
            "chat_messagePanelIcons",
            "chat_messagePanelVoiceDelete",
            "chat_messagePanelVoiceDuration",
            "chat_messagePanelCancelInlineBot",
            "chat_replyPanelIcons",
            "chat_recordTime",
            "chat_recordVoiceCancel",
            "chat_topPanelClose",
            "chat_topPanelLine",
            "chat_secretTimeText",
            "windowBackgroundWhiteHintText",
        ],
        gray3,
        255,
    );
    put(
        &[
            "divider",
            "dialogGrayLine",
            "dialogShadowLine",
            "chat_emojiPanelShadowLine",
            "windowBackgroundWhiteInputField",
        ],
        divider,
        255,
    );
    put(&["windowBackgroundWhiteInputFieldActivated"], accent, 255);
    // ===== Accent =====
    put(
        &[
            "windowBackgroundWhiteBlueText",
            "windowBackgroundWhiteBlueText2",
            "windowBackgroundWhiteBlueText3",
            "windowBackgroundWhiteBlueText4",
            "windowBackgroundWhiteBlueText6",
            "windowBackgroundWhiteBlueText7",
            "windowBackgroundWhiteBlueHeader",
            "windowBackgroundWhiteValueText",
            "dialogTextBlue",
            "dialogTextBlue2",
            "dialogTextBlue3",
            "dialogTextBlue4",
            "chat_status",
            "chat_addContact",
            "chat_adminText",
            "chat_adminSelectedText",
            "chat_botSwitchToInlineText",
            "dialogInputFieldActivated",
            "groupcreate_cursor",
            "groupcreate_onlineText",
            "chat_messagePanelSend",
            "chat_unreadMessagesStartText",
            "chat_editDoneIcon",
            "chats_sentCheck",
            "chats_sentClock",
            "chats_sentReadCheck",
            "sharedMedia_startStopLoadIcon",
            "PreviewBack",
            "PreviewBackLinear",
        ],
        acc_text,
        a_acc,
    );
    put(
        &[
            "chat_attachGalleryBackground",
            "chat_attachVideoBackground",
            "chat_attachAudioBackground",
            "chat_attachFileBackground",
            "chat_attachContactBackground",
            "chat_attachLocationBackground",
            "chat_attachHideBackground",
            "chat_attachSendBackground",
            "chat_attachMediaBanBackground",
            "undo_background",
            "checkbox",
            "checkboxSquareBackground",
            "dialogCheckboxSquareBackground",
            "radioBackgroundChecked",
            "dialogRadioBackgroundChecked",
            "dialogRoundCheckBox",
            "switchTrackChecked",
            "switch2TrackChecked",
            "dialogLineProgress",
            "dialogProgressCircle",
            "progressCircle",
            "contextProgressInner1",
            "contextProgressOuter1",
            "featuredStickers_addButton",
            "location_sendLocationBackground",
            "location_sendLiveLocationBackground",
            "location_placeLocationBackground",
            "chat_messagePanelVoicePressed",
            "chat_botProgress",
        ],
        accent,
        a_acc,
    );
    put(
        &[
            "chat_attachGalleryIcon",
            "chat_attachVideoIcon",
            "chat_attachFileIcon",
            "chat_attachContactIcon",
            "chat_attachLocationIcon",
            "chat_attachHideIcon",
            "chat_attachSendIcon",
            "chat_attachMediaBanText",
            "chat_attachCameraIcon1",
            "chat_attachCameraIcon2",
            "chat_attachCameraIcon3",
            "chat_attachCameraIcon4",
            "chat_attachCameraIcon5",
            "chat_attachCameraIcon6",
            "location_sendLocationIcon",
            "checkboxCheck",
            "checkboxSquareCheck",
            "dialogCheckboxSquareCheck",
            "dialogRoundCheckBoxCheck",
            "featuredStickers_buttonText",
            "undo_cancelColor",
            "undo_infoColor",
            "musicPicker_checkboxCheck",
            "groupcreate_checkboxCheck",
        ],
        on_acc,
        255,
    );
    // ===== Counters & badges =====
    put(
        &[
            "chats_unreadCounter",
            "chats_verifiedBackground",
            "chats_archivePinBackground",
            "chat_emojiPanelBadgeBackground",
            "picker_badge",
            "dialogBadgeBackground",
        ],
        counter_bg,
        255,
    );
    put(
        &[
            "chats_unreadCounterText",
            "chats_verifiedCheck",
            "picker_badgeText",
            "dialogBadgeText",
            "chat_emojiPanelBadgeText",
            "chats_menuItemCheck",
        ],
        on_counter,
        255,
    );
    // ===== Circle buttons (FAB) =====
    put(
        &[
            "chats_actionBackground",
            "dialogFloatingButton",
            "chat_goDownButtonCounterBackground",
        ],
        fab,
        255,
    );
    put(
        &[
            "chats_actionPressedBackground",
            "dialogFloatingButtonPressed",
            "featuredStickers_addButtonPressed",
        ],
        fab_pressed,
        255,
    );
    put(&["chats_actionIcon", "dialogFloatingIcon"], on_fab, 255);
    // ===== Switches / checkboxes =====
    put(&["switchTrack", "switch2Track"], deep2, 150);
    put(
        &[
            "switchThumb",
            "switchThumbChecked",
            "switch2Thumb",
            "switch2ThumbChecked",
        ],
        thumb,
        255,
    );
    put(
        &[
            "checkboxSquareUnchecked",
            "dialogCheckboxSquareUnchecked",
            "radioBackground",
            "dialogRadioBackground",
        ],
        deep2,
        255,
    );
    put(&["checkboxSquareDisabled"], deep1, 255);
    put(
        &[
            "switch2Check",
            "musicPicker_checkbox",
            "groupcreate_checkbox",
        ],
        accent,
        255,
    );
    // ===== Dialogs / selectors =====
    put(&["dialogButton"], acc_text, 255);
    put(&["dialogButtonSelector"], accent, 60);
    put(&["dialogLineProgressBackground"], deep1, 80);
    put(&["dialogSearchBackground"], deep1, 90);
    put(&["dialogScrollGlow"], gray2, 255);
    put(&["dialogLinkSelection"], link, 60);
    put(&["windowBackgroundWhiteLinkSelection"], link, 60);
    put(&["listSelectorSDK21"], text, if dark { 30 } else { 25 });
    put(&["stickers_menuSelector"], text, if dark { 30 } else { 25 });
    put(
        &["chat_emojiPanelIconSelector"],
        text,
        if dark { 30 } else { 25 },
    );
    put(
        &["chat_emojiPanelStickerPackSelector"],
        text,
        if dark { 40 } else { 30 },
    );
    put(&["chat_selectedBackground"], accent, 90);
    put(&["chat_textSelectBackground"], accent, 100);
    put(&["chat_linkSelectBackground"], link, 50);
    put(
        &["chats_tabletSelectedOverlay"],
        text,
        if dark { 40 } else { 30 },
    );
    // ===== Service chip =====
    put(
        &["chat_serviceBackground", "chat_serviceBackgroundSelected"],
        deep1,
        190,
    );
    put(&["chat_serviceText", "chat_serviceIcon"], text, 255);
    // ===== Bots / emoji panel =====
    put(&["chat_botKeyboardButtonText"], text, 255);
    put(
        &[
            "chat_emojiPanelIconSelected",
            "chat_emojiPanelMasksIconSelected",
            "chat_emojiPanelMasksIcon",
        ],
        accent,
        255,
    );
    put(&["chat_emojiPanelNewTrending"], error, 255);
    put(&["chat_emojiPanelTrendingDescription"], gray2, 255);
    put(
        &[
            "chat_emojiPanelStickerSetNameIcon",
            "chat_stickerViaBotNameText",
            "chat_stickerReplyLine",
        ],
        gray2,
        255,
    );
    // ===== Player =====
    put(&["player_progress"], accent, 255);
    put(&["player_progressBackground"], deep2, 255);
    put(&["player_button", "player_placeholder"], gray3, 255);
    put(&["player_buttonActive"], acc_text, 255);
    put(&["inappPlayerTitle"], text, 255);
    put(&["inappPlayerPlayPause", "inappPlayerClose"], acc_text, 255);
    // ===== Red / green semantics =====
    put(
        &[
            "chats_draft",
            "chat_reportSpam",
            "chat_sentError",
            "chats_sentError",
            "dialogTextRed",
            "windowBackgroundWhiteRedText",
            "windowBackgroundWhiteRedText2",
            "windowBackgroundWhiteRedText3",
            "windowBackgroundWhiteRedText4",
            "windowBackgroundWhiteRedText5",
            "windowBackgroundWhiteRedText6",
            "chat_sentErrorIcon",
            "chats_sentErrorIcon",
            "calls_callReceivedRedIcon",
        ],
        error,
        255,
    );
    put(
        &[
            "windowBackgroundWhiteGreenText2",
            "featuredStickers_addedIcon",
            "calls_callReceivedGreenIcon",
        ],
        success,
        255,
    );
    put(&["chats_unreadCounterMuted"], gray2, 255);
    put(&["chats_unreadCounterMutedText"], text, 255);

    // ===== Assemble =====
    // Python dict preserved first-write wins; replicate with dedup.
    let mut seen = std::collections::HashSet::new();
    let mut lines = String::new();
    for (k, rgb, a) in m {
        if !seen.insert(k.clone()) {
            continue;
        }
        lines.push_str(&format!("{}={}\n", k, attheme_int(rgb, a)));
    }
    let body = lines.into_bytes();

    if let Some(wall) = wallpaper {
        // ORIGINAL image (no tint). Blur only if the user asked for it.
        let wall = if blur {
            blur_wallpaper(wall, 14).unwrap_or_else(|| wall.to_vec())
        } else {
            wall.to_vec()
        };
        let marker = b"\nWPS\n";
        let mut header = b"wallpaperFileOffset=0\n".to_vec();
        for _ in 0..8 {
            let offset = header.len() + body.len() + marker.len();
            let new_header = format!("wallpaperFileOffset={}\n", offset).into_bytes();
            if new_header == header {
                break;
            }
            header = new_header;
        }
        let mut out = Vec::with_capacity(header.len() + body.len() + marker.len() + wall.len());
        out.extend_from_slice(&header);
        out.extend_from_slice(&body);
        out.extend_from_slice(marker);
        out.extend_from_slice(&wall);
        return out;
    }

    let (r, g, b) = wall_flat.unwrap_or(deep2).tuple();
    let mut out = body;
    out.extend_from_slice(
        format!(
            "chat_wallpaper={}\n",
            attheme_int(Rgb::new(r as u8, g as u8, b as u8), 255)
        )
        .as_bytes(),
    );
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_roundtrip() {
        assert_eq!(hex_to_rgb("#34c7a4"), Some(Rgb::new(0x34, 0xc7, 0xa4)));
        assert_eq!(rgb_to_hex(Rgb::new(0x34, 0xc7, 0xa4)), "#34c7a4");
        assert_eq!(hex_to_rgb("#abc"), Some(Rgb::new(0xaa, 0xbb, 0xcc)));
    }

    #[test]
    fn mix_weights() {
        let a = Rgb::new(255, 0, 0);
        let b = Rgb::new(0, 0, 255);
        assert_eq!(mix(a, b, 1.0), a);
        assert_eq!(mix(a, b, 0.0), b);
        assert_eq!(mix(a, b, 0.5), Rgb::new(128, 0, 128));
    }

    #[test]
    fn contrast_white_black() {
        let r = contrast_ratio(Rgb::new(255, 255, 255), Rgb::new(0, 0, 0));
        assert!(r > 20.0, "ratio {r}");
    }

    #[test]
    fn attheme_int_negative_encode() {
        // alpha 255 → sign bit set → two's complement negative, like Python.
        let v = attheme_int(Rgb::new(1, 2, 3), 255);
        assert!(v < 0);
        assert_eq!(v, 0xFF010203u32 as i32 as i64); // two's complement like Python
    }

    #[test]
    fn palette_never_empty() {
        let pal = extract_palette(&[], 6);
        assert_eq!(pal.len(), 6);
        assert!(pal.iter().all(|c| c.starts_with('#')));
    }

    #[test]
    fn resolve_theme_dark_defaults() {
        let pal = vec![
            "#334155".to_string(),
            "#e11d48".to_string(),
            "#22c55e".to_string(),
            "#0ea5e9".to_string(),
            "#f59e0b".to_string(),
            "#8b5cf6".to_string(),
        ];
        let mut secs = HashMapSections::new();
        for k in ["bg", "bar", "in", "out", "text", "accent", "reply"] {
            secs.insert(k.to_string(), SectionChoice::default());
        }
        let r = resolve_theme(&pal, &secs, "dark");
        assert_eq!(
            r.text,
            Rgb::new(255, 255, 255),
            "dark text must be pure white"
        );
        assert!(r.dark());
        assert!(
            luminance(r.bg) < 0.2,
            "dark bg must be near-black, got {}",
            rgb_to_hex(r.bg)
        );
    }

    #[test]
    fn attheme_has_core_keys_and_wallpaper() {
        let pal = vec![
            "#1d3557".to_string(),
            "#e63946".to_string(),
            "#457b9d".to_string(),
            "#a8dadc".to_string(),
            "#f1faee".to_string(),
            "#6d597a".to_string(),
        ];
        let mut secs = HashMapSections::new();
        for k in ["bg", "bar", "in", "out", "text", "accent", "reply"] {
            secs.insert(k.to_string(), SectionChoice::default());
        }
        let r = resolve_theme(&pal, &secs, "dark");
        let mut alphas = std::collections::BTreeMap::new();
        for k in ["bar", "in", "out", "text", "accent", "reply"] {
            alphas.insert(k.to_string(), 0);
        }
        let bytes = build_attheme(&r, &alphas, None, None, false);
        let s = String::from_utf8(bytes).unwrap();
        for key in [
            "chat_inBubble",
            "chat_outBubble",
            "chat_messageTextIn",
            "actionBarDefault",
            "chats_unreadCounter",
            "chat_wallpaper",
        ] {
            assert!(s.contains(&format!("{key}=")), "missing {key}");
        }
    }
}
