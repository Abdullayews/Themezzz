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
from colors import extract_palette, prepare_wallpaper, build_attheme
from preview import render_preview

logging.basicConfig(format="%(asctime)s | %(levelname)s | %(message)s", level=logging.INFO)
logger = logging.getLogger("theme_bot")

HEX_RE = re.compile(r"^#?\s*([0-9a-fA-F]{6})$")
ROLES = ["primary", "secondary", "tertiary"]

WELCOME = (
    "🎨 <b>Material You Theme Bot</b>\n\n"
    "Hi! I create Telegram themes from your pictures.\n\n"
    "<b>How it works</b>\n"
    "1️⃣ Send me any image\n"
    "2️⃣ I extract its colors into a Material 3 palette\n"
    "3️⃣ Tune the options with the buttons under the preview\n"
    "4️⃣ Press ✅ and get your <code>.attheme</code>\n\n"
    "📸 <b>Send a picture to begin!</b>"
)


# ---------- State ----------

def new_state(palette, wall):
    return {
        "palette": palette,          # ["#rrggbb", ...]
        "custom": None,              # "#rrggbb" or None
        "seed": 0,                   # selected color index
        "alpha": 0,                  # bubble transparency 0..100
        "style": "dark",             # "dark" | "light"
        "role": "primary",           # outgoing bubble color
        "wall": wall,                # wallpaper JPEG bytes | None
        "wall_mode": "image" if wall else "flat",
        "msg_id": None,
        "mode": "photo",             # preview message type: "photo" | "text"
    }


def color_choices(st):
    return st["palette"] + ([st["custom"]] if st["custom"] else [])


def current_seed(st):
    ch = color_choices(st)
    return ch[max(0, min(st["seed"], len(ch) - 1))]


# ---------- UI ----------

def editor_caption(st):
    wall = "Image" if (st["wall_mode"] == "image" and st["wall"]) else "Flat color"
    return (
        "🎨 <b>Your theme</b>\n"
        "──────────────────\n"
        f"🌗 Style: <b>{'Dark' if st['style'] == 'dark' else 'Light'}</b>\n"
        f"🫧 Bubble transparency: <b>{st['alpha']}%</b>\n"
        f"🎨 Color: <b>{current_seed(st)}</b> (№{st['seed'] + 1})\n"
        f"💬 Outgoing bubble: <b>{st['role'].capitalize()}</b>\n"
        f"🖼 Wallpaper: <b>{wall}</b>\n"
        "──────────────────\n"
        "1️⃣ Pick a color №   2️⃣ Set transparency\n"
        "3️⃣ Press ✅ <b>Create theme</b>\n\n"
        "💡 Custom color? Just type it: <code>#34c7a4</code>"
    )


def keyboard(st):
    ch = color_choices(st)
    rows = [
        [
            InlineKeyboardButton("-10", callback_data="a:-10"),
            InlineKeyboardButton("-5", callback_data="a:-5"),
            InlineKeyboardButton(f"🫧 {st['alpha']}%", callback_data="noop"),
            InlineKeyboardButton("+5", callback_data="a:5"),
            InlineKeyboardButton("+10", callback_data="a:10"),
        ],
        [
            InlineKeyboardButton("0%", callback_data="s:0"),
            InlineKeyboardButton("25%", callback_data="s:25"),
            InlineKeyboardButton("50%", callback_data="s:50"),
            InlineKeyboardButton("75%", callback_data="s:75"),
            InlineKeyboardButton("100%", callback_data="s:100"),
        ],
        [InlineKeyboardButton(f"🌗 Style: {'Dark' if st['style'] == 'dark' else 'Light'}",
                              callback_data="st:toggle")],
        [InlineKeyboardButton(f"💬 Bubble: {st['role'].capitalize()}",
                              callback_data="role:cycle")],
    ]
    if st["wall"]:
        mode = "Image" if st["wall_mode"] == "image" else "Flat"
        rows.append([InlineKeyboardButton(f"🖼 Wallpaper: {mode}",
                                          callback_data="wp:toggle")])
    color_row = []
    for i in range(len(ch)):
        mark = "●" if i == st["seed"] else "○"
        label = f"{mark} C" if (i == len(ch) - 1 and st["custom"]) else f"{mark} {i + 1}"
        color_row.append(InlineKeyboardButton(label, callback_data=f"seed:{i}"))
    rows.append(color_row)
    rows.append([
        InlineKeyboardButton("✅ Create theme", callback_data="make"),
        InlineKeyboardButton("🔄 Reset", callback_data="reset"),
    ])
    return InlineKeyboardMarkup(rows)


