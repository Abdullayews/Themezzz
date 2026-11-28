//! Left phone of the glass-mockup preview — the CHAT screen, transcribed 1:1
//! from the reference HTML ("Glass Phone UI").
//!
//! Every coordinate below is a mockup unit from the reference DOM dump; only
//! the PLACEMENT is fixed. Every COLOUR is derived from the compiled
//! `.attheme` of the user's current choices, mapped onto the reference's
//! colour ROLES:
//!
//! | reference role | mockup value | theme source |
//! |---|---|---|
//! | card / panels | #c9cdd4 | chat_messagePanelBackground family |
//! | skeleton ink  | #5d7394 | actionBarDefaultIcon (title/strong bars) |
//! | skeleton gray | #8c8f96 | chat_inTimeText (secondary bars, pill) |
//! | bubble glass  | rgba(199,203,210,.82–.85) | chat_inBubble / chat_outBubble |
//! | wallpaper blobs | #466088 / #5d779e / #eef1f6 | wallpaper colour family |
//! | heart | navy radial | wallpaper hue, dark clamped |
//! | captions / sparkles | #fff | chat_messageTextIn |
//!
//! so a different image changes every colour on the canvas while the layout
//! stays pixel-identical to the reference.

use image::RgbaImage;

use super::draw::{cover_rgba, gaussian_blur_rgba, PathBuilder, Polys, C, D};
use super::scene::{
    card, dot, pill, Scene, CHAT_SCREEN_BOTTOM, CHAT_SCREEN_TOP, CHAT_X, CHAT_Y, PHONE_H, PHONE_R,
    PHONE_W,
};
use crate::colors::{clamp_lightness, luminance, mix, Rgb};

/* ====================== colour roles ====================== */

/// Colour roles used by the scene, all derived from the theme.
///
/// The reference mockup is a LIGHT glass composition. Its roles are a
/// lightness LADDER over one hue family; we rebuild that ladder on the
/// user's colours (via the compiled theme), so any image recolours every
/// role while the style stays glass:
///
///   card 0.80 / underlay 0.845 / ink 0.42 / gray 0.58 / heart 0.28
///
/// Dark mode flips the ladder (dark glass): card 0.22, glass darkens,
/// white accents stay light. Source hues: accent family for ink (the
/// reference's #5d7394 is exactly a desaturated accent), dominant/bg hue
/// for surfaces, wallpaper key for the blob family.
struct Roles {
    /// phone card + header + input panel base
    card: C,
    /// slightly lighter screen underlay (behind the blobs)
    underlay: C,
    /// strong skeleton ink (circles, title bars, waveform)
    ink: C,
    /// secondary skeleton ink (gray bars, pill)
    gray: C,
    /// white-ish accents: captions, sparkles, dots
    white: C,
    /// wallpaper blob colours, dark→light family
    blob_dark: Rgb,
    blob_mid: Rgb,
    blob_light: Rgb,
    /// heart body colour + its rim
    heart: Rgb,
    heart_rim: Rgb,
    /// true when the compiled theme is dark mode
    dark: bool,
}

/// Frosted-glass fill for bubbles on either side.
fn glass_col(sc: &Scene, r: &Roles, out: bool) -> C {
    let t = sc.t;
    let base = if out {
        t.rgb("chat_outBubble")
    } else {
        t.rgb("chat_inBubble")
    };
    let l = if r.dark {
        if out {
            0.34
        } else {
            0.30
        }
    } else if out {
        0.825
    } else {
        0.79
    };
    let col = relight(base, l);
    (col, if out { 217u8 } else { 207u8 }).into()
}

/// Hue/sat of `c` re-lightnessed to `l` (keeps the user's colour family).
fn relight(c: Rgb, l: f64) -> Rgb {
    let (h, _, s) = hls_of(c);
    hls_rgb(h, l, s)
}

