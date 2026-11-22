//! Right phone: Telegram for Android chat list painted from theme keys.

use image::{Rgba, RgbaImage};

use super::draw::{C, D};
use super::icons;
use super::scene::{avatar, status_bar, Scene, BAR_H, PHONE_H, PHONE_W, STATUS_H};
use super::text::{ellipsize, measure, Weight};

#[derive(Clone, Copy, PartialEq)]
enum Tail {
    None,
    Unread(u32),
    UnreadMuted(u32),
    Mention(u32),
    Pinned,
    Sent,
    Read,
    Error,
}

enum Preview<'a> {
    Plain(&'a str),
    /// (label colour key, label, rest)
    Labeled(&'static str, &'a str, &'a str),
    Typing,
}

struct Row<'a> {
    name: &'a str,
    avatar: usize,
    initials: &'a str,
    preview: Preview<'a>,
    time: &'a str,
    tail: Tail,
    muted: bool,
    verified: bool,
    online: bool,
}

const ROW_H: f64 = 70.0;

fn badge(d: &mut D, sc: &Scene, right: f64, top: f64, n: &str, bg: C, fg: C) -> f64 {
    let u = sc.u;
    let em = 12.0 * u;
    let tw = measure(Weight::Medium, em, n);
    let w = (tw + 12.7 * u).max(22.0 * u);
    let x0 = right - w;
    d.round_rect(x0, top, right, top + 22.0 * u, 11.0 * u, bg);
    d.text(
        Weight::Medium,
        em,
        x0 + (w - tw) / 2.0,
        top + 15.5 * u,
        n,
        fg,
    );
    w
}

