import asyncio
import io
import logging
import re

from telegram import InlineKeyboardButton, InlineKeyboardMarkup, InputMediaPhoto, Update
from telegram.error import BadRequest
from telegram.ext import (
    Application, CallbackQueryHandler, CommandHandler,
    ContextTypes, MessageHandler, filters,
)

from config import BOT_TOKEN
from server import start_server
from colors import extract_palette, prepare_wallpaper, build_attheme
from preview import render_preview

logging.basicConfig(format="%(asctime)s | %(levelname)s | %(message)s", level=logging.INFO)
logger = logging.getLogger("theme_bot")

HEX_RE = re.compile(r"^#?\s*([0-9a-fA-F]{6})$")

CHROMA_OPTS = [("Muted", 0.65), ("Balanced", 1.0), ("Vivid", 1.35)]
CONTRAST_OPTS = [("Soft", -1), ("Normal", 0), ("Sharp", 1)]
ROLE_OPTS = [("Primary", "primary"), ("Secondary", "secondary"), ("Tertiary", "tertiary")]


# ---------- State ----------

def default_state(palette, wall_bytes):
    return dict(
        palette=palette, custom=None, seed=0, alpha=0, style="dark",
        role_idx=0, chroma_idx=1, contrast_idx=1,
        wall_mode="image" if wall_bytes else "flat",
        wall_bytes=wall_bytes, msg_id=None,
    )


def choices(st) -> list:
    return st["palette"] + ([st["custom"]] if st["custom"] else [])


# ---------- UI ----------

def summary(st) -> str:
    seed = choices(st)[st["seed"]]
    wall = "Image" if (st["wall_mode"] == "image" and st["wall_bytes"]) else "Flat color"
    return (
        f"Style: {'Dark' if st['style'] == 'dark' else 'Light'}\n"
        f"Transparency: {st['alpha']}%\n"
        f"Seed color: {st['seed'] + 1} - {seed}\n"
        f"Outgoing bubble: {ROLE_OPTS[st['role_idx']][0]}\n"
        f"Chroma: {CHROMA_OPTS[st['chroma_idx']][0]} | "
        f"Contrast: {CONTRAST_OPTS[st['contrast_idx']][0]}\n"
        f"Wallpaper: {wall}"
    )


def short_summary(st) -> str:
    return (
        f"{st['style'].capitalize()} | {st['alpha']}% transparent | "
        f"{choices(st)[st['seed']]} | {ROLE_OPTS[st['role_idx']][0]} | "
        f"{CHROMA_OPTS[st['chroma_idx']][0]} | {CONTRAST_OPTS[st['contrast_idx']][0]}"
    )


def keyboard(st) -> InlineKeyboardMarkup:
    sel = st["seed"]
    rows = [
        [   # Transparency slider
            InlineKeyboardButton("-10", callback_data="a:-10"),
            InlineKeyboardButton("-5", callback_data="a:-5"),
            InlineKeyboardButton(f"🫧 {st['alpha']}%", callback_data="noop"),
            InlineKeyboardButton("+5", callback_data="a:5"),
            InlineKeyboardButton("+10", callback_data="a:10"),
        ],
        [   # Quick transparency
            InlineKeyboardButton("0%", callback_data="s:0"),
            InlineKeyboardButton("25%", callback_data="s:25"),
            InlineKeyboardButton("50%", callback_data="s:50"),
            InlineKeyboardButton("75%", callback_data="s:75"),
            InlineKeyboardButton("100%", callback_data="s:100"),
        ],
        [   # Style
            InlineKeyboardButton(("● " if st["style"] == "dark" else "○ ") + "🌙 Dark",
                                 callback_data="st:dark"),
            InlineKeyboardButton(("● " if st["style"] == "light" else "○ ") + "☀️ Light",
                                 callback_data="st:light"),
        ],
        [   # Outgoing bubble color role
            InlineKeyboardButton(("● " if i == st["role_idx"] else "○ ") + name,
                                 callback_data=f"role:{i}")
            for i, (name, _) in enumerate(ROLE_OPTS)
        ],
        [   # Chroma
            InlineKeyboardButton(("● " if i == st["chroma_idx"] else "○ ") + name,
                                 callback_data=f"ch:{i}")
            for i, (name, _) in enumerate(CHROMA_OPTS)
        ],
        [   # Contrast
            InlineKeyboardButton(("● " if i == st["contrast_idx"] else "○ ") + name,
                                 callback_data=f"ct:{i}")
            for i, (name, _) in enumerate(CONTRAST_OPTS)
        ],
    ]
    if st["wall_bytes"]:
        rows.append([
            InlineKeyboardButton(("● " if st["wall_mode"] == "image" else "○ ") + "🖼 Image bg",
                                 callback_data="wp:image"),
            InlineKeyboardButton(("● " if st["wall_mode"] == "flat" else "○ ") + "🎨 Flat bg",
                                 callback_data="wp:flat"),
        ])
    # Seed color choices (shown as numbered circles in the preview)
    rows.append([
        InlineKeyboardButton(("● " if i == sel else "○ ") + str(i + 1),
                             callback_data=f"seed:{i}")
        for i in range(len(choices(st)))
    ])
    rows.append([
        InlineKeyboardButton("✅ Create theme", callback_data="make"),
        InlineKeyboardButton("🔄 Reset", callback_data="reset"),
    ])
    return InlineKeyboardMarkup(rows)


