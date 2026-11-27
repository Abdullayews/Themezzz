//! Skeleton-style chat preview, a 1:1 mirror of the user's reference mockup.
//!
//! Layout (element order, tails, gaps) is fixed from the reference; every
//! colour comes from the compiled theme. All "text" is drawn as rounded
//! skeleton bars and circles so the picture never contains static sample
//! data — only placement. The renderer paints with the same `.attheme` keys
//! Telegram would use, alpha included, so translucent bars and bubbles show
//! the wallpaper exactly as on a real phone.

use image::RgbaImage;

use super::draw::{PathBuilder, Polys, C, D};
use super::icons;
use super::scene::{status_bar, Scene, BAR_H, PHONE_H, PHONE_W, STATUS_H};
use crate::colors::mix;

/// Corner radius of the skeleton bubbles (dp).
const BUBBLE_R: f64 = 20.0;
/// Horizontal padding inside bubbles (dp).
const PAD: f64 = 12.0;

/* ================= header ================= */

fn action_bar(d: &mut D, sc: &Scene) {
    let u = sc.u;
    let t = sc.t;
    let bar = t.c("actionBarDefault");
    d.rectf(0.0, 0.0, sc.w(), (STATUS_H + BAR_H) * u, bar);
    status_bar(d, sc, t.rgb("actionBarDefault"));

    let cy = (STATUS_H + BAR_H / 2.0) * u;

    // left avatar circle (back / profile entry)
    let avatar_col = t.c("actionBarDefaultIcon");
    d.circle(52.0 * u, cy, 15.0 * u, avatar_col);

    // title + subtitle skeleton bars
    let title_col = t.c("actionBarDefaultTitle");
    d.round_rect(
        84.0 * u,
        cy - 13.0 * u,
        196.0 * u,
        cy - 4.0 * u,
        4.5 * u,
        title_col,
    );
    d.round_rect(
        84.0 * u,
        cy + 3.0 * u,
        160.0 * u,
        cy + 12.0 * u,
        4.5 * u,
        t.c("actionBarDefaultSubtitle"),
    );

    // right menu circle
    d.circle((PHONE_W - 36.0) * u, cy, 15.0 * u, avatar_col);
}

/* ================= skeleton pieces ================= */

/// One rounded skeleton bar.
fn bar(d: &mut D, x0: f64, y0: f64, x1: f64, y1: f64, r: f64, col: C) {
    d.round_rect(x0, y0, x1, y1, r, col);
}

fn bubble_polys(x0: f64, y0: f64, x1: f64, y1: f64, r: f64, tail: Option<bool>, u: f64) -> Polys {
    let r = r.min((x1 - x0) / 2.0).min((y1 - y0) / 2.0);
    let k = 0.5523 * r;
    let mut p = PathBuilder::new();
    p.move_to(x0 + r, y0);
    p.line_to(x1 - r, y0);
    p.cubic_to((x1 - r + k, y0), (x1, y0 + r - k), (x1, y0 + r));
    match tail {
        Some(true) => {
            p.line_to(x1, y1 - 16.0 * u);
            p.cubic_to(
                (x1, y1 - 6.0 * u),
                (x1 + 2.5 * u, y1 - 1.5 * u),
                (x1 + 15.0 * u, y1),
            );
            p.line_to(x0 + r, y1);
        }
        _ => {
            p.line_to(x1, y1 - r);
            p.cubic_to((x1, y1 - r + k), (x1 - r + k, y1), (x1 - r, y1));
            p.line_to(x0 + r, y1);
        }
    }
    match tail {
        Some(false) => {
            // mirrored tail on the left side
            p.line_to(x0 - 15.0 * u, y1);
            p.cubic_to(
                (x0 - 2.5 * u, y1 - 1.5 * u),
                (x0, y1 - 6.0 * u),
                (x0, y1 - 16.0 * u),
            );
        }
        _ => {
            p.cubic_to((x0 + r - k, y1), (x0, y1 - r + k), (x0, y1 - r));
        }
    }
    p.line_to(x0, y0 + r);
    p.cubic_to((x0, y0 + r - k), (x0 + r - k, y0), (x0 + r, y0));
    p.close();
    p.finish()
}