fn row(d: &mut D, sc: &Scene, r: &Row, y: f64) {
    let u = sc.u;
    let t = sc.t;
    let w = sc.w();
    // avatar
    let (acx, acy) = (39.0 * u, y + 37.0 * u);
    if r.initials == "#bookmark" {
        d.circle_vgrad(
            acx,
            acy,
            28.0 * u,
            t.rgb("avatar_backgroundSaved"),
            t.rgb("avatar_backgroundSaved"),
            255,
        );
        d.icon(icons::BOOKMARK, acx, acy, 28.0 * u, t.c("avatar_text"));
    } else if r.initials == "#telegram" {
        avatar(d, sc, acx, acy, 56.0, 5, "");
        d.icon(
            icons::SEND_PLANE,
            acx - u,
            acy,
            30.0 * u,
            t.c("avatar_text"),
        );
    } else {
        avatar(d, sc, acx, acy, 56.0, r.avatar, r.initials);
    }
    if r.online {
        let (ox, oy) = (acx + 20.0 * u, acy + 20.0 * u);
        d.circle(ox, oy, 7.5 * u, t.solid("windowBackgroundWhite"));
        d.circle(
            ox,
            oy,
            5.5 * u,
            t.c_or("chats_onlineCircle", Rgba([0x4b, 0xcb, 0x1c, 255])),
        );
    }

    let left = 72.0 * u;
    // time + status (right side, top line)
    let tcol = t.c("chats_date").at_least(200);
    let time_w = measure(Weight::Regular, 12.0 * u, r.time);
    let tx = w - 15.0 * u - time_w;
    d.text(Weight::Regular, 12.0 * u, tx, y + 27.0 * u, r.time, tcol);
    let mut right_name = tx - 6.0 * u;
    match r.tail {
        Tail::Read => {
            d.icon(
                icons::DONE_ALL,
                tx - 12.0 * u,
                y + 22.5 * u,
                17.0 * u,
                t.c("chats_sentReadCheck"),
            );
            right_name = tx - 24.0 * u;
        }
        Tail::Sent => {
            d.icon(
                icons::CHECK,
                tx - 10.0 * u,
                y + 22.5 * u,
                15.0 * u,
                t.c("chats_sentCheck"),
            );
            right_name = tx - 21.0 * u;
        }
        _ => {}
    }

    // name (+ verified / muted adornments)
    let nem = 17.0 * u;
    let mut adorn = 0.0;
    if r.verified {
        adorn += 22.0 * u;
    }
    if r.muted {
        adorn += 20.0 * u;
    }
    let name = ellipsize(Weight::Medium, nem, right_name - left - adorn, r.name);
    let nw = measure(Weight::Medium, nem, &name);
    d.text(
        Weight::Medium,
        nem,
        left,
        y + 30.0 * u,
        &name,
        t.c("chats_name"),
    );
    let mut ax = left + nw + 6.0 * u;
    if r.verified {
        d.circle(
            ax + 8.0 * u,
            y + 24.0 * u,
            8.0 * u,
            t.c("chats_verifiedBackground"),
        );
        d.icon(
            icons::CHECK,
            ax + 8.0 * u,
            y + 24.0 * u,
            12.0 * u,
            t.c("chats_verifiedCheck"),
        );
        ax += 20.0 * u;
    }
    if r.muted {
        d.icon(
            icons::VOLUME_OFF,
            ax + 7.0 * u,
            y + 24.0 * u,
            15.0 * u,
            t.c("chats_muteIcon"),
        );
    }

    // tail badge area (bottom right)
    let right_edge = w - 16.0 * u;
    let top = y + 38.0 * u;
    let mut reserved = 0.0;
    match r.tail {
        Tail::Unread(n) => {
            let bw = badge(
                d,
                sc,
                right_edge,
                top,
                &n.to_string(),
                t.c("chats_unreadCounter"),
                t.c("chats_unreadCounterText"),
            );
            reserved = bw + 12.0 * u;
        }
        Tail::UnreadMuted(n) => {
            let bw = badge(
                d,
                sc,
                right_edge,
                top,
                &n.to_string(),
                t.c("chats_unreadCounterMuted"),
                t.c("chats_unreadCounterMutedText"),
            );
            reserved = bw + 12.0 * u;
        }
        Tail::Mention(n) => {
            let bw = badge(
                d,
                sc,
                right_edge,
                top,
                &n.to_string(),
                t.c("chats_unreadCounter"),
                t.c("chats_unreadCounterText"),
            );
            let mx = right_edge - bw - 6.0 * u - 11.0 * u;
            d.circle(mx, top + 11.0 * u, 11.0 * u, t.c("chats_unreadCounter"));
            d.icon(
                icons::AT,
                mx,
                top + 11.0 * u,
                14.0 * u,
                t.c("chats_unreadCounterText"),
            );
            reserved = bw + 6.0 * u + 22.0 * u + 8.0 * u;
        }
        Tail::Pinned => {
            d.icon(
                icons::PUSH_PIN,
                right_edge - 9.0 * u,
                top + 12.0 * u,
                20.0 * u,
                t.c("chats_pinnedIcon"),
            );
            reserved = 28.0 * u;
        }
        Tail::Error => {
            d.circle(
                right_edge - 13.0 * u,
                top + 11.0 * u,
                13.0 * u,
                t.c("chats_sentError"),
            );
            let ex = right_edge - 13.0 * u;
            // bolder exclamation so the badge reads as an error, not an empty circle
            d.round_rect(
                ex - 1.8 * u,
                top + 4.0 * u,
                ex + 1.8 * u,
                top + 14.0 * u,
                1.8 * u,
                t.c("chats_sentErrorIcon"),
            );
            d.circle(ex, top + 17.5 * u, 1.8 * u, t.c("chats_sentErrorIcon"));
            reserved = 36.0 * u;
        }
        _ => {}
    }

    // message line
    let mem = 16.0 * u;
    let base = y + 55.0 * u;
    let max = right_edge - left - reserved;
    let mut x = left;
    match &r.preview {
        Preview::Plain(s) => {
            let s = ellipsize(Weight::Regular, mem, max, s);
            d.text(Weight::Regular, mem, x, base, &s, t.c("chats_message"));
        }
        Preview::Labeled(key, label, rest) => {
            let lw = d.text(Weight::Regular, mem, x, base, label, t.c(key));
            x += lw;
            let s = ellipsize(Weight::Regular, mem, max - lw, rest);
            let col = if *key == "chats_draft" || *key == "chats_nameMessage" {
                t.c("chats_message")
            } else {
                t.c("chats_attachMessage")
            };
            d.text(Weight::Regular, mem, x, base, &s, col);
        }
        Preview::Typing => {
            d.text(
                Weight::Regular,
                mem,
                x,
                base,
                "typing\u{2026}",
                t.c("chats_actionMessage"),
            );
        }
    }
    // divider
    d.rectf(left, y + ROW_H * u - u, w, y + ROW_H * u, t.c("divider"));
}

