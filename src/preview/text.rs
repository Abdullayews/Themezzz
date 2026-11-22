//! Text rendering with the embedded Roboto subset (Telegram for Android's
//! typeface): kerning, anti-aliasing, ellipsis and word wrapping.

use std::sync::OnceLock;

use ab_glyph::{point, Font, FontRef, GlyphId, PxScale, ScaleFont};

use super::draw::{C, D};

/// Telegram uses Roboto Regular for body text and Roboto Medium for names,
/// titles and badges.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Weight {
    Regular,
    Medium,
}

fn font(w: Weight) -> &'static FontRef<'static> {
    static REG: OnceLock<FontRef<'static>> = OnceLock::new();
    static MED: OnceLock<FontRef<'static>> = OnceLock::new();
    match w {
        Weight::Regular => REG.get_or_init(|| {
            FontRef::try_from_slice(include_bytes!("../../assets/fonts/Roboto-Regular.ttf"))
                .expect("embedded Roboto-Regular.ttf is valid")
        }),
        Weight::Medium => MED.get_or_init(|| {
            FontRef::try_from_slice(include_bytes!("../../assets/fonts/Roboto-Medium.ttf"))
                .expect("embedded Roboto-Medium.ttf is valid")
        }),
    }
}

/// Android `sp` is the em size; ab_glyph's `PxScale` is the ascent+descent
/// height, so convert.
fn scale(f: &FontRef<'_>, em_px: f64) -> PxScale {
    let upem = f.units_per_em().unwrap_or(1000.0);
    PxScale::from(em_px as f32 * f.height_unscaled() / upem)
}

/// Advance width of `s` in pixels.
pub(crate) fn measure(w: Weight, em_px: f64, s: &str) -> f64 {
    let f = font(w);
    let sf = f.as_scaled(scale(f, em_px));
    let (mut x, mut prev): (f32, Option<GlyphId>) = (0.0, None);
    for ch in s.chars() {
        let id = sf.glyph_id(ch);
        if let Some(p) = prev {
            x += sf.kern(p, id);
        }
        x += sf.h_advance(id);
        prev = Some(id);
    }
    f64::from(x)
}

/// Shorten `s` with an ellipsis so it fits `max_w` pixels.
pub(crate) fn ellipsize(w: Weight, em_px: f64, max_w: f64, s: &str) -> String {
    if measure(w, em_px, s) <= max_w {
        return s.to_string();
    }
    let mut out: Vec<char> = s.chars().collect();
    while !out.is_empty() {
        out.pop();
        let cand: String = out.iter().collect::<String>().trim_end().to_string() + "\u{2026}";
        if measure(w, em_px, &cand) <= max_w {
            return cand;
        }
    }
    "\u{2026}".to_string()
}

impl D<'_> {
    /// Draw `s` with its baseline at `baseline`; returns the advance width.
    pub(crate) fn text(
        &mut self,
        w: Weight,
        em_px: f64,
        x: f64,
        baseline: f64,
        s: &str,
        col: C,
    ) -> f64 {
        let f = font(w);
        let sc = scale(f, em_px);
        let sf = f.as_scaled(sc);
        let (mut caret, mut prev): (f32, Option<GlyphId>) = (0.0, None);
        for ch in s.chars() {
            let id = sf.glyph_id(ch);
            if let Some(p) = prev {
                caret += sf.kern(p, id);
            }
            let glyph = id.with_scale_and_position(sc, point(x as f32 + caret, baseline as f32));
            if let Some(og) = f.outline_glyph(glyph) {
                let b = og.px_bounds();
                let (ox, oy) = (b.min.x.floor() as i64, b.min.y.floor() as i64);
                og.draw(|gx, gy, cov| {
                    self.blend(ox + i64::from(gx), oy + i64::from(gy), col, cov);
                });
            }
            caret += sf.h_advance(id);
            prev = Some(id);
        }
        f64::from(caret)
    }
}

/// One wrapped line: `(text, style, x offset)` segments plus total width.
pub(crate) struct Line {
    pub(crate) segs: Vec<(String, usize, f64)>,
    pub(crate) width: f64,
}

/// Greedy word wrap of styled runs (`style` is an index the caller maps to a
/// colour, e.g. normal text vs link).
pub(crate) fn wrap_runs(w: Weight, em_px: f64, max_w: f64, runs: &[(&str, usize)]) -> Vec<Line> {
    let mut lines: Vec<Line> = vec![Line {
        segs: Vec::new(),
        width: 0.0,
    }];
    for &(text, style) in runs {
        for token in text.split_inclusive(' ') {
            let trimmed = token.trim_end();
            let full = measure(w, em_px, token);
            let visible = measure(w, em_px, trimmed);
            let cur = lines.last_mut().expect("at least one line");
            if cur.width + visible > max_w && !cur.segs.is_empty() {
                lines.push(Line {
                    segs: Vec::new(),
                    width: 0.0,
                });
            }
            let cur = lines.last_mut().expect("at least one line");
            cur.segs.push((token.to_string(), style, cur.width));
            cur.width += full;
        }
    }
    for l in &mut lines {
        if let Some(last) = l.segs.last() {
            // trailing space does not count towards the visual width
            let trail = last.0.len() - last.0.trim_end().len();
            if trail > 0 {
                l.width -= measure(w, em_px, &last.0[last.0.len() - trail..]);
            }
        }
    }
    lines
}
