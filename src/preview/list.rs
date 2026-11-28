//! Right phone of the glass-mockup preview — the CHAT LIST screen,
//! transcribed 1:1 from the reference HTML.
//!
//! Same rules as the chat screen: coordinates are mockup units from the
//! reference DOM dump (placement only), every colour comes from the compiled
//! theme. This gives the user the second half of the reference composition —
//! the list phone — so the preview shows the full picture of how the theme
//! looks, without shipping any static data (all rows are skeleton bars).

use image::RgbaImage;

use super::draw::{PathBuilder, C, D};
use super::scene::{card, dot, pill, Scene, LIST_X, LIST_Y, PHONE_H, PHONE_R, PHONE_W};
use crate::colors::{luminance, Rgb};

/// Row spec: everything is relative to the row's avatar top `y` (the
/// reference DOM dump gives absolute boxes; they are transcribed relative
/// here so the row loop stays uniform).
struct Row {
    avatar_y: f64,
    title: (f64, f64),
    gray: (f64, f64),
    /// optional dot before the title: (x, y) relative to the row
    lead_dot: Option<(f64, f64)>,
    /// right-side marks: (x, y, w, h, gray?) relative to the row; w==h → dot
    right: &'static [(f64, f64, f64, f64, bool)],
}

const ROWS: &[Row] = &[
    // 1 — avatar 443, title 505/205 56x8, gray 223 42x8, right: dot+bar 653/205, circle 677/219
    Row {
        avatar_y: 195.0,
        title: (56.0, 8.0),
        gray: (42.0, 8.0),
        lead_dot: None,
        right: &[
            (148.0, 10.0, 8.0, 8.0, false),
            (160.0, 10.0, 24.0, 8.0, false),
            (234.0, 24.0, 18.0, 18.0, false),
        ],
    },
    // 2 — avatar 443/261, title 76x8 + inline dot 584, gray 296 54x8, right: bar 669, gray circle 677
    Row {
        avatar_y: 261.0,
        title: (76.0, 8.0),
        gray: (54.0, 8.0),
        lead_dot: Some((79.0, 5.0)),
        right: &[
            (226.0, 5.0, 22.0, 8.0, false),
            (234.0, 17.0, 18.0, 18.0, true),
        ],
    },
    // 3 — avatar 443/329, title 78x8, gray 60x8, right bar 671
    Row {
        avatar_y: 329.0,
        title: (78.0, 8.0),
        gray: (60.0, 8.0),
        lead_dot: None,
        right: &[(228.0, 10.0, 22.0, 8.0, false)],
    },
    // 4 — avatar 443/397, title 42x8, gray 118x8, right bar 671
    Row {
        avatar_y: 397.0,
        title: (42.0, 8.0),
        gray: (118.0, 8.0),
        lead_dot: None,
        right: &[(228.0, 10.0, 22.0, 8.0, false)],
    },
    // 5 — avatar 443/465, dot+title 48x8, gray 42x8, right: dot 656 + bar 669
    Row {
        avatar_y: 465.0,
        title: (48.0, 8.0),
        gray: (42.0, 8.0),
        lead_dot: Some((0.0, 10.0)),
        right: &[
            (213.0, 10.0, 8.0, 8.0, false),
            (226.0, 10.0, 22.0, 8.0, false),
        ],
    },
    // 6 — avatar 443/533, title 68x8, gray 54x8, right: dot + bar
    Row {
        avatar_y: 533.0,
        title: (68.0, 8.0),
        gray: (54.0, 8.0),
        lead_dot: None,
        right: &[
            (213.0, 10.0, 8.0, 8.0, false),
            (226.0, 10.0, 22.0, 8.0, false),
        ],
    },
    // 7 — avatar 443/601, title 56x8, gray 42x8, right bar
    Row {
        avatar_y: 601.0,
        title: (56.0, 8.0),
        gray: (42.0, 8.0),
        lead_dot: None,
        right: &[(228.0, 10.0, 22.0, 8.0, false)],
    },
];

