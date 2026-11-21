//! Bridge between the teloxide webhook listener and the dispatcher loop.
//!
//! `axum_to_router` gives an `UpdateListener` (a stream) plus an axum router.
//! We spawn the dispatcher with that listener so webhook updates flow into
//! the same handler tree as polled ones.

use teloxide::dispatching::Dispatcher;
use teloxide::error_handlers::LoggingErrorHandler;
use teloxide::prelude::*;
use teloxide::update_listeners::UpdateListener;

/// Spawn the dispatcher fed by a webhook listener.
pub fn spawn_bridge<L>(listener: L, bot: Bot)
where
    L: UpdateListener<Err = std::convert::Infallible> + Send + 'static,
{
    let sessions = crate::bot::Sessions::default();
    let handler = crate::bot::handler_tree();
    let mut dispatcher = Dispatcher::builder(bot, handler)
        .dependencies(dptree::deps![sessions])
        .build();
    tokio::spawn(async move {
        let eh = LoggingErrorHandler::with_custom_text("An error from the update listener");
        dispatcher.dispatch_with_listener(listener, eh).await;
    });
}

/// Fallback when webhook registration fails: long-poll the same handler tree
/// so the bot still answers while the API server keeps the port open.
pub fn spawn_polling(bot: Bot) {
    let sessions = crate::bot::Sessions::default();
    let handler = crate::bot::handler_tree();
    let poll_bot = bot.clone();
    let mut dispatcher = Dispatcher::builder(bot, handler)
        .dependencies(dptree::deps![sessions])
        .build();
    tokio::spawn(async move {
        let eh = LoggingErrorHandler::with_custom_text("An error from the update listener");
        // polling_default deletes any half-registered webhook first.
        let listener = teloxide::update_listeners::polling_default(poll_bot).await;
        // A Telegram API failure (e.g. bad token) panics inside dispatch;
        // guard so the API server and Mini App stay up.
        let res = futures_util::FutureExt::catch_unwind(std::panic::AssertUnwindSafe(
            dispatcher.dispatch_with_listener(listener, eh),
        ))
        .await;
        if let Err(p) = res {
            let msg = p
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| p.downcast_ref::<&str>().map(|s| s.to_string()))
                .unwrap_or_else(|| "unknown panic".into());
            tracing::error!("polling fallback stopped: {msg}");
        }
    });
}
