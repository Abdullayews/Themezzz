<div align="center">

# 🎨 Themezzz

**Turn any photo into a fully customized Telegram theme.**

*Send an image → get a color palette → tweak every surface → receive a ready-to-apply `.attheme` file.*

[![Python](https://img.shields.io/badge/Python-3.10+-3776AB?style=for-the-badge&logo=python&logoColor=white)](https://python.org)
[![python-telegram-bot](https://img.shields.io/badge/Telegram_Bot-v20+-26A5E4?style=for-the-badge&logo=telegram&logoColor=white)](https://python-telegram-bot.readthedocs.io)
[![Pillow](https://img.shields.io/badge/Pillow-9.0+-F9BC27?style=for-the-badge&logo=python&logoColor=black)](https://python-pillow.org)
[![Deploy](https://img.shields.io/badge/Ready_for-Render-46E3B7?style=for-the-badge&logo=render&logoColor=white)](https://render.com)

</div>

---

## 💡 What is this?

Themezzz is a **Telegram theme-creator bot**. Upload any picture and it
instantly extracts a balanced 6-color palette from it, then lets you paint
every surface of a Telegram theme with those colors — window background,
message bubbles, bars, accents, wallpaper — with live opacity control and
custom `#hex` support. When you're happy, it compiles everything into an
**`.attheme` file** you can apply in Telegram with one tap.

The twist: while you tweak, the bot renders a **real-time dual-phone
preview** (chat view + dialog list view), so you see exactly what the theme
looks like *before* applying it — no more guess-and-retry theming.

## ✨ Features

| | |
|---|---|
| 🎨 **Automatic palette extraction** | Image quantization (Pillow) pulls the 6 dominant, visually balanced colors out of any picture |
| 📱 **Real-time dual-phone previews** | PIL-rendered wireframes of the Chat View **and** Dialog List View — instant feedback on every tweak |
| 🌗 **Dark & light modes** | Background, text contrast and UI surfaces recalculated instantly per mode |
| 🎛️ **Granular section editing** | `BG` window background · `Bar` top & status bar · `In` incoming bubbles · `Out` outgoing bubbles · `Text` primary text · `Accent` actions, avatars, tabs & FAB · `Reply` quoted text headers · `Wall` wallpaper |
| 🫧 **Opacity sliders** | 0–100% transparency for supported surfaces |
| 🎯 **Custom hex colors** | Type any `#hex` code (e.g. `#34c7a4`) directly in chat |
| ⚡ **Auto mode** | One tap — the bot picks the best palette mapping for you |
| 🚀 **24/7 hosting ready** | Built-in web server + self-ping keep-alive for PaaS platforms |
| 🧪 **Smoke-tested** | `smoke_test.py` sanity-checks theme generation offline |

## 📁 Repository structure

```
Themezzz/
├── bot.py           # Telegram bot — handlers, flows & inline keyboard UI
├── colors.py        # Palette extraction, contrast logic, theme resolution & .attheme generator
├── preview.py       # PIL-based dual-phone wireframe renderer
├── config.py        # Configuration loader (BOT_TOKEN)
├── server.py        # Lightweight web server for health checks
├── keep_alive.py    # Self-ping loop for 24/7 hosting
├── smoke_test.py    # Offline sanity checks
├── render.yaml      # Render blueprint
├── requirements.txt # python-telegram-bot, Pillow, aiohttp
└── Procfile
```

## 🚀 Installation

**Prerequisites:** Python 3.10+ and a bot token from [@BotFather](https://t.me/BotFather).

```bash
git clone https://github.com/Abdullayews/Themezzz.git
cd Themezzz

pip install -r requirements.txt

export BOT_TOKEN="your_telegram_bot_token_here"
python bot.py
```

Sanity-check your setup without touching Telegram:

```bash
python smoke_test.py
```

## ☁️ Deployment (Render / Railway)

| Setting | Value |
|---|---|
| Build command | `pip install -r requirements.txt` |
| Start command | `python bot.py` |
| Env variable | `BOT_TOKEN` — your token from BotFather |

The repo ships a `render.yaml` blueprint and a self-ping keep-alive loop,
so the free instance never falls asleep.

## 📖 How it works — the 5-step flow

1. **Send an image** — any photo goes in.
2. **Choose a mode** — Dark 🌙 or Light ☀️.
3. **Pick a section** — `BG`, `Bar`, `In`, `Out`, `Text`, `Accent`, `Reply` or `Wall`.
4. **Apply a color** — palette number 1–6, ⚡ Auto, a custom `#hex`, or a 🫧 transparency tweak.
5. **Create the theme** — ✅ and the compiled `.attheme` file lands in your chat, ready to apply.

## 📄 License

Personal / educational project — made for people who are tired of default Telegram themes.
