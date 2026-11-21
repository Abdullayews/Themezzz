//! Configuration loader (BOT_TOKEN + server/webhook env) — mirrors config.py.

pub fn bot_token() -> String {
    let _ = dotenvy::dotenv();
    std::env::var("BOT_TOKEN")
        .unwrap_or_default()
        .trim()
        .to_string()
}

/// Port Render assigns (PORT) — health-check + webhook + API all live here.
pub fn port() -> u16 {
    std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8080)
}

/// Public base URL (Render sets RENDER_EXTERNAL_URL).
pub fn external_url() -> String {
    std::env::var("RENDER_EXTERNAL_URL")
        .unwrap_or_default()
        .trim()
        .trim_end_matches('/')
        .to_string()
}

/// True when running on Render (webhook mode); false → long polling locally.
pub fn webhook_mode() -> bool {
    !external_url().is_empty() && std::env::var("RENDER").is_ok()
}

/// Full webhook URL the Telegram API will POST updates to.
pub fn webhook_url() -> String {
    format!("{}/telegram", external_url())
}

/// Shared secret for the webhook endpoint. WEBHOOK_SECRET (1..=256 chars)
/// when set; otherwise a fixed built-in default so the webhook is always
/// signed, even with no env config on Render.
pub fn webhook_secret() -> String {
    let s = std::env::var("WEBHOOK_SECRET")
        .unwrap_or_default()
        .trim()
        .to_string();
    if s.is_empty() {
        "themezzz-default-webhook-secret-v1".to_string()
    } else {
        s
    }
}
