//! Themezzz library: color engine + preview renderer + web server + bot.
//! The binary in `src/main.rs` is a thin wrapper around [`run_bot`].

pub mod bot;
pub mod colors;
pub mod config;
pub mod preview;
pub mod server;
pub mod state;
pub mod store;
pub mod webhook_bridge;

use teloxide::prelude::*;

/// Run the bot: webhook + API + mini app in prod, polling + API locally.
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

    let bot = Bot::new(token);
    let sessions = bot::Sessions::default();

    if config::webhook_mode() {
        // One server: webhook (under /telegram) + theme API + mini app.
        let app = server::build_router(Some(bot.clone())).await;
        server::start_keep_alive().await;
        // The dispatcher is spawned inside build_router (webhook bridge).
        let _ = sessions;
        server::serve(app).await;
    } else {
        // Local: API server (no webhook) + polling dispatcher.
        let app = server::build_router(None).await;
        tokio::spawn(server::serve(app));
        server::start_keep_alive().await;

        let handler = bot::handler_tree();
        let mut dispatcher = Dispatcher::builder(bot, handler)
            .dependencies(dptree::deps![sessions])
            .build();
        // Spawned so a Telegram API failure (bad token / no network) does
        // not take down the health-check + theme API server. dispatch()
        // panics on API errors, so guard with catch_unwind.
        tokio::spawn(async move {
            let res = futures_util::FutureExt::catch_unwind(std::panic::AssertUnwindSafe(
                dispatcher.dispatch(),
            ))
            .await;
            if res.is_err() {
                tracing::error!(
                    "bot dispatch panicked (bad token or no network) — web server keeps running"
                );
            }
        });
        // Keep the process alive alongside the server.
        futures_util::future::pending::<()>().await;
    }
}
