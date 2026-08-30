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
        return f"color {cs['idx'] + 1}"
    return "auto"


# ---------- UI ----------

def caption(st):
    res = resolve_theme(st["palette"], st["sections"])
    lines = ["🎨 <b>Theme Editor</b>", "──────────────"]
    for k in SEC_KEYS:
        mark = "▸ " if st["active"] == k else "· "
        if k == "wall":
            if st["wall_mode"] == "image" and st["wall_bytes"]:
                val = "your image"
            else:
                val = rgb_to_hex(resolve_wall(st["palette"], st["wall_idx"], st["wall_custom"]))
        else:
            s = st["sections"][k]
            tr = f" · {s['alpha']}%" if s["alpha"] else ""
            val = f"{rgb_to_hex(res[k])}{tr}"
        lines.append(f"{mark}{SEC_FULL[k]}: <b>{val}</b>")
    lines.append("──────────────")
    lines.append(f"Editing <b>{SEC_FULL[st['active']]}</b> — pick a color below "
                 "or type your own: <code>#34c7a4</code>")
    if st["active"] in ALPHA_SECTIONS:
        lines.append("🫧 slider changes transparency of this part")
    return "\n".join(lines)


def color_row(st):
    cs = active_color_state(st)
    is_auto = cs["idx"] == -1 and not cs["custom"]
    row = [InlineKeyboardButton("⚡" + ("●" if is_auto else ""), callback_data="auto")]
    for i in range(len(st["palette"])):
        mark = "●" if cs["idx"] == i else ""
        row.append(InlineKeyboardButton(f"{mark}{i + 1}", callback_data=f"c:{i}"))
    if cs["custom"]:
        row.append(InlineKeyboardButton("🎯●", callback_data="noop"))
    return row


def keyboard(st):
    rows = [
        [InlineKeyboardButton(("● " if st["active"] == k else "") + lbl,
                              callback_data=f"sec:{k}") for k, lbl in SECTIONS[:4]],
        [InlineKeyboardButton(("● " if st["active"] == k else "") + lbl,
                              callback_data=f"sec:{k}") for k, lbl in SECTIONS[4:]],
    ]
    if st["active"] == "wall":
        rows.append([
            InlineKeyboardButton(("● " if st["wall_mode"] == "image" and st["wall_bytes"] else "○ ")
                                 + "🖼 Image", callback_data="wp:image"),
            InlineKeyboardButton(("● " if st["wall_mode"] == "flat" else "○ ")
                                 + "🎨 Flat", callback_data="wp:flat"),
        ])
        if st["wall_mode"] == "flat":
            rows.append(color_row(st))
    else:
        rows.append(color_row(st))
        a = st["sections"][st["active"]]["alpha"]
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


def build_payload(st):
    res = resolve_theme(st["palette"], st["sections"])
    alphas = {k: st["sections"][k]["alpha"] for k in ALPHA_SECTIONS}
    wall = st["wall_bytes"] if (st["wall_mode"] == "image" and st["wall_bytes"]) else None
    wall_flat = resolve_wall(st["palette"], st["wall_idx"], st["wall_custom"])
    info = f"{SEC_FULL[st['active']]} · {color_src(st)}"
    png = render_preview(res, alphas, st["palette"], st["active"],
                         active_color_state(st), info, wall, wall_flat)
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
                ok = True  # nothing changed
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

    palette = extract_palette(raw)                    # never raises
    try:
        wall = prepare_wallpaper(raw)
    except Exception:
        wall = None

    old = context.user_data.get("msg_id")
    if old:
        try:
            await context.bot.delete_message(chat_id=msg.chat_id, message_id=old)
        except Exception:
            pass

    context.user_data.clear()
    context.user_data.update(default_state(palette, wall))
    await send_editor(msg, context.user_data, context)


async def on_text(update: Update, context: ContextTypes.DEFAULT_TYPE):
    st = context.user_data
    m = HEX_RE.match((update.message.text or "").strip())
    if not m:
        if "palette" in st:
            await update.message.reply_text(
                "💡 Type a color like <code>#34c7a4</code> — it applies to the "
                "part you are editing.", parse_mode="HTML")
        else:
            await update.message.reply_text("📸 Send me an image to start!")
        return
    if "palette" not in st:
        await update.message.reply_text("📸 Send me an image first.")
        return
    set_custom(st, "#" + m.group(1).lower())
    await refresh_editor(update.effective_chat.id, st, context)


async def on_callback(update: Update, context: ContextTypes.DEFAULT_TYPE):
    q = update.callback_query
    st = context.user_data
    data = q.data

    if "palette" not in st:
        await q.answer("Old session — send a new image 📸", show_alert=True)
        return

    if data == "make":
        await q.answer("⏳ Creating theme...")
        res = resolve_theme(st["palette"], st["sections"])
        alphas = {k: st["sections"][k]["alpha"] for k in ALPHA_SECTIONS}
        wall = st["wall_bytes"] if (st["wall_mode"] == "image" and st["wall_bytes"]) else None
        wall_flat = None if wall else resolve_wall(st["palette"], st["wall_idx"], st["wall_custom"])
        try:
            theme = build_attheme(res, alphas, wall, wall_flat)
        except Exception as e:
            logger.error(f"Build failed: {e}")
            await context.bot.send_message(
                chat_id=update.effective_chat.id,
                text="😕 Something broke — press ✅ again or 🔄 Reset.")
            return
        fname = (f"Theme_{rgb_to_hex(res['accent'])[1:]}"
                 f"_{alphas['in']}_{alphas['out']}.attheme")
        await context.bot.send_document(
            chat_id=update.effective_chat.id, document=theme, filename=fname,
            caption="🎨 <b>Your theme is ready!</b>\n"
                    f"Accent {rgb_to_hex(res['accent'])} · "
                    f"In {alphas['in']}% · Out {alphas['out']}%\n\n"
                    "Open the file — Telegram applies it instantly ✨",
            parse_mode="HTML")
        return

    if data == "noop":
        await q.answer()
        return

    if data.startswith("sec:"):
        st["active"] = data[4:]
    elif data == "auto":
        if st["active"] == "wall":
            st["wall_idx"], st["wall_custom"] = -1, None
        else:
            s = st["sections"][st["active"]]
            s["idx"], s["custom"] = -1, None
    elif data.startswith("c:"):
        set_color_idx(st, int(data[2:]))
    elif data.startswith("a:") and st["active"] in ALPHA_SECTIONS:
        s = st["sections"][st["active"]]
        s["alpha"] = max(0, min(100, s["alpha"] + int(data[2:])))
    elif data.startswith("s:") and st["active"] in ALPHA_SECTIONS:
        st["sections"][st["active"]]["alpha"] = max(0, min(100, int(data[2:])))
    elif data == "wp:image" and st["wall_bytes"]:
        st["wall_mode"] = "image"
    elif data == "wp:flat":
        st["wall_mode"] = "flat"
    elif data == "reset":
        pal, wall, mid, mode = st["palette"], st["wall_bytes"], st["msg_id"], st["mode"]
        st.clear()
        st.update(default_state(pal, wall))
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
