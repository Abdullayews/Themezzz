//! Shared layout context for the 800×800 dual-phone glass-mockup preview.
//!
//! The canvas reproduces the reference mockup 1:1 in logical "HTML pixels":
//! two 300×656 rounded phone cards (left = chat, right = chat list) floating
//! on a flat backdrop with drop shadows. All element coordinates in the scene
//! modules use this same space, so they can be transcribed straight from the
//! reference HTML. Scale `u` converts to real pixels.

use super::draw::{C, D};
use super::theme::ThemeView;

/// Full canvas, reference-mockup units ("m" = mockup pixel).
pub(crate) const CANVAS: f64 = 800.0;
/// Phone card size (300×656) and corner radius (34).
pub(crate) const PHONE_W: f64 = 300.0;
pub(crate) const PHONE_H: f64 = 656.0;
pub(crate) const PHONE_R: f64 = 34.0;
/// Left (chat) phone card origin.
pub(crate) const CHAT_X: f64 = 76.0;
pub(crate) const CHAT_Y: f64 = 60.0;
/// Right (list) phone card origin.
pub(crate) const LIST_X: f64 = 418.0;
pub(crate) const LIST_Y: f64 = 92.0;
/// Wallpaper inset of the chat screen (reference: inset(114px … 140px …)).
pub(crate) const CHAT_SCREEN_TOP: f64 = 114.0;
pub(crate) const CHAT_SCREEN_BOTTOM: f64 = 660.0;
/// List screen inset (reference: inset(92px 82px 52px 418px round 34px)).
#[allow(dead_code)] // documented layout constants for future list-screen work
pub(crate) const LIST_SCREEN_TOP: f64 = 92.0;
#[allow(dead_code)]
pub(crate) const LIST_SCREEN_BOTTOM: f64 = 748.0;

/// Scale and theme for the whole canvas.
pub(crate) struct Scene<'a> {
    pub(crate) t: &'a ThemeView,
    /// Pixels per mockup unit.
    pub(crate) u: f64,
}

impl Scene<'_> {
    #[allow(dead_code)]
    pub(crate) fn dp(&self, v: f64) -> f64 {
        v * self.u
    }
    #[allow(dead_code)]
    pub(crate) fn w(&self) -> f64 {
        CANVAS * self.u
    }
    #[allow(dead_code)]
    pub(crate) fn h(&self) -> f64 {
        CANVAS * self.u
    }
}

/// Rounded-rect helper in mockup units.
#[allow(clippy::too_many_arguments)] // mockup box + radius + colour reads best flat
pub(crate) fn card(d: &mut D, sc: &Scene, x: f64, y: f64, w: f64, h: f64, r: f64, col: C) {
    d.round_rect(
        x * sc.u,
        y * sc.u,
        (x + w) * sc.u,
        (y + h) * sc.u,
        r * sc.u,
        col,
    );
}

/// Circle helper in mockup units.
pub(crate) fn dot(d: &mut D, sc: &Scene, cx: f64, cy: f64, r: f64, col: C) {
    d.circle(cx * sc.u, cy * sc.u, r * sc.u, col);
}

/// Pill bar helper in mockup units (fully rounded).
pub(crate) fn pill(d: &mut D, sc: &Scene, x: f64, y: f64, w: f64, h: f64, col: C) {
    let r = h / 2.0;
    d.round_rect(
        x * sc.u,
        y * sc.u,
        (x + w) * sc.u,
        (y + h) * sc.u,
        r * sc.u,
        col,
    );
}