fn roles(sc: &Scene) -> Roles {
    let t = sc.t;

    // Hue sources — user picks are respected because the theme is compiled
    // from them. The ACCENT carries the stable, vivid hue (near-black
    // surfaces have quantization-noisy hues, so they are never a hue source).
    let accent = t.rgb("chat_outBubble"); // out bubble = accent family
    let panel_src = t.rgb("chat_messagePanelBackground");
    let wall_l = luminance(t.rgb("chat_wallpaper"));
    let dark = wall_l < 0.35 || luminance(panel_src) < 0.35;

    // Reference lightness ladder, re-hued onto the user's accent. Chrome
    // (card/ink/gray/heart) flips in dark mode; the screen underlay stays
    // light because the wallpaper pane is light glass in the reference.
    let (card_l, ink_l, gray_l, heart_l) = if dark {
        (0.24, 0.72, 0.55, 0.34)
    } else {
        (0.80, 0.42, 0.58, 0.28)
    };
    let under_l = 0.845;
    // Saturation ladder measured from the reference family (#c9cdd4/#5d7394).
    let (card_s, ink_s, gray_s) = (0.16, 0.30, 0.10);
    let heart_s = 0.52;

    let hue = |c: Rgb| hls_of(c).0;
    let h_acc = hue(accent);

    let card = hls_rgb(h_acc, card_l, card_s);
    let underlay = hls_rgb(h_acc, under_l, card_s);
    let ink = hls_rgb(h_acc, ink_l, ink_s);
    let gray = hls_rgb(h_acc, gray_l, gray_s);

    // white accents: near-white in light mode, softly dim in dark
    let white = if dark {
        Rgb::new(235, 238, 244)
    } else {
        Rgb::new(255, 255, 255)
    };

    // blobs: accent hue at three ladder points (reference: dark/mid/light)
    // The reference screen is a LIGHT glass pane; keep the blob ladder light
    // in both modes (hue/saturation from the user's image) so the blobs stay
    // dramatic. Chrome may flip dark; the wallpaper does not.
    let (bd, bm, bl, b_s) = (0.42, 0.58, 0.90, 0.38);
    let blob_dark = hls_rgb(h_acc, bd, b_s);
    let blob_mid = hls_rgb(h_acc, bm, b_s);
    let blob_light = hls_rgb(h_acc, bl, (b_s * 0.4).min(0.18));

    // heart: deep vivid tone of the accent hue
    let heart = hls_rgb(h_acc, heart_l, heart_s);
    let heart_rim = hls_rgb(h_acc, (heart_l + 0.20).min(0.9), heart_s);

    Roles {
        card: (card, 255u8).into(),
        underlay: (underlay, 255u8).into(),
        ink: (ink, 255u8).into(),
        gray: (gray, 255u8).into(),
        white: (white, 255u8).into(),
        blob_dark,
        blob_mid,
        blob_light,
        heart,
        heart_rim,
        dark,
    }
}

fn hls_of(c: Rgb) -> (f64, f64, f64) {
    let (r, g, b) = (
        f64::from(c.r) / 255.0,
        f64::from(c.g) / 255.0,
        f64::from(c.b) / 255.0,
    );
    let mx = r.max(g).max(b);
    let mn = r.min(g).min(b);
    let l = (mx + mn) / 2.0;
    if (mx - mn).abs() < 1e-9 {
        return (0.0, l, 0.0);
    }
    let d = mx - mn;
    let s = if l > 0.5 {
        d / (2.0 - mx - mn)
    } else {
        d / (mx + mn)
    };
    let h = (if (mx - r).abs() < 1e-9 {
        (g - b) / d + if g < b { 6.0 } else { 0.0 }
    } else if (mx - g).abs() < 1e-9 {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    }) / 6.0;
    (h, l, s)
}

