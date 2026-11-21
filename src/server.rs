//! Web server: webhook + theme-building REST API + static Mini App.
//!
//! Replaces the old health-check-only server. In webhook mode (when
//! `RENDER_EXTERNAL_URL` is set) the bot uses one shared axum server for
//! updates and the API; otherwise (local dev) the bot polls and only the
//! API server runs.

use crate::config;
use crate::store::WebState;
use axum::extract::State;
use axum::http::{header, StatusCode};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::Router;
use std::sync::Arc;
use teloxide::prelude::*;
use teloxide::update_listeners::webhooks::Options as WebhookOptions;

/// Build the API router (theme endpoints + static app, + webhook if in prod).
pub async fn build_router(bot: Option<Bot>) -> Router<()> {
    let state = Arc::new(WebState::default());
    let app: Router<Arc<WebState>> = Router::new()
        .route("/", get(|| async { "Themezzz bot is running 🚀" }))
        .route("/app", get(app_page))
        .route("/api/palette", post(api_palette))
        .route("/api/state", post(api_state))
        .route("/api/theme/:id/preview.png", get(theme_preview))
        .route("/api/theme/:id/thumb.png", get(theme_thumb))
        .route("/api/theme/:id/file.attheme", get(theme_file));

    if let Some(bot) = bot {
        // Webhook mode: mount teloxide's webhook router under /telegram.
        let mut opts = WebhookOptions::new(
            addr(),
            url::Url::parse(&config::webhook_url()).expect("webhook url"),
        )
        .drop_pending_updates();
        // Only pass a secret when one is configured: teloxide validates
        // it eagerly (1..=256 chars) and panics on an empty string, e.g.
        // when WEBHOOK_SECRET is unset on Render. Without it teloxide
        // generates a random 32-char secret per boot and registers it
        // with Telegram on its own.
        let secret = config::webhook_secret();
        if !secret.is_empty() {
            opts = opts.secret_token(secret);
        }
        let (listener, _stop, webhook_router) =
            match teloxide::update_listeners::webhooks::axum_to_router(bot.clone(), opts).await {
                Ok(setup) => setup,
                Err(e) => {
                    // Webhook registration with Telegram failed (network /
                    // API hiccup). Keep the port open and keep serving the
                    // API + Mini App; updates fall back to long polling.
                    tracing::error!("webhook setup failed ({e}); falling back to long polling");
                    crate::webhook_bridge::spawn_polling(bot);
                    return app.with_state(state);
                }
            };
        // Feed webhook updates into the dispatcher via a channel bridging.
        crate::webhook_bridge::spawn_bridge(listener, bot.clone());
        // with_state first → both routers are state-less, then merge works.
        return app.with_state(state).merge(webhook_router);
    }
    app.with_state(state)
}

fn addr() -> std::net::SocketAddr {
    let port = config::port();
    ([0, 0, 0, 0], port).into()
}

/// Serve the Mini App (single self-contained HTML page).
async fn app_page() -> impl IntoResponse {
    let html = include_str!("../web/app.html");
    ([(header::CONTENT_TYPE, "text/html; charset=utf-8")], html)
}

/// POST /api/palette — extract palette + wallpaper preview from an image upload.
async fn api_palette(
    State(_ws): State<Arc<WebState>>,
    body: axum::body::Bytes,
) -> Result<impl IntoResponse, StatusCode> {
    let body: Vec<u8> = body.to_vec();
    let out = tokio::task::spawn_blocking(move || {
        let palette = crate::colors::extract_palette(&body, 6);
        let wall = crate::colors::prepare_wallpaper(&body, 720);
        (palette, wall)
    })
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let (palette, wall) = out;
    let wall_b64 = wall
        .as_deref()
        .map(crate::store::b64_encode)
        .unwrap_or_default();
    let json = serde_json::json!({ "palette": palette, "wallpaper": wall_b64 });
    Ok((
        [(header::CONTENT_TYPE, "application/json")],
        serde_json::to_string(&json).unwrap_or_default(),
    ))
}

