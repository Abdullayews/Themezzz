//! Theme store: builds .attheme + preview from a Mini App request and
//! keeps finished themes in memory for the bot to fetch and send.

use crate::colors::{
    build_attheme, hex_to_rgb, resolve_theme, resolve_wall, rgb_to_hex, HashMapSections, Rgb,
    SectionChoice,
};
use crate::state::{EditorState, COLOR_SECTIONS};
use std::collections::BTreeMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// One finished theme kept for the bot to send.
#[derive(Clone)]
pub struct ThemeEntry {
    pub filename: String,
    pub preview: Vec<u8>,
    pub thumb: Vec<u8>,
    pub attheme: Vec<u8>,
}

/// Shared state for the axum handlers: finished themes by id.
#[derive(Default)]
pub struct WebState {
    themes: Mutex<std::collections::HashMap<String, ThemeEntry>>,
    order: Mutex<Vec<(String, Instant)>>,
}

const MAX_THEMES: usize = 200;
const THEME_TTL: Duration = Duration::from_secs(60 * 60 * 2);

impl WebState {
    /// Store a finished theme (also remembers insertion order for GC).
    pub fn put(
        &self,
        id: String,
        filename: String,
        preview: Vec<u8>,
        thumb: Vec<u8>,
        attheme: Vec<u8>,
    ) {
        {
            let mut m = self.themes.lock().unwrap();
            if m.len() >= MAX_THEMES {
                // drop the oldest entry by insertion order
                let mut o = self.order.lock().unwrap();
                while o.len() > MAX_THEMES.saturating_sub(1) {
                    let (old, _) = o.remove(0);
                    m.remove(&old);
                }
            }
            m.insert(
                id.clone(),
                ThemeEntry {
                    filename,
                    preview,
                    thumb,
                    attheme,
                },
            );
        }
        self.order.lock().unwrap().push((id, Instant::now()));
        self.gc();
    }

    /// Fetch one theme.
    pub fn get(&self, id: &str) -> Option<ThemeEntry> {
        self.gc();
        self.themes.lock().unwrap().get(id).cloned()
    }

    fn gc(&self) {
        let now = Instant::now();
        let expired: Vec<String> = {
            let o = self.order.lock().unwrap();
            o.iter()
                .filter(|(_, t)| now.duration_since(*t) > THEME_TTL)
                .map(|(id, _)| id.clone())
                .collect()
        };
        if !expired.is_empty() {
            let mut o = self.order.lock().unwrap();
            o.retain(|(id, _)| !expired.contains(id));
            let mut m = self.themes.lock().unwrap();
            for id in &expired {
                m.remove(id);
            }
        }
    }

    /// Async wrapper: run the heavy build off the async runtime.
    /// Returns (id, filename, preview, thumb, attheme, accent_hex, a_in, a_out).
    pub async fn build(
        &self,
        st: EditorState,
    ) -> (String, String, Vec<u8>, Vec<u8>, Vec<u8>, String, u32, u32) {
        let out = tokio::task::spawn_blocking(move || Self::build_blocking(st)).await;
        match out {
            Ok((fname, preview, thumb, attheme, accent, a_in, a_out)) => {
                let id = new_id();
                (id, fname, preview, thumb, attheme, accent, a_in, a_out)
            }
            Err(_) => (
                new_id(),
                String::new(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
                String::new(),
                0,
                0,
            ),
        }
    }

    /// Build preview/thumb/attheme from an editor state (blocking).
    fn build_blocking(st: EditorState) -> (String, Vec<u8>, Vec<u8>, Vec<u8>, String, u32, u32) {
        let res = resolve_theme(&st.palette, &st.sections, &st.mode);
        let alphas = st.alpha_map();
        let wall = if st.wall_mode == "image" {
            st.wall_bytes.as_deref()
        } else {
            None
        };
        let wall_flat = if wall.is_none() {
            Some(resolve_wall(
                &st.palette,
                st.wall_idx,
                st.wall_custom,
                &st.mode,
            ))
        } else {
            None
        };
        let preview = crate::preview::render_preview(&crate::preview::PreviewArgs {
            colors: &res,
            alphas: &alphas,
            wall_bytes: wall,
            wall_flat,
            blur: st.wall_blur,
            screen: st.screen.as_str(),
        });
        let thumb = render_thumb(&res, &alphas, wall, wall_flat, st.wall_blur);
        let attheme = build_attheme(&res, &alphas, wall, wall_flat, st.wall_blur);
        let a_in = alphas.get("in").copied().unwrap_or(0);
        let a_out = alphas.get("out").copied().unwrap_or(0);
        let fname = format!(
            "Theme_{}_{}_{}.attheme",
            rgb_to_hex(res.accent).trim_start_matches('#'),
            a_in,
            a_out
        );
        let accent = rgb_to_hex(res.accent);
        (fname, preview, thumb, attheme, accent, a_in, a_out)
    }
}

fn new_id() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(1);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let t = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("{:x}{:06x}", t, n & 0xffffff)
}