fn hls_rgb(h: f64, l: f64, s: f64) -> Rgb {
    if s <= 0.0 {
        let v = (l * 255.0).round() as u8;
        return Rgb::new(v, v, v);
    }
    let q = if l < 0.5 {
        l * (1.0 + s)
    } else {
        l + s - l * s
    };
    let p = 2.0 * l - q;
    let f = |mut t: f64| {
        if t < 0.0 {
            t += 1.0;
        }
        if t > 1.0 {
            t -= 1.0;
        }
        let v = if t < 1.0 / 6.0 {
            p + (q - p) * 6.0 * t
        } else if t < 0.5 {
            q
        } else if t < 2.0 / 3.0 {
            p + (q - p) * (2.0 / 3.0 - t) * 6.0
        } else {
            p
        };
        (v * 255.0).round().clamp(0.0, 255.0) as u8
    };
    Rgb::new(f(h + 1.0 / 3.0), f(h), f(h - 1.0 / 3.0))
}

/* ====================== wallpaper (blobs + heart) ====================== */

/// Paint the chat screen wallpaper: flat underlay, five blurred colour blobs
/// and the glossy heart — everything clipped to the screen rect.
fn wallpaper(d: &mut D, sc: &Scene, r: &Roles, a_wall: Option<&[u8]>) {
    let u = sc.u;
    // screen rect in canvas px
    let sx = CHAT_X * u;
    let sw = PHONE_W * u;
    let sy = CHAT_SCREEN_TOP * u;
    let sh = (CHAT_SCREEN_BOTTOM - CHAT_SCREEN_TOP) * u;

    // 1) underlay
    d.rectf(sx, sy, sx + sw, sy + sh, r.underlay);

    // 2) blobs on an OPAQUE layer (underlay base, no transparency anywhere —
    // transparent RGBA resizing produces color garbage), blurred, blitted.
    let (lw, lh) = (sw.ceil() as u32, sh.ceil() as u32);
    let mut layer = image::RgbaImage::from_fn(lw.max(1), lh.max(1), |_, _| {
        image::Rgba([r.underlay.0[0], r.underlay.0[1], r.underlay.0[2], 255])
    });
    {
        let mut ld = D { img: &mut layer };
        // blob boxes in canvas units (reference coordinates, some poke outside)
        let blobs: [(f64, f64, f64, f64, Rgb); 5] = [
            (180.0, 50.0, 280.0, 260.0, r.blob_dark),
            (195.0, 130.0, 190.0, 180.0, r.blob_mid),
            (70.0, 225.0, 230.0, 270.0, r.blob_light),
            (-5.0, 450.0, 300.0, 270.0, r.blob_dark),
            (95.0, 460.0, 200.0, 180.0, r.blob_mid),
        ];
        for (bx, by, bw, bh, col) in blobs {
            let x0 = (bx - CHAT_X) * u;
            let y0 = (by - CHAT_SCREEN_TOP) * u;
            let col: C = (col, 255u8).into();
            ld.ellipse(x0, y0, x0 + bw * u, y0 + bh * u, col);
        }
    }
    let blurred = gaussian_blur_rgba(&layer, 24.0 * u);
    // blit opaquely (every layer pixel is opaque)
    for (x, y, p) in blurred.enumerate_pixels() {
        d.img.put_pixel(sx as u32 + x, sy as u32 + y, *p);
    }

    // 3) the heart sits CRISP on top of the blobs: with a wallpaper image
    //    the user's picture fills the heart body (cover-fit, masked by the
    //    heart path); without one the theme-derived glass heart is used.
    {
        let hd = HeartCtx { d };
        draw_heart_direct(hd, sc, r, a_wall);
    }
}

/// Minimal context so the heart can draw straight onto the main canvas.
struct HeartCtx<'a, 'b> {
    d: &'a mut D<'b>,
}