/// Skeleton bubble body with optional tail. `tail` points to the sender:
/// bottom-left for in, bottom-right for out — per the reference mockup.
fn bubble(d: &mut D, sc: &Scene, out: bool, x0: f64, y0: f64, x1: f64, y1: f64) {
    let u = sc.u;
    let (fill, shadow) = if out {
        (sc.t.c("chat_outBubble"), sc.t.c("chat_outBubbleShadow"))
    } else {
        (sc.t.c("chat_inBubble"), sc.t.c("chat_inBubbleShadow"))
    };
    let tail = Some(out);
    let sh = bubble_polys(x0, y0 + u, x1, y1 + u, BUBBLE_R * u, tail, u);
    d.fill_polys(&sh, shadow.faded(0.10));
    let body = bubble_polys(x0, y0, x1, y1, BUBBLE_R * u, tail, u);
    d.fill_polys(&body, fill);
}

/// Waveform skeleton: vertical rounded bars of varying height.
fn waveform(d: &mut D, sc: &Scene, x: f64, cy: f64, n: usize, col: C) {
    let u = sc.u;
    for i in 0..n {
        let f = i as f64;
        let bh = (3.0 + 9.0 * ((f * 1.7).sin() * (f * 0.53).cos()).abs()) * u;
        let bx = x + f * 5.0 * u;
        d.round_rect(bx, cy - bh / 2.0, bx + 2.6 * u, cy + bh / 2.0, 1.3 * u, col);
    }
}

/// Tiny 4-point sparkle ornament (decoration, theme-tinted).
fn sparkle(d: &mut D, sc: &Scene, cx: f64, cy: f64, s: f64, col: C) {
    let u = sc.u;
    let (s, half) = (s * u, s * 0.5 * u);
    let thin = s * 0.22;
    // vertical lens
    d.ellipse(cx - thin, cy - half, cx + thin, cy + half, col);
    // horizontal lens
    d.ellipse(cx - half, cy - thin, cx + half, cy + thin, col);
}

/* ================= rows ================= */

const IN_X0: f64 = 14.0;
const OUT_X1: f64 = PHONE_W - 14.0;

