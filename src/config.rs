//! Configuration loader (BOT_TOKEN) — mirrors config.py.

pub fn bot_token() -> String {
    let _ = dotenvy::dotenv();
    std::env::var("BOT_TOKEN")
        .unwrap_or_default()
        .trim()
        .to_string()
}
