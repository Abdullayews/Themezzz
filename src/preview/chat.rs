//! Left phone: a Telegram for Android group chat painted from theme keys.

use image::{Rgba, RgbaImage};

use super::draw::{PathBuilder, Polys, D};
use super::icons;
use super::scene::{avatar, name_color, status_bar, Scene, BAR_H, PHONE_H, PHONE_W, STATUS_H};
use super::text::{ellipsize, measure, wrap_runs, Weight};

const BUBBLE_R: f64 = 17.0; // SharedConfig.bubbleRadius default
const PAD: f64 = 10.0;
const LINE_H: f64 = 19.0;
const MAX_BUBBLE: f64 = 262.0;

#[derive(Clone, Copy, PartialEq)]
enum Status {
    None,
    Sent,
    Read,
}

struct Msg<'a> {
    out: bool,
    sender: Option<(&'a str, usize)>,
    avatar: Option<(usize, &'a str)>,
    reply: Option<(&'a str, &'a str)>,
    runs: &'a [(&'a str, usize)],
    preview: Option<(&'a str, &'a str, &'a str)>,
    time: &'a str,
    status: Status,
}

/// Rounded bubble outline with Telegram's curved tail on the bottom corner.
fn bubble_polys(x0: f64, y0: f64, x1: f64, y1: f64, r: f64, tail: Option<bool>, u: f64) -> Polys {
    let r = r.min((x1 - x0) / 2.0).min((y1 - y0) / 2.0);
    let k = 0.5523 * r;
    let mut p = PathBuilder::new();
    p.move_to(x0 + r, y0);
    p.line_to(x1 - r, y0);
    p.cubic_to((x1 - r + k, y0), (x1, y0 + r - k), (x1, y0 + r));
    match tail {
        Some(true) => {
            p.line_to(x1, y1 - 13.0 * u);
            p.cubic_to(
                (x1, y1 - 5.0 * u),
                (x1 + 1.5 * u, y1 - 1.2 * u),
                (x1 + 6.0 * u, y1),
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
            p.line_to(x0 - 6.0 * u, y1);
            p.cubic_to(
                (x0 - 1.5 * u, y1 - 1.2 * u),
                (x0, y1 - 5.0 * u),
                (x0, y1 - 13.0 * u),
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

fn draw_bubble(d: &mut D, sc: &Scene, out: bool, x0: f64, y0: f64, x1: f64, y1: f64) {
    let u = sc.u;
    let (fill, shadow) = if out {
        (sc.t.c("chat_outBubble"), sc.t.c("chat_outBubbleShadow"))
    } else {
        (sc.t.c("chat_inBubble"), sc.t.c("chat_inBubbleShadow"))
    };
    let tail = Some(out);
    let sh = bubble_polys(x0, y0 + u, x1, y1 + u, BUBBLE_R * u, tail, u);
    d.fill_polys(&sh, shadow.faded(0.22));
    let body = bubble_polys(x0, y0, x1, y1, BUBBLE_R * u, tail, u);
    d.fill_polys(&body, fill);
}

fn draw_time(d: &mut D, sc: &Scene, m_out: bool, status: Status, time: &str, x1: f64, y1: f64) {
    let u = sc.u;
    let col = sc.t.c(if m_out {
        "chat_outTimeText"
    } else {
        "chat_inTimeText"
    });
    let base = y1 - 8.0 * u;
    let mut right = x1 - if m_out { 8.0 } else { 9.0 } * u;
    match status {
        Status::Read => {
            let c = sc.t.c("chat_outSentCheckRead");
            d.icon(
                icons::DONE_ALL,
                right - 8.5 * u,
                base - 4.0 * u,
                17.0 * u,
                c,
            );
            right -= 20.0 * u;
        }
        Status::Sent => {
            let c = sc.t.c("chat_outSentCheck");
            d.icon(icons::CHECK, right - 7.0 * u, base - 4.0 * u, 15.0 * u, c);
            right -= 17.0 * u;
        }
        Status::None => {}
    }
    let em = 12.0 * u;
    let w = measure(Weight::Regular, em, time);
    d.text(Weight::Regular, em, right - w, base, time, col);
}

fn message(d: &mut D, sc: &Scene, m: &Msg, y: f64) -> f64 {
    let u = sc.u;
    let t = sc.t;
    let em = 16.0 * u;
    let small = 14.0 * u;
    let max_content = (MAX_BUBBLE - 2.0 * PAD) * u;
    let (txt, link) = if m.out {
        (t.c("chat_messageTextOut"), t.c("chat_messageLinkOut"))
    } else {
        (t.c("chat_messageTextIn"), t.c("chat_messageLinkIn"))
    };
    let styles = [txt, link];

    let lines = wrap_runs(Weight::Regular, em, max_content, m.runs);
    let text_w = lines.iter().fold(0.0f64, |a, l| a.max(l.width));
    let last_w = lines.last().map_or(0.0, |l| l.width);
    let name_w = m
        .sender
        .map_or(0.0, |(n, _)| measure(Weight::Medium, small, n));
    let reply = m.reply.map(|(n, s)| {
        (
            n,
            ellipsize(Weight::Regular, small, max_content - 10.0 * u, s),
        )
    });
    let reply_w = reply.as_ref().map_or(0.0, |(n, s)| {
        measure(Weight::Medium, small, n).max(measure(Weight::Regular, small, s)) + 8.0 * u
    });
    let prev = m.preview.map(|(site, title, desc)| {
        let dl = wrap_runs(Weight::Regular, em, max_content - 10.0 * u, &[(desc, 0)]);
        (site, title, dl)
    });
    let prev_w = prev.as_ref().map_or(0.0, |(site, title, dl)| {
        let mut w = measure(Weight::Medium, small, site).max(measure(Weight::Medium, em, title));
        for l in dl {
            w = w.max(l.width);
        }
        w + 10.0 * u
    });
    let check_w = match m.status {
        Status::Read => 20.0,
        Status::Sent => 17.0,
        Status::None => 0.0,
    };
    let time_w = measure(Weight::Regular, 12.0 * u, m.time) + check_w * u;

    let mut content = name_w.max(text_w).max(reply_w).max(prev_w);
    let time_inline = prev.is_none() && last_w + 10.0 * u + time_w <= max_content;
    if time_inline {
        content = content.max(last_w + 10.0 * u + time_w);
    } else {
        content = content.max(time_w);
    }
    let w = content + 2.0 * PAD * u;

    let mut h = 8.0 + lines.len() as f64 * LINE_H + 8.0;
    if m.sender.is_some() {
        h += 20.0;
    }
    if reply.is_some() {
        h += 40.0;
    }
    if let Some((_, _, dl)) = &prev {
        h += 6.0 + 19.0 + 19.0 + dl.len() as f64 * LINE_H;
    }
    if !time_inline {
        h += 14.0;
    }
    let h = h * u;

    let right_edge = (PHONE_W - 12.0) * u;
    let x0 = if m.out { right_edge - w } else { 54.0 * u };
    let x1 = x0 + w;
    let y1 = y + h;
    draw_bubble(d, sc, m.out, x0, y, x1, y1);

    if let Some((idx, ini)) = m.avatar {
        avatar(d, sc, 27.0 * u, y1 - 21.0 * u, 42.0, idx, ini);
    }

    let cx = x0 + PAD * u;
    let mut cy = y + 8.0 * u;
    if let Some((name, idx)) = m.sender {
        d.text(
            Weight::Medium,
            small,
            cx,
            cy + 14.0 * u,
            name,
            name_color(sc, idx),
        );
        cy += 20.0 * u;
    }
    if let Some((n, s)) = &reply {
        let (line, nc, mc) = if m.out {
            (
                t.c("chat_outReplyLine"),
                t.c("chat_outReplyNameText"),
                t.c("chat_outReplyMessageText"),
            )
        } else {
            (
                t.c("chat_inReplyLine"),
                t.c("chat_inReplyNameText"),
                t.c("chat_inReplyMessageText"),
            )
        };
        d.round_rect(cx, cy + u, cx + 2.0 * u, cy + 33.0 * u, u, line);
        d.text(Weight::Medium, small, cx + 8.0 * u, cy + 13.0 * u, n, nc);
        d.text(Weight::Regular, small, cx + 8.0 * u, cy + 29.0 * u, s, mc);
        cy += 40.0 * u;
    }
    for l in &lines {
        for (s, style, off) in &l.segs {
            d.text(
                Weight::Regular,
                em,
                cx + off,
                cy + 15.0 * u,
                s,
                styles[*style],
            );
        }
        cy += LINE_H * u;
    }
    if let Some((site, title, dl)) = &prev {
        cy += 6.0 * u;
        let hh = (19.0 + 19.0 + dl.len() as f64 * LINE_H) * u;
        d.round_rect(cx, cy, cx + 2.0 * u, cy + hh, u, t.c("chat_inPreviewLine"));
        let tx = cx + 8.0 * u;
        d.text(
            Weight::Medium,
            small,
            tx,
            cy + 14.0 * u,
            site,
            t.c("chat_inSiteNameText"),
        );
        d.text(Weight::Medium, em, tx, cy + 34.0 * u, title, txt);
        let mut ly = cy + 38.0 * u;
        for l in dl {
            for (s, _, off) in &l.segs {
                d.text(Weight::Regular, em, tx + off, ly + 15.0 * u, s, txt);
            }
            ly += LINE_H * u;
        }
    }
    draw_time(d, sc, m.out, m.status, m.time, x1, y1);
    h
}

#[allow(clippy::too_many_arguments)]
fn voice(
    d: &mut D,
    sc: &Scene,
    out: bool,
    sender: Option<(&str, usize)>,
    avatar_spec: Option<(usize, &str)>,
    dur: &str,
    played: f64,
    time: &str,
    status: Status,
    y: f64,
) -> f64 {
    let u = sc.u;
    let t = sc.t;
    let w = 204.0 * u;
    let mut h = 8.0 + 44.0 + 14.0;
    if sender.is_some() {
        h += 20.0;
    }
    let h = h * u;
    let right_edge = (PHONE_W - 12.0) * u;
    let x0 = if out { right_edge - w } else { 54.0 * u };
    let (x1, y1) = (x0 + w, y + h);
    draw_bubble(d, sc, out, x0, y, x1, y1);
    if let Some((idx, ini)) = avatar_spec {
        avatar(d, sc, 27.0 * u, y1 - 21.0 * u, 42.0, idx, ini);
    }
    let mut top = y + 8.0 * u;
    if let Some((name, idx)) = sender {
        d.text(
            Weight::Medium,
            14.0 * u,
            x0 + PAD * u,
            top + 14.0 * u,
            name,
            name_color(sc, idx),
        );
        top += 20.0 * u;
    }
    let (loader, icon, seek, fill, dur_col) = if out {
        (
            t.c("chat_outLoader"),
            t.c_or("chat_outMediaIcon", Rgba([0xef, 0xff, 0xde, 255])),
            t.c("chat_outVoiceSeekbar"),
            t.c("chat_outVoiceSeekbarFill"),
            t.c("chat_outAudioDurationText"),
        )
    } else {
        (
            t.c("chat_inLoader"),
            t.c_or("chat_inMediaIcon", Rgba([255, 255, 255, 255])),
            t.c("chat_inVoiceSeekbar"),
            t.c("chat_inVoiceSeekbarFill"),
            t.c("chat_inAudioDurationText"),
        )
    };
    let (ccx, ccy) = (x0 + (PAD + 22.0) * u, top + 22.0 * u);
    d.circle(ccx, ccy, 22.0 * u, loader);
    d.icon(icons::PLAY, ccx + u, ccy, 30.0 * u, icon);
    let wx = ccx + 22.0 * u + 9.0 * u;
    let bars = 33usize;
    for i in 0..bars {
        let f = i as f64;
        let bh = (3.0 + 17.0 * ((f * 1.7).sin() * (f * 0.53).cos()).abs()) * u;
        let col = if (i as f64) < played * bars as f64 {
            fill
        } else {
            seek
        };
        let bx = wx + f * 3.0 * u;
        d.round_rect(
            bx,
            ccy - 5.0 * u - bh / 2.0,
            bx + 2.0 * u,
            ccy - 5.0 * u + bh / 2.0,
            u,
            col.at_least(200),
        );
    }
    d.text(Weight::Regular, 12.0 * u, wx, ccy + 17.0 * u, dur, dur_col);
    draw_time(d, sc, out, status, time, x1, y1);
    h
}

fn date_pill(d: &mut D, sc: &Scene, y: f64, label: &str) -> f64 {
    let u = sc.u;
    let em = 14.0 * u;
    let tw = measure(Weight::Medium, em, label);
    let (w, h) = (tw + 24.0 * u, 26.0 * u);
    let x0 = (sc.w() - w) / 2.0;
    d.round_rect(
        x0,
        y,
        x0 + w,
        y + h,
        h / 2.0,
        sc.t.c("chat_serviceBackground"),
    );
    d.text(
        Weight::Medium,
        em,
        x0 + 12.0 * u,
        y + 18.0 * u,
        label,
        sc.t.c("chat_serviceText"),
    );
    h
}

fn action_bar(d: &mut D, sc: &Scene) {
    let u = sc.u;
    let t = sc.t;
    let bar = t.c("actionBarDefault");
    d.rectf(0.0, 0.0, sc.w(), (STATUS_H + BAR_H) * u, bar);
    status_bar(d, sc, t.rgb("actionBarDefault"));
    let icon = t.c("actionBarDefaultIcon");
    let cy = (STATUS_H + BAR_H / 2.0) * u;
    d.icon(icons::ARROW_BACK, 28.0 * u, cy, 24.0 * u, icon);
    avatar(d, sc, 71.0 * u, cy, 42.0, 4, "TC");
    d.text(
        Weight::Medium,
        18.0 * u,
        102.0 * u,
        cy - 4.0 * u,
        "Themezzz Club",
        t.c("actionBarDefaultTitle"),
    );
    d.text(
        Weight::Regular,
        14.0 * u,
        102.0 * u,
        cy + 14.0 * u,
        "5 members, 2 online",
        t.c("actionBarDefaultSubtitle"),
    );
    d.icon(icons::CALL, (PHONE_W - 64.0) * u, cy, 24.0 * u, icon);
    d.icon(icons::MORE_VERT, (PHONE_W - 28.0) * u, cy, 24.0 * u, icon);
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
    d.rectf(0.0, top - u, sc.w(), top, t.c("chat_messagePanelShadow"));
    let cy = top + 24.0 * u;
    let icons_col = t.c("chat_messagePanelIcons").at_least(200);
    d.icon(icons::EMOJI, 28.0 * u, cy, 26.0 * u, icons_col);
    d.text(
        Weight::Regular,
        18.0 * u,
        54.0 * u,
        cy + 6.0 * u,
        "Message",
        t.c("chat_messagePanelHint").at_least(200),
    );
    d.icon(icons::ATTACH, (PHONE_W - 74.0) * u, cy, 26.0 * u, icons_col);
    d.icon(icons::MIC, (PHONE_W - 30.0) * u, cy, 26.0 * u, icons_col);
    // gesture-navigation pill
    let (pw, py) = (110.0 * u, sc.h() - 8.0 * u);
    d.round_rect(
        (sc.w() - pw) / 2.0,
        py - 2.0 * u,
        (sc.w() + pw) / 2.0,
        py + 2.0 * u,
        2.0 * u,
        icons_col.faded(0.6),
    );
}

fn go_down(d: &mut D, sc: &Scene) {
    let u = sc.u;
    let t = sc.t;
    let (cx, cy, r) = (
        (PHONE_W - 34.0) * u,
        (PHONE_H - 16.0 - 48.0 - 34.0) * u,
        20.0 * u,
    );
    d.circle(
        cx,
        cy + 1.5 * u,
        r + u,
        t.c("chat_goDownButtonShadow").faded(0.5),
    );
    d.circle(cx, cy, r, t.c("chat_goDownButton"));
    d.icon(
        icons::CHEVRON_DOWN,
        cx,
        cy,
        26.0 * u,
        t.c("chat_messagePanelIcons"),
    );
    let bw = 22.0 * u;
    let by = cy - r - 8.0 * u;
    d.round_rect(
        cx - bw / 2.0,
        by,
        cx + bw / 2.0,
        by + 22.0 * u,
        11.0 * u,
        t.c("chat_goDownButtonCounterBackground"),
    );
    let white = t.c_or("chat_goDownButtonCounter", Rgba([255, 255, 255, 255]));
    let w = measure(Weight::Medium, 12.0 * u, "3");
    d.text(
        Weight::Medium,
        12.0 * u,
        cx - w / 2.0,
        by + 15.5 * u,
        "3",
        white,
    );
}

/// Paint the whole chat screen onto `img` (wallpaper already in place).
pub(crate) fn draw(img: &mut RgbaImage, sc: &Scene) {
    let u = sc.u;
    let mut d = D { img };
    action_bar(&mut d, sc);

    let mut y = (STATUS_H + BAR_H + 8.0) * u;
    let gap = 5.0 * u;
    y += date_pill(&mut d, sc, y, "November 22") + 8.0 * u;

    y += message(
        &mut d,
        sc,
        &Msg {
            out: false,
            sender: Some(("Nigar", 2)),
            avatar: Some((2, "N")),
            reply: None,
            runs: &[
                ("Check out the new theme build ", 0),
                ("t.me/addtheme/ocean", 1),
            ],
            preview: None,
            time: "10:21",
            status: Status::None,
        },
        y,
    ) + gap;
    y += message(
        &mut d,
        sc,
        &Msg {
            out: true,
            sender: None,
            avatar: None,
            reply: Some(("Nigar", "Shared the palette source")),
            runs: &[("Looks great, the colors match the photo!", 0)],
            preview: None,
            time: "10:22",
            status: Status::Read,
        },
        y,
    ) + gap;
    y += voice(
        &mut d,
        sc,
        false,
        Some(("Rauf", 3)),
        Some((3, "R")),
        "0:23",
        0.45,
        "10:24",
        Status::None,
        y,
    ) + gap;
    y += voice(
        &mut d,
        sc,
        true,
        None,
        None,
        "0:11",
        0.0,
        "10:25",
        Status::Sent,
        y,
    ) + gap;
    y += message(
        &mut d,
        sc,
        &Msg {
            out: false,
            sender: Some(("Aylin", 0)),
            avatar: Some((0, "A")),
            reply: None,
            runs: &[("Try this one ", 0), ("themezzz.app", 1)],
            preview: Some((
                "Themezzz",
                "Theme creator bot",
                "Turn any photo into a Telegram theme.",
            )),
            time: "10:26",
            status: Status::None,
        },
        y,
    );
    debug_assert!(y < (PHONE_H - 64.0) * u, "chat content overflows the panel");

    go_down(&mut d, sc);
    input_panel(&mut d, sc);
}
