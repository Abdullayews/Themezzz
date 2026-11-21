//! Dual-phone Telegram UI renderer: chat screen + chat list.
//!
//! Ported from the Python `preview.py` (PIL) to the `image` crate.
//! Drawn at 1000×1000 with supersampling S=2 and downscaled with
//! Lanczos3, exactly like the original.

use crate::colors::{ensure_contrast, luminance, mix, readable_on, Rgb, Resolved};
use image::{DynamicImage, GenericImageView, Rgba, RgbaImage};
use std::collections::BTreeMap;

/// Everything the renderer needs for one preview.
pub struct PreviewArgs<'a> {
    pub colors: &'a Resolved,
    /// transparency percentages 0-100 per section key
    pub alphas: &'a BTreeMap<String, u32>,
    pub wall_bytes: Option<&'a [u8]>,
    pub wall_flat: Option<Rgb>,
    pub blur: bool,
}

fn a_of(alphas: &BTreeMap<String, u32>, k: &str) -> u8 {
    let pct = alphas.get(k).copied().unwrap_or(0).min(100);
    (255.0 * (1.0 - f64::from(pct) / 100.0)).round() as u8
}

#[derive(Clone, Copy)]
struct C(Rgba<u8>);

impl From<(Rgb, u8)> for C {
    fn from((c, a): (Rgb, u8)) -> Self {
        C(Rgba([c.r, c.g, c.b, a]))
    }
}

// ---------- Vector drawing helpers (all sizes S-scaled) ----------

/// Overwrite mode draw: like PIL fill (source-over without blending).
struct D<'a> { img: &'a mut RgbaImage }

impl<'a> D<'a> {
    fn px(&mut self, x: i64, y: i64, col: C) {
        if x < 0 || y < 0 { return; }
        if let Some(p) = self.img.get_pixel_mut_checked(x as u32, y as u32) { *p = col.0; }
    }

    /// Filled rectangle in integer coordinates (PIL rectangle).
    fn rect(&mut self, x0: i64, y0: i64, x1: i64, y1: i64, col: C) {
        if x1 < x0 || y1 < y0 { return; }
        for y in y0..=y1 { for x in x0..=x1 { self.px(x, y, col); } }
    }

    /// Filled rectangle with float coords → inclusive int box (PIL semantics).
    fn rectf(&mut self, x0: f64, y0: f64, x1: f64, y1: f64, col: C) {
        self.rect(x0.ceil() as i64, y0.ceil() as i64, x1.floor() as i64, y1.floor() as i64, col);
    }

    /// Filled ellipse from a bounding box (PIL ellipse).
    fn ellipse(&mut self, x0: f64, y0: f64, x1: f64, y1: f64, col: C) {
        if x1 <= x0 || y1 <= y0 { return; }
        let (cx, cy) = ((x0 + x1) / 2.0, (y0 + y1) / 2.0);
        let (rx, ry) = ((x1 - x0) / 2.0, (y1 - y0) / 2.0);
        let rx2 = rx * rx;
        let ry2 = ry * ry;
        for y in y0.floor() as i64..y1.ceil() as i64 {
            for x in x0.floor() as i64..x1.ceil() as i64 {
                let (dx, dy) = ((x as f64 + 0.5) - cx, (y as f64 + 0.5) - cy);
                if (dx * dx) / rx2 + (dy * dy) / ry2 <= 1.0 { self.px(x, y, col); }
            }
        }
    }

    /// Thick line with rounded caps (approximates PIL line with width).
    fn line(&mut self, x0: f64, y0: f64, x1: f64, y1: f64, w: f64, col: C) {
        let r = (w / 2.0).max(0.5);
        for y in (y0 - r).floor() as i64..(y1 + r).ceil() as i64 + 1 {
            for x in (x0 - r).floor() as i64..(x1 + r).ceil() as i64 + 1 {
                // distance from point to segment
                let (px, py) = (x as f64 + 0.5, y as f64 + 0.5);
                let (vx, vy) = (x1 - x0, y1 - y0);
                let len2 = vx * vx + vy * vy;
                let t = if len2 <= f64::EPSILON { 0.0 } else {
                    (((px - x0) * vx + (py - y0) * vy) / len2).clamp(0.0, 1.0)
                };
                let (qx, qy) = (x0 + t * vx, y0 + t * vy);
                let d2 = (px - qx) * (px - qx) + (py - qy) * (py - qy);
                if d2 <= r * r { self.px(x, y, col); }
            }
        }
    }