/// The reference heart: 130×122 path scaled ×1.18. The body shows the USER'S
/// IMAGE when one is set (cover-cropped to the heart silhouette); otherwise
/// the theme-derived radial glass fill. The rim, drop shadow and gloss arc
/// are style and stay in both cases. Position (149.3, 287.7) canvas units.
fn draw_heart_direct(hc: HeartCtx, sc: &Scene, r: &Roles, wall: Option<&[u8]>) {
    let u = sc.u;
    let (hx, hy) = (149.30, 287.68);
    let s = 1.18; // reference transform scale
    let x0 = hx * u;
    let y0 = hy * u;

    let path = |k: f64| -> Polys {
        let mut p = PathBuilder::new();
        let m = |x: f64, y: f64| {
            (
                x0 + (x * k - 65.0 * (k - 1.0)) * s * u,
                y0 + (y * k - 61.0 * (k - 1.0)) * s * u,
            )
        };
        let (a, b) = m(65.0, 122.0);
        p.move_to(a, b);
        let (a, b) = m(50.0, 106.0);
        let (c, e) = m(0.0, 79.0);
        let (g, i) = m(0.0, 42.0);
        p.cubic_to((a, b), (c, e), (g, i));
        let (a, b) = m(0.0, 16.0);
        let (c, e) = m(23.0, 0.0);
        let (g, i) = m(45.0, 0.0);
        p.cubic_to((a, b), (c, e), (g, i));
        let (a, b) = m(59.0, 0.0);
        let (c, e) = m(65.0, 10.0);
        let (g, i) = m(65.0, 18.0);
        p.cubic_to((a, b), (c, e), (g, i));
        let (a, b) = m(65.0, 10.0);
        let (c, e) = m(71.0, 0.0);
        let (g, i) = m(85.0, 0.0);
        p.cubic_to((a, b), (c, e), (g, i));
        let (a, b) = m(107.0, 0.0);
        let (c, e) = m(130.0, 16.0);
        let (g, i) = m(130.0, 42.0);
        p.cubic_to((a, b), (c, e), (g, i));
        let (a, b) = m(130.0, 79.0);
        let (c, e) = m(80.0, 106.0);
        let (g, i) = m(65.0, 122.0);
        p.cubic_to((a, b), (c, e), (g, i));
        p.close();
        p.finish()
    };

    // soft drop shadow (reference: 0 8px 10px rgba(20,41,74,.4))
    let mix3 = |c: u8, t: u8| ((u16::from(c) * 3 + u16::from(t)) / 4) as u8;
    let sh_col = C(image::Rgba([
        mix3(r.heart.r, 20),
        mix3(r.heart.g, 41),
        mix3(r.heart.b, 74),
        102,
    ]));
    let shadow_polys = path(1.032);
    // offset the shadow: quick translate of every point
    let off: Vec<Vec<(f64, f64)>> = shadow_polys
        .iter()
        .map(|poly| {
            poly.iter()
                .map(|&(x, y)| (x + 3.0 * u, y + 8.0 * u))
                .collect()
        })
        .collect();
    hc.d.fill_polys(&off, sh_col);

    // glass rim (outer, translucent light)
    let rim = C(image::Rgba([
        mix(r.heart_rim, Rgb::new(235, 240, 248), 0.35).r,
        mix(r.heart_rim, Rgb::new(235, 240, 248), 0.35).g,
        mix(r.heart_rim, Rgb::new(235, 240, 248), 0.35).b,
        191,
    ]));
    hc.d.fill_polys(&path(1.032), rim);

    // body
    let body = path(0.97);
    match wall.and_then(|b| image::load_from_memory(b).ok()) {
        Some(src) => {
            // cover-fit the user's image to the heart's bounding box, then
            // paste it masked by the heart path (rim stays visible around it)
            let (bw, bh) = (130.0 * s * u, 122.0 * s * u);
            let cov = cover_rgba(&src, bw.ceil() as u32, bh.ceil() as u32);
            paste_masked(hc.d, &cov, x0, y0, &body);
        }
        None => {
            // theme-derived radial glass fill (no image set)
            let hl = luminance(r.heart);
            let bands = [(0.97, 0.0), (0.90, 0.16), (0.80, 0.32), (0.68, 0.46)];
            for (k, light) in bands {
                let col = clamp_lightness(r.heart, Some(hl + light), None);
                hc.d.fill_polys(&path(k), (col, 255u8).into());
            }
        }
    }

    // gloss arc: white stroke top-left (reference 33×31 box) — drawn bold
    // enough to read as the reference's specular shine
    let gx = x0 + 11.5 * s * u;
    let gy = y0 + 10.5 * s * u;
    let (gw, gh) = (33.5 * s * u, 31.5 * s * u);
    let arc = gloss_arc(gx, gy, gw, gh);
    let gloss = C(image::Rgba([r.white.0[0], r.white.0[1], r.white.0[2], 215]));
    hc.d.fill_polys(&arc, gloss);
}