pub(crate) fn draw(img: &mut RgbaImage, sc: &Scene) {
    let u = sc.u;
    let t = sc.t;
    let mut d = D { img };

    let panel = t.solid("chat_messagePanelBackground");
    // header slightly lighter, like the reference (#ced2d9 over #c9cdd4)
    let header = lift_c(panel, 0.02);
    let ink = t.solid("actionBarDefaultIcon");
    let gray = t.solid("chat_inTimeText");

    /* ---- phone card + drop shadow ---- */
    let shadow: C = (Rgb::new(0, 0, 0), 64u8).into();
    d.round_rect(
        (LIST_X - 3.0) * u,
        (LIST_Y + 10.0) * u,
        (LIST_X + PHONE_W + 3.0) * u,
        (LIST_Y + PHONE_H + 14.0) * u,
        (PHONE_R + 3.0) * u,
        shadow,
    );
    card(&mut d, sc, LIST_X, LIST_Y, PHONE_W, PHONE_H, PHONE_R, panel);

    /* ---- header: two bar rows + 1px divider ---- */
    let hw = PHONE_W * u;
    // header fills the card's top with the card's rounded top corners
    d.round_rect(
        LIST_X * u,
        LIST_Y * u,
        LIST_X * u + hw,
        (LIST_Y + 88.0 + PHONE_R) * u,
        PHONE_R * u,
        header,
    );
    d.rectf(
        LIST_X * u,
        (LIST_Y + 88.0) * u,
        LIST_X * u + hw,
        (LIST_Y + 88.0 + PHONE_R) * u,
        header,
    );
    // row 1 (y ≈ 112..132)
    dot(&mut d, sc, 452.0, 122.0, 9.5, ink);
    pill(&mut d, sc, 488.0, 117.0, 88.0, 10.0, ink);
    dot(&mut d, sc, 651.0, 122.0, 9.5, ink);
    dot(&mut d, sc, 683.0, 122.0, 9.5, ink);
    // divider (reference: 1px #b8bdc6 — a clear but subtle separator)
    let div: C = (mix_black(panel, 0.10), 255u8).into();
    d.rectf(LIST_X * u, 140.5 * u, LIST_X * u + hw, 143.0 * u, div);
    // row 2 (y ≈ 151..171)
    dot(&mut d, sc, 452.0, 161.0, 9.5, ink);
    pill(&mut d, sc, 488.0, 156.0, 58.0, 10.0, ink);
    dot(&mut d, sc, 655.0, 161.0, 9.5, ink);
    dot(&mut d, sc, 683.0, 161.0, 9.5, ink);

    /* ---- rows ---- */
    for row in ROWS {
        let y = row.avatar_y;
        dot(&mut d, sc, 466.0, y + 23.0, 23.0, ink);
        if let Some((dx, dy)) = row.lead_dot {
            dot(&mut d, sc, 505.0 + dx + 4.0, y + dy + 4.0, 4.0, ink);
        }
        pill(&mut d, sc, 505.0, y + 10.0, row.title.0, row.title.1, ink);
        pill(&mut d, sc, 505.0, y + 28.0, row.gray.0, row.gray.1, gray);
        for &(rx, ry, rw, rh, is_gray) in row.right {
            let col = if is_gray { gray } else { ink };
            if (rw - rh).abs() < 0.1 && rh > 4.0 {
                dot(
                    &mut d,
                    sc,
                    418.0 + rx + rw / 2.0,
                    y + ry + rw / 2.0,
                    rw / 2.0,
                    col,
                );
            } else {
                pill(&mut d, sc, 418.0 + rx, y + ry, rw, rh.max(8.0), col);
            }
        }
    }

    /* ---- last row: gradient-faded avatar + faded bars ---- */
    // avatar with vertical fade (reference: linear-gradient to transparent)
    let y = 669.0;
    fade_avatar(&mut d, sc, ink, y);
    pill(&mut d, sc, 505.0, y + 10.0, 84.0, 8.0, ink.faded(0.8));
    pill(&mut d, sc, 505.0, y + 28.0, 44.0, 8.0, gray.faded(0.6));

    /* ---- black dot badge (row 5 avatar) ---- */
    let badge: C = if luminance(Rgb::new(panel.0[0], panel.0[1], panel.0[2])) < 0.5 {
        (Rgb::new(255, 255, 255), 255u8).into()
    } else {
        (Rgb::new(13, 13, 13), 255u8).into()
    };
    // ring = panel colour, dot = badge colour (dark in light mode)
    dot(&mut d, sc, 483.0, 504.0, 8.5, panel);
    dot(&mut d, sc, 483.0, 504.0, 7.0, badge);

    /* ---- FAB: 44px circle + paper-plane triangle ---- */
    let fab_shadow = C(image::Rgba([ink.0[0], ink.0[1], ink.0[2], 89]));
    d.circle(675.0 * u, 696.0 * u, 26.0 * u, fab_shadow);
    dot(&mut d, sc, 673.0, 694.0, 22.0, ink);
    // paper-plane (reference polygon 685,677 → 693,677 → 691,685): a small
    // up-right triangle in the panel colour
    let mut p = PathBuilder::new();
    // reference glyph: plane notch inside the FAB's upper-right quadrant,
    // scaled up to read clearly (reference is subtle; we keep it visible)
    p.move_to(680.0 * u, 681.0 * u);
    p.line_to(691.0 * u, 681.0 * u);
    p.line_to(688.0 * u, 692.0 * u);
    p.close();
    let polys = p.finish();
    d.fill_polys(&polys, panel);
}

/// Vertical-fade avatar (the reference's gradient avatar, theme-tinted):
/// 12 stacked slices, alpha fading 255 → 30 top-to-bottom.
fn fade_avatar(d: &mut D, sc: &Scene, ink: C, y: f64) {
    let u = sc.u;
    let (cx, r) = (466.0, 23.0);
    let cy = y + 23.0;
    // contiguous bands from top (opaque) to bottom (mostly transparent),
    // each band's chord width following the circle
    let n = 26;
    let band = 46.0 / n as f64;
    for i in 0..n {
        let y0 = y + i as f64 * band;
        let y1 = y0 + band + 0.5; // +0.5 avoids hairline seams
        let ym = (y0 + y1) / 2.0;
        let f = (ym - y) / 46.0;
        let a = (255.0 * (1.0 - 0.80 * f)).round() as u8;
        let col = C(image::Rgba([ink.0[0], ink.0[1], ink.0[2], a]));
        let dy = (ym - cy) / r;
        if dy.abs() >= 1.0 {
            continue;
        }
        let half = (r * (1.0 - dy * dy).sqrt()).max(1.2);
        d.rectf((cx - half) * u, y0 * u, (cx + half) * u, y1 * u, col);
    }
}

fn lift_c(c: C, d: f64) -> C {
    let base = Rgb::new(c.0[0], c.0[1], c.0[2]);
    let l = luminance(base);
    let out = crate::colors::clamp_lightness(base, Some((l + d).min(1.0)), None);
    (out, c.0[3]).into()
}

fn mix_black(c: C, k: f64) -> Rgb {
    let base = Rgb::new(c.0[0], c.0[1], c.0[2]);
    crate::colors::mix(base, Rgb::new(0, 0, 0), k)
}