# ---------- Editing helpers ----------

async def _edit_media_safe(bot, chat_id, msg_id, png, cap, kb):
    for _ in range(2):
        png.seek(0)
        media = InputMediaPhoto(png, caption=cap, parse_mode="HTML")
        try:
            await bot.edit_message_media(chat_id=chat_id, message_id=msg_id,
                                         media=media, reply_markup=kb)
            return True
        except RetryAfter as e:
            await asyncio.sleep(float(e.retry_after) + 1)
        except BadRequest:
            return False  # "message is not modified" etc. — fine
        except Exception as e:
            logger.warning(f"Media edit failed: {e}")
            return False
    return False


async def refresh_editor(chat_id, st, context):
    kb = keyboard(st)
    cap = editor_caption(st)
    msg_id = st["msg_id"]
    if not msg_id:
        return

    if st["mode"] == "photo":
        png = None
        try:
            png = render_preview(current_seed(st), st["style"], st["alpha"], st["role"],
                                 color_choices(st), st["seed"],
                                 st["wall"] if st["wall_mode"] == "image" else None)
        except Exception as e:
            logger.error(f"Preview render failed: {e}")

        if png is not None and await _edit_media_safe(context.bot, chat_id, msg_id,
                                                      png, cap, kb):
            return
        # Fallback: preview şəkli köhnə qala bilər, amma mətn + düymələr yenilənir
        try:
            await context.bot.edit_message_caption(chat_id=chat_id, message_id=msg_id,
                                                   caption=cap, reply_markup=kb,
                                                   parse_mode="HTML")
        except Exception:
            pass
    else:
        try:
            await context.bot.edit_message_text(cap, chat_id=chat_id, message_id=msg_id,
                                                reply_markup=kb, parse_mode="HTML")
        except RetryAfter as e:
            await asyncio.sleep(float(e.retry_after) + 1)
        except Exception:
            pass


# ---------- Handlers ----------

async def cmd_start(update: Update, context: ContextTypes.DEFAULT_TYPE):
    context.user_data.clear()
    await update.message.reply_text(WELCOME, parse_mode="HTML")


async def send_editor(msg, st, context):
    kb = keyboard(st)
    cap = editor_caption(st)
    try:
        png = render_preview(current_seed(st), st["style"], st["alpha"], st["role"],
                             color_choices(st), st["seed"],
                             st["wall"] if st["wall_mode"] == "image" else None)
        png.name = "preview.png"
        sent = await msg.reply_photo(photo=png, caption=cap, reply_markup=kb,
                                     parse_mode="HTML")
        st["mode"] = "photo"
    except Exception as e:
        logger.error(f"Preview failed → text mode: {e}")
        sent = await msg.reply_text(cap, reply_markup=kb, parse_mode="HTML")
        st["mode"] = "text"
    st["msg_id"] = sent.message_id


async def on_image(update: Update, context: ContextTypes.DEFAULT_TYPE):
    msg = update.message
    ref = msg.photo[-1] if msg.photo else msg.document
    try:
        f = await ref.get_file()
        buf = io.BytesIO()
        await f.download_to_memory(buf)
        raw = buf.getvalue()
    except Exception as e:
        logger.error(f"Download error: {e}")
        await msg.reply_text("😕 I couldn't download that image. "
                             "Send it again as a photo (max 20 MB).")
        return

    if not raw:
        await msg.reply_text("😕 Empty file — try another image.")
        return

    palette = extract_palette(raw)          # never raises
    try:
        wall = prepare_wallpaper(raw)
    except Exception:
        wall = None

    # Delete previous editor message
    old = context.user_data.get("msg_id")
    if old:
        try:
            await context.bot.delete_message(chat_id=msg.chat_id, message_id=old)
        except Exception:
            pass

    context.user_data.clear()
    st = context.user_data
    st.update(new_state(palette, wall))
    await send_editor(msg, st, context)