/// Paste `src` at (dx, dy) keeping only the pixels whose centre lies inside
/// `mask` (a polygon set in the same canvas pixel space). Anti-aliased mask
/// edges come free from the polygon coverage of the first and last pixel
/// column/row spans.
fn paste_masked(d: &mut D, src: &image::RgbaImage, dx: f64, dy: f64, mask: &[Vec<(f64, f64)>]) {
    // per-row [min_x, max_x] span of the mask, rasterized the same way
    // fill_polys does (scanline, centre sampling) so the paste aligns with
    // any polygon fill
    let (mw, mh) = src.dimensions();
    let y_lo = mask
        .iter()
        .flatten()
        .map(|p| p.1)
        .fold(f64::INFINITY, f64::min)
        .floor()
        .max(0.0) as i64;
    let y_hi = mask
        .iter()
        .flatten()
        .map(|p| p.1)
        .fold(f64::NEG_INFINITY, f64::max)
        .ceil() as i64;
    let mut spans: Vec<(i64, i64, i64)> = Vec::new(); // (y, x_lo, x_hi)
    for y in y_lo..=y_hi {
        let cy = f64::from(y as u32) + 0.5;
        // collect crossings of every sub-path against this scanline
        let mut xs: Vec<f64> = Vec::new();
        for poly in mask {
            let n = poly.len();
            if n < 2 {
                continue;
            }
            // PathBuilder closes its polylines (first point == last point);
            // iterating segments i→(i+1)%n would walk the closing edge twice
            // and double every crossing, collapsing the even-odd spans. Walk
            // n-1 edges only when the ring is already closed.
            let last = if n > 1 && poly[0] == poly[n - 1] {
                n - 1
            } else {
                n
            };
            for i in 0..last {
                let (ax, ay) = poly[i];
                let (bx, by) = poly[(i + 1) % n];
                if (ay <= cy && by > cy) || (by <= cy && ay > cy) {
                    let t = (cy - ay) / (by - ay);
                    xs.push(ax + (bx - ax) * t);
                }
            }
        }
        if xs.len() < 2 {
            continue;
        }
        xs.sort_by(|a, b| a.partial_cmp(b).unwrap());
        // even-odd fill between pairs
        let mut i = 0;
        while i + 1 < xs.len() {
            spans.push((y, xs[i].ceil() as i64, xs[i + 1].floor() as i64));
            i += 2;
        }
    }
    // paste within the mask spans, sampling the source
    for &(y, x_lo, x_hi) in &spans {
        let py = y - dy.floor() as i64;
        if py < 0 || py >= mh as i64 {
            continue;
        }
        for x in x_lo..=x_hi {
            let pxx = x - dx.floor() as i64;
            if pxx < 0 || pxx >= mw as i64 {
                continue;
            }
            let p = src.get_pixel(pxx as u32, py as u32);
            if p[3] == 0 {
                continue;
            }
            let tx = x;
            let ty = y;
            let (iw, ih) = d.img.dimensions();
            if tx >= 0 && ty >= 0 && (tx as u32) < iw && (ty as u32) < ih {
                let dst = d.img.get_pixel_mut(tx as u32, ty as u32);
                if p[3] == 255 {
                    *dst = *p;
                } else {
                    // blend straight alpha over the destination
                    let sa = f32::from(p[3]) / 255.0;
                    let da = f32::from(dst[3]) / 255.0;
                    let oa = sa + da * (1.0 - sa);
                    if oa > 0.0 {
                        for c in 0..3 {
                            let v =
                                (f32::from(p[c]) * sa + f32::from(dst[c]) * da * (1.0 - sa)) / oa;
                            dst[c] = v.round().clamp(0.0, 255.0) as u8;
                        }
                        dst[3] = (oa * 255.0).round().clamp(0.0, 255.0) as u8;
                    }
                }
            }
        }
    }
}