pub(crate) fn draw(img: &mut RgbaImage, sc: &Scene) {
    let u = sc.u;
    let mut d = D { img };
    action_bar(&mut d, sc);

    let t = sc.t;
    let in_bubble = t.c("chat_inBubble");
    // Skeleton ink colors: real message text and secondary text keys keep
    // bars legible on both bubble tones.
    let in_ink = t.c("chat_messageTextIn");
    let out_ink = t.c("chat_messageTextOut");
    let in_soft = t.c("chat_inTimeText");

    let mut y = (STATUS_H + BAR_H + 10.0) * u;

    /* -- row 1: OUT text bubble (2 bars) -- */
    let (x0, x1) = ((OUT_X1 - 168.0) * u, OUT_X1 * u);
    let h1 = 42.0 * u;
    bubble(&mut d, sc, true, x0, y, x1, y + h1);
    bar(
        &mut d,
        x0 + PAD * u,
        y + 10.0 * u,
        x1 - 34.0 * u,
        y + 17.0 * u,
        3.5 * u,
        out_ink,
    );
    bar(
        &mut d,
        x0 + PAD * u,
        y + 24.0 * u,
        x1 - 62.0 * u,
        y + 31.0 * u,
        3.5 * u,
        out_ink,
    );
    y += h1 + 5.0 * u;

    /* -- row 2: IN bubble with quote block (accent edge + 2 bars) -- */
    let (x0, x1) = (IN_X0 * u, IN_X0 * u + 196.0 * u);
    let h2 = 48.0 * u;
    bubble(&mut d, sc, false, x0, y, x1, y + h2);
    let q = t.c("chat_inReplyNameText");
    bar(
        &mut d,
        x0 + PAD * u,
        y + 10.0 * u,
        x0 + (PAD + 3.0) * u,
        y + 40.0 * u,
        1.5 * u,
        q,
    );
    bar(
        &mut d,
        x0 + (PAD + 10.0) * u,
        y + 12.0 * u,
        x1 - 44.0 * u,
        y + 19.0 * u,
        3.5 * u,
        in_ink,
    );
    bar(
        &mut d,
        x0 + (PAD + 10.0) * u,
        y + 26.0 * u,
        x1 - 78.0 * u,
        y + 33.0 * u,
        3.5 * u,
        in_soft,
    );
    bar(
        &mut d,
        x0 + (PAD + 10.0) * u,
        y + 40.0 * u,
        x1 - 102.0 * u,
        y + 44.0 * u,
        2.0 * u,
        in_soft,
    );
    y += h2 + 5.0 * u;

    /* -- row 3: IN short bubble -- */
    let (x0, x1) = (IN_X0 * u, IN_X0 * u + 128.0 * u);
    let h3 = 34.0 * u;
    bubble(&mut d, sc, false, x0, y, x1, y + h3);
    bar(
        &mut d,
        x0 + PAD * u,
        y + 14.0 * u,
        x1 - 30.0 * u,
        y + 21.0 * u,
        3.5 * u,
        in_ink,
    );
    y += h3 + 5.0 * u;

    /* -- row 4: IN voice-note skeleton (circle + waveform + duration) -- */
    let (x0, x1) = (IN_X0 * u, (IN_X0 + 196.0) * u);
    let h4 = 54.0 * u;
    bubble(&mut d, sc, false, x0, y, x1, y + h4);
    let play_bg = t.c("chat_inLoader");
    let mut play_bg = play_bg;
    play_bg.0[3] = play_bg.0[3].min(70);
    d.circle(x0 + (PAD + 15.0) * u, y + 24.0 * u, 15.0 * u, play_bg);
    d.icon(
        icons::PLAY,
        x0 + (PAD + 16.0) * u,
        y + 24.0 * u,
        20.0 * u,
        t.c_or("chat_inMediaIcon", image::Rgba([255, 255, 255, 255])),
    );
    waveform(
        &mut d,
        sc,
        x0 + (PAD + 38.0) * u,
        y + 22.0 * u,
        18,
        t.c("chat_inVoiceSeekbar"),
    );
    bar(
        &mut d,
        x0 + (PAD + 38.0) * u,
        y + 38.0 * u,
        x0 + (PAD + 74.0) * u,
        y + 44.0 * u,
        3.0 * u,
        t.c("chat_inAudioDurationText"),
    );
    y += h4 + 5.0 * u;

    /* -- row 5: IN bubble with avatar circle + 2 bars -- */
    let (x0, x1) = (IN_X0 * u, IN_X0 * u + 172.0 * u);
    let h5 = 54.0 * u;
    bubble(&mut d, sc, false, x0, y, x1, y + h5);
    let av = t.c("avatar_backgroundActionBarBlue");
    d.circle(x0 + (PAD + 13.0) * u, y + 25.0 * u, 13.0 * u, av);
    bar(
        &mut d,
        x0 + (PAD + 32.0) * u,
        y + 14.0 * u,
        x1 - 30.0 * u,
        y + 21.0 * u,
        3.5 * u,
        in_ink,
    );
    bar(
        &mut d,
        x0 + (PAD + 32.0) * u,
        y + 30.0 * u,
        x1 - 58.0 * u,
        y + 37.0 * u,
        3.5 * u,
        in_soft,
    );
    y += h5 + 5.0 * u;

    /* -- row 6: IN short bubble (tail) -- */
    let (x0, x1) = (IN_X0 * u, IN_X0 * u + 96.0 * u);
    let h6 = 32.0 * u;
    bubble(&mut d, sc, false, x0, y, x1, y + h6);
    bar(
        &mut d,
        x0 + PAD * u,
        y + 13.0 * u,
        x1 - 26.0 * u,
        y + 20.0 * u,
        3.5 * u,
        in_ink,
    );
    y += h6 + 12.0 * u;

    /* -- ornament caption: sparkles + thin bars (centered) -- */
    let orn = t.c("chat_messageTextIn");
    let orn_soft = t.c("chat_inTimeText");
    let cx = sc.w() / 2.0;
    sparkle(
        &mut d,
        sc,
        cx - 72.0 * u,
        y + 16.0 * u,
        7.0,
        orn.faded(0.63),
    );
    bar(
        &mut d,
        cx - 76.0 * u,
        y + 8.0 * u,
        cx + 76.0 * u,
        y + 15.0 * u,
        3.5 * u,
        orn_soft,
    );
    bar(
        &mut d,
        cx - 46.0 * u,
        y + 22.0 * u,
        cx + 46.0 * u,
        y + 29.0 * u,
        3.5 * u,
        orn_soft,
    );
    sparkle(
        &mut d,
        sc,
        cx + 72.0 * u,
        y + 16.0 * u,
        7.0,
        orn.faded(0.63),
    );
    y += 40.0 * u + 12.0 * u;

    /* -- row 7: OUT empty bubble -- */
    let (x0, x1) = ((OUT_X1 - 118.0) * u, OUT_X1 * u);
    let h7 = 34.0 * u;
    bubble(&mut d, sc, true, x0, y, x1, y + h7);
    bar(
        &mut d,
        x0 + PAD * u,
        y + 14.0 * u,
        x1 - 28.0 * u,
        y + 21.0 * u,
        3.5 * u,
        out_ink,
    );
    y += h7 + 5.0 * u;

    /* -- row 8: OUT bubble with reply (accent edge + 2 bars) -- */
    let (x0, x1) = ((OUT_X1 - 176.0) * u, OUT_X1 * u);
    let h8 = 52.0 * u;
    bubble(&mut d, sc, true, x0, y, x1, y + h8);
    let q = t.c("chat_outReplyNameText");
    bar(
        &mut d,
        x0 + PAD * u,
        y + 10.0 * u,
        x0 + (PAD + 3.0) * u,
        y + 40.0 * u,
        1.5 * u,
        q,
    );
    bar(
        &mut d,
        x0 + (PAD + 10.0) * u,
        y + 12.0 * u,
        x1 - 40.0 * u,
        y + 19.0 * u,
        3.5 * u,
        out_ink,
    );
    bar(
        &mut d,
        x0 + (PAD + 10.0) * u,
        y + 26.0 * u,
        x1 - 72.0 * u,
        y + 33.0 * u,
        3.5 * u,
        out_ink,
    );
    bar(
        &mut d,
        x0 + (PAD + 10.0) * u,
        y + 40.0 * u,
        x1 - 96.0 * u,
        y + 44.0 * u,
        2.0 * u,
        out_ink,
    );
    y += h8 + 5.0 * u;

    /* -- row 9: IN wide bubble (3 bars) -- */
    let (x0, x1) = (IN_X0 * u, IN_X0 * u + 208.0 * u);
    let h9 = 56.0 * u;
    bubble(&mut d, sc, false, x0, y, x1, y + h9);
    bar(
        &mut d,
        x0 + PAD * u,
        y + 12.0 * u,
        x1 - 36.0 * u,
        y + 19.0 * u,
        3.5 * u,
        in_ink,
    );
    bar(
        &mut d,
        x0 + PAD * u,
        y + 26.0 * u,
        x1 - 64.0 * u,
        y + 33.0 * u,
        3.5 * u,
        in_soft,
    );
    bar(
        &mut d,
        x0 + PAD * u,
        y + 40.0 * u,
        x1 - 92.0 * u,
        y + 47.0 * u,
        3.5 * u,
        in_soft,
    );
    y += h9 + 5.0 * u;

    /* -- row 10: OUT short bubble -- */
    let (x0, x1) = ((OUT_X1 - 104.0) * u, OUT_X1 * u);
    let h10 = 32.0 * u;
    bubble(&mut d, sc, true, x0, y, x1, y + h10);
    bar(
        &mut d,
        x0 + PAD * u,
        y + 13.0 * u,
        x1 - 26.0 * u,
        y + 20.0 * u,
        3.5 * u,
        out_ink,
    );
    y += h10 + 12.0 * u;

    debug_assert!(
        y < (PHONE_H - 72.0) * u,
        "chat content overflows the input panel"
    );

    /* -- input panel: circle + pill + 2 circles -- */
    input_panel(&mut d, sc);
    let _ = in_bubble;
}

