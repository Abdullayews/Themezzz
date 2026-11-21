//! Lightweight health-check web server + 24/7 keep-alive self-ping.
//! Ported from server.py / keep_alive.py (Render port requirement).

use std::time::Duration;

pub async fn start_server() {
    let app = axum::Router::new().route(
        "/",
        axum::routing::get(|| async { "Themezzz bot is running 🚀" }),
    );
    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8080);
    let listener = tokio::net::TcpListener::bind(("0.0.0.0", port))
        .await
        .unwrap_or_else(|e| panic!("cannot bind health-check server on port {port}: {e}"));
    tokio::spawn(async move {
        if let Err(e) = axum::serve(listener, app).await {
            tracing::error!("health-check server stopped: {e}");
        }
    });
    tracing::info!("health-check server listening on :{port}");
}

/// Self-ping every 9 minutes prevents the free instance from spinning down.
pub async fn start_keep_alive() {
    let url = std::env::var("RENDER_EXTERNAL_URL").unwrap_or_default();
    if url.is_empty() {
        tracing::info!("RENDER_EXTERNAL_URL not set — keep-alive disabled (local run)");
        return;
    }
    let url = format!("{}/", url.trim_end_matches('/'));
    tokio::spawn(async move {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("reqwest client");
        loop {
            match client.get(&url).send().await {
                Ok(_) => tracing::debug!("keep-alive ping ok"),
                Err(e) => tracing::warn!("keep-alive ping failed: {e}"),
            }
            tokio::time::sleep(Duration::from_secs(60 * 9)).await;
        }
    });
}
