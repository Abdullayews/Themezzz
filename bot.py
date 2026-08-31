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
from keep_alive import start_keep_alive
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
    ("text", "🐻‍❄️ Text"), ("accent", "🔵 Accent"), ("reply", "🏷 Reply"),
    ("wall", "🌇 Wall"),
]
SEC_LABEL = dict(SECTIONS)
SEC_FULL = {"bg": "Background", "bar": "Top bar", "in": "Incoming bubble",
            "out": "Outgoing bubble", "text": "Text", "accent": "Accent",
            "reply": "Reply (tag)", "wall": "Wallpaper"}

# Sections with a color choice
COLOR_SECTIONS = ("bg", "bar", "in", "out", "text", "accent", "reply")
# Sections with a transparency slider.
# ⚠️ "bg" excluded on purpose: transparent windows broke the drawer
# and the forward screen — those are always solid now.
ALPHA_SECTIONS = ("bar", "in", "out", "text", "accent", "reply")

WELCOME = (
    "🎨 <b>Theme Creator</b>\n\n"
    "Hi! I build Telegram themes (.attheme) from your picture.\n\n"
    "<b>How it works</b>\n"
    "1️⃣ Send me any image — I grab its colors\n"
    "2️⃣ Pick 🌙 Dark or ☀️ Light mode\n"
    "3️⃣ Pick a part: BG, Bar, In/Out bubbles, 🐻‍❄️ Text, Accent, Reply, Wallpaper\n"
    "4️⃣ Choose a color (⚡ auto, №1-6 suggested, or type #hex)\n"
    "    and set transparency with the slider\n"
    "5️⃣ ✅ Create theme — done!\n\n"
    "🐻‍❄️ Text is white by default in Dark mode\n"
    "🔍 Wallpaper can be blurred or original — toggle it in the Wall section\n"
    "🏷 Reply = username + quoted text in replies (white by default)\n"
    "🔄 Reset clears only the part you're editing.\n"
    "📸 <b>Send a picture to start!</b>"
)


# ---------- State ----------

def default_state(palette, wall_bytes):
    return {
        "palette": palette,                       # 6 suggested colors
        "mode": "dark",                           # 🌗 general coloring theme
        "sections": {k: {"idx": -1, "alpha": 0, "custom": None}
                     for k in COLOR_SECTIONS},
        "active": "bg",
        "wall_mode": "image" if wall_bytes else "flat",
        "wall_idx": -1,
        "wall_custom": None,
        "wall_blur": False,                       # 🔍 blur toggle
        "wall_bytes": wall_bytes,
        "msg_id": None,
        "mode_img": "photo",
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


# ---------- UI ----------

def caption(st):
    res = resolve_theme(st["palette"], st["sections"], st["mode"])

    def fmt(k):
        if k == "wall":
            if st["wall_mode"] == "image" and st["wall_bytes"]:
                return "image" + (" · blur" if st.get("wall_blur") else "")
            return rgb_to_hex(resolve_wall(st["palette"], st["wall_idx"],
                                           st["wall_custom"], st["mode"]))
        s = st["sections"][k]
        tr = f" · {s['alpha']}%" if (k in ALPHA_SECTIONS and s["alpha"]) else ""
        return rgb_to_hex(res[k]) + tr

    lines = [
        "🎨 <b>Theme Editor</b>",
        f"🌗 Mode: <b>{'Dark' if st['mode'] == 'dark' else 'Light'}</b>",
        "──────────────",
    ]
    for a, b in (("bg", "bar"), ("in", "out"),
                 ("text", "accent"), ("reply", "wall")):
        lines.append(f"{SEC_LABEL[a]} {fmt(a)}   {SEC_LABEL[b]} {fmt(b)}")
    lines.append("──────────────")
    lines.append(f"▸ Editing: <b>{SEC_FULL[st['active']]}</b>")
    pal = st["palette"]
    lines.append("⚡ auto  " + "  ".join(f"{i+1} {h}" for i, h in enumerate(pal[:3])))
    lines.append("          " + "  ".join(f"{i+4} {h}" for i, h in enumerate(pal[3:6])))
    lines.append("Type your own: <code>#34c7a4</code>")
    return "\n".join(lines)


def keyboard(st):
    rows = [
        [   # 🌗 mode row — drives all auto colors
            InlineKeyboardButton(("● " if st["mode"] == "dark" else "○ ") + "🌙 Dark",
                                 callback_data="mode:dark"),
            InlineKeyboardButton(("● " if st["mode"] == "light" else "○ ") + "☀️ Light",
                                 callback_data="mode:light"),
        ],
        [InlineKeyboardButton(("● " if st["active"] == k else "") + lbl,
                              callback_data=f"sec:{k}") for k, lbl in SECTIONS[:4]],
        [InlineKeyboardButton(("● " if st["active"] == k else "") + lbl,
                              callback_data=f"sec:{k}") for k, lbl in SECTIONS[4:]],
    ]

    cs = active_color_state(st)
    if st["active"] == "wall":
        rows.append([
            InlineKeyboardButton(("● " if st["wall_mode"] == "image" and st["wall_bytes"]
                                  else "○ ") + "🖼 Image", callback_data="wp:image"),
            InlineKeyboardButton(("● " if st["wall_mode"] == "flat" else "○ ")
                                 + "🎨 Flat", callback_data="wp:flat"),
        ])
        if st["wall_bytes"]:
            rows.append([InlineKeyboardButton(
                f"🔍 Blur: {'On' if st.get('wall_blur') else 'Off'}",
                callback_data="wb:toggle")])
        if st["wall_mode"] == "flat" or not st["wall_bytes"]:
            row = [InlineKeyboardButton(
                "⚡" + ("●" if cs["idx"] == -1 and not cs["custom"] else ""),
                callback_data="auto")]
            for i in range(len(st["palette"])):
                row.append(InlineKeyboardButton(
                    "●" if cs["idx"] == i else str(i + 1),
                    callback_data=f"c:{i}"))
            rows.append(row)
    else:
        row = [InlineKeyboardButton(
            "⚡" + ("●" if cs["idx"] == -1 and not cs["custom"] else ""),
            callback_data="auto")]
        for i in range(len(st["palette"])):
            row.append(InlineKeyboardButton(
                "●" if cs["idx"] == i else str(i + 1), callback_data=f"c:{i}"))
        if cs["custom"]:
            row.append(InlineKeyboardButton("🎯●", callback_data="noop"))
        rows.append(row)

        # slider only for sections that support transparency
        if st["active"] in ALPHA_SECTIONS:
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
        InlineKeyboardButton(f"🔄 Reset {SEC_LABEL[st['active']]}",
                             callback_data="reset"),
    ])
    return InlineKeyboardMarkup(rows)


