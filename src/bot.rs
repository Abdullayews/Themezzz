//! Telegram bot: handlers, flows & inline keyboard UI (teloxide 0.13 port of bot.py).

use themezzz::colors::{
    build_attheme, extract_palette, hex_to_rgb, prepare_wallpaper, resolve_theme, resolve_wall,
    rgb_to_hex, HashMapSections, Rgb, Resolved, SectionChoice,
};
use themezzz::preview::{render_preview, PreviewArgs};
use themezzz::state::{sec_full, sec_label, EditorState, ALPHA_SECTIONS, COLOR_SECTIONS, SECTIONS, WELCOME};
use std::collections::{BTreeMap, HashMap};
use std::sync::Mutex;
use tokio::io::AsyncWriteExt;
use teloxide::prelude::*;
use teloxide::types::{
    InputFile, InlineKeyboardButton, InlineKeyboardButtonKind, InlineKeyboardMarkup,
    InputMedia, InputMediaPhoto, ParseMode,
};

/// One pending editor per chat (chat id → state). Replaced by new images.
/// `Arc` inside so the whole map can be injected into every handler (dptree
/// clones its dependencies).
#[derive(Clone, Default)]
pub struct Sessions(pub std::sync::Arc<Mutex<HashMap<ChatId, EditorState>>>);

impl std::ops::Deref for Sessions {
    type Target = Mutex<HashMap<ChatId, EditorState>>;
    fn deref(&self) -> &Self::Target { &self.0 }
}

pub fn build_payload(st: &EditorState) -> (Vec<u8>, String, InlineKeyboardMarkup) {
    let res = resolve_theme(&st.palette, &st.sections, &st.mode);
    let alphas = st.alpha_map();
    let wall = if st.wall_mode == "image" { st.wall_bytes.as_deref() } else { None };
    let wall_flat = if wall.is_none() {
        Some(resolve_wall(&st.palette, st.wall_idx, st.wall_custom, &st.mode))
    } else { None };
    let png = render_preview(&PreviewArgs {
        colors: &res, alphas: &alphas, wall_bytes: wall, wall_flat, blur: st.wall_blur,
    });
    (png, caption(st, &res), keyboard(st))
}

fn fmt_section(st: &EditorState, res: &Resolved, k: &str) -> String {
    if k == "wall" {
        if st.wall_mode == "image" && st.wall_bytes.is_some() {
            return format!("image{}", if st.wall_blur { " · blur" } else { "" });
        }
        return rgb_to_hex(resolve_wall(&st.palette, st.wall_idx, st.wall_custom, &st.mode));
    }
    let tr = match st.sections.get(k) {
        Some(sec) if ALPHA_SECTIONS.contains(&k) && sec.alpha > 0 => format!(" · {}%", sec.alpha),
        _ => String::new(),
    };
    rgb_to_hex(res_value(res, k)) + &tr
}

fn res_value(res: &Resolved, k: &str) -> Rgb {
    match k {
        "bar" => res.bar, "in" => res.r#in, "out" => res.out,
        "text" => res.text, "accent" => res.accent, "reply" => res.reply,
        _ => res.bg,
    }
}

fn caption(st: &EditorState, res: &Resolved) -> String {
    let mut lines = vec![
        "🎨 <b>Theme Editor</b>".to_string(),
        format!("🌗 Mode: <b>{}</b>", if st.mode == "dark" { "Dark" } else { "Light" }),
        "──────────────".to_string(),
    ];
    for (a, b) in [("bg", "bar"), ("in", "out"), ("text", "accent"), ("reply", "wall")] {
        lines.push(format!("{} {}   {} {}",
            sec_label(a), fmt_section(st, res, a), sec_label(b), fmt_section(st, res, b)));
    }
    lines.push("──────────────".to_string());
    lines.push(format!("▸ Editing: <b>{}</b>", sec_full(&st.active)));
    lines.push(format!("⚡ auto  {}", st.palette.iter().take(3).enumerate()
        .map(|(i, h)| format!("{} {}", i + 1, h)).collect::<Vec<_>>().join("  ")));
    lines.push(format!("         {}", st.palette.iter().skip(3).take(3).enumerate()
        .map(|(i, h)| format!("{} {}", i + 4, h)).collect::<Vec<_>>().join("  ")));
    lines.push("Type your own: <code>#34c7a4</code>".to_string());
    lines.join("\n")
}

fn btn(text: impl Into<String>, data: &str) -> InlineKeyboardButton {
    InlineKeyboardButton::new(text, InlineKeyboardButtonKind::CallbackData(data.to_string()))
}

