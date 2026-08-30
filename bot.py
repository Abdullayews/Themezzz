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
    DEFAULT_CATS, DEFAULT_WALL,
)
from preview import render_preview

logging.basicConfig(format="%(asctime)s | %(levelname)s | %(message)s", level=logging.INFO)
logger = logging.getLogger("theme_bot")

HEX_RE = re.compile(r"^#?\s*([0-9a-fA-F]{6})$")

CATS = [
    ("bg", "🖼 BG"), ("bar", "📊 Bar"), ("in", "📥 In"), ("out", "📤 Out"),
    ("link", "🔗 Link"), ("accent", "🔵 Accent"), ("wall", "🌇 Wall"),
]
CAT_LABEL = dict(CATS)
ALPHA_CATS = {"bg", "bar", "in", "out", "accent"}

WELCOME = (
    "🎨 <b>Theme Creator</b>\n\n"
    "Hi! I build a Telegram theme (.attheme) from <b>your</b> choices — "
    "I never invent colors.\n\n"
    "<b>How it works</b>\n"
    "1️⃣ Send me any picture — it becomes the wallpaper and its colors "
    "appear as suggested swatches\n"
    "2️⃣ Pick each part: <b>BG, Bar, In/Out bubbles, Link, Accent, Wallpaper</b>\n"
    "3️⃣ For every part: choose a color (swatch or type #rrggbb) and set "
    "its <b>own</b> transparency\n"
    "4️⃣ ✅ Create theme\n\n"
    "✏️ Note: all text is always <b>white</b> — links are the only "
    "colored text.\n\n"
    "📸 <b>Send a picture to start!</b>"
)


# ---------- State ----------

def new_state(swatches, wall_bytes):
    return {
        "swatches": swatches,
        "wall_bytes": wall_bytes,
        "cats": {k: dict(v) for k, v in DEFAULT_CATS.items()},
        "wall": dict(DEFAULT_WALL),
        "wall_mode": "image" if wall_bytes else "flat",
        "active": "bg",
        "msg_id": None,
        "mode": "photo",
    }


def active_hex(st):
    if st["active"] == "wall":
        return st["wall"]["hex"]
    return st["cats"][st["active"]]["hex"]


def active_alpha(st):
    if st["active"] == "wall":
        return st["wall"]["alpha"]
    return st["cats"][st["active"]]["alpha"]


def set_active_hex(st, hexcol):
    if st["active"] == "wall":
        st["wall"]["hex"] = hexcol
        st["wall_mode"] = "flat"
    else:
        st["cats"][st["active"]]["hex"] = hexcol


def set_active_alpha(st, val):
    val = max(0, min(100, val))
    if st["active"] == "wall":
        st["wall"]["alpha"] = val
    else:
        st["cats"][st["active"]]["alpha"] = val


def slider_shown(st):
    if st["active"] in ALPHA_CATS:
        return True
    return st["active"] == "wall" and st["wall_mode"] == "flat"


# ---------- UI ----------

def caption(st):
    rows = ["🎨 <b>Theme Editor</b> — every color is your pick", "────────────"]
    for k, lbl in CATS:
        mark = "▸ " if st["active"] == k else "· "
        if k == "wall":
            if st["wall_mode"] == "image" and st["wall_bytes"]:
                val = "your image"
            else:
                val = st["wall"]["hex"]
                if st["wall"]["alpha"]:
                    val += f" · {st['wall']['alpha']}%"
        else:
            c = st["cats"][k]
            val = c["hex"]
            if k in ALPHA_CATS and c["alpha"]:
                val += f" · {c['alpha']}%"
        rows.append(f"{mark}{lbl}: <b>{val}</b>")
    rows += [
        "────────────",
        "✏️ Text is always <b>white</b> — only links use the 🔗 color.",
        f"Now editing: <b>{CAT_LABEL[st['active']]}</b> — tap a swatch "
        "or type <code>#rrggbb</code>",
    ]
    if slider_shown(st):
        rows.append("🫧 The slider sets transparency for THIS part only")
    return "\n".join(rows)