/// POST /api/state — build a theme from a full editor state, return id + urls.
async fn api_state(
    State(ws): State<Arc<WebState>>,
    axum::Json(req): axum::Json<crate::store::ThemeRequest>,
) -> Result<impl IntoResponse, StatusCode> {
    let st = crate::store::into_state(req);
    let (id, fname, preview_png, thumb_png, attheme, accent, a_in, a_out) = ws.build(st).await;
    let json = serde_json::json!({
        "id": id,
        "preview": format!("/api/theme/{}/preview.png", id),
        "thumb": format!("/api/theme/{}/thumb.png", id),
        "file": format!("/api/theme/{}/file.attheme", id),
        "filename": fname,
        "accent": accent,
        "alphaIn": a_in,
        "alphaOut": a_out,
    });
    ws.put(id, fname.clone(), preview_png, thumb_png, attheme);
    Ok((
        [(header::CONTENT_TYPE, "application/json")],
        serde_json::to_string(&json).unwrap_or_default(),
    ))
}

async fn theme_preview(
    State(ws): State<Arc<WebState>>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Result<impl IntoResponse, StatusCode> {
    match ws.get(&id) {
        Some(t) if !t.preview.is_empty() => Ok((
            [
                (header::CONTENT_TYPE, "image/png"),
                (header::CACHE_CONTROL, "public, max-age=86400"),
            ],
            t.preview,
        )),
        _ => Err(StatusCode::NOT_FOUND),
    }
}

async fn theme_thumb(
    State(ws): State<Arc<WebState>>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Result<impl IntoResponse, StatusCode> {
    match ws.get(&id) {
        Some(t) if !t.thumb.is_empty() => Ok((
            [
                (header::CONTENT_TYPE, "image/png"),
                (header::CACHE_CONTROL, "public, max-age=86400"),
            ],
            t.thumb,
        )),
        _ => Err(StatusCode::NOT_FOUND),
    }
}

async fn theme_file(
    State(ws): State<Arc<WebState>>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Result<impl IntoResponse, StatusCode> {
    match ws.get(&id) {
        Some(t) => {
            let filename = t.filename.clone();
            let attheme = t.attheme;
            Ok((
                [
                    (header::CONTENT_TYPE, "application/octet-stream".to_string()),
                    (
                        header::CONTENT_DISPOSITION,
                        format!("attachment; filename=\"{filename}.attheme\""),
                    ),
                ],
                attheme,
            ))
        }
        _ => Err(StatusCode::NOT_FOUND),
    }
}

/// Start the combined server (webhook + API). Never returns.
pub async fn serve(app: Router<()>) {
    let listener = tokio::net::TcpListener::bind(addr())
        .await
        .unwrap_or_else(|e| panic!("cannot bind server on port {}: {e}", config::port()));
    tracing::info!(
        "server listening on :{port} (webhook + theme API + mini app)",
        port = config::port()
    );
    if let Err(e) = axum::serve(listener, app).await {
        tracing::error!("server stopped: {e}");
    }
}

/// Legacy entry point used by local runs (no webhook): start API server only.
pub async fn start_server() {
    let app = build_router(None).await;
    serve(app).await;
}

/// Keep-alive pings so the free instance does not spin down (webhook mode too).
pub async fn start_keep_alive() {
    let url = config::external_url();
    if url.is_empty() {
        tracing::info!("RENDER_EXTERNAL_URL not set — keep-alive disabled (local run)");
        return;
    }
    let url = format!("{}/", url.trim_end_matches('/'));
    tokio::spawn(async move {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .pool_idle_timeout(std::time::Duration::from_secs(60))
            .build()
            .expect("reqwest client");
        loop {
            match client.get(&url).send().await {
                Ok(_) => tracing::debug!("keep-alive ping ok"),
                Err(e) => tracing::warn!("keep-alive ping failed: {e}"),
            }
            tokio::time::sleep(std::time::Duration::from_secs(60 * 9)).await;
        }
    });
}