fn keyboard(st: &EditorState) -> InlineKeyboardMarkup {
    let mut rows: Vec<Vec<InlineKeyboardButton>> = Vec::new();
    rows.push(vec![
        btn(format!("{} 🌙 Dark", if st.mode == "dark" { "●" } else { "○" }), "mode:dark"),
        btn(format!("{} ☀️ Light", if st.mode == "light" { "●" } else { "○" }), "mode:light"),
    ]);
    for chunk in SECTIONS.chunks(4) {
        rows.push(chunk.iter().map(|(k, lbl)| {
            btn(format!("{}{}", if st.active == *k { "● " } else { "" }, lbl), &format!("sec:{}", k))
        }).collect());
    }

    let active = st.active_color_state();
    if st.active == "wall" {
        rows.push(vec![
            btn(format!("{} 🖼 Image", if st.wall_mode == "image" && st.wall_bytes.is_some() { "●" } else { "○" }), "wp:image"),
            btn(format!("{} 🎨 Flat", if st.wall_mode == "flat" { "●" } else { "○" }), "wp:flat"),
        ]);
        if st.wall_bytes.is_some() {
            rows.push(vec![btn(format!("🔍 Blur: {}", if st.wall_blur { "On" } else { "Off" }), "wb:toggle")]);
        }
        if st.wall_mode == "flat" || st.wall_bytes.is_none() {
            let mut row = vec![btn(format!("⚡{}", if active.idx == -1 && active.custom.is_none() { "●" } else { "" }), "auto")];
            for i in 0..st.palette.len() {
                row.push(btn(if active.idx == i as i32 { "●".to_string() } else { format!("{}", i + 1) }, &format!("c:{}", i)));
            }
            rows.push(row);
        }
    } else {
        let mut row = vec![btn(format!("⚡{}", if active.idx == -1 && active.custom.is_none() { "●" } else { "" }), "auto")];
        for i in 0..st.palette.len() {
            row.push(btn(if active.idx == i as i32 { "●".to_string() } else { format!("{}", i + 1) }, &format!("c:{}", i)));
        }
        if active.custom.is_some() {
            row.push(btn("🎯●", "noop"));
        }
        rows.push(row);

        if ALPHA_SECTIONS.contains(&st.active.as_str()) {
            let a = st.sections.get(st.active.as_str()).map(|s| s.alpha).unwrap_or(0);
            rows.push(vec![
                btn("-10", "a:-10"), btn("-5", "a:-5"), btn(format!("🫧 {}%", a), "noop"),
                btn("+5", "a:5"), btn("+10", "a:10"),
            ]);
            rows.push([0u32, 25, 50, 75, 100].iter()
                .map(|v| btn(format!("{}%", v), &format!("s:{}", v))).collect());
        }
    }

    rows.push(vec![
        btn("✅ Create theme", "make"),
        btn(format!("🔄 Reset {}", sec_label(&st.active)), "reset"),
    ]);
    InlineKeyboardMarkup::new(rows)
}

// ---------- Editing actions (mirror bot.py handlers) ----------

fn apply_callback(st: &mut EditorState, data: &str) {
    if let Some(m) = data.strip_prefix("mode:") {
        st.mode = m.to_string();
    } else if let Some(k) = data.strip_prefix("sec:") {
        st.active = k.to_string();
    } else if data == "auto" {
        if st.active == "wall" {
            st.wall_idx = -1;
            st.wall_custom = None;
        } else if let Some(s) = st.sections.get_mut(st.active.as_str()) {
            s.idx = -1;
            s.custom = None;
        }
    } else if let Some(i) = data.strip_prefix("c:") {
        let idx: i32 = i.parse().unwrap_or(-1);
        if st.active == "wall" {
            st.wall_idx = idx;
            st.wall_custom = None;
        } else if let Some(s) = st.sections.get_mut(st.active.as_str()) {
            s.idx = idx;
            s.custom = None;
        }
    } else if let Some(d) = data.strip_prefix("a:") {
        if ALPHA_SECTIONS.contains(&st.active.as_str()) {
            let delta: i32 = d.parse().unwrap_or(0);
            if let Some(s) = st.sections.get_mut(st.active.as_str()) {
                s.alpha = (s.alpha as i32 + delta).clamp(0, 100) as u32;
            }
        }
    } else if let Some(v) = data.strip_prefix("s:") {
        if ALPHA_SECTIONS.contains(&st.active.as_str()) {
            let val: i32 = v.parse().unwrap_or(0);
            if let Some(s) = st.sections.get_mut(st.active.as_str()) {
                s.alpha = val.clamp(0, 100) as u32;
            }
        }
    } else if data == "wp:image" && st.wall_bytes.is_some() {
        st.wall_mode = "image".into();
    } else if data == "wp:flat" {
        st.wall_mode = "flat".into();
    } else if data == "wb:toggle" && st.wall_bytes.is_some() {
        st.wall_blur = !st.wall_blur;
    } else if data == "reset" {
        if st.active == "wall" {
            st.wall_mode = if st.wall_bytes.is_some() { "image".into() } else { "flat".into() };
            st.wall_idx = -1;
            st.wall_custom = None;
            st.wall_blur = false;
        } else if let Some(s) = st.sections.get_mut(st.active.as_str()) {
            *s = SectionChoice::default();
        }
    }
}