def build_payload(st):
    seed = choices(st)[st["seed"]]
    chroma = CHROMA_OPTS[st["chroma_idx"]][1]
    contrast = CONTRAST_OPTS[st["contrast_idx"]][1]
    role = ROLE_OPTS[st["role_idx"]][1]
    wall = st["wall_bytes"] if st["wall_mode"] == "image" else None

    png = render_preview(seed, st["style"], st["alpha"], role, chroma, contrast,
                         choices(st), st["seed"], short_summary(st), wall)
    caption = (
        "🎨 Material You — Theme Editor\n"
        "─────────────────────\n"
        f"{summary(st)}\n"
        "─────────────────────\n"
        "• Pick a seed color by number (circles in the preview)\n"
        "• Or type your own: #34c7a4\n"
        "• Preview updates on every change"
    )
    return png, caption, keyboard(st)


async def refresh(chat_id, st, context):
    png, caption, kb = build_payload(st)
    try:
        await context.bot.edit_message_media(
            chat_id=chat_id, message_id=st["msg_id"],
            media=InputMediaPhoto(png, caption=caption), reply_markup=kb,
        )
    except BadRequest:
        pass  # "message is not modified" — nothing changed, fine


# ---------- Handlers ----------

async def cmd_start(update: Update, context: ContextTypes.DEFAULT_TYPE):
    context.user_data.clear()
    await update.message.reply_text(
        "👋 Welcome to the Material You Theme Editor!\n\n"
        "📸 Send me any image — I'll build a Material 3 palette from its colors.\n\n"
        "Then tune everything with the buttons below the preview:\n"
        "🫧 transparency • 🌗 style • 🎨 seed color\n"
        "⚡ chroma • 🔆 contrast • 🖼 wallpaper\n\n"
        "Type your own color anytime: #34c7a4\n\n"
        "Press ✅ Create theme and your .attheme is generated instantly."
    )


async def on_image(update: Update, context: ContextTypes.DEFAULT_TYPE):
    msg = update.message
    ref = msg.photo[-1] if msg.photo else msg.document
    try:
        tg_file = await ref.get_file()
        buf = io.BytesIO()
        await tg_file.download_to_memory(buf)
        raw = buf.getvalue()
        palette = extract_palette(raw)
        wall = prepare_wallpaper(raw)
    except Exception as e:
        logger.error(f"Image error: {e}")
        await msg.reply_text("😕 Couldn't read the image (max 20 MB). Please try another one.")
        return

    # Remove the old preview message
    old_id = context.user_data.get("msg_id")
    if old_id:
        try:
            await context.bot.delete_message(chat_id=msg.chat_id, message_id=old_id)
        except Exception:
            pass

    st = default_state(palette, wall)
    png, caption, kb = build_payload(st)
    m = await msg.reply_photo(photo=png, caption=caption, reply_markup=kb)
    st["msg_id"] = m.message_id
    context.user_data.update(st)