    /// Filled circle with radius r at (cx, cy).
    fn circle(&mut self, cx: f64, cy: f64, r: f64, col: C) {
        for y in (cy - r).floor() as i64..(cy + r).ceil() as i64 {
            for x in (cx - r).floor() as i64..(cx + r).ceil() as i64 {
                let (dx, dy) = (x as f64 + 0.5 - cx, y as f64 + 0.5 - cy);
                if dx * dx + dy * dy <= r * r { self.px(x, y, col); }
            }
        }
    }

    /// Circle outline with stroke width (PIL ellipse outline).
    fn circle_outline(&mut self, cx: f64, cy: f64, r: f64, w: f64, col: C) {
        for y in (cy - r - w).floor() as i64..(cy + r + w).ceil() as i64 {
            for x in (cx - r - w).floor() as i64..(cx + r + w).ceil() as i64 {
                let (dx, dy) = (x as f64 + 0.5 - cx, y as f64 + 0.5 - cy);
                let d = (dx * dx + dy * dy).sqrt();
                if d >= r - w / 2.0 && d <= r + w / 2.0 { self.px(x, y, col); }
            }
        }
    }

    /// Filled rounded rectangle (PIL rounded_rectangle).
    fn round_rect(&mut self, x0: f64, y0: f64, x1: f64, y1: f64, radius: f64, col: C) {
        if x1 <= x0 || y1 <= y0 { return; }
        let radius = radius.min((x1 - x0) / 2.0).min((y1 - y0) / 2.0).max(0.0);
        // straight edges
        self.rectf(x0, y0 + radius, x1, y1 - radius, col);
        self.rectf(x0 + radius, y0, x1 - radius, y0 + radius, col);
        self.rectf(x0 + radius, y1 - radius, x1 - radius, y1, col);
        // corners
        self.corner(x1 - radius, y0 + radius, radius, col);
        self.corner(x0 + radius, y0 + radius, radius, col);
        self.corner(x0 + radius, y1 - radius, radius, col);
        self.corner(x1 - radius, y1 - radius, radius, col);
    }

    /// Quarter-circle fill for rounded corners (dx≥0 half).
    fn corner(&mut self, cx: f64, cy: f64, r: f64, col: C) {
        for y in (cy - r).floor() as i64..(cy + r).ceil() as i64 {
            for x in (cx - r).floor() as i64..(cx + r).ceil() as i64 {
                let (dx, dy) = (x as f64 + 0.5 - cx, y as f64 + 0.5 - cy);
                if dx * dx + dy * dy <= r * r && dx >= -0.5 && dy >= -0.5 { self.px(x, y, col); }
            }
        }
    }

    /// Polygon fill via point-in-polygon test (PIL polygon).
    fn polygon(&mut self, pts: &[(f64, f64)], col: C) {
        let minx = pts.iter().fold(f64::MAX, |a, p| a.min(p.0)).floor() as i64;
        let maxx = pts.iter().fold(f64::MIN, |a, p| a.max(p.0)).ceil() as i64;
        let miny = pts.iter().fold(f64::MAX, |a, p| a.min(p.1)).floor() as i64;
        let maxy = pts.iter().fold(f64::MIN, |a, p| a.max(p.1)).ceil() as i64;
        for y in miny..=maxy {
            for x in minx..=maxx {
                // ray casting
                let (px, py) = (x as f64 + 0.5, y as f64 + 0.5);
                let mut inside = false;
                let mut j = pts.len() - 1;
                for i in 0..pts.len() {
                    let (xi, yi) = pts[i];
                    let (xj, yj) = pts[j];
                    if (yi > py) != (yj > py) &&
                        px < (xj - xi) * (py - yi) / (yj - yi) + xi { inside = !inside; }
                    j = i;
                }
                if inside { self.px(x, y, col); }
            }
        }
    }

    /// Thin horizontal line (PIL line with width=1 semantics).
    fn hline(&mut self, x0: f64, x1: f64, y: f64, col: C) {
        let yy = y.round() as i64;
        for x in x0.ceil() as i64..=x1.floor() as i64 { self.px(x, yy, col); }
    }
}

/// Alpha-blend source-over composite (PIL paste with mask).
fn paste(img: &mut RgbaImage, src: &RgbaImage, dx: i64, dy: i64) {
    for (_sy, row) in src.enumerate_rows() {
        for (x, y, p) in row {
            let (tx, ty) = (dx + x as i64, dy + y as i64);
            if tx < 0 || ty < 0 { continue; }
            let Some(dst) = img.get_pixel_mut_checked(tx as u32, ty as u32) else { continue };
            let sa = f64::from(p[3]) / 255.0;
            for c in 0..3 {
                let s = f64::from(p[c]);
                let d = f64::from(dst[c]);
                let v = s * sa + d * (1.0 - sa);
                dst[c] = v.round().clamp(0.0, 255.0) as u8;
            }
            dst[3] = (sa * 255.0 + f64::from(dst[3]) * (1.0 - sa)).round().clamp(0.0, 255.0) as u8;
        }
    }
}