/// Quadrant arc stroke approximated as a filled wedge (top-left corner).
fn gloss_arc(x: f64, y: f64, w: f64, h: f64) -> Polys {
    let mut p = PathBuilder::new();
    let (x0, y0) = (x, y + h); // bottom-left of the box
    p.move_to(x0, y0);
    // outer curve up to (x+w, y)
    p.cubic_to((x0, y - h * 0.55), (x + w * 0.45, y), (x + w, y));
    // inner curve back (4.5px thick)
    let t = 4.5;
    p.line_to(x + w, y + t);
    p.cubic_to(
        (x + w * 0.45, y + t),
        (x0 + t, y - h * 0.55 + t * 1.6),
        (x0 + t, y0),
    );
    p.close();
    p.finish()
}

/* ====================== skeleton pieces ====================== */

/// Glass bubble: rounded 14px + small triangle tail on the given side.
/// Glass = the panel hue lifted toward white (frosted), alpha like the
/// reference (.82 in / .85 out); dark mode uses a dark tint instead.
#[allow(clippy::too_many_arguments)] // mockup box + side + roles reads best flat
fn bubble(d: &mut D, sc: &Scene, out: bool, x: f64, y: f64, w: f64, h: f64, r: &Roles) {
    let u = sc.u;
    let glass = glass_col(sc, r, out);
    let (x0, y0, x1, y1) = (x * u, y * u, (x + w) * u, (y + h) * u);
    d.round_rect(x0, y0, x1, y1, 14.0 * u, glass);
    // tail wedge (reference: ~16px wide, 8px tall at the bottom corner;
    // drawn 1.5× so it reads on translucent glass)
    let mut p = PathBuilder::new();
    if out {
        p.move_to(x1 - 10.0 * u, y1 - 10.0 * u);
        p.line_to(x1 + 4.0 * u, y1 + 1.0 * u);
        p.line_to(x1 - 18.0 * u, y1 - 1.0 * u);
    } else {
        p.move_to(x0 + 10.0 * u, y1 - 10.0 * u);
        p.line_to(x0 - 4.0 * u, y1 + 1.0 * u);
        p.line_to(x0 + 18.0 * u, y1 - 1.0 * u);
    }
    p.close();
    let polys = p.finish();
    d.fill_polys(&polys, glass);
}

/// 4-point sparkle (reference: two crossing lenses, ~8×10).
fn sparkle(d: &mut D, sc: &Scene, cx: f64, cy: f64, col: C) {
    let u = sc.u;
    let (w, h) = (8.0 * u, 10.0 * u);
    let thin = 1.1 * u;
    d.ellipse(
        cx * u - thin,
        cy * u - h / 2.0,
        cx * u + thin,
        cy * u + h / 2.0,
        col,
    );
    d.ellipse(
        cx * u - w / 2.0,
        cy * u - thin,
        cx * u + w / 2.0,
        cy * u + thin,
        col,
    );
}

/// Orbit ellipse ring (33×11) drawn as a true stroke: short segments along
/// the ellipse path, plus two dots sitting on it (reference element).
fn orbit(d: &mut D, sc: &Scene, cx: f64, cy: f64, r: &Roles) {
    let (a, b) = (16.65, 5.65); // semi-axes in canvas units
    let segs = 28;
    let wc = C(image::Rgba([r.white.0[0], r.white.0[1], r.white.0[2], 240]));
    for i in 0..segs {
        let t0 = (i as f64) * std::f64::consts::TAU / segs as f64;
        let t1 = ((i + 1) as f64) * std::f64::consts::TAU / segs as f64;
        let p0 = (cx + a * t0.cos(), cy + b * t0.sin());
        let p1 = (cx + a * t1.cos(), cy + b * t1.sin());
        // segment as a thin quad approximated by a pill between the points
        let mx = (p0.0 + p1.0) / 2.0;
        let my = (p0.1 + p1.1) / 2.0;
        let len = ((p1.0 - p0.0).hypot(p1.1 - p0.1)).max(1.2);
        // orient: draw small rect via round_rect with rotation is not
        // available; approximate each segment with a small dot of thickness
        // 2.4 — dense enough (28 segs) to read as a continuous stroke.
        dot(d, sc, mx, my, (len / 2.0 + 1.0).min(2.4), wc);
    }
    // dots on the ring (reference: 3px at +1.2,-4.3 and +29.2,+1.7)
    dot(d, sc, cx + 1.2, cy - 4.3, 1.5, wc);
    let solid = C(image::Rgba([r.white.0[0], r.white.0[1], r.white.0[2], 255]));
    dot(d, sc, cx + 29.2, cy + 1.7, 1.5, solid);
}

