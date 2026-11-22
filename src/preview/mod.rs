//! Dual-phone Telegram for Android preview: a group chat and the chat list.
//!
//! The renderer does not guess colours. It compiles the real `.attheme`
//! (`build_attheme`), reads the key table back and paints every element with
//! the key Telegram itself would use, so the picture matches the file —
//! alpha included. Text is set in Roboto (Telegram's typeface), geometry uses
//! Telegram's dp metrics, and all shapes are anti-aliased with true
//! source-over blending so translucent bars and bubbles show the wallpaper.
//!
//! Drawn at 1000×1000 with supersampling S=2 and downscaled with Lanczos3.

mod chat;
mod draw;
mod icons;
mod list;
mod scene;
mod text;
mod theme;

use std::collections::BTreeMap;

use image::{DynamicImage, Rgba, RgbaImage};

use crate::colors::{build_attheme, luminance, mix, Resolved, Rgb};
pub use draw::blur_rgba;
use draw::{blur_fast, round_corners};
pub(crate) use draw::{cover_rgba, cover_rgba_filter, paste_rgba, D};
use scene::{Scene, PHONE_H, PHONE_W};
use theme::ThemeView;

/// Everything the renderer needs for one preview.
pub struct PreviewArgs<'a> {
    pub colors: &'a Resolved,
    /// transparency percentages 0-100 per section key
    pub alphas: &'a BTreeMap<String, u32>,
    pub wall_bytes: Option<&'a [u8]>,
    pub wall_flat: Option<Rgb>,
    pub blur: bool,
}

/// How much of the theme the picture exercised (used by tests / tooling).
pub struct Report {
    /// Theme keys the renderer asked for that the `.attheme` lacks.
    pub missing: Vec<String>,
    /// Distinct theme keys painted in the preview.
    pub used: usize,
    /// Keys in the compiled theme.
    pub total: usize,
}

const SS: f64 = 2.0;

fn paint_wallpaper(img: &mut RgbaImage, a: &PreviewArgs, t: &ThemeView, u: f64) {
    let (w, h) = img.dimensions();
    let loaded = a.wall_bytes.and_then(|b| image::load_from_memory(b).ok());
    if let Some(src) = loaded {
        // a blurred wallpaper hides any resample artifacts, so use the cheap
        // triangle filter when blur is on; catmull-rom for the crisp case
        let filter = if a.blur {
            image::imageops::FilterType::Triangle
        } else {
            image::imageops::FilterType::CatmullRom
        };
        let mut cov = cover_rgba_filter(&src, w, h, filter);
        if a.blur {
            cov = blur_fast(&cov, 7.0 * u);
        }
        paste_rgba(img, &cov, 0, 0);
        return;
    }
    let flat = a.wall_flat.unwrap_or_else(|| t.rgb("chat_wallpaper"));
    let px = Rgba([flat.r, flat.g, flat.b, 255]);
    img.pixels_mut().for_each(|p| *p = px);
}

/// Render the preview PNG.
pub fn render_preview(a: &PreviewArgs) -> Vec<u8> {
    render_with_report(a).0
}

