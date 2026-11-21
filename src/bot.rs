//! Telegram bot: handlers, flows & inline keyboard UI (teloxide 0.13 port of bot.py).
//!
//! New flow: the editor lives in the Mini App (web UI at /app); the chat side
//! only (1) opens the web app, (2) receives finished themes through
//! `web_app_data`, (3) still accepts a raw image → palette quick start.

use crate::colors::{extract_palette, prepare_wallpaper};
use crate::state::WELCOME;
use std::collections::HashMap;
use std::sync::Mutex;
use teloxide::prelude::*;
use teloxide::types::{
    ChatAction, InlineKeyboardButton, InlineKeyboardButtonKind, InlineKeyboardMarkup, InputFile,
    ParseMode, WebAppInfo,
};
use tokio::io::AsyncWriteExt;

/// One pending quick-start per chat (image → palette + web app link).
#[derive(Clone, Default)]
pub struct Sessions(pub std::sync::Arc<Mutex<HashMap<ChatId, QuickStart>>>);

/// Palette extracted from a chat image, so the Mini App can pick it up.
#[derive(Clone, Debug, Default)]
pub struct QuickStart {
    pub palette: Vec<String>,
    pub wall_b64: String,
}

impl std::ops::Deref for Sessions {
    type Target = Mutex<HashMap<ChatId, QuickStart>>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// Build the Mini App URL (same origin as the webhook host).
fn web_app_url() -> String {
    let base = crate::config::external_url();
    if base.is_empty() {
        // Local dev fallback: whatever host the user set.
        std::env::var("WEBAPP_URL").unwrap_or_default()
    } else {
        format!("{}/app", base)
    }
}

/// WebAppInfo from a plain URL string (teloxide wants a `url::Url`).
fn web_app_info(url: &str) -> WebAppInfo {
    WebAppInfo {
        url: url::Url::parse(url)
            .unwrap_or_else(|_| url::Url::parse("https://example.org/").expect("static url")),
    }
}

/// The full update handler tree (shared by polling and webhook modes).
pub fn handler_tree() -> teloxide::dispatching::UpdateHandler<teloxide::RequestError> {
    dptree::entry()
        .branch(Update::filter_message().branch(dptree::endpoint(
            |bot: Bot, msg: Message, sessions: Sessions| async move {
                dispatch_message(bot, msg, sessions).await
            },
        )))
        .branch(
            // web_app_data arrives as a message with a special kind
            Update::filter_message().branch(Message::filter_web_app_data().branch(
                dptree::endpoint(|bot: Bot, msg: Message, sessions: Sessions| async move {
                    on_web_app_data(bot, msg, &sessions).await
                }),
            )),
        )
        .branch(Update::filter_callback_query().branch(dptree::endpoint(
            |bot: Bot, q: CallbackQuery, sessions: Sessions| async move {
                on_callback(bot, q, &sessions).await
            },
        )))
}

async fn dispatch_message(bot: Bot, msg: Message, sessions: Sessions) -> ResponseResult<()> {
    match msg.text().and_then(|t| t.split_whitespace().next()) {
        Some(cmd) if cmd == "/start" || cmd == "/help" => cmd_start(bot, msg).await,
        _ => {
            if msg.photo().is_some() || msg.document().is_some() {
                on_image(bot, msg, &sessions).await
            } else if msg.text().is_some() {
                on_text(bot, msg, &sessions).await
            } else {
                Ok(())
            }
        }
    }
}

pub async fn cmd_start(bot: Bot, msg: Message) -> ResponseResult<()> {
    send_menu(bot, msg.chat.id).await
}

/// The chat menu, Nagram-style: two choices, everything else in the web UI.
/// `web_app=true` buttons open the Mini App directly; `CallbackData` rows are
/// kept for menu navigation (Back to menu).
async fn send_menu(bot: Bot, chat_id: ChatId) -> ResponseResult<()> {
    let url = web_app_url();
    if url.is_empty() {
        bot.send_message(
            chat_id,
            format!(
                "{WELCOME}\n\n⚠️ WEBAPP_URL is not configured on the server — \
                 tell the admin (web editor disabled)."
            ),
        )
        .parse_mode(ParseMode::Html)
        .await?;
        return Ok(());
    }
    // "?mode=solo" / "?mode=paired": the Mini App reads this on load.
    let solo = format!("{url}?mode=solo");
    let paired = format!("{url}?mode=paired");
    let kb = InlineKeyboardMarkup::new(vec![
        vec![
            InlineKeyboardButton::new(
                "👤 For myself",
                InlineKeyboardButtonKind::WebApp(web_app_info(&solo)),
            ),
            InlineKeyboardButton::new(
                "👥 Paired",
                InlineKeyboardButtonKind::WebApp(web_app_info(&paired)),
            ),
        ],
        vec![InlineKeyboardButton::new(
            "« Back to menu",
            InlineKeyboardButtonKind::CallbackData("menu".to_string()),
        )],
    ]);
    bot.send_message(chat_id, WELCOME)
        .parse_mode(ParseMode::Html)
        .reply_markup(kb)
        .await?;
    Ok(())
}

/// web_app_data from the Mini App: `{"id": "...", "filename": "...",
/// "paired": {"id": "...", "filename": "..."}}` — the second entry is the
/// partner half when the user built a paired set.
async fn on_web_app_data(bot: Bot, msg: Message, _sessions: &Sessions) -> ResponseResult<()> {
    let Some(data) = msg.web_app_data() else {
        return Ok(());
    };
    let parsed: Option<serde_json::Value> = serde_json::from_str(&data.data).ok();
    let Some(v) = parsed else {
        bot.send_message(msg.chat.id, "😕 Couldn't read the theme data. Try again.")
            .await?;
        return Ok(());
    };

    let base = crate::config::external_url();
    let client = reqwest::Client::new();

    // Main theme (required) + optional paired partner theme.
    let main_id = v.get("id").and_then(|x| x.as_str()).unwrap_or_default();
    let paired_id = v
        .get("paired")
        .and_then(|p| p.get("id"))
        .and_then(|x| x.as_str())
        .unwrap_or_default();

    let fetch = |id: &str| {
        let file_url = format!("{base}/api/theme/{id}/file.attheme");
        let preview_url = format!("{base}/api/theme/{id}/preview.png");
        let client = client.clone();
        async move {
            let (f, p) = tokio::join!(
                client.get(&file_url).send(),
                client.get(&preview_url).send()
            );
            let (fb, pb) = match (f, p) {
                (Ok(f), Ok(p)) => match tokio::join!(f.bytes(), p.bytes()) {
                    (Ok(fb), Ok(pb)) => (fb.to_vec(), pb.to_vec()),
                    _ => (Vec::new(), Vec::new()),
                },
                _ => (Vec::new(), Vec::new()),
            };
            (fb, pb)
        }
    };

    if main_id.is_empty() {
        bot.send_message(
            msg.chat.id,
            "😕 Theme not found (expired?). Build it again.",
        )
        .await?;
        return Ok(());
    }
    // Python 1:1: "⏳ Creating theme..." indicator while the theme is built
    // and fetched — send_chat_action shows the typing status in chat.
    let _ = bot.send_chat_action(msg.chat.id, ChatAction::Typing).await;
    bot.send_message(msg.chat.id, "⏳ Creating theme...")
        .await?;
    let (file_bytes, preview_bytes) = fetch(main_id).await;
    if file_bytes.is_empty() {
        bot.send_message(
            msg.chat.id,
            "😕 Theme expired on the server. Open the editor and press Create again.",
        )
        .await?;
        return Ok(());
    }

    let filename = v
        .get("filename")
        .and_then(|x| x.as_str())
        .unwrap_or("Theme.attheme")
        .to_string();
    let accent = v
        .get("accent")
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_string();
    let a_in = v.get("alphaIn").and_then(|x| x.as_u64()).unwrap_or(0);
    let a_out = v.get("alphaOut").and_then(|x| x.as_u64()).unwrap_or(0);

    let is_paired = !paired_id.is_empty();
    let caption = if is_paired {
        "💜 <b>Your paired themes are ready!</b>\n\
         First one is yours, second is for your favorite person — send it to them!\n\n\
         Open the files — Telegram applies them instantly ✨"
            .to_string()
    } else {
        format!(
            "🎨 <b>Your theme is ready!</b>\nAccent {} · In {}% · Out {}%\n\n\
             Open the file — Telegram applies it instantly ✨",
            accent, a_in, a_out
        )
    };

    // Preview photo first, then the file(s) with the caption on the last one.
    if !preview_bytes.is_empty() {
        let _ = bot
            .send_photo(msg.chat.id, InputFile::memory(preview_bytes))
            .caption(if is_paired {
                "💜 Your half of the pair"
            } else {
                &caption
            })
            .parse_mode(ParseMode::Html)
            .await;
    }
    if is_paired {
        let (p_file, p_preview) = fetch(paired_id).await;
        if !p_file.is_empty() {
            if !p_preview.is_empty() {
                let _ = bot
                    .send_photo(msg.chat.id, InputFile::memory(p_preview))
                    .caption("💙 Their half of the pair")
                    .parse_mode(ParseMode::Html)
                    .await;
            }
            let _ = bot
                .send_document(
                    msg.chat.id,
                    InputFile::memory(p_file)
                        .file_name(filename.replace(".attheme", "_paired.attheme")),
                )
                .await;
        }
    }
    bot.send_document(
        msg.chat.id,
        InputFile::memory(file_bytes).file_name(filename),
    )
    .caption(&caption)
    .parse_mode(ParseMode::Html)
    .await?;
    Ok(())
}

/// Old flow kept as a shortcut: image → palette + "open editor" link.
pub async fn on_image(bot: Bot, msg: Message, sessions: &Sessions) -> ResponseResult<()> {
    let file_id = if let Some(photos) = msg.photo() {
        photos.last().map(|p| p.file.id.clone())
    } else {
        msg.document().map(|doc| doc.file.id.clone())
    };
    let Some(file_id) = file_id else {
        bot.send_message(
            msg.chat.id,
            "😕 Couldn't read that image. Try again (max 20 MB).",
        )
        .await?;
        return Ok(());
    };
    let raw = download_file(&bot, &file_id).await;
    if raw.is_empty() {
        bot.send_message(
            msg.chat.id,
            "😕 Couldn't download that image. Try again (max 20 MB).",
        )
        .await?;
        return Ok(());
    }

    let (palette, wall) = tokio::task::spawn_blocking(move || {
        (extract_palette(&raw, 6), prepare_wallpaper(&raw, 720))
    })
    .await
    .unwrap_or_else(|_| (Vec::new(), None));

    let wall_b64 = wall
        .as_deref()
        .map(crate::store::b64_encode)
        .unwrap_or_default();
    let pal_line = palette
        .iter()
        .enumerate()
        .map(|(i, h)| format!("{} <code>{}</code>", i + 1, h))
        .collect::<Vec<_>>()
        .join("  ");
    sessions
        .0
        .lock()
        .unwrap()
        .insert(msg.chat.id, QuickStart { palette, wall_b64 });

    let url = web_app_url();
    let text = format!(
        "🎨 <b>Colors grabbed!</b>\n⚡ {pal_line}\n\n\
         Tap below to open the editor with this palette preloaded 👇"
    );
    if url.is_empty() {
        bot.send_message(msg.chat.id, text)
            .parse_mode(ParseMode::Html)
            .await?;
    } else {
        bot.send_message(msg.chat.id, text)
            .parse_mode(ParseMode::Html)
            .reply_markup(InlineKeyboardMarkup::new([[InlineKeyboardButton::new(
                "🎨 Open Theme Editor",
                InlineKeyboardButtonKind::WebApp(web_app_info(&url)),
            )]]))
            .await?;
    }
    Ok(())
}

async fn on_text(bot: Bot, msg: Message, _sessions: &Sessions) -> ResponseResult<()> {
    bot.send_message(
        msg.chat.id,
        "📸 Send me an image and I'll grab its colors — or tap <b>For myself</b> \
         / <b>Paired</b> to open the web editor!",
    )
    .parse_mode(ParseMode::Html)
    .await?;
    Ok(())
}

/// Menu navigation + legacy keyboards: "menu" re-sends the start menu,
/// anything else (old editor callbacks) points to the web app.
pub async fn on_callback(bot: Bot, q: CallbackQuery, _sessions: &Sessions) -> ResponseResult<()> {
    let Some(data) = q.data.clone() else {
        bot.answer_callback_query(&q.id).await?;
        return Ok(());
    };
    match data.as_str() {
        "menu" => {
            bot.answer_callback_query(&q.id).await?;
            if let Some(chat_id) = q.message.map(|m| m.chat().id) {
                // send a fresh menu message to the originating chat
                send_menu(bot, chat_id).await?;
            }
        }
        _ => {
            bot.answer_callback_query(&q.id)
                .text("This editor has moved to the web app — tap \"For myself\" or \"Paired\".")
                .show_alert(true)
                .await?;
        }
    }
    Ok(())
}

/// Download a Telegram file into memory (empty Vec on failure).
async fn download_file(bot: &Bot, file_id: &str) -> Vec<u8> {
    use teloxide::net::Download;
    let Ok(f) = bot.get_file(file_id).await else {
        return Vec::new();
    };
    let mut buf: Vec<u8> = Vec::new();
    let mut writer = tokio::io::BufWriter::new(&mut buf);
    if bot.download_file(&f.path, &mut writer).await.is_err() {
        return Vec::new();
    }
    writer.flush().await.ok();
    buf
}