def keyboard(st):
    act = st["active"]
    rows = [
        [InlineKeyboardButton(("● " if act == k else "") + lbl, callback_data=f"sec:{k}")
         for k, lbl in CATS[:4]],
        [InlineKeyboardButton(("● " if act == k else "") + lbl, callback_data=f"sec:{k}")
         for k, lbl in CATS[4:]],
    ]
    if act == "wall":
        if st["wall_bytes"]:
            rows.append([
                InlineKeyboardButton(("● " if st["wall_mode"] == "image" else "○ ")
                                     + "🖼 Image", callback_data="wp:image"),
                InlineKeyboardButton(("● " if st["wall_mode"] == "flat" else "○ ")
                                     + "🎨 Flat color", callback_data="wp:flat"),
            ])
        if st["wall_mode"] == "flat":
            rows.append(swatch_row(st))
    else:
        rows.append(swatch_row(st))

    if slider_shown(st):
        a = active_alpha(st)
        rows.append([
            InlineKeyboardButton("-10", callback_data="a:-10"),
            InlineKeyboardButton("-5", callback_data="a:-5"),
            InlineKeyboardButton(f"🫧 {a}%", callback_data="noop"),
            InlineKeyboardButton("+5", callback_data="a:5"),
            InlineKeyboardButton("+10", callback_data="a:10"),
        ])
        rows.append([InlineKeyboardButton(f"{v}%", callback_data=f"s:{v}")
                     for v in (0, 25, 50, 75, 100)])

    rows.append([
        InlineKeyboardButton("✅ Create theme", callback_data="make"),
        InlineKeyboardButton("🔄 Reset", callback_data="reset"),
    ])
    return InlineKeyboardMarkup(rows)


def swatch_row(st):
    cur = active_hex(st)
    row = []
    for i, hexcol in enumerate(st["swatches"][:6]):
        mark = "●" if cur == hexcol else ""
        row.append(InlineKeyboardButton(f"{mark}{i + 1}", callback_data=f"sw:{i}"))
    if cur not in st["swatches"]:
        row.append(InlineKeyboardButton("🎯", callback_data="noop"))
    return row


def build_payload(st):
    png = render_preview(st["cats"], st["wall"], st["wall_mode"],
                         st["wall_bytes"], st["swatches"], st["active"])
    return png, caption(st), keyboard(st)


async def refresh_editor(chat_id, st, context):
    if not st.get("msg_id"):
        return
    png, cap, kb = build_payload(st)
    if st["mode"] == "photo":
        ok = False
        for _ in range(2):
            png.seek(0)
            try:
                await context.bot.edit_message_media(
                    chat_id=chat_id, message_id=st["msg_id"],
                    media=InputMediaPhoto(png, caption=cap, parse_mode="HTML"),
                    reply_markup=kb)
                ok = True
                break
            except RetryAfter as e:
                await asyncio.sleep(float(e.retry_after) + 1)
            except BadRequest:
                ok = True
                break
            except Exception as e:
                logger.warning(f"Media edit failed: {e}")
                break
        if not ok:
            try:
                await context.bot.edit_message_caption(
                    chat_id=chat_id, message_id=st["msg_id"],
                    caption=cap, reply_markup=kb, parse_mode="HTML")
            except Exception:
                pass
    else:
        try:
            await context.bot.edit_message_text(
                cap, chat_id=chat_id, message_id=st["msg_id"],
                reply_markup=kb, parse_mode="HTML")
        except Exception:
            pass


# ---------- Handlers ----------

async def cmd_start(update: Update, context: ContextTypes.DEFAULT_TYPE):
    context.user_data.clear()
    await update.message.reply_text(WELCOME, parse_mode="HTML")


