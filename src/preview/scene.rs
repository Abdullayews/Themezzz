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
