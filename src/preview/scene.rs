//! Shared layout context and chrome (status bar) for both phones.

use super::draw::{C, D};
use super::icons;
use super::text::Weight;
use super::theme::ThemeView;
use crate::colors::{luminance, mix, Rgb};

/// Logical phone size in dp (Android density-independent pixels).
pub(crate) const PHONE_W: f64 = 360.0;
pub(crate) const PHONE_H: f64 = 746.0;
pub(crate) const STATUS_H: f64 = 24.0;
pub(crate) const BAR_H: f64 = 56.0;

/// Pixel scale and theme for one phone.
pub(crate) struct Scene<'a> {
    pub(crate) t: &'a ThemeView,
    /// Pixels per dp (supersample included).
    pub(crate) u: f64,
}

impl Scene<'_> {
    pub(crate) fn dp(&self, v: f64) -> f64 {
        v * self.u
    }
    pub(crate) fn w(&self) -> f64 {
        PHONE_W * self.u
    }
    pub(crate) fn h(&self) -> f64 {
        PHONE_H * self.u
    }
}

/// Telegram default avatar palette: (key suffix, top colour, bottom colour).
/// Values from `ThemeColors.java` (`avatar_background*`, `avatar_background2*`).
pub(crate) const AVATARS: [(&str, [u8; 3], [u8; 3]); 7] = [
    ("Red", [0xFF, 0x84, 0x5E], [0xD4, 0x52, 0x46]),
    ("Orange", [0xFE, 0xBB, 0x5B], [0xF6, 0x81, 0x36]),
    ("Violet", [0xB6, 0x94, 0xF9], [0x6C, 0x61, 0xDF]),
    ("Green", [0x9A, 0xD1, 0x64], [0x46, 0xBA, 0x43]),
    ("Cyan", [0x5B, 0xCB, 0xE3], [0x35, 0x9A, 0xD4]),
    ("Blue", [0x5C, 0xAF, 0xFA], [0x40, 0x8A, 0xCF]),
    ("Pink", [0xFF, 0x8A, 0xAC], [0xD9, 0x55, 0x74]),
];

fn rgb3(c: [u8; 3]) -> Rgb {
    Rgb::new(c[0], c[1], c[2])
}

/// Draw a gradient avatar with centred initials (or an icon path).
pub(crate) fn avatar(
    d: &mut D,
    sc: &Scene,
    cx: f64,
    cy: f64,
    size_dp: f64,
    idx: usize,
    initials: &str,
) {
    let (_, top, bottom) = AVATARS[idx % AVATARS.len()];
    let r = sc.dp(size_dp) / 2.0;
    d.circle_vgrad(cx, cy, r, rgb3(top), rgb3(bottom), 255);
    let em = sc.dp(size_dp * 0.375);
    let col = sc.t.c("avatar_text");
    let w = super::text::measure(Weight::Medium, em, initials);
    d.text(
        Weight::Medium,
        em,
        cx - w / 2.0,
        cy + em * 0.36,
        initials,
        col,
    );
}

/// Name colour used for the sender line of a group message.
pub(crate) fn name_color(sc: &Scene, idx: usize) -> C {
    sc.t.c(&format!(
        "avatar_nameInMessage{}",
        AVATARS[idx % AVATARS.len()].0
    ))
}

/// Android status bar: clock on the left, camera cut-out in the middle and
/// signal / wifi / battery on the right. Icon tint follows the bar colour.
pub(crate) fn status_bar(d: &mut D, sc: &Scene, bar_rgb: Rgb) {
    let dark_bar = luminance(bar_rgb) < 0.5;
    let ink = if dark_bar {
        Rgb::new(255, 255, 255)
    } else {
        Rgb::new(20, 20, 20)
    };
    let col: C = (mix(ink, bar_rgb, 0.08), 235u8).into();
    let w = sc.w();
    d.text(
        Weight::Medium,
        sc.dp(13.0),
        sc.dp(18.0),
        sc.dp(17.0),
        "9:41",
        col,
    );
    d.circle(
        w / 2.0,
        sc.dp(12.0),
        sc.dp(5.0),
        (Rgb::new(0, 0, 0), 215u8).into(),
    );
    d.icon(
        icons::SIGNAL,
        w - sc.dp(60.0),
        sc.dp(12.5),
        sc.dp(13.0),
        col,
    );
    d.icon(icons::WIFI, w - sc.dp(43.0), sc.dp(12.5), sc.dp(13.0), col);
    d.icon(
        icons::BATTERY,
        w - sc.dp(25.0),
        sc.dp(12.0),
        sc.dp(15.0),
        col,
    );
}