/// Two thin rounded lines standing in for the serif caption (no sample text
/// ever ships in the preview).
fn caption(d: &mut D, sc: &Scene, x: f64, y: f64, widths: &[f64; 2], r: &Roles) {
    let wc: C = C(image::Rgba([r.white.0[0], r.white.0[1], r.white.0[2], 217]));
    pill(d, sc, x + 19.0, y, widths[0], 7.0, wc);
    pill(d, sc, x + 33.0, y + 12.0, widths[1], 7.0, wc);
}

/// The voice waveform: 20 bars, heights straight from the reference dump.
fn voice_wave(d: &mut D, sc: &Scene, r: &Roles) {
    let heights = [7.0, 14.0, 19.0, 11.0, 17.0, 10.0, 18.0, 13.0, 8.0, 5.0];
    let mut x = 236.0;
    for (i, h) in heights.iter().enumerate() {
        let cy = 339.5 - h / 2.0 + if i % 2 == 1 { 1.0 } else { 0.0 };
        pill(d, sc, x, cy, 2.5, *h, r.ink);
        x += 5.0;
    }
    let mut x = 286.0;
    let heights2 = [12.0, 8.0, 15.0, 10.0, 17.0, 11.0, 14.0, 9.0, 6.0, 4.0];
    for h in heights2.iter() {
        let cy = 339.5 - h / 2.0 + 1.0;
        pill(d, sc, x, cy, 2.5, *h, r.gray);
        x += 5.0;
    }
}

/* ====================== the scene ====================== */

