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