/// Center-crop image to fully cover w×h maintaining aspect ratio.
fn cover(img: &DynamicImage, w: u32, h: u32) -> RgbaImage {
    let (iw, ih) = img.dimensions();
    let scale = (f64::from(w) / f64::from(iw).max(1.0)).max(f64::from(h) / f64::from(ih).max(1.0));
    let (nw, nh) = ((f64::from(iw) * scale) as u32 + 1, (f64::from(ih) * scale) as u32 + 1);
    let resized = img.resize_exact(nw.max(1), nh.max(1), image::imageops::FilterType::Lanczos3);
    let rgba = resized.to_rgba8();
    let (x, y) = ((nw - w) / 2, (nh - h) / 2);
    image::imageops::crop_imm(&rgba, x.min(nw.saturating_sub(w)), y.min(nh.saturating_sub(h)), w.min(nw), h.min(nh)).to_image()
}

/// Gaussian blur on RGBA (std dev = radius).
fn gaussian_blur_rgba(img: &RgbaImage, radius: f64) -> RgbaImage {
    // separable convolution with dynamic kernel
    let (w, h) = img.dimensions();
    if w == 0 || h == 0 { return img.clone(); }
    let sigma = radius.max(0.1);
    let k_len = (sigma * 3.0).ceil() as usize * 2 + 1;
    let mid = k_len / 2;
    let mut kernel = vec![0f64; k_len];
    let mut sum = 0f64;
    for i in 0..k_len {
        let d = i as f64 - mid as f64;
        let v = (-0.5 * d * d / (sigma * sigma)).exp();
        kernel[i] = v;
        sum += v;
    }
    for k in kernel.iter_mut() { *k /= sum; }
    let sample = |im: &RgbaImage, x: i64, y: i64| -> [f64; 4] {
        let (cx, cy) = (x.clamp(0, w as i64 - 1), y.clamp(0, h as i64 - 1));
        let p = im.get_pixel(cx as u32, cy as u32);
        [f64::from(p[0]), f64::from(p[1]), f64::from(p[2]), f64::from(p[3])]
    };
    let mut tmp = RgbaImage::new(w, h);
    for y in 0..h as i64 {
        for x in 0..w as i64 {
            let mut acc = [0f64; 4];
            for (i, k) in kernel.iter().enumerate() {
                let s = sample(img, x + i as i64 - mid as i64, y);
                for c in 0..4 { acc[c] += s[c] * k; }
            }
            tmp.put_pixel(x as u32, y as u32, Rgba(acc.map(|v| v.round().clamp(0.0, 255.0) as u8)));
        }
    }
    let mut out = RgbaImage::new(w, h);
    for y in 0..h as i64 {
        for x in 0..w as i64 {
            let mut acc = [0f64; 4];
            for (i, k) in kernel.iter().enumerate() {
                let s = sample(&tmp, x, y + i as i64 - mid as i64);
                for c in 0..4 { acc[c] += s[c] * k; }
            }
            out.put_pixel(x as u32, y as u32, Rgba(acc.map(|v| v.round().clamp(0.0, 255.0) as u8)));
        }
    }
    out
}

/// Multiply alpha channel by a rounded-rect mask (PIL _round_corners).
fn round_corners(img: &mut RgbaImage, radius: f64) {
    let (w, h) = img.dimensions();
    let mut mask = RgbaImage::new(w, h);
    {
        let mut d = D { img: &mut mask };
        d.round_rect(0.0, 0.0, w as f64 - 0.5, h as f64 - 0.5, radius, C(Rgba([255, 255, 255, 255])));
    }
    for (x, y, p) in img.enumerate_pixels_mut() {
        p[3] = (f64::from(p[3]) * f64::from(mask.get_pixel(x, y)[3]) / 255.0).round() as u8;
    }
}

// ---------- Small icon helpers ----------