pub(crate) fn draw(img: &mut RgbaImage, sc: &Scene, a_wall: Option<&[u8]>) {
    let u = sc.u;
    let r = roles(sc);
    let mut d = D { img };

    /* ---- phone card + drop shadow ---- */
    let shadow: C = (Rgb::new(0, 0, 0), 64u8).into();
    d.round_rect(
        (CHAT_X - 3.0) * u,
        (CHAT_Y + 10.0) * u,
        (CHAT_X + PHONE_W + 3.0) * u,
        (CHAT_Y + PHONE_H + 14.0) * u,
        (PHONE_R + 3.0) * u,
        shadow,
    );
    card(
        &mut d, sc, CHAT_X, CHAT_Y, PHONE_W, PHONE_H, PHONE_R, r.card,
    );

    /* ---- header (on card colour, reference y 75..105) ---- */
    dot(&mut d, sc, 111.0, 87.0, 11.0, r.ink);
    pill(&mut d, sc, 142.0, 75.0, 76.0, 10.0, r.ink);
    pill(&mut d, sc, 142.0, 93.0, 46.0, 10.0, r.ink);
    dot(&mut d, sc, 341.0, 87.0, 11.0, r.ink);

    /* ---- screen: wallpaper (underlay + blobs + heart) ---- */
    wallpaper(&mut d, sc, &r, a_wall);

    /* ---- caption 1 + sparkles + orbit (over the wallpaper) ---- */
    caption(&mut d, sc, 198.0, 154.0, &[112.0, 96.0], &r);
    let spark: C = C(image::Rgba([r.white.0[0], r.white.0[1], r.white.0[2], 230]));
    sparkle(&mut d, sc, 241.0, 164.0, spark);
    sparkle(&mut d, sc, 354.0, 164.0, spark);
    orbit(&mut d, sc, 298.0, 186.0, &r);

    /* ---- bubble rows (reference order) ---- */
    // 1) OUT 128×28 @ (234,124), tail right
    bubble(&mut d, sc, true, 234.0, 124.0, 128.0, 28.0, &r);
    // 2) IN 174×58 @ (90,170) quote block: accent 3.5×22 + bars 54/92×6
    bubble(&mut d, sc, false, 90.0, 170.0, 174.0, 58.0, &r);
    pill(&mut d, sc, 99.0, 179.0, 3.5, 22.0, r.ink);
    pill(&mut d, sc, 108.0, 181.0, 54.0, 6.0, r.ink);
    pill(&mut d, sc, 108.0, 193.0, 92.0, 6.0, r.ink);
    // 3) IN 132×28 @ (90,234), tail left
    bubble(&mut d, sc, false, 90.0, 234.0, 132.0, 28.0, &r);
    // 4) OUT 178×28 @ (184,274)
    bubble(&mut d, sc, true, 184.0, 274.0, 178.0, 28.0, &r);
    // 5) OUT 178×54 @ (184,308) voice: circle 32 + waveform + duration bar
    bubble(&mut d, sc, true, 184.0, 308.0, 178.0, 54.0, &r);
    dot(&mut d, sc, 212.0, 335.0, 16.0, r.ink);
    voice_wave(&mut d, sc, &r);
    pill(&mut d, sc, 236.0, 345.0, 38.0, 5.0, r.ink);
    dot(&mut d, sc, 279.0, 347.5, 2.5, r.ink);
    // 6) OUT 132×28 @ (230,368), tail right
    bubble(&mut d, sc, true, 230.0, 368.0, 132.0, 28.0, &r);
    // 7) IN 166×54 @ (90,406) avatar row: 32 circle + 13 mini + 2 bars
    bubble(&mut d, sc, false, 90.0, 406.0, 166.0, 54.0, &r);
    dot(&mut d, sc, 118.0, 433.0, 16.0, r.ink);
    dot(&mut d, sc, 130.0, 445.0, 6.5, r.ink);
    pill(&mut d, sc, 148.0, 424.0, 84.0, 6.0, r.ink);
    pill(&mut d, sc, 148.0, 436.0, 54.0, 6.0, r.ink);
    // 8) IN 144×28 @ (90,466)
    bubble(&mut d, sc, false, 90.0, 466.0, 144.0, 28.0, &r);
    // 9) IN 132×28 @ (90,500), tail left
    bubble(&mut d, sc, false, 90.0, 500.0, 132.0, 28.0, &r);

    /* ---- caption 2 + sparkles + orbit ---- */
    caption(&mut d, sc, 66.0, 548.0, &[120.0, 104.0], &r);
    sparkle(&mut d, sc, 60.0, 555.0, spark);
    sparkle(&mut d, sc, 115.0, 559.0, spark);
    sparkle(&mut d, sc, 218.0, 559.0, spark);
    orbit(&mut d, sc, 166.0, 580.0, &r);

    /* ---- trailing out bubbles ---- */
    // 10) OUT 122×28 @ (240,546)
    bubble(&mut d, sc, true, 240.0, 546.0, 122.0, 28.0, &r);
    // 11) OUT 152×28 @ (210,580)
    bubble(&mut d, sc, true, 210.0, 580.0, 152.0, 28.0, &r);
    // 12) OUT 132×28 @ (230,614), tail right
    bubble(&mut d, sc, true, 230.0, 614.0, 132.0, 28.0, &r);

    /* ---- input bar (on card colour, reference y 677..699) ---- */
    dot(&mut d, sc, 108.0, 688.0, 11.0, r.ink);
    pill(&mut d, sc, 134.0, 683.0, 62.0, 10.0, r.gray);
    dot(&mut d, sc, 314.0, 688.0, 11.0, r.ink);
    dot(&mut d, sc, 350.0, 688.0, 11.0, r.ink);
}