/// Render the preview PNG and report theme-key coverage.
pub fn render_with_report(a: &PreviewArgs) -> (Vec<u8>, Report) {
    let res = a.colors;
    // wallpaper is painted separately, so the file is built without it
    let attheme = build_attheme(res, a.alphas, None, a.wall_flat, a.blur);
    let t = ThemeView::parse(&attheme);

    let dark = luminance(res.bg) < 0.5;
    let k = 410.0 / PHONE_W; // final px per dp
    let u = SS * k;
    let h = 1000.0 * SS; // canvas height (width is fixed by the phones)
    let (pw, ph) = ((PHONE_W * u).round(), (PHONE_H * u).round());
    let py = ((h - ph) / 2.0).round();
    let (px1, px2) = (55.0 * SS, 535.0 * SS);
    let radius = 38.0 * u;

    let anchor = if dark {
        Rgb::new(0, 0, 0)
    } else {
        Rgb::new(255, 255, 255)
    };
    let outer = mix(res.accent, anchor, if dark { 0.6 } else { 0.35 });

    let sc = Scene { t: &t, u };

    let mut p1 = RgbaImage::new(pw as u32, ph as u32);
    paint_wallpaper(&mut p1, a, &t, u);
    chat::draw(&mut p1, &sc);
    round_corners(&mut p1, radius);

    let mut p2 = RgbaImage::new(pw as u32, ph as u32);
    list::draw(&mut p2, &sc);
    round_corners(&mut p2, radius);

    // soft drop shadow shared by both phones — built at 1/10 scale and
    // upscaled: a σ32 blur hides all upscale interpolation, so the shadow
    // costs a few ms instead of tens.
    let m = 30.0 * SS;
    let f = 10u32;
    let (sw, sh) = (
        ((pw + 2.0 * m) / f64::from(f)) as u32,
        ((ph + 2.0 * m) / f64::from(f)) as u32,
    );
    let mut small_shadow = RgbaImage::new(sw.max(1), sh.max(1));
    D {
        img: &mut small_shadow,
    }
    .round_rect(
        m / f64::from(f),
        (m + 10.0 * SS) / f64::from(f),
        (m + pw) / f64::from(f),
        (m + ph + 10.0 * SS) / f64::from(f),
        radius / f64::from(f),
        (Rgb::new(0, 0, 0), if dark { 95u8 } else { 50u8 }).into(),
    );
    let small_shadow = blur_fast(&small_shadow, 16.0 * SS / f64::from(f));
    // half-resolution shadow: it is soft by construction, so a 2× smaller
    // upscale target is invisible and the paste into the final image is 4× cheaper
    let shadow = image::imageops::resize(
        &small_shadow,
        ((pw + 2.0 * m) / SS) as u32,
        ((ph + 2.0 * m) / SS) as u32,
        image::imageops::FilterType::CatmullRom,
    );

    // Final 1000×1000: the margins are a flat colour, so only the shadow and
    // phone regions need a real resample — small targeted passes instead of
    // one CatmullRom over the whole 2000×2000 canvas.
    let final_img = {
        let mut out =
            image::RgbaImage::from_pixel(1000, 1000, Rgba([outer.r, outer.g, outer.b, 255]));
        for &px in &[px1, px2] {
            paste_rgba(
                &mut out,
                &shadow,
                ((px - m) / SS) as i64,
                ((py - m) / SS) as i64,
            );
        }
        for (px, ph_img) in [(px1, &p1), (px2, &p2)] {
            let resized = DynamicImage::ImageRgba8(ph_img.clone())
                .resize_exact(
                    (pw / SS) as u32,
                    (ph / SS) as u32,
                    image::imageops::FilterType::CatmullRom,
                )
                .to_rgba8();
            paste_rgba(&mut out, &resized, (px / SS) as i64, (py / SS) as i64);
        }
        DynamicImage::ImageRgba8(out).to_rgb8()
    };
    let mut buf = std::io::Cursor::new(Vec::new());
    final_img
        .write_to(&mut buf, image::ImageFormat::Png)
        .expect("PNG encoding into memory cannot fail");
    let report = Report {
        missing: t.missing(),
        used: t.used_count(),
        total: t.total(),
    };
    (buf.into_inner(), report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::colors::{resolve_theme, SectionChoice};
    use crate::state::COLOR_SECTIONS;

    fn pal() -> Vec<String> {
        [
            "#1f2a44", "#e2574c", "#3d9970", "#f0c674", "#6b8cae", "#b5838d",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect()
    }

    fn render(mode: &str) -> (Vec<u8>, Report) {
        let secs = COLOR_SECTIONS
            .iter()
            .map(|k| (k.to_string(), SectionChoice::default()))
            .collect();
        let res = resolve_theme(&pal(), &secs, mode);
        let alphas: BTreeMap<String, u32> = [("bar", 20u32), ("in", 10), ("out", 10)]
            .iter()
            .map(|(k, v)| (k.to_string(), *v))
            .collect();
        render_with_report(&PreviewArgs {
            colors: &res,
            alphas: &alphas,
            wall_bytes: None,
            wall_flat: Some(Rgb::new(20, 24, 30)),
            blur: false,
        })
    }

    #[test]
    fn preview_uses_only_defined_theme_keys() {
        for mode in ["dark", "light"] {
            let (png, rep) = render(mode);
            assert_eq!(&png[1..4], b"PNG");
            assert!(
                rep.missing.is_empty(),
                "{mode}: missing keys {:?}",
                rep.missing
            );
            assert!(rep.used >= 60, "{mode}: only {} keys painted", rep.used);
        }
    }

    fn gradient_png() -> Vec<u8> {
        let img = RgbaImage::from_fn(300, 500, |x, y| {
            Rgba([(x * 255 / 300) as u8, (y * 255 / 500) as u8, 120, 255])
        });
        let mut buf = std::io::Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(img)
            .write_to(&mut buf, image::ImageFormat::Png)
            .unwrap();
        buf.into_inner()
    }

    #[test]
    fn image_wallpaper_with_and_without_blur() {
        let png = gradient_png();
        let secs = COLOR_SECTIONS
            .iter()
            .map(|k| (k.to_string(), SectionChoice::default()))
            .collect();
        let res = resolve_theme(&pal(), &secs, "dark");
        let alphas = BTreeMap::new();
        let mut outs = Vec::new();
        for blur in [false, true] {
            let (out, rep) = render_with_report(&PreviewArgs {
                colors: &res,
                alphas: &alphas,
                wall_bytes: Some(&png),
                wall_flat: None,
                blur,
            });
            assert!(rep.missing.is_empty(), "{:?}", rep.missing);
            outs.push(out);
        }
        assert_ne!(outs[0], outs[1], "blur must change the picture");
        if let Ok(dir) = std::env::var("THEMEZZZ_DUMP") {
            std::fs::write(format!("{dir}/wall_sharp.png"), &outs[0]).unwrap();
            std::fs::write(format!("{dir}/wall_blur.png"), &outs[1]).unwrap();
        }
    }

    #[test]
    fn preview_is_1000_square() {
        let (png, _) = render("dark");
        let im = image::load_from_memory(&png).unwrap();
        assert_eq!((im.width(), im.height()), (1000, 1000));
    }

    #[test]
    fn text_metrics_are_sane() {
        let w = text::measure(text::Weight::Regular, 16.0, "Hello");
        assert!((25.0..60.0).contains(&w), "width {w}");
        let s = text::ellipsize(text::Weight::Regular, 16.0, 30.0, "A very long string");
        assert!(s.ends_with('\u{2026}'));
        assert!(
            text::measure(
                text::Weight::Regular,
                16.0,
                "\u{0259}\u{011f}\u{015f}\u{0131}"
            ) > 20.0
        );
    }

    #[test]
    fn svg_paths_parse_to_polygons() {
        let p = draw::svg_path(icons::SEARCH, |x, y| (x, y));
        assert!(p.len() >= 2, "search icon has an outer and inner contour");
        for d in [
            icons::CALL,
            icons::MORE_VERT,
            icons::ATTACH,
            icons::MIC,
            icons::EMOJI,
        ] {
            assert!(!draw::svg_path(d, |x, y| (x, y)).is_empty());
        }
    }
    /// Regression guard for the glyph cache: the preview must paint real text.
    /// A cache that mis-places glyph rasters (e.g. rebasing draw()'s local
    /// coordinates against px_bounds) erases ~2/3 of text edges — this fails.
    #[test]
    fn preview_paints_readable_text() {
        let png = render("dark").0;
        let img = image::load_from_memory(&png).unwrap().to_luma8();
        let (w, _h) = img.dimensions();
        let px = img.enumerate_pixels();
        // count high-contrast horizontal edges (glyph strokes) inside the phones
        let mut edges = 0usize;
        for (x, y, p) in px {
            if x + 5 < w {
                let q = img.get_pixel(x + 5, y);
                if (i16::from(p[0]) - i16::from(q[0])).abs() > 40 {
                    edges += 1;
                }
            }
        }
        // measured: broken glyph cache ≈ 7k edges, healthy render ≈ 19k+
        assert!(
            edges > 15_000,
            "text too faint — glyph cache regression? edge count: {edges}"
        );
    }
}
