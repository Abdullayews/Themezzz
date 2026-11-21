<div align="center">

# 🦀 Themezzz

**Turn any photo into a fully customized Telegram theme — in Rust, with a live web editor.**

*Send an image → get a color palette → tweak every surface in the Mini App → receive a ready-to-apply `.attheme` file.*

[![Rust](https://img.shields.io/badge/Rust-1.75+-DEA584?style=for-the-badge&logo=rust&logoColor=black)](https://www.rust-lang.org)
[![teloxide](https://img.shields.io/badge/teloxide-0.13-26A5E4?style=for-the-badge&logo=telegram&logoColor=white)](https://docs.rs/teloxide)
[![Tests](https://img.shields.io/badge/tests-8%20passing-2ea44f?style=for-the-badge)](#-testing)
[![Deploy](https://img.shields.io/badge/Ready_for-Render-46E3B7?style=for-the-badge&logo=render&logoColor=white)](https://render.com)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg?style=for-the-badge)](LICENSE)

</div>

---

## 💡 What is this?

Themezzz is a **Telegram theme-creator bot with a web editor**. Send any
picture and it instantly extracts a balanced 6-color palette, then opens a
**Telegram Mini App** where you paint every surface of the theme — window
background, message bubbles, bars, text, accents, wallpaper — with live
previews, opacity sliders and custom `#hex` support. When you're happy, it
compiles everything into an **`.attheme` file** that lands straight in your
chat, ready to apply with one tap.

The editor runs in Telegram's built-in browser, so tweaking is instant: every
change re-renders a **real dual-phone preview** (chat view + dialog list) on
the server and streams it back — what you see is what you get. The bot talks
to Telegram over a **webhook** (no more slow long-polling), so buttons and
files arrive immediately.

## ✨ Features

| | |
|---|---|
| 🌐 **Mini App editor** | Full editor inside Telegram — upload, sliders, color pickers, live canvas preview |
| 🎨 **Automatic palette extraction** | Median-cut quantization pulls the 6 dominant, visually balanced colors out of any picture |
| 📱 **Real-time dual-phone previews** | Rust-rendered mockups of the Chat View **and** Dialog List View — instant feedback on every tweak |
| 🌗 **Dark & light modes** | Background, text contrast and UI surfaces recalculated instantly per mode |
| 🎛️ **Granular section editing** | `BG` window background · `Bar` top & status bar · `In` incoming bubbles · `Out` outgoing bubbles · `Text` primary text · `Accent` actions, avatars, tabs & FAB · `Reply` quoted text headers · `Wall` wallpaper |
| 🫧 **Transparency sliders** | 0–100% opacity for every supported surface — bubbles, bars, text, accent, reply |
| 🖼 **Wallpaper control** | Original image (with optional blur) or a flat color — switchable any time |
| 🎯 **Custom hex colors** | Type any `#hex` code, apply to any section |
| ⚡ **Auto mode** | One tap — the best palette mapping is picked for you |
| 🎲 **Dice roll** | Shuffle palette picks + alphas for instant inspiration |
| ⚡ **Webhook-fast responses** | Updates arrive via webhook on Render; local dev falls back to polling |
| ♿ **WCAG-aware contrast** | Text, icons and accents are always readable — no invisible UI |
| 🚀 **24/7 hosting ready** | One axum server: webhook + theme API + Mini App; self-ping keep-alive |

## 🖼 Preview

<p align="center">
  <img src="assets/preview_dark.png" width="45%" alt="Dark theme preview" />
  <img src="assets/preview_light.png" width="45%" alt="Light theme preview" />
</p>

The renderer draws both phones from the same resolved palette the
`.attheme` file uses, so **what you see is what you get**.

## 🦀 Why Rust?

The original Python bot used python-telegram-bot + Pillow + aiohttp. The
Rust port replaces each piece:

| Python | Rust | Notes |
|---|---|---|
| `python-telegram-bot` | [`teloxide`](https://docs.rs/teloxide) 0.13 | Async Telegram bot framework, webhook mode |
| `Pillow` quantize/blur | [`image`](https://docs.rs/image) 0.25 + custom median-cut | Palette extraction, wallpaper resize/blur |
| `PIL` drawing | custom rasterizer on `RgbaImage` | Rounded rects, circles, arcs, polygons, soft shadows |
| `aiohttp` server | [`axum`](https://docs.rs/axum) 0.7 | webhook + `/api/*` theme API + `/app` Mini App |
| `aiohttp` self-ping | [`reqwest`](https://docs.rs/reqwest) 0.12 | 9-min keep-alive loop |
| `requirements.txt` | `Cargo.toml` + `Cargo.lock` | Locked, reproducible builds |

Result: a **single static binary**, ~10 MB RAM, milliseconds-fast preview
rendering, and zero interpreter restarts on free hosting tiers.

## 🏗 Architecture

```
src/
├── main.rs            # thin binary wrapper
├── lib.rs             # run_bot(): webhook mode (Render) or polling (local)
├── config.rs          # BOT_TOKEN, PORT, RENDER_EXTERNAL_URL, WEBHOOK_SECRET
├── colors.rs          # WCAG math, HLS, palette extraction, .attheme builder
├── preview.rs         # dual-phone Telegram UI renderer (also live previews)
├── state.rs           # sections, labels, editor state
├── store.rs           # theme store: build + cache finished themes by id
├── bot.rs             # teloxide handlers: /start, images, web_app_data
├── webhook_bridge.rs  # feeds webhook updates into the dispatcher
└── server.rs          # axum: /app, /api/palette, /api/state, /api/theme/:id/*
web/app.html           # the Mini App (single self-contained page)
```

### HTTP API

| endpoint | what it does |
|---|---|
| `GET /` | health check (Render port binding) |
| `GET /app` | the Mini App HTML (Telegram WebApp SDK included) |
| `POST /api/palette` | body: raw image bytes → `{ palette: [#hex ×6], wallpaper: <b64 png> }` |
| `POST /api/state` | body: full editor state JSON → `{ id, preview, thumb, file, filename, … }` |
| `GET /api/theme/:id/preview.png` | 1000×1000 dual-phone preview |
| `GET /api/theme/:id/thumb.png` | small chat-bubble thumbnail |
| `GET /api/theme/:id/file.attheme` | the compiled theme file |

Themes are kept in memory for 2 h (or 200 entries), then expire.

### The flow

1. **`/start`** → the bot answers with an **Open Theme Editor** button.
2. The Mini App opens: **pick an image** (or start from a stock palette).
   The server extracts the palette and the wallpaper.
3. **Tweak everything live** — mode, sections, palette picks, custom hex,
   transparency sliders, wallpaper image/flat/blur. Every change rebuilds
   the preview server-side and streams it back to the app.
4. **Create theme** → `sendData()` hands the theme id to the bot; the bot
   replies in chat with the **preview picture + the `.attheme` file**.
5. Sending an image directly in chat still works: the bot grabs the colors
   and links straight to the editor.

## 📖 Color pipeline

1. **Extract** — the image is thumbnailed to 256 px and median-cut
   quantized to 24 boxes; the most populated boxes that pass a luminance
   filter (0.03–0.97) and are ≥ 45 RGB-distance apart become the palette.
   Fewer than 6 candidates are topped up with lightness variants of the
   most saturated color — hues are never invented.
2. **Resolve** — each section (bg, bar, in/out bubbles, text, accent,
   reply) is either a user pick or an auto derivation
   (`set_lightness` / `clamp_lightness` on palette colors).
3. **Semantics** — link/error/success/flame colors pick the most
   saturated palette color inside each hue range, with fallback to the
   accent; every text color is `ensure_contrast`-checked (WCAG ≥ 3.0).
4. **Serialize** — all 300+ `.attheme` keys are derived from those 7
   colors (bars, gradients, counters, checkboxes, seekbars, …), written
   as `key=argb_int` lines, with an optional wallpaper appended after
   the `WPS` marker.

## 🚀 Getting started

```bash
# 1. get the code
git clone https://github.com/Abdullayews/Themezzz.git
cd Themezzz

# 2. configure
cp .env.example .env        # then put your bot token into .env

# 3. build & run (polling mode locally, API on :8080)
cargo run --release
```

Get a bot token from [@BotFather](https://t.me/BotFather).

### Environment variables

| var | required | meaning |
|---|---|---|
| `BOT_TOKEN` | ✅ | Telegram bot token from @BotFather |
| `PORT` | — | server port (default 8080; Render sets it) |
| `RENDER_EXTERNAL_URL` | — | set by Render: switches to webhook mode + keep-alive |
| `RENDER` | — | set by Render; together with the URL above enables webhooks |
| `WEBAPP_URL` | — | local override for the Mini App URL shown on buttons |
| `WEBHOOK_SECRET` | — | optional shared secret for the `/telegram` webhook route |
| `RUST_LOG` | — | log filter (default `info`) |

## ☁️ Deploy on Render

The repo ships a `render.yaml` blueprint:

1. Push to GitHub.
2. In Render: **New → Blueprint**, pick this repo.
3. Set the `BOT_TOKEN` env var when prompted.
4. Done — the service builds with `cargo build --release --locked`,
   starts `cargo run --release` (via `default-run`), binds `PORT`, and
   registers the webhook automatically. The Mini App is served at
   `https://<service>.onrender.com/app`.

## 🧪 Testing

```bash
cargo test                        # 7 unit tests + 1 end-to-end API test:
                                   # palette → state → preview/thumb/file
cargo run --bin render_preview     # regenerate assets/preview_*.png
cargo run --bin make_theme         # offline .attheme smoke test
```

The integration test spins up the real axum router in-process and walks the
whole chain: generated image → `/api/palette` → `/api/state` → PNG/ATtheme
fetches → 404 handling.

## 🗺 Roadmap

- [ ] Inline theme gallery (top community themes)
- [ ] Multi-image palette blending
- [ ] Direct .attheme preview inside Telegram's own theme player

## 📄 License

MIT — see [LICENSE](LICENSE). Do whatever, credit appreciated.