fn draw_status_bar(d: &mut D, pw: i64, text: Rgb, body: Rgb, s: f64) {
    let col = C(Rgba([mix(text, body, 0.35).r, mix(text, body, 0.35).g, mix(text, body, 0.35).b, 220]));
    let w = (1.5 * s).max(1.0);
    // Camera punch hole (hardware black — achromatic)
    d.ellipse(pw as f64 / 2.0 - 5.0 * s, 11.0 * s, pw as f64 / 2.0 + 5.0 * s, 21.0 * s,
              C(Rgba([0, 0, 0, 200])));
    // Time
    d.round_rect(24.0 * s, 13.0 * s, 64.0 * s, 21.0 * s, 3.0 * s, col);
    // Signal bars
    for i in 0..4 {
        let h = (3.0 + i as f64 * 2.5) * s;
        d.rectf(pw as f64 - 65.0 * s + i as f64 * 5.0 * s, 22.0 * s - h,
                pw as f64 - 62.0 * s + i as f64 * 5.0 * s, 22.0 * s, col);
    }
    // Wifi arc (upper arc of a circle outline)
    let (wx, wy, wr) = (pw as f64 - 36.0 * s, 18.0 * s, 6.0 * s);
    let w_arc = w.max(1.0);
    for y in (wy - wr).floor() as i64..(wy + wr).ceil() as i64 {
        for x in (wx - wr).floor() as i64..(wx + wr).ceil() as i64 {
            let (dx, dy) = (x as f64 + 0.5 - wx, y as f64 + 0.5 - wy);
            let dist = (dx * dx + dy * dy).sqrt();
            let ang = dy.atan2(dx).to_degrees();
            let ang = if ang < 0.0 { ang + 360.0 } else { ang };
            if dist >= wr - w_arc && dist <= wr && ang >= 210.0 && ang <= 330.0 { d.px(x, y, col); }
        }
    }
    // Battery
    let bw = (1.5 * s).max(1.0);
    d.round_rect(pw as f64 - 26.0 * s, 13.0 * s, pw as f64 - 10.0 * s, 23.0 * s, 2.0 * s, col);
    d.rectf(pw as f64 - 24.0 * s, 15.0 * s, pw as f64 - 14.0 * s, 21.0 * s, col);
    d.rectf(pw as f64 - 9.0 * s, 15.0 * s, pw as f64 - 8.0 * s, 21.0 * s, col);
    let _ = (bw, w);
}

fn draw_back_arrow(d: &mut D, x: f64, y: f64, size: f64, color: Rgb, a: u8) {
    let col = C(Rgba([color.r, color.g, color.b, a]));
    let w = (size * 0.22).max(2.0);
    d.line(x + size, y, x, y + size / 2.0, w, col);
    d.line(x, y + size / 2.0, x + size, y + size, w, col);
}

fn draw_search_icon(d: &mut D, x: f64, y: f64, r: f64, color: Rgb, a: u8) {
    let col = C(Rgba([color.r, color.g, color.b, a]));
    let w = (r * 0.18).max(2.0);
    d.circle_outline(x + r / 2.0, y + r / 2.0, r / 2.0, w, col);
    d.line(x + r * 0.75, y + r * 0.75, x + r * 1.3, y + r * 1.3, w, col);
}

fn draw_more_dots(d: &mut D, x: f64, y: f64, dot: f64, gap: f64, color: Rgb, a: u8) {
    let col = C(Rgba([color.r, color.g, color.b, a]));
    for i in 0..3 {
        let yy = y + i as f64 * gap;
        d.circle(x + dot / 2.0, yy + dot / 2.0, dot / 2.0, col);
    }
}

fn draw_hamburger(d: &mut D, x: f64, y: f64, w_total: f64, color: Rgb, a: u8) {
    let col = C(Rgba([color.r, color.g, color.b, a]));
    let t = (w_total * 0.12).max(2.0);
    for i in 0..3 {
        let yy = y + i as f64 * (t * 2.5).round();
        d.round_rect(x, yy, x + w_total, yy + t, t / 2.0, col);
    }
}

fn draw_checkmarks(d: &mut D, x: f64, y: f64, s: f64, color: Rgb, a: u8) {
    let col = C(Rgba([color.r, color.g, color.b, a]));
    let w = (s * 0.18).max(2.0);
    d.line(x, y + s * 0.5, x + s * 0.35, y + s * 0.85, w, col);
    d.line(x + s * 0.35, y + s * 0.85, x + s * 0.9, y, w, col);
    d.line(x + s * 0.45, y + s * 0.5, x + s * 0.8, y + s * 0.85, w, col);
    d.line(x + s * 0.8, y + s * 0.85, x + s * 1.35, y, w, col);
}

fn draw_clip_icon(d: &mut D, x: f64, y: f64, s: f64, color: Rgb, a: u8) {
    let col = C(Rgba([color.r, color.g, color.b, a]));
    let w = (s * 0.18).max(2.0);
    let r = s * 0.4;
    // top hook (upper half circle)
    d.arc(x, y, x + 2.0 * r, y + 2.0 * r, 180.0, 360.0, w, col);
    d.line(x, y + r, x, y + s, w, col);
    d.line(x + 2.0 * r, y + r, x + 2.0 * r, y + s * 0.85, w, col);
    d.arc(x, y + s * 0.5, x + 2.0 * r, y + s * 1.3, 0.0, 180.0, w, col);
}