def build_payload(st):
    res = resolve_theme(st["palette"], st["sections"], st["mode"])
    alphas = {k: st["sections"][k]["alpha"] for k in ALPHA_SECTIONS}
    wall = st["wall_bytes"] if (st["wall_mode"] == "image" and st["wall_bytes"]) else None
    wall_flat = resolve_wall(st["palette"], st["wall_idx"], st["wall_custom"],
                             st["mode"])
    png = render_preview(res, alphas, wall, wall_flat, st.get("wall_blur", False))
    return png, caption(st), keyboard(st)


async def refresh_editor(chat_id, st, context):
    if not st.get("msg_id"):
        return
    png, cap, kb = build_payload(st)
    if st["mode_img"] == "photo":
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
        st["mode_img"] = "photo"
    except Exception as e:
        logger.error(f"Preview failed → text mode: {e}")
        sent = await msg.reply_text(cap, reply_markup=kb, parse_mode="HTML")
        st["mode_img"] = "text"
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

    palette = extract_palette(raw)          # never raises
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
                "part you're editing.", parse_mode="HTML")
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
        res = resolve_theme(st["palette"], st["sections"], st["mode"])
        alphas = {k: st["sections"][k]["alpha"] for k in ALPHA_SECTIONS}
        wall = st["wall_bytes"] if (st["wall_mode"] == "image" and st["wall_bytes"]) else None
        wall_flat = None if wall else resolve_wall(st["palette"], st["wall_idx"],
                                                   st["wall_custom"], st["mode"])
        try:
            theme = build_attheme(res, alphas, wall, wall_flat,
                                  blur=st.get("wall_blur", False))
        except Exception as e:
            logger.error(f"Build failed: {e}")
            await context.bot.send_message(
                chat_id=update.effective_chat.id,
                text="😕 Something broke — press ✅ again or 🔄 Reset.")
            return
        fname = f"Theme_{rgb_to_hex(res['accent'])[1:]}_{alphas['in']}_{alphas['out']}.attheme"
        await context.bot.send_document(
            chat_id=update.effective_chat.id, document=theme, filename=fname,
            caption="🎨 <b>Your theme is ready!</b>\n"
                    f"Mode: {st['mode'].capitalize()} · "
                    f"Accent {rgb_to_hex(res['accent'])} · "
                    f"In {alphas['in']}% · Out {alphas['out']}%\n\n"
                    "Open the file — Telegram applies it instantly ✨",
            parse_mode="HTML")
        return

    if data == "noop":
        await q.answer()
        return

    if data.startswith("mode:"):
        st["mode"] = data[5:]
    elif data.startswith("sec:"):
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
    elif data == "wb:toggle" and st["wall_bytes"]:
        st["wall_blur"] = not st.get("wall_blur", False)
    elif data == "reset":
        # ONLY the active part is reset (mode is not touched)
        if st["active"] == "wall":
            st["wall_mode"] = "image" if st["wall_bytes"] else "flat"
            st["wall_idx"], st["wall_custom"] = -1, None
            st["wall_blur"] = False
        else:
            s = st["sections"][st["active"]]
            s["idx"], s["custom"], s["alpha"] = -1, None, 0
        await q.answer(f"🔄 {SEC_FULL[st['active']]} reset")

    await q.answer()
    await refresh_editor(update.effective_chat.id, st, context)


async def on_error(update, context):
    logger.error("Unhandled error:", exc_info=context.error)


def main():
    if not BOT_TOKEN:
        raise SystemExit("❌ BOT_TOKEN is not set!")

    start_server()       # Render port requirement
    start_keep_alive()   # 24/7: self-ping every 9 min prevents spin-down

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

    logger.info("🚀 Theme bot is up (24/7 keep-alive active)")
    app.run_polling(drop_pending_updates=True)


if __name__ == "__main__":
    main()
