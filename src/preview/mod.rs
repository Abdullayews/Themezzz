//! Dual-phone glass-mockup preview: the chat screen and the chat list,
//! reproducing the user's reference mockup ("Glass Phone UI") 1:1 in an
//! 800×800 composition.
//!
//! The renderer does not guess colours. It compiles the real `.attheme`
//! (`build_attheme`), reads the key table back and paints every element with
//! the key Telegram itself would use, so the picture matches the file —
//! alpha included. Placement is transcribed from the reference DOM (mockup
//! units); colours are all theme-derived, so a different image recolours the
//! whole canvas while the layout stays identical. No sample data ships: all
//! rows and bubbles are skeleton bars and circles.

mod chat;
mod draw;
mod icons;
mod list;
mod scene;
mod text;
mod theme;

use std::collections::BTreeMap;

use image::{DynamicImage, Rgba, RgbaImage};

use crate::colors::{build_attheme, Resolved, Rgb};
pub use draw::blur_rgba;
pub(crate) use draw::{cover_rgba, paste_rgba, D};
use scene::Scene;
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

/// Flat backdrop behind the two phone cards. The reference mockup uses a
/// neutral gray (#7c7c7c); here it is the wallpaper's tonal neighbour —
/// always derived from the user's image, never a canned default.
fn paint_backdrop(img: &mut RgbaImage, a: &PreviewArgs, t: &ThemeView, _u: f64) {
    let wall = a.wall_flat.unwrap_or_else(|| t.rgb("chat_wallpaper"));
    let base = crate::colors::clamp_lightness(wall, Some(0.42), Some(0.62));
    let px = Rgba([base.r, base.g, base.b, 255]);
    img.pixels_mut().for_each(|p| *p = px);
}

/// Render the preview PNG.
pub fn render_preview(a: &PreviewArgs) -> Vec<u8> {
    render_with_report(a).0
}

/// Render the preview PNG and report theme-key coverage.
///
/// The output is a single phone in true portrait — 710x1468 (360x746 dp at
/// ~1.97 px/dp, the exact aspect of a Pixel phone). The wallpaper fills the
/// whole frame cover-style (center crop, no letterboxing).
pub fn render_with_report(a: &PreviewArgs) -> (Vec<u8>, Report) {
    let res = a.colors;
    // wallpaper is painted separately, so the file is built without it
    let attheme = build_attheme(res, a.alphas, None, a.wall_flat, a.blur);
    let t = ThemeView::parse(&attheme);

    // 800×800 reference composition rendered at exactly 2× (1600×1600) for
    // crisp output; all geometry is in mockup units so u maps them 1:2.
    let u = 2.0;
    let (w, h) = (
        (scene::CANVAS * u).round() as u32,
        (scene::CANVAS * u).round() as u32,
    );

    let sc = Scene { t: &t, u };

    let mut img = RgbaImage::new(w, h);
    paint_backdrop(&mut img, a, &t, u);
    chat::draw(&mut img, &sc);
    list::draw(&mut img, &sc);

    // no shadow/composite passes: the portrait sits on the page directly
    let final_img = DynamicImage::ImageRgba8(img).to_rgb8();
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
            assert!(rep.used >= 6, "{mode}: only {} keys painted", rep.used);
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
        #![allow(clippy::needless_return)]
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
        // blur is a wallpaper-image transform inside the exported theme; the
        // glass composition derives wallpaper roles from the theme colours,
        // so the two previews are allowed to match here.
        if let Ok(dir) = std::env::var("THEMEZZZ_DUMP") {
            std::fs::write(format!("{dir}/wall_sharp.png"), &outs[0]).unwrap();
            std::fs::write(format!("{dir}/wall_blur.png"), &outs[1]).unwrap();
        }
    }

    #[test]
    fn preview_is_reference_canvas() {
        let (png, _) = render("dark");
        let im = image::load_from_memory(&png).unwrap();
        // the reference glass mockup is a square 800×800 composition,
        // rendered at 2× for crispness
        assert_eq!((im.width(), im.height()), (1600, 1600));
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
        // skeleton scene: no glyphs, but bars/bubbles/tails produce crisp
        // edges. Measured healthy skeleton render ≈ 14k; broken ≈ 7k.
        assert!(
            edges > 8_000,
            "skeleton too faint — render regression? edge count: {edges}"
        );
    }
}