/// Small square thumbnail for chat bubbles (160×160, two bubbles + text bars).
fn render_thumb(
    res: &crate::colors::Resolved,
    alphas: &BTreeMap<String, u32>,
    wall: Option<&[u8]>,
    wall_flat: Option<Rgb>,
    blur: bool,
) -> Vec<u8> {
    use image::{DynamicImage, Rgba, RgbaImage};
    let s = 2.0;
    let (w, h) = (320u32, 320u32);
    let mut img = RgbaImage::new(w, h);
    // bg
    let bgc = Rgba([res.bg.r, res.bg.g, res.bg.b, 255]);
    for p in img.pixels_mut() {
        *p = bgc;
    }
    // wallpaper
    if let Some(bytes) = wall {
        if let Ok(src) = image::load_from_memory(bytes) {
            let src = if blur {
                DynamicImage::ImageRgba8(crate::preview::blur_rgba(&src.to_rgba8(), 8.0 * s))
            } else {
                src
            };
            let cover = crate::preview::cover_rgba(&src, w, h);
            crate::preview::paste_rgba(&mut img, &cover, 0, 0);
        }
    } else if let Some(f) = wall_flat {
        for p in img.pixels_mut() {
            *p = Rgba([f.r, f.g, f.b, 255]);
        }
    }
    // bubbles
    let a_of = |k: &str| -> u8 {
        let pct = alphas.get(k).copied().unwrap_or(0).min(100);
        (255.0 * (1.0 - f64::from(pct) / 100.0)).round() as u8
    };
    let mut d = crate::preview::D { img: &mut img };
    d.round_rect(
        12.0 * s,
        60.0 * s,
        120.0 * s,
        116.0 * s,
        12.0 * s,
        (res.r#in, a_of("in")).into(),
    );
    d.round_rect(
        40.0 * s,
        118.0 * s,
        148.0 * s,
        150.0 * s,
        12.0 * s,
        (res.out, a_of("out")).into(),
    );
    d.rectf(
        24.0 * s,
        70.0 * s,
        100.0 * s,
        78.0 * s,
        (res.text, a_of("text")).into(),
    );
    d.rectf(
        52.0 * s,
        126.0 * s,
        130.0 * s,
        134.0 * s,
        (res.text, a_of("text")).into(),
    );
    let out = DynamicImage::ImageRgba8(img);
    let resized = out
        .resize_exact(160, 160, image::imageops::FilterType::CatmullRom)
        .to_rgb8();
    let mut buf = std::io::Cursor::new(Vec::new());
    resized.write_to(&mut buf, image::ImageFormat::Png).unwrap();
    buf.into_inner()
}

/// serde shape POSTed by the Mini App (camelCase like the JS side).
#[derive(serde::Deserialize)]
pub struct ThemeRequest {
    #[serde(default)]
    pub palette: Vec<String>,
    pub mode: String,
    #[serde(default)]
    pub sections: BTreeMap<String, NetSection>,
    #[serde(default)]
    pub active: String,
    #[serde(default)]
    pub wall_mode: String,
    #[serde(default)]
    pub wall_idx: i32,
    #[serde(default)]
    pub wall_custom: Option<String>,
    #[serde(default)]
    pub wall_blur: bool,
    #[serde(default)]
    pub wall_b64: Option<String>,
    /// which screen the preview shows: "chat" or "list"
    #[serde(default)]
    pub screen: Option<String>,
}

#[derive(serde::Deserialize, Clone)]
pub struct NetSection {
    #[serde(default)]
    pub idx: i32,
    #[serde(default)]
    pub alpha: u32,
    #[serde(default)]
    pub custom: Option<String>,
}

/// Convert a network request into the library editor state.
pub fn into_state(req: ThemeRequest) -> EditorState {
    let mut sections: HashMapSections = COLOR_SECTIONS
        .iter()
        .map(|k| (k.to_string(), SectionChoice::default()))
        .collect();
    for (k, v) in req.sections {
        let custom = v.custom.as_deref().and_then(hex_to_rgb);
        sections.insert(
            k,
            SectionChoice {
                idx: v.idx,
                alpha: v.alpha.min(100),
                custom,
            },
        );
    }
    let wall_custom = req.wall_custom.as_deref().and_then(hex_to_rgb);
    let wall_bytes = req
        .wall_b64
        .as_deref()
        .and_then(b64_decode)
        .and_then(|b| crate::colors::prepare_wallpaper(&b, 1080));
    EditorState {
        palette: req.palette,
        mode: req.mode,
        sections,
        active: req.active,
        wall_mode: if wall_bytes.is_some() {
            req.wall_mode
        } else if req.wall_mode == "image" {
            "flat".into()
        } else {
            req.wall_mode
        },
        wall_idx: req.wall_idx,
        wall_custom,
        wall_blur: req.wall_blur,
        wall_bytes,
        msg_id: None,
        mode_img: "photo".into(),
        screen: req.screen.unwrap_or_else(|| "chat".to_string()),
    }
}

pub fn b64_encode(data: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(data)
}

pub fn b64_decode(data: &str) -> Option<Vec<u8>> {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.decode(data).ok()
}