pub(crate) fn draw(img: &mut RgbaImage, sc: &Scene) {
    let u = sc.u;
    let t = sc.t;
    let mut d = D { img };
    d.rectf(0.0, 0.0, sc.w(), sc.h(), t.solid("windowBackgroundWhite"));

    let rows = [
        Row {
            name: "Saved Messages",
            avatar: 5,
            initials: "#bookmark",
            preview: Preview::Labeled("chats_attachMessage", "Photo", ""),
            time: "Mon",
            tail: Tail::Pinned,
            muted: false,
            verified: false,
            online: false,
        },
        Row {
            name: "Nigar",
            avatar: 2,
            initials: "N",
            preview: Preview::Typing,
            time: "10:25",
            tail: Tail::Unread(2),
            muted: false,
            verified: false,
            online: true,
        },
        Row {
            name: "Themezzz Club",
            avatar: 4,
            initials: "TC",
            preview: Preview::Labeled("chats_nameMessage", "Rauf: ", "Voice message"),
            time: "10:24",
            tail: Tail::Mention(5),
            muted: false,
            verified: false,
            online: false,
        },
        Row {
            name: "Design Team",
            avatar: 3,
            initials: "DT",
            preview: Preview::Labeled("chats_nameMessage", "Aylin: ", "The new icons are ready"),
            time: "10:12",
            tail: Tail::UnreadMuted(12),
            muted: true,
            verified: false,
            online: false,
        },
        Row {
            name: "Telegram",
            avatar: 5,
            initials: "#telegram",
            preview: Preview::Plain("Login code: 48217. Do not give it"),
            time: "9:50",
            tail: Tail::Unread(1),
            muted: false,
            verified: true,
            online: false,
        },
        Row {
            name: "Aysel",
            avatar: 6,
            initials: "A",
            preview: Preview::Labeled("chats_nameMessage", "You: ", "See you tomorrow!"),
            time: "9:41",
            tail: Tail::Read,
            muted: false,
            verified: false,
            online: false,
        },
        Row {
            name: "Mom",
            avatar: 1,
            initials: "M",
            preview: Preview::Labeled("chats_draft", "Draft: ", "I'll call you later"),
            time: "Tue",
            tail: Tail::None,
            muted: false,
            verified: false,
            online: false,
        },
        Row {
            name: "Rauf",
            avatar: 0,
            initials: "R",
            preview: Preview::Plain("Message was not delivered"),
            time: "Tue",
            tail: Tail::Error,
            muted: false,
            verified: false,
            online: false,
        },
        Row {
            name: "Orkhan",
            avatar: 5,
            initials: "O",
            preview: Preview::Labeled("chats_nameMessage", "You: ", "Done"),
            time: "Mon",
            tail: Tail::Sent,
            muted: false,
            verified: false,
            online: false,
        },
    ];
    let top = (STATUS_H + BAR_H) * u;
    // 9 rows would reach 710dp and collide with the FAB (610..666dp);
    // 8 rows end at 640dp, leaving clear space for the FAB and margins.
    rows.iter().take(8).enumerate().for_each(|(i, r)| {
        row(&mut d, sc, r, top + i as f64 * ROW_H * u);
    });

    // action bar
    d.rectf(0.0, 0.0, sc.w(), top, t.c("actionBarDefault"));
    status_bar(&mut d, sc, t.rgb("actionBarDefault"));
    let icon = t.c("actionBarDefaultIcon");
    let cy = (STATUS_H + BAR_H / 2.0) * u;
    d.icon(icons::MENU, 28.0 * u, cy, 24.0 * u, icon);
    d.text(
        Weight::Medium,
        20.0 * u,
        72.0 * u,
        cy + 7.0 * u,
        "Telegram",
        t.c("actionBarDefaultTitle"),
    );
    d.icon(icons::SEARCH, (PHONE_W - 32.0) * u, cy, 24.0 * u, icon);

    // floating action button — anchored above the bottom margin,
    // below the last row (640dp), so no row content is ever occluded
    let (fx, fy) = ((PHONE_W - 44.0) * u, (PHONE_H - 16.0 - 28.0) * u);
    d.circle(
        fx,
        fy + 3.0 * u,
        28.0 * u,
        (
            crate::colors::mix(
                t.rgb("chats_actionBackground"),
                crate::colors::Rgb::new(0, 0, 0),
                0.6,
            ),
            70u8,
        )
            .into(),
    );
    d.circle(fx, fy, 28.0 * u, t.solid("chats_actionBackground"));
    d.icon(icons::EDIT, fx, fy, 24.0 * u, t.c("chats_actionIcon"));
    let (gw, gy) = (110.0 * u, sc.h() - 8.0 * u);
    d.round_rect(
        (sc.w() - gw) / 2.0,
        gy - 2.0 * u,
        (sc.w() + gw) / 2.0,
        gy + 2.0 * u,
        2.0 * u,
        t.c("actionBarDefaultIcon").faded(0.45),
    );
}
