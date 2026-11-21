//! End-to-end API smoke test: spins up the router in-process and exercises
//! the palette → state → preview/thumb/file chain with a generated image.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use themezzz::server::build_router;
use tower::ServiceExt; // for oneshot

/// Generate a small test PNG (gradient) without external files.
fn test_png() -> Vec<u8> {
    use image::{ImageBuffer, Rgb, RgbImage};
    let img: RgbImage = ImageBuffer::from_fn(64, 64, |x, y| {
        Rgb([(x * 4) as u8, (y * 4) as u8, ((x + y) * 2) as u8])
    });
    let mut buf = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgb8(img)
        .write_to(&mut buf, image::ImageFormat::Png)
        .unwrap();
    buf.into_inner()
}

#[tokio::test]
async fn api_full_flow() {
    let app = build_router(None).await;

    // 1. Health root
    let res = app
        .clone()
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // 2. /app serves the Mini App HTML
    let res = app
        .clone()
        .oneshot(Request::builder().uri("/app").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = axum::body::to_bytes(res.into_body(), 1 << 20)
        .await
        .unwrap();
    let html = String::from_utf8_lossy(&body).to_string();
    assert!(html.contains("Themezzz"), "mini app html must load");
    assert!(html.contains("telegram-web-app"), "must init Telegram SDK");

    // 3. Palette extraction from a generated image
    let png = test_png();
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/palette")
                .header("content-type", "application/octet-stream")
                .body(Body::from(png))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = axum::body::to_bytes(res.into_body(), 1 << 20)
        .await
        .unwrap();
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let palette = v["palette"].as_array().unwrap().clone();
    assert_eq!(palette.len(), 6, "6 palette colors, got {palette:?}");
    let wall = v["wallpaper"].as_str().unwrap().to_string();
    assert!(!wall.is_empty(), "wallpaper preview must be returned");

    // 4. Build a theme with that palette
    let state = serde_json::json!({
        "palette": palette,
        "mode": "dark",
        "sections": {
            "bg":   {"idx": -1, "alpha": 0},
            "bar":  {"idx": 0,  "alpha": 30},
            "in":   {"idx": -1, "alpha": 50},
            "out":  {"idx": 1,  "alpha": 0},
            "text": {"idx": -1, "alpha": 0},
            "accent": {"idx": 2, "alpha": 0},
            "reply": {"idx": -1, "alpha": 0}
        },
        "active": "bg",
        "wall_mode": "image",
        "wall_idx": -1,
        "wall_blur": true,
        "wall_b64": wall
    });
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/state")
                .header("content-type", "application/json")
                .body(Body::from(state.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = axum::body::to_bytes(res.into_body(), 1 << 20)
        .await
        .unwrap();
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let id = v["id"].as_str().unwrap().to_string();
    assert!(!id.is_empty());
    assert_eq!(v["alphaIn"], 50);
    assert_eq!(v["alphaOut"], 0);
    assert!(v["filename"].as_str().unwrap().ends_with(".attheme"));

    // 5. Fetch the preview PNG
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/theme/{id}/preview.png"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let ct = res.headers().get("content-type").unwrap().to_str().unwrap();
    assert_eq!(ct, "image/png");
    let body = axum::body::to_bytes(res.into_body(), 1 << 24)
        .await
        .unwrap();
    assert!(
        body.len() > 10_000,
        "preview should be a real PNG, got {} bytes",
        body.len()
    );

    // 6. Fetch the thumbnail
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/theme/{id}/thumb.png"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = axum::body::to_bytes(res.into_body(), 1 << 20)
        .await
        .unwrap();
    assert!(body.len() > 500, "thumb should be a real PNG");

    // 7. Fetch the .attheme file
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/theme/{id}/file.attheme"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let cd = res
        .headers()
        .get("content-disposition")
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    assert!(cd.contains(".attheme"), "cd header: {cd}");
    let body = axum::body::to_bytes(res.into_body(), 1 << 24)
        .await
        .unwrap();
    let theme = String::from_utf8_lossy(&body).to_string();
    assert!(theme.contains("chat_inBubble="));
    assert!(theme.contains("chat_outBubble="));
    assert!(theme.contains("wallpaperFileOffset=") || theme.contains("chat_wallpaper="));
    assert!(theme.contains("actionBarDefault="));

    // 8. Unknown id → 404
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/theme/nope/file.attheme")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}