pub fn set_custom(st: &mut EditorState, hexcol: &str) {
    if let Some(c) = hex_to_rgb(hexcol) {
        if st.active == "wall" {
            st.wall_mode = "flat".into();
            st.wall_custom = Some(c);
            st.wall_idx = -1;
        } else if let Some(s) = st.sections.get_mut(st.active.as_str()) {
            s.custom = Some(c);
            s.idx = -1;
        }
    }
}

// ---------- Bot wiring ----------

fn sections_default() -> HashMapSections {
    COLOR_SECTIONS.iter().map(|k| (k.to_string(), SectionChoice::default())).collect()
}

pub async fn cmd_start(bot: Bot, msg: Message, sessions: &Sessions) -> ResponseResult<()> {
    sessions.0.lock().unwrap().remove(&msg.chat.id);
    bot.send_message(msg.chat.id, WELCOME)
        .parse_mode(ParseMode::Html)
        .await?;
    Ok(())
}

pub async fn on_image(bot: Bot, msg: Message, sessions: &Sessions) -> ResponseResult<()> {
    // Photo (largest size) or image document.
    let file_id = if let Some(photos) = msg.photo() {
        photos.last().map(|p| p.file.id.clone())
    } else if let Some(doc) = msg.document() {
        Some(doc.file.id.clone())
    } else {
        None
    };
    let Some(file_id) = file_id else {
        bot.send_message(msg.chat.id, "😕 Couldn't read that image. Try again (max 20 MB).").await?;
        return Ok(());
    };
    let raw = download_file(&bot, &file_id).await;
    if raw.is_empty() {
        bot.send_message(msg.chat.id, "😕 Couldn't download that image. Try again (max 20 MB).").await?;
        return Ok(());
    }

    // Heavy decode/quantize work off the async runtime.
    let (palette, wall) = tokio::task::spawn_blocking(move || {
        (extract_palette(&raw, 6), prepare_wallpaper(&raw, 1080))
    }).await.unwrap_or_else(|_| (Vec::new(), None));

    let mut st = EditorState {
        palette, mode: "dark".into(), sections: sections_default(),
        active: "bg".into(),
        wall_mode: if wall.is_some() { "image".into() } else { "flat".into() },
        wall_idx: -1, wall_custom: None, wall_blur: false,
        wall_bytes: wall, msg_id: None, mode_img: "photo".into(),
    };

    // Delete the old editor, then send a fresh one.
    let old_msg_id = sessions.0.lock().unwrap().get(&msg.chat.id).and_then(|s| s.msg_id);
    if let Some(mid) = old_msg_id {
        let _ = bot.delete_message(msg.chat.id, mid).await;
    }

    let (png, cap, kb) = build_payload(&st);
    let sent = match bot.send_photo(msg.chat.id, InputFile::memory(png))
        .caption(cap.clone()).parse_mode(ParseMode::Html)
        .reply_markup(kb.clone())
        .await
    {
        Ok(m) => { st.mode_img = "photo".into(); m }
        Err(e) => {
            tracing::error!("preview failed → text mode: {e}");
            let m = bot.send_message(msg.chat.id, cap)
                .reply_markup(kb.clone())
                .parse_mode(ParseMode::Html).await?;
            st.mode_img = "text".into();
            m
        }
    };
    st.msg_id = Some(sent.id);
    sessions.0.lock().unwrap().insert(msg.chat.id, st);
    Ok(())
}

/// Download a Telegram file into memory (empty Vec on failure).
async fn download_file(bot: &Bot, file_id: &str) -> Vec<u8> {
    use teloxide::net::Download;
    let Ok(f) = bot.get_file(file_id).await else { return Vec::new() };
    let mut buf: Vec<u8> = Vec::new();
    let mut writer = tokio::io::BufWriter::new(&mut buf);
    if bot.download_file(&f.path, &mut writer).await.is_err() {
        return Vec::new();
    }
    writer.flush().await.ok();
    buf
}