fn input_panel(d: &mut D, sc: &Scene) {
    let u = sc.u;
    let t = sc.t;
    let top = (PHONE_H - 16.0 - 48.0) * u;
    d.rectf(
        0.0,
        top,
        sc.w(),
        sc.h(),
        t.solid("chat_messagePanelBackground"),
    );
    let icons_col = t.c("chat_messagePanelIcons").at_least(200);
    let cy = top + 24.0 * u;

    // attach circle
    d.circle(30.0 * u, cy, 14.0 * u, icons_col);
    // input pill
    let hint = t.c("chat_messagePanelHint").at_least(200);
    d.round_rect(
        56.0 * u,
        cy - 9.0 * u,
        208.0 * u,
        cy + 9.0 * u,
        9.0 * u,
        hint.faded(0.47),
    );
    // right circles (mic + send)
    d.circle((PHONE_W - 66.0) * u, cy, 14.0 * u, icons_col);
    d.circle((PHONE_W - 30.0) * u, cy, 14.0 * u, icons_col);

    // gesture-navigation pill
    let (pw, py) = (110.0 * u, sc.h() - 8.0 * u);
    let nav = mix(
        crate::colors::Rgb::new(255, 255, 255),
        crate::colors::Rgb::new(0, 0, 0),
        0.35,
    );
    let nav: C = (nav, 150u8).into();
    d.round_rect(
        (sc.w() - pw) / 2.0,
        py - 2.0 * u,
        (sc.w() + pw) / 2.0,
        py + 2.0 * u,
        2.0 * u,
        nav,
    );
}