async def on_text(update: Update, context: ContextTypes.DEFAULT_TYPE):
    st = context.user_data
    m = HEX_RE.match(update.message.text.strip())
    if not m:
        if "palette" in st:
            await update.message.reply_text("💡 Type a color in #RRGGBB format (e.g. #34c7a4)")
        return
    if "palette" not in st:
        await update.message.reply_text("📸 Send an image first, then type your color.")
        return
    st["custom"] = "#" + m.group(1).lower()
    st["seed"] = len(choices(st)) - 1
    await refresh(update.message.chat_id, st, context)


async def on_callback(update: Update, context: ContextTypes.DEFAULT_TYPE):
    q = update.callback_query
    st = context.user_data

    if "palette" not in st:
        await q.answer("Old session — send a new image 📸", show_alert=True)
        return

    data = q.data

    if data == "noop":
        await q.answer()
        return

    if data.startswith("a:"):
        await q.answer()
        st["alpha"] = max(0, min(100, st["alpha"] + int(data[2:])))
    elif data.startswith("s:"):
        await q.answer()
        st["alpha"] = int(data[2:])
    elif data.startswith("st:"):
        await q.answer()
        st["style"] = data[3:]
    elif data.startswith("role:"):
        await q.answer()
        st["role_idx"] = int(data[5:])
    elif data.startswith("ch:"):
        await q.answer()
        st["chroma_idx"] = int(data[3:])
    elif data.startswith("ct:"):
        await q.answer()
        st["contrast_idx"] = int(data[3:])
    elif data.startswith("wp:"):
        await q.answer()
        st["wall_mode"] = data[3:]
    elif data.startswith("seed:"):
        await q.answer()
        st["seed"] = max(0, min(len(choices(st)) - 1, int(data[5:])))
    elif data == "reset":
        await q.answer("🔄 Reset done")
        keep = {"palette": st["palette"], "wall_bytes": st["wall_bytes"],
                "msg_id": st["msg_id"]}
        st.clear()
        st.update(default_state(keep["palette"], keep["wall_bytes"]))
        st["msg_id"] = keep["msg_id"]
    elif data == "make":
        await q.answer("⏳ Generating theme...")
        seed = choices(st)[st["seed"]]
        theme = build_attheme(
            seed, st["style"], st["alpha"],
            ROLE_OPTS[st["role_idx"]][1],
            CHROMA_OPTS[st["chroma_idx"]][1],
            CONTRAST_OPTS[st["contrast_idx"]][1],
            st["wall_bytes"] if st["wall_mode"] == "image" else None,
        )
        fname = f"MaterialYou_{st['style']}_{seed.lstrip('#')}_{st['alpha']}pct.attheme"
        await context.bot.send_document(
            chat_id=q.message.chat_id, document=theme, filename=fname,
            caption=(f"🎨 Your theme is ready!\n{summary(st)}\n\n"
                     "Open the file — Telegram applies it instantly ✨"),
        )
        return
    else:
        await q.answer()
        return

    await refresh(q.message.chat_id, st, context)


def main():
    if not BOT_TOKEN:
        raise SystemExit("❌ BOT_TOKEN not found!")

    start_server()  # Port binding required by Render

    # Python 3.12+/3.14: asyncio.get_event_loop() no longer auto-creates
    # a loop. Required regardless of PTB version — keep as is.
    try:
        asyncio.get_event_loop()
    except RuntimeError:
        asyncio.set_event_loop(asyncio.new_event_loop())

    app = Application.builder().token(BOT_TOKEN).build()
    app.add_handler(CommandHandler(["start", "help"], cmd_start))
    app.add_handler(MessageHandler(filters.PHOTO | filters.Document.IMAGE, on_image))
    app.add_handler(MessageHandler(filters.TEXT & ~filters.COMMAND, on_text))
    app.add_handler(CallbackQueryHandler(on_callback))

    logger.info("🚀 Material You Theme Bot started")
    app.run_polling(drop_pending_updates=True)


if __name__ == "__main__":
    main()