pub async fn on_text(bot: Bot, msg: Message, sessions: &Sessions) -> ResponseResult<()> {
    let text = msg.text().unwrap_or_default().trim().to_string();
    let has_session = sessions.0.lock().unwrap().contains_key(&msg.chat.id);
    if !has_session {
        bot.send_message(msg.chat.id, "📸 Send me an image to start!").await?;
        return Ok(());
    }
    let hex_clean = text.trim_start_matches('#').trim().to_lowercase();
    let is_hex = hex_clean.len() == 6 && hex_clean.chars().all(|c| c.is_ascii_hexdigit());
    if is_hex {
        {
            let mut guard = sessions.0.lock().unwrap();
            if let Some(st) = guard.get_mut(&msg.chat.id) {
                set_custom(st, &format!("#{}", hex_clean));
            }
        }
        refresh_editor(bot, msg.chat.id, sessions).await;
    } else {
        bot.send_message(msg.chat.id, "💡 Type a color like <code>#34c7a4</code> — it applies to the part you're editing.")
            .parse_mode(ParseMode::Html).await?;
    }
    Ok(())
}

pub async fn on_callback(bot: Bot, q: CallbackQuery, sessions: &Sessions) -> ResponseResult<()> {
    let Some(data) = q.data.clone() else {
        bot.answer_callback_query(&q.id).await?;
        return Ok(());
    };
    let chat_id = match q.message.clone().and_then(|m| m.regular_message().cloned()) {
        Some(m) => m.chat.id,
        None => {
            bot.answer_callback_query(&q.id).text("Old session — send a new image 📸").show_alert(true).await?;
            return Ok(());
        }
    };

    let has_session = sessions.0.lock().unwrap().contains_key(&chat_id);
    if !has_session {
        bot.answer_callback_query(&q.id).text("Old session — send a new image 📸").show_alert(true).await?;
        return Ok(());
    }

    if data == "make" {
        let st = sessions.0.lock().unwrap().get(&chat_id).cloned().unwrap();
        bot.answer_callback_query(&q.id).text("⏳ Creating theme...").await?;
        let res = resolve_theme(&st.palette, &st.sections, &st.mode);
        let alphas = st.alpha_map();
        let wall = if st.wall_mode == "image" { st.wall_bytes.clone() } else { None };
        let wall_flat = if wall.is_none() {
            Some(resolve_wall(&st.palette, st.wall_idx, st.wall_custom, &st.mode))
        } else { None };
        let theme = build_attheme(&res, &alphas, wall.as_deref(), wall_flat, st.wall_blur);
        let fname = format!("Theme_{}_{}_{}.attheme",
            rgb_to_hex(res.accent).trim_start_matches('#'),
            alphas.get("in").copied().unwrap_or(0), alphas.get("out").copied().unwrap_or(0));
        let caption = format!(
            "🎨 <b>Your theme is ready!</b>\nMode: {} · Accent {} · In {}% · Out {}%\n\n\
             Open the file — Telegram applies it instantly ✨",
            capitalize(&st.mode), rgb_to_hex(res.accent),
            alphas.get("in").copied().unwrap_or(0), alphas.get("out").copied().unwrap_or(0));
        bot.send_document(chat_id, InputFile::memory(theme).file_name(fname))
            .caption(caption).parse_mode(ParseMode::Html).await?;
        return Ok(());
    }

    if data == "noop" {
        bot.answer_callback_query(&q.id).await?;
        return Ok(());
    }

    let answer_text = {
        let mut guard = sessions.0.lock().unwrap();
        let answer = if data == "reset" {
            guard.get(&chat_id).map(|s| format!("🔄 {} reset", sec_full(&s.active))).unwrap_or_default()
        } else { String::new() };
        if let Some(st) = guard.get_mut(&chat_id) {
            apply_callback(st, &data);
        }
        answer
    };

    if answer_text.is_empty() {
        bot.answer_callback_query(&q.id).await?;
    } else {
        bot.answer_callback_query(&q.id).text(answer_text).await?;
    }
    refresh_editor(bot, chat_id, sessions).await;
    Ok(())
}

async fn refresh_editor(bot: Bot, chat_id: ChatId, sessions: &Sessions) {
    let snap = sessions.0.lock().unwrap().get(&chat_id)
        .map(|s| (s.msg_id, s.mode_img.clone(), s.clone()));
    let Some((Some(msg_id), mode_img, st)) = snap else { return };
    let (png, cap, kb) = build_payload(&st);

    if mode_img == "photo" {
        let media = InputMedia::Photo(InputMediaPhoto::new(InputFile::memory(png))
            .caption(cap.clone()).parse_mode(ParseMode::Html));
        let edit = bot.edit_message_media(chat_id, msg_id, media)
            .reply_markup(kb.clone());
        if edit.await.is_err() {
            // media edits can fail if the photo stayed the same → fall back
            let _ = bot.edit_message_caption(chat_id, msg_id)
                .caption(cap)
                .parse_mode(ParseMode::Html)
                .reply_markup(kb)
                .await;
        }
    } else {
        let _ = bot.edit_message_text(chat_id, msg_id, cap)
            .parse_mode(ParseMode::Html)
            .reply_markup(kb)
            .await;
    }
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

// The MediaAttachment import is only used in doc examples; keep imports tidy.
