//! Editable sections, labels and per-chat editor state.

use crate::colors::{HashMapSections, Rgb, SectionChoice};
use teloxide::types::MessageId;

/// Section key + short button label (order matters: rows of 4).
pub const SECTIONS: [(&str, &str); 8] = [
    ("bg", "🖼 BG"),
    ("bar", "📊 Bar"),
    ("in", "📥 In"),
    ("out", "📤 Out"),
    ("text", "🐻‍❄️ Text"),
    ("accent", "🔵 Accent"),
    ("reply", "🏷 Reply"),
    ("wall", "🌇 Wall"),
];

/// Full display name of a section.
pub fn sec_full(k: &str) -> &'static str {
    match k {
        "bar" => "Top bar",
        "in" => "Incoming bubble",
        "out" => "Outgoing bubble",
        "text" => "Text",
        "accent" => "Accent",
        "reply" => "Reply (tag)",
        "wall" => "Wallpaper",
        _ => "Background",
    }
}

/// Short label lookup for captions.
pub fn sec_label(k: &str) -> &'static str {
    SECTIONS
        .iter()
        .find(|(key, _)| *key == k)
        .map(|&(_, l)| l)
        .unwrap_or("BG")
}

/// Sections with a color choice.
pub const COLOR_SECTIONS: [&str; 7] = ["bg", "bar", "in", "out", "text", "accent", "reply"];
/// Sections with a transparency slider. ⚠️ "bg" excluded on purpose:
/// transparent windows broke the drawer and the forward screen.
pub const ALPHA_SECTIONS: [&str; 6] = ["bar", "in", "out", "text", "accent", "reply"];

pub const WELCOME: &str = "🎨 <b>Theme creation</b>\n\n\
Create a theme for yourself — or pair two matching themes for you and your \
favorite person 💜\n\n\
Everything happens in the web editor: pick an image, choose colors, \
transparency and the wallpaper — live preview on every step.\n\n\
📸 You can also just send me an image right here — I'll grab its colors \
and open the editor with them.";

/// Everything one chat's editor needs (mirrors bot.py default_state).
#[derive(Clone, Debug)]
pub struct EditorState {
    pub palette: Vec<String>,
    pub mode: String,
    pub sections: HashMapSections,
    pub active: String,
    pub wall_mode: String,
    pub wall_idx: i32,
    pub wall_custom: Option<Rgb>,
    pub wall_blur: bool,
    pub wall_bytes: Option<Vec<u8>>,
    pub msg_id: Option<MessageId>,
    pub mode_img: String,
}

impl EditorState {
    /// Color choice of the currently active section (or the wallpaper).
    pub fn active_color_state(&self) -> SectionChoice {
        if self.active == "wall" {
            SectionChoice {
                idx: self.wall_idx,
                alpha: 0,
                custom: self.wall_custom,
            }
        } else {
            self.sections
                .get(self.active.as_str())
                .cloned()
                .unwrap_or_default()
        }
    }

    /// ALPHA_SECTIONS → alpha percentages, for rendering and .attheme build.
    pub fn alpha_map(&self) -> std::collections::BTreeMap<String, u32> {
        ALPHA_SECTIONS
            .iter()
            .map(|k| {
                (
                    k.to_string(),
                    self.sections.get(*k).map(|s| s.alpha).unwrap_or(0),
                )
            })
            .collect()
    }
}
