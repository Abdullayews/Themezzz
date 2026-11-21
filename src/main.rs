//! Themezzz — Telegram theme-creator bot, full Rust port.
//!
//! Flow: config → server (health check) → keep-alive → bot polling.

mod bot;
mod config;
mod server;

use teloxide::prelude::*;

/// Run the bot with long polling, 24/7 keep-alive and a health-check server.
pub async fn run_bot() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    let token = config::bot_token();
    if token.is_empty() {
        eprintln!("❌ BOT_TOKEN is not set!");
        std::process::exit(1);
    }

    server::start_server().await;
    server::start_keep_alive().await;

    let bot = Bot::new(token);
    let sessions = bot::Sessions::default();

    let handler = dptree::entry()
        .branch(Update::filter_message().branch(dptree::endpoint(
            |bot: Bot, msg: Message, sessions: bot::Sessions| async move {
                dispatch_message(bot, msg, sessions).await
            },
        )))
        .branch(Update::filter_callback_query().branch(dptree::endpoint(
            |bot: Bot, q: CallbackQuery, sessions: bot::Sessions| async move {
                bot::on_callback(bot, q, &sessions).await
            },
        )));

    let mut dispatcher = Dispatcher::builder(bot, handler)
        .dependencies(dptree::deps![sessions])
        .build();
    dispatcher.dispatch().await;
}

async fn dispatch_message(bot: Bot, msg: Message, sessions: bot::Sessions) -> ResponseResult<()> {
    match msg.text().and_then(|t| t.split_whitespace().next()) {
        Some(cmd) if cmd == "/start" || cmd == "/help" => bot::cmd_start(bot, msg, &sessions).await,
        _ => {
            if msg.photo().is_some() || msg.document().is_some() {
                bot::on_image(bot, msg, &sessions).await
            } else if msg.text().is_some() {
                bot::on_text(bot, msg, &sessions).await
            } else {
                Ok(())
            }
        }
    }
}

fn main() {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("tokio runtime")
        .block_on(run_bot());
}