async def send_editor(msg, st, context):
    png, cap, kb = build_payload(st)
    try:
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
        await msg.reply_text("😕 Couldn't download that image. Try again (max 20 MB).")
        return
    if not raw:
        await msg.reply_text("😕 Empty file — try another image.")
        return

    swatches = extract_palette(raw)               # suggestions only, never raises
    try:
        wall = prepare_wallpaper(raw)             # used as-is, untouched colors
    except Exception:
        wall = None

    old = context.user_data.get("msg_id")
    if old:
        try:
            await context.bot.delete_message(chat_id=msg.chat_id, message_id=old)
        except Exception:
            pass

    context.user_data.clear()
    context.user_data.update(new_state(swatches, wall))
    await send_editor(msg, context.user_data, context)


async def on_text(update: Update, context: ContextTypes.DEFAULT_TYPE):
    st = context.user_data
    m = HEX_RE.match((update.message.text or "").strip())
    if not m:
        if "cats" in st:
            await update.message.reply_text(
                "💡 Type a color like <code>#34c7a4</code> — it applies to the "
                "part you're editing.", parse_mode="HTML")
        else:
            await update.message.reply_text("📸 Send me an image to start!")
        return
    if "cats" not in st:
        await update.message.reply_text("📸 Send me an image first.")
        return
    set_active_hex(st, "#" + m.group(1).lower())
    await refresh_editor(update.effective_chat.id, st, context)


async def on_callback(update: Update, context: ContextTypes.DEFAULT_TYPE):
    q = update.callback_query
    st = context.user_data
    data = q.data

    if "cats" not in st:
        await q.answer("Old session — send a new image 📸", show_alert=True)
        return

    if data == "make":
        await q.answer("⏳ Creating theme...")
        wall_img = st["wall_bytes"] if (st["wall_mode"] == "image"
                                        and st["wall_bytes"]) else None
        try:
            theme = build_attheme(st["cats"], wall_img,
                                  st["wall"]["hex"], st["wall"]["alpha"])
        except Exception as e:
            logger.error(f"Build failed: {e}")
            await context.bot.send_message(
                chat_id=update.effective_chat.id,
                text="😕 Something broke — press ✅ again or 🔄 Reset.")
            return
        fname = f"MyTheme_{st['cats']['accent']['hex'][1:]}.attheme"
        await context.bot.send_document(
            chat_id=update.effective_chat.id, document=theme, filename=fname,
            caption="🎨 <b>Your theme is ready!</b>\n"
                    "All text white · links colored · your colors everywhere else\n\n"
                    "Open the file — Telegram applies it instantly ✨",
            parse_mode="HTML")
        return

    if data == "noop":
        await q.answer()
        return

    if data.startswith("sec:"):
        st["active"] = data[4:]
    elif data.startswith("sw:"):
        i = int(data[3:])
        if 0 <= i < len(st["swatches"]):
            set_active_hex(st, st["swatches"][i])
    elif data == "wp:image" and st["wall_bytes"]:
        st["wall_mode"] = "image"
    elif data == "wp:flat":
        st["wall_mode"] = "flat"
    elif data.startswith("a:") and slider_shown(st):
        set_active_alpha(st, active_alpha(st) + int(data[2:]))
    elif data.startswith("s:") and slider_shown(st):
        set_active_alpha(st, int(data[2:]))
    elif data == "reset":
        sw, wb, mid, mode = (st["swatches"], st["wall_bytes"],
                             st["msg_id"], st["mode"])
        st.clear()
        st.update(new_state(sw, wb))
        st["msg_id"], st["mode"] = mid, mode

    await q.answer()
    await refresh_editor(update.effective_chat.id, st, context)


async def on_error(update, context):
    logger.error("Unhandled error:", exc_info=context.error)


def main():
    if not BOT_TOKEN:
        raise SystemExit("❌ BOT_TOKEN is not set!")

    start_server()

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