async def on_text(update: Update, context: ContextTypes.DEFAULT_TYPE):
    st = context.user_data
    text = (update.message.text or "").strip()
    m = HEX_RE.match(text)

    if not m:
        if "palette" in st:
            await update.message.reply_text(
                "💡 Type a color like <code>#34c7a4</code>, "
                "or use the buttons under the preview.", parse_mode="HTML")
        else:
            await update.message.reply_text("📸 Send me an image to create a theme!")
        return

    if "palette" not in st:
        await update.message.reply_text("📸 Send me an image first, then type your color.")
        return

    st["custom"] = "#" + m.group(1).lower()
    st["seed"] = len(color_choices(st)) - 1
    await refresh_editor(update.effective_chat.id, st, context)


async def send_theme(chat_id, st, context):
    seed = current_seed(st)
    try:
        theme = build_attheme(seed, st["style"], st["alpha"], st["role"],
                              st["wall"] if st["wall_mode"] == "image" else None)
    except Exception as e:
        logger.error(f"Theme build failed: {e}")
        await context.bot.send_message(chat_id=chat_id,
                                       text="😕 Something broke while creating the theme. "
                                            "Press ✅ again or press 🔄 Reset.")
        return

    fname = f"MaterialYou_{st['style']}_{seed.lstrip('#')}_{st['alpha']}.attheme"
    await context.bot.send_document(
        chat_id=chat_id, document=theme, filename=fname,
        caption=(f"🎨 <b>Your theme is ready!</b>\n"
                 f"Seed: {seed} | Style: {st['style']} | Transparency: {st['alpha']}%\n\n"
                 "Open the file — Telegram applies it instantly ✨"),
        parse_mode="HTML",
    )


async def on_callback(update: Update, context: ContextTypes.DEFAULT_TYPE):
    q = update.callback_query
    st = context.user_data
    data = q.data

    if "palette" not in st:
        await q.answer("Send a new image 📸", show_alert=True)
        return

    if data == "make":
        await q.answer("⏳ Creating your theme...")
        await send_theme(update.effective_chat.id, st, context)
        return

    if data == "noop":
        await q.answer()
        return

    if data.startswith("a:"):
        st["alpha"] = max(0, min(100, st["alpha"] + int(data[2:])))
    elif data.startswith("s:"):
        st["alpha"] = max(0, min(100, int(data[2:])))
    elif data == "st:toggle":
        st["style"] = "light" if st["style"] == "dark" else "dark"
    elif data == "role:cycle":
        st["role"] = ROLES[(ROLES.index(st["role"]) + 1) % len(ROLES)]
    elif data == "wp:toggle" and st["wall"]:
        st["wall_mode"] = "flat" if st["wall_mode"] == "image" else "image"
    elif data.startswith("seed:"):
        st["seed"] = max(0, min(len(color_choices(st)) - 1, int(data[5:])))
    elif data == "reset":
        pal, wall, mid, mode = st["palette"], st["wall"], st["msg_id"], st["mode"]
        st.clear()
        st.update(new_state(pal, wall))
        st["msg_id"], st["mode"] = mid, mode

    await q.answer()
    await refresh_editor(update.effective_chat.id, st, context)


async def on_error(update, context):
    logger.error("Unhandled error:", exc_info=context.error)


def main():
    if not BOT_TOKEN:
        raise SystemExit("❌ BOT_TOKEN is not set!")

    start_server()  # Render port requirement

    # Python 3.12+: asyncio needs an explicit event loop
    try:
        asyncio.get_event_loop()
    except RuntimeError:
        asyncio.set_event_loop(asyncio.new_event_loop())

    app = Application.builder().token(BOT_TOKEN).build()
    app.add_handler(CommandHandler(["start", "help"], cmd_start))
    app.add_handler(MessageHandler(filters.PHOTO | filters.Document.IMAGE, on_image))
    app.add_handler(MessageHandler(filters.TEXT & ~filters.COMMAND, on_text))
    app.add_handler(CallbackQueryHandler(on_callback))
    app.add_error_handler(on_error)

    logger.info("🚀 Theme bot is up")
    app.run_polling(drop_pending_updates=True)


if __name__ == "__main__":
    main()