fn draw_pencil_icon(d: &mut D, cx: f64, cy: f64, size: f64, color: Rgb, a: u8) {
    let col = C(Rgba([color.r, color.g, color.b, a]));
    let w = (size * 0.2).max(2.0);
    d.line(cx - size / 2.0, cy + size / 2.0, cx + size / 2.0, cy - size / 2.0, w, col);
    // nib tip (small triangle)
    let k = size / 2.0;
    d.polygon(&[(cx + k - 1.0, cy - k - 3.0), (cx + k + 3.0, cy - k + 1.0), (cx + k + 2.0, cy - k - 2.0)], col);
}

// ---------- Renderer ----------

pub fn render_preview(a: &PreviewArgs) -> Vec<u8> {
    let res = a.colors;
    let (bg, bar) = (res.bg, res.bar);
    let (inb, outb) = (res.r#in, res.out);
    let (text, accent) = (res.text, res.accent);
    let reply = res.reply;
    let err = res.error;

    let (a_bar, a_in, a_out) = (a_of(a.alphas, "bar"), a_of(a.alphas, "in"), a_of(a.alphas, "out"));
    let (a_text, a_acc, a_reply) = (a_of(a.alphas, "text"), a_of(a.alphas, "accent"), a_of(a.alphas, "reply"));

    let dark = luminance(bg) < 0.5;
    let s = 2.0; // supersampling
    let (w, h) = (1000.0 * s, 1000.0 * s);

    // Backdrop — accent mixed toward black/white (achromatic anchors only)
    let anchor = if dark { Rgb::new(0, 0, 0) } else { Rgb::new(255, 255, 255) };
    let outer = mix(accent, anchor, if dark { 0.6 } else { 0.35 });
    let mut canvas = RgbaImage::from_pixel(w as u32, h as u32, Rgba([outer.r, outer.g, outer.b, 255]));

    let (pw, ph) = (410.0 * s, 850.0 * s);
    let py = (h - ph) / 2.0;
    let (px1, px2) = (55.0 * s, 535.0 * s);
    let p_radius = 44.0 * s;

    let phone_body_1 = mix(bg, Rgb::new(0, 0, 0), if dark { 0.35 } else { 0.08 });
    let phone_body_2 = mix(bg, Rgb::new(0, 0, 0), if dark { 0.30 } else { 0.10 });

    // ================= LEFT PHONE: CHAT VIEW =================
    let mut p1 = RgbaImage::new(pw as u32, ph as u32);
    {
        // Chat wallpaper — ORIGINAL image; blurred only if user chose so.
        // Pasted BEFORE the drawer exists (flat fill can go through `d`).
        let chat_h = ph - 116.0 * s;
        let chat_y = 60.0 * s;
        let wimg: Option<RgbaImage> = a.wall_bytes.and_then(|b| image::load_from_memory(b).ok()).map(|src| {
            let src = if a.blur { DynamicImage::ImageRgba8(gaussian_blur_rgba(&src.to_rgba8(), 12.0 * s)) } else { src };
            cover(&src, pw as u32, chat_h as u32)
        });
        match wimg {
            Some(wimg) => paste(&mut p1, &wimg, 0, chat_y as i64),
            None => {
                let wf = a.wall_flat.unwrap_or_else(|| mix(bg, Rgb::new(0, 0, 0), if dark { 0.15 } else { 0.03 }));
                for y in chat_y as i64..(chat_y + chat_h) as i64 {
                    for x in 0..pw as i64 {
                        if let Some(p) = p1.get_pixel_mut_checked(x as u32, y as u32) {
                            *p = Rgba([wf.r, wf.g, wf.b, 255]);
                        }
                    }
                }
            }
        }
    }
    {
        let mut d = D { img: &mut p1 };
        d.round_rect(0.0, 0.0, pw, ph, p_radius, (phone_body_1, 255u8).into());

        // Status bar
        draw_status_bar(&mut d, pw as i64, text, phone_body_1, s);
        let bar_col = C(Rgba([ensure_contrast(text, bar, 3.0).r, ensure_contrast(text, bar, 3.0).g, ensure_contrast(text, bar, 3.0).b, a_text]));
        d.rectf(0.0, 30.0 * s, pw, 86.0 * s, (bar, a_bar).into());
        draw_back_arrow(&mut d, 16.0 * s, 51.0 * s, 14.0 * s, ensure_contrast(text, bar, 3.0), a_text);
        d.ellipse(46.0 * s, 44.0 * s, 82.0 * s, 80.0 * s, (accent, a_acc).into());
        d.round_rect(94.0 * s, 49.0 * s, 210.0 * s, 59.0 * s, 4.0 * s, bar_col);
        d.round_rect(94.0 * s, 65.0 * s, 150.0 * s, 72.0 * s, 3.0 * s,
                     (mix(accent, bar, 0.3), a_acc).into());
        draw_search_icon(&mut d, pw - 68.0 * s, 51.0 * s, 12.0 * s, ensure_contrast(text, bar, 3.0), a_text);
        draw_more_dots(&mut d, pw - 30.0 * s, 50.0 * s, 4.0 * s, 7.0 * s, ensure_contrast(text, bar, 3.0), a_text);

        // Date pill
        let pill_bg = mix(bg, Rgb::new(0, 0, 0), if dark { 0.40 } else { 0.12 });
        d.round_rect(pw / 2.0 - 40.0 * s, 98.0 * s, pw / 2.0 + 40.0 * s, 116.0 * s, 9.0 * s,
                     (pill_bg, 160u8).into());
        d.round_rect(pw / 2.0 - 25.0 * s, 104.0 * s, pw / 2.0 + 25.0 * s, 110.0 * s, 3.0 * s,
                     (readable_on(pill_bg), 200u8).into());

        // 1. Incoming message with reply block
        d.round_rect(16.0 * s, 130.0 * s, 250.0 * s, 185.0 * s, 14.0 * s, (inb, a_in).into());
        d.round_rect(26.0 * s, 138.0 * s, 240.0 * s, 162.0 * s, 4.0 * s, (mix(reply, inb, 0.85), 40u8).into());
        d.rectf(26.0 * s, 138.0 * s, 29.0 * s, 162.0 * s, (reply, a_reply).into());
        d.round_rect(36.0 * s, 142.0 * s, 120.0 * s, 149.0 * s, 3.0 * s, (reply, a_reply).into());
        d.round_rect(36.0 * s, 152.0 * s, 210.0 * s, 158.0 * s, 3.0 * s, (mix(text, inb, 0.3), a_text).into());
        d.round_rect(26.0 * s, 168.0 * s, 190.0 * s, 176.0 * s, 4.0 * s,
                     (ensure_contrast(text, inb, 3.0), a_text).into());
        d.round_rect(205.0 * s, 172.0 * s, 240.0 * s, 178.0 * s, 3.0 * s, (mix(text, inb, 0.5), a_text).into());

        // 2. Outgoing text message
        d.round_rect(140.0 * s, 200.0 * s, 394.0 * s, 248.0 * s, 14.0 * s, (outb, a_out).into());
        d.round_rect(154.0 * s, 212.0 * s, 370.0 * s, 221.0 * s, 4.0 * s,
                     (ensure_contrast(text, outb, 3.0), a_text).into());
        d.round_rect(154.0 * s, 226.0 * s, 280.0 * s, 234.0 * s, 4.0 * s,
                     (ensure_contrast(text, outb, 3.0), a_text).into());
        draw_checkmarks(&mut d, 366.0 * s, 230.0 * s, 10.0 * s, ensure_contrast(text, outb, 3.0), a_text);

        // 3. Outgoing voice note
        d.round_rect(110.0 * s, 265.0 * s, 394.0 * s, 335.0 * s, 18.0 * s, (outb, a_out).into());
        d.ellipse(124.0 * s, 277.0 * s, 168.0 * s, 321.0 * s, (accent, a_acc).into());
        d.polygon(&[(143.0 * s, 292.0 * s), (143.0 * s, 306.0 * s), (154.0 * s, 299.0 * s)],
                  (readable_on(accent), 255u8).into());
        let waveform = [6.0, 14.0, 10.0, 22.0, 26.0, 16.0, 20.0, 12.0, 24.0, 18.0, 14.0, 8.0, 16.0, 10.0, 6.0];
        for (i, hv) in waveform.iter().enumerate() {
            let col2 = if i < 7 { ensure_contrast(text, outb, 3.0) } else { mix(text, outb, 0.5) };
            d.round_rect(180.0 * s + i as f64 * 13.0 * s, 296.0 * s - hv * s / 2.0,
                          180.0 * s + i as f64 * 13.0 * s + 5.0 * s, 296.0 * s + hv * s / 2.0,
                          2.0 * s, (col2, a_text).into());
        }
        d.round_rect(180.0 * s, 316.0 * s, 225.0 * s, 323.0 * s, 3.0 * s, (mix(text, outb, 0.4), a_text).into());
        draw_checkmarks(&mut d, 366.0 * s, 316.0 * s, 10.0 * s, ensure_contrast(text, outb, 3.0), a_text);

        // 4. Incoming message with avatar
        d.ellipse(14.0 * s, 355.0 * s, 46.0 * s, 387.0 * s, (mix(accent, err, 0.3), 230u8).into());
        d.round_rect(54.0 * s, 350.0 * s, 280.0 * s, 405.0 * s, 14.0 * s, (inb, a_in).into());
        d.round_rect(66.0 * s, 360.0 * s, 140.0 * s, 368.0 * s, 3.0 * s, (reply, a_reply).into());
        d.round_rect(66.0 * s, 374.0 * s, 250.0 * s, 383.0 * s, 4.0 * s,
                     (ensure_contrast(text, inb, 3.0), a_text).into());
        d.round_rect(66.0 * s, 388.0 * s, 180.0 * s, 395.0 * s, 3.0 * s, (mix(text, inb, 0.3), a_text).into());

        // 5. Outgoing short bubble
        d.round_rect(210.0 * s, 420.0 * s, 394.0 * s, 460.0 * s, 14.0 * s, (outb, a_out).into());
        d.round_rect(224.0 * s, 434.0 * s, 340.0 * s, 444.0 * s, 4.0 * s,
                     (ensure_contrast(text, outb, 3.0), a_text).into());
        draw_checkmarks(&mut d, 366.0 * s, 440.0 * s, 10.0 * s, ensure_contrast(text, outb, 3.0), a_text);

        // Bottom input bar (elevation is darker, never lighter)
        d.rectf(0.0, ph - 60.0 * s, pw, ph, (bg, 255u8).into());
        d.round_rect(12.0 * s, ph - 52.0 * s, pw - 62.0 * s, ph - 10.0 * s, 21.0 * s,
                     (mix(bg, Rgb::new(0, 0, 0), if dark { 0.25 } else { 0.07 }), 255u8).into());
        d.circle_outline(34.0 * s, ph - 32.0 * s, 10.0 * s, s as f64, (mix(text, bg, 0.4), a_text).into());
        d.round_rect(56.0 * s, ph - 36.0 * s, 180.0 * s, ph - 26.0 * s, 4.0 * s,
                     (mix(text, bg, 0.4), a_text).into());
        draw_clip_icon(&mut d, pw - 98.0 * s, ph - 46.0 * s, 20.0 * s, mix(text, bg, 0.4), a_text);
        d.ellipse(pw - 52.0 * s, ph - 52.0 * s, pw - 10.0 * s, ph - 10.0 * s, (accent, a_acc).into());
        d.ellipse(pw - 36.0 * s, ph - 38.0 * s, pw - 26.0 * s, ph - 20.0 * s,
                  (readable_on(accent), 255u8).into());
    }
    round_corners(&mut p1, p_radius);

    // ================= RIGHT PHONE: CHAT LIST =================
    let mut p2 = RgbaImage::new(pw as u32, ph as u32);
    {
        let mut d = D { img: &mut p2 };
        d.round_rect(0.0, 0.0, pw, ph, p_radius, (phone_body_2, 255u8).into());
        draw_status_bar(&mut d, pw as i64, text, phone_body_2, s);

        // Header bar
        let bt = ensure_contrast(text, bar, 3.0);
        d.rectf(0.0, 30.0 * s, pw, 86.0 * s, (bar, a_bar).into());
        draw_hamburger(&mut d, 18.0 * s, 51.0 * s, 18.0 * s, bt, a_text);
        d.round_rect(56.0 * s, 51.0 * s, 170.0 * s, 65.0 * s, 5.0 * s, (bt, a_text).into());
        draw_search_icon(&mut d, pw - 34.0 * s, 51.0 * s, 13.0 * s, bt, a_text);

        // Chat list rows
        let row_h = 76.0 * s;
        for i in 0..8u32 {
            let ry = 90.0 * s + f64::from(i) * row_h;
            let av_col = mix(accent, if dark { Rgb::new(255, 255, 255) } else { Rgb::new(0, 0, 0) },
                             f64::from(i % 4) * 0.18);
            d.ellipse(16.0 * s, ry + 10.0 * s, 68.0 * s, ry + 62.0 * s, (av_col, 240u8).into());

            let name_w = 70.0 * s + f64::from((i * 43) % 90) * s;
            let msg_w = 110.0 * s + f64::from((i * 61) % 130) * s;
            d.round_rect(82.0 * s, ry + 18.0 * s, 82.0 * s + name_w, ry + 29.0 * s, 5.0 * s,
                         (text, a_text).into());
            d.round_rect(82.0 * s, ry + 39.0 * s, 82.0 * s + msg_w, ry + 49.0 * s, 4.0 * s,
                         (mix(text, bg, 0.45), a_text).into());
            d.round_rect(pw - 58.0 * s, ry + 18.0 * s, pw - 18.0 * s, ry + 26.0 * s, 3.0 * s,
                         (mix(text, bg, 0.5), a_text).into());

            if i % 3 == 1 {
                // Unread badge — dark tone bg + white digits
                let counter_bg = mix(accent, Rgb::new(0, 0, 0), 0.42);
                d.ellipse(pw - 40.0 * s, ry + 36.0 * s, pw - 18.0 * s, ry + 58.0 * s,
                          (counter_bg, 255u8).into());
                let (bcx, bcy) = (pw - 29.0 * s, ry + 47.0 * s);
                d.round_rect(bcx - 4.0 * s, bcy - 5.0 * s, bcx + 6.0 * s, bcy + 3.0 * s, 2.0 * s,
                             (readable_on(counter_bg), 255u8).into());
            } else if i % 3 == 2 {
                draw_checkmarks(&mut d, pw - 40.0 * s, ry + 38.0 * s, 9.0 * s,
                                mix(accent, text, 0.4), a_acc);
            }
            d.hline(82.0 * s, pw, ry + row_h, (mix(text, bg, 0.15), 40u8).into());
        }

        // FAB (pencil)
        let fab_col = mix(accent, Rgb::new(0, 0, 0), if dark { 0.15 } else { 0.05 });
        d.ellipse(pw - 76.0 * s, ph - 90.0 * s, pw - 16.0 * s, ph - 30.0 * s, (fab_col, 245u8).into());
        draw_pencil_icon(&mut d, pw - 46.0 * s, ph - 60.0 * s, 14.0 * s, readable_on(fab_col), 255);
    }
    round_corners(&mut p2, p_radius);

    // ================= COMPOSITING & SHADOWS =================
    let mut shadow = RgbaImage::new(pw as u32 + (30.0 * s) as u32, ph as u32 + (30.0 * s) as u32);
    {
        let mut d = D { img: &mut shadow };
        d.round_rect(15.0 * s, 15.0 * s, pw + 15.0 * s, ph + 15.0 * s, p_radius,
                     C(Rgba([0, 0, 0, if dark { 95 } else { 50 }])));
    }
    let shadow_blur = gaussian_blur_rgba(&shadow, 16.0 * s);

    paste(&mut canvas, &shadow_blur, px1 as i64 - 15, py as i64 - 10);
    paste(&mut canvas, &p1, px1 as i64, py as i64);
    paste(&mut canvas, &shadow_blur, px2 as i64 - 15, py as i64 - 10);
    paste(&mut canvas, &p2, px2 as i64, py as i64);

    // Downscale supersampled canvas to target resolution
    let canvas_img = DynamicImage::ImageRgba8(canvas);
    let final_img = canvas_img.resize_exact(1000, 1000, image::imageops::FilterType::Lanczos3).to_rgb8();
    let mut buf = std::io::Cursor::new(Vec::new());
    final_img.write_to(&mut buf, image::ImageFormat::Png).unwrap();
    buf.into_inner()
}

// helper used by draw_status_bar's wifi arc & clip's arcs
impl<'a> D<'a> {
    fn arc(&mut self, x0: f64, y0: f64, x1: f64, y1: f64, start_deg: f64, end_deg: f64, w: f64, col: C) {
        let (cx, cy) = ((x0 + x1) / 2.0, (y0 + y1) / 2.0);
        let (rx, ry) = ((x1 - x0) / 2.0, (y1 - y0) / 2.0);
        for y in y0.floor() as i64..y1.ceil() as i64 {
            for x in x0.floor() as i64..x1.ceil() as i64 {
                let (dx, dy) = (x as f64 + 0.5 - cx, y as f64 + 0.5 - cy);
                let (nx, ny) = (dx / rx, dy / ry);
                let dist = (nx * nx + ny * ny).sqrt();
                let (mx, my) = (dx / dist.max(f64::EPSILON), dy / dist.max(f64::EPSILON));
                let d_perp = (my * rx).abs().hypot((mx * ry).abs());
                if dist >= 0.0 && d_perp >= rx - w && d_perp <= rx + w {
                    let ang = dy.atan2(dx).to_degrees();
                    let ang = if ang < 0.0 { ang + 360.0 } else { ang };
                    if start_deg <= end_deg {
                        if ang >= start_deg && ang <= end_deg { self.px(x, y, col); }
                    } else if ang >= start_deg || ang <= end_deg { self.px(x, y, col); }
                }
            }
        }
    }
}
