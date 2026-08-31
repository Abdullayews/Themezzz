Themezzz — Telegram Theme Creator Bot
Themezzz is a feature-rich Telegram bot that automatically extracts color palettes from user-uploaded images to build fully customized Telegram theme files (.attheme). It features real-time 2-device preview rendering, per-section opacity controls, and custom #hex color support.
✨ Features
 * 🎨 Automatic Palette Extraction: Uses image quantization (Pillow) to extract 6 dominant, visually balanced colors from any picture.
 * 📱 Real-Time Dual-Phone Previews: Generates a live dual-device wireframe preview (Chat View + Dialog List View) showing immediate visual feedback for every tweak.
 * 🌗 Dark & Light Modes: Instantly recalculates dynamic background, text contrast, and UI surfaces based on mode selection.
 * 🎛️ Granular Section Editing: Customize colors and transparency for:
   * BG — Window background
   * Bar — Top action bar & status bar
   * In — Incoming message bubbles
   * Out — Outgoing message bubbles
   * Text — Primary UI text
   * Accent — Action elements, avatars, active tabs & FAB buttons
   * Reply — Username & quoted text headers
   * Wall — Image wallpaper or flat background
 * 🫧 Opacity Sliders: Adjust transparency levels (0% – 100%) for supported surface sections.
 * 🎯 Custom Hex Color Support: Accepts explicit hex codes (e.g., #34c7a4) directly in chat.
 * 🚀 24/7 Hosting Ready: Integrated web server and self-ping mechanism to keep the bot alive on PaaS platforms like Render.
📁 Repository Structure
Themezzz/
├── colors.py          # Color extraction, contrast logic, theme resolution & .attheme generator
├── preview.py         # PIL-based dual-phone minimal wireframe renderer
├── main.py            # Telegram bot initialization, handlers, and inline keyboard UI
├── config.py          # Configuration loader (BOT_TOKEN)
├── server.py          # Lightweight web server for health checks
├── keep_alive.py      # Self-pinging loop for 24/7 hosting
└── requirements.txt   # Python dependencies

🛠️ Installation & Setup
Prerequisites
 * Python 3.10+
 * A Telegram Bot Token (from @BotFather)
1. Clone the repository
git clone https://github.com/Abdullayews/Themezzz.git
cd Themezzz

2. Install dependencies
pip install -r requirements.txt

3. Environment Configuration
Create a .env file or export your environment variables directly:
export BOT_TOKEN="your_telegram_bot_token_here"

4. Run the Bot
python main.py

📋 Requirements (requirements.txt)
python-telegram-bot>=20.0
Pillow>=9.0.0
aiohttp

☁️ Deployment (Render / Railway / Heroku)
 * Build Command:
   pip install -r requirements.txt

 * Start Command:
   python main.py

 * Environment Variables:
   * BOT_TOKEN: Your Telegram bot token provided by BotFather.
📖 How It Works
 * Send an Image: Upload any photo to the bot.
 * Choose Mode: Select Dark 🌙 or Light ☀️ mode.
 * Select Section: Click on the section you want to edit (BG, Bar, In, Out, Text, Accent, Reply, Wall).
 * Apply Colors & Alpha: Pick an extracted color palette number (1-6), use ⚡ Auto, type a #hex code, or adjust transparency with the 🫧 slider.
 * Generate Theme: Click ✅ Create theme to instantly receive your compiled .attheme file ready to apply in Te
 legram.
