import asyncio
import io
import logging
import re

from telegram import InlineKeyboardButton, InlineKeyboardMarkup, InputMediaPhoto, Update
from telegram.error import BadRequest, RetryAfter
from telegram.ext import (
    Application, CallbackQueryHandler, CommandHandler,
    ContextTypes, MessageHandler, filters,
)

from config import BOT_TOKEN
from server import start_server
from colors import (
    extract_palette, prepare_wallpaper, build_attheme,
    resolve_theme, resolve_wall, rgb_to_hex,
)
from preview import render_preview

logging.basicConfig(format="%(asctime)s | %(levelname)s | %(message)s", level=logging.INFO)
logger = logging.getLogger("theme_bot")

HEX_RE = re.compile(r"^#?\s*([0-9a-fA-F]{6})$")

SECTIONS = [
    ("bg", "🖼 BG"), ("bar", "📊 Bar"), ("in", "📥 In"), ("out", "📤 Out"),
    ("text", "✏️ Text"), ("accent", "🔵 Accent"), ("wall", "🌇 Wall"),
]
SEC_KEYS = [k for k, _ in SECTIONS]
SEC_LABEL = dict(SECTIONS)
SEC_FULL = {"bg": "Background", "bar": "Top bar", "in": "Incoming bubble",
            "out": "Outgoing bubble", "text": "Text", "accent": "Accent",
            "wall": "Wallpaper"}
ALPHA_SECTIONS = ("bg", "bar", "in", "out", "text", "accent")

WELCOME = (
    "🎨 <b>Theme Creator</b>\n\n"
    "Hi! I build Telegram themes (.attheme) from your picture.\n\n"
    "<b>How it works</b>\n"
    "1️⃣ Send me any image — I grab its colors\n"
    "2️⃣ Pick a part: BG, Bar, In/Out bubbles, Text, Accent, Wallpaper\n"
    "3️⃣ For each part choose a color (⚡ auto, № swatch, or type #hex)\n"
    "    and set its transparency with the slider\n"
    "4️⃣ ✅ Create theme — done!\n\n"
    "📸 <b>Send a picture to start!</b>"
)


# ---------- State ----------

def default_state(palette, wall_bytes):
    return {
        "palette": palette,
        "sections": {k: {"idx": -1, "alpha": 0, "custom": None} for k in ALPHA_SECTIONS},
        "active": "bg",
        "wall_mode": "image" if wall_bytes else "flat",
        "wall_idx": -1,
        "wall_custom": None,
        "wall_bytes": wall_bytes,
        "msg_id": None,
        "mode": "photo",
    }


def active_color_state(st):
    if st["active"] == "wall":
        return {"idx": st["wall_idx"], "custom": st["wall_custom"]}
    return st["sections"][st["active"]]


def set_color_idx(st, idx):
    if st["active"] == "wall":
        st["wall_idx"] = idx
        st["wall_custom"] = None
    else:
        s = st["sections"][st["active"]]
        s["idx"] = idx
        s["custom"] = None


def set_custom(st, hexcol):
    if st["active"] == "wall":
        st["wall_mode"] = "flat"
        st["wall_custom"] = hexcol
        st["wall_idx"] = -1
    else:
        s = st["sections"][st["active"]]
        s["custom"] = hexcol
        s["idx"] = -1


def color_src(st):
    cs = active_color_state(st)
    if cs["custom"]:
        return "custom"
    if cs["idx"] >= 0:
        return f"color {cs['idx'] + 1
