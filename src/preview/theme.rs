//! Read-only view of the compiled `.attheme`: the preview is painted from the
//! very same key → colour table that ends up in the file, so what the user
//! sees is what Telegram will show (alpha included).

use std::cell::RefCell;
use std::collections::{BTreeSet, HashMap};

use super::draw::C;
use crate::colors::Rgb;
use image::Rgba;

pub(crate) struct ThemeView {
    map: HashMap<String, Rgba<u8>>,
    used: RefCell<BTreeSet<String>>,
    missing: RefCell<BTreeSet<String>>,
}

impl ThemeView {
    /// Parse `.attheme` bytes (`key=int` lines, optional `WPS` wallpaper blob).
    pub(crate) fn parse(bytes: &[u8]) -> Self {
        let head_end = bytes
            .windows(5)
            .position(|w| w == b"\nWPS\n")
            .unwrap_or(bytes.len());
        let text = String::from_utf8_lossy(&bytes[..head_end]);
        let mut map = HashMap::new();
        for line in text.lines() {
            let Some((k, v)) = line.split_once('=') else {
                continue;
            };
            let Ok(n) = v.trim().parse::<i64>() else {
                continue;
            };
            let u = (n & 0xffff_ffff) as u32;
            map.insert(
                k.trim().to_string(),
                Rgba([(u >> 16) as u8, (u >> 8) as u8, u as u8, (u >> 24) as u8]),
            );
        }
        Self {
            map,
            used: RefCell::default(),
            missing: RefCell::default(),
        }
    }

    fn lookup(&self, k: &str) -> Rgba<u8> {
        self.used.borrow_mut().insert(k.to_string());
        match self.map.get(k) {
            Some(c) => *c,
            None => {
                self.missing.borrow_mut().insert(k.to_string());
                // loud placeholder so a missing key is obvious in review
                Rgba([255, 0, 255, 255])
            }
        }
    }

    /// Colour with the theme's own alpha.
    pub(crate) fn c(&self, k: &str) -> C {
        C(self.lookup(k))
    }

    /// Colour forced fully opaque.
    pub(crate) fn solid(&self, k: &str) -> C {
        let mut p = self.lookup(k);
        p[3] = 255;
        C(p)
    }

    /// Colour of `k`, or Telegram's built-in default when the theme file
    /// leaves the key out (as `ThemeColors.java` does for these keys).
    pub(crate) fn c_or(&self, k: &str, default: Rgba<u8>) -> C {
        self.used.borrow_mut().insert(k.to_string());
        C(self.map.get(k).copied().unwrap_or(default))
    }

    pub(crate) fn rgb(&self, k: &str) -> Rgb {
        let p = self.lookup(k);
        Rgb::new(p[0], p[1], p[2])
    }

    /// Keys the renderer asked for that the theme does not define.
    pub(crate) fn missing(&self) -> Vec<String> {
        self.missing.borrow().iter().cloned().collect()
    }

    /// Number of distinct theme keys the renderer used.
    pub(crate) fn used_count(&self) -> usize {
        self.used.borrow().len()
    }

    pub(crate) fn total(&self) -> usize {
        self.map.len()
    }
}
