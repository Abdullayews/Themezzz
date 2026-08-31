import io

from PIL import Image, ImageDraw, ImageFont

from colors import readable_on, mix, ensure_contrast, luminance


# ---------- fonts ----------

def _font(size, bold=False):
    weight = "Bold" if bold else "Regular"
    for path in (
        f"/usr/share/fonts/truetype/roboto/unhinted/Roboto-{weight}.ttf",
        f"/usr/share/fonts/truetype/roboto/Roboto-{weight}.ttf",
        f"/usr/share/fonts/truetype/dejavu/DejaVuSans{'-Bold' if bold else ''}.ttf",
    ):
        try:
            return ImageFont.truetype(path, size)
        except Exception:
            continue
    try:
        return ImageFont.load_default(size=size)
    except TypeError:
        return ImageFont.load_default()


# ---------- icon primitives ----------

def _paperclip(d, cx, cy, s, col):
    r = s * 0.26
    w = max(2, int(s * 0.10))
    d.arc([cx - r, cy - r, cx + r, cy + r], 90, 270, fill=col, width=w)
    d.arc([cx - r, cy - r * 1.75, cx + r, cy + r * 0.25], 270, 90, fill=col, width=w)
    d.line([(cx, cy - r), (cx, cy + r)], fill=col, width=w)


def _mic(d, cx, cy, s, col):
    r = s * 0.14
    w = max(2, int(s * 0.08))
    d.rounded_rectangle([cx - r, cy - s * 0.32, cx + r, cy + s * 0.08],
                        radius=r, fill=col)
    d.arc([cx - s * 0.22, cy - s * 0.08, cx + s * 0.22, cy + s * 0.38],
          0, 180, fill=col, width=w)
    d.line([(cx, cy + s * 0.38), (cx, cy + s * 0.48)], fill=col, width=w)
    d.line([(cx - s * 0.13, cy + s * 0.48), (cx + s * 0.13, cy + s * 0.48)],
           fill=col, width=w)


def _checks(d, x, y, s, col):
    """✓✓ read marks."""
    w = max(2, int(s * 0.12))
    d.line([(x, y + s * 0.35), (x + s * 0.3, y + s * 0.7)], fill=col, width=w)
    d.line([(x + s * 0.3, y + s * 0.7), (x + s * 0.7, y)], fill=col, width=w)
    d.line([(x + s * 0.45, y + s * 0.35), (x + s * 0.75, y + s * 0.7)], fill=col, width=w)
    d.line([(x + s * 0.75, y + s * 0.7), (x + s * 1.15, y)], fill=col, width=w)


def _flame(d, cx, cy, h):
    """🔥 drawn manually (no emoji font on servers)."""
    outer = (255, 109, 0)
    inner = (255, 214, 0)
    d.polygon([(cx - 0.30 * h, cy + 0.18 * h), (cx, cy - 0.50 * h),
               (cx + 0.30 * h, cy + 0.18 * h)], fill=outer)
    d.ellipse([cx - 0.32 * h, cy - 0.05 * h, cx + 0.32 * h, cy + 0.45 * h], fill=outer)
    d.polygon([(cx - 0.14 * h, cy + 0.22 * h), (cx, cy - 0.12 * h),
               (cx + 0.14 * h, cy + 0.22 * h)], fill=inner)
    d.ellipse([cx - 0.15 * h, cy + 0.12 * h, cx + 0.15 * h, cy + 0.42 * h], fill=inner)


def _chevron_down(d, cx, cy, s, col):
    w = max(2, int(s * 0.10))
    d.line([(cx - s * 0.4, cy - s * 0.12), (cx, cy + s * 0.28)], fill=col, width=w)
    d.line([(cx, cy + s * 0.28), (cx + s * 0.4, cy - s * 0.12)], fill=col, width=w)


def _battery(d, x, cy, s, col):
    w = max(1, int(s * 0.07))
    d.rounded_rectangle([x, cy - s * 0.30, x + s * 0.95, cy + s * 0.30],
                        radius=int(s * 0.08), outline=col, width=w)
    d.rectangle([x + s * 0.15, cy - s * 0.16, x + s * 0.62, cy + s * 0.16], fill=col)
    d.rectangle([x + s * 0.99, cy - s * 0.12, x + s * 1.08, cy + s * 0.12], fill=col)


def _signal(d, x, cy, s, col):
    w = s * 0.16
    gap = s * 0.10
    bottom = cy + s * 0.28
    for i, h in enumerate((0.20, 0.36, 0.54, 0.74)):
        hh = s * h
        d.rectangle([x + i * (w + gap), bottom - hh,
                     x + i * (w + gap) + w, bottom], fill=col)


def _cover(img, w, h):
    """Center-crop image to fully cover w×h."""
    iw, ih = img.size
    scale = max(w / iw, h / ih)
    img = img.resize((int(iw * scale) + 1, int(ih * scale) + 1),
                     Image.Resampling.LANCZOS)
    x, y = (img.width - w) // 2, (img.height - h) // 2
    return img.crop((x, y, x + w, y + h))


# ---------- main ----------

def render_preview(colors, alphas, wall_bytes, wall_flat):
    """
    1:1 Telegram Android chat screen:
      status bar → action bar → date chip → bubbles (with reply block)
      → gray hint → scroll-down FAB → input bar with pill + send button.
    Two-pass alpha compositing = how Telegram layers per-key transparency.
    Monochrome styling: elevation is darker, never lighter (Forest-style).
    """
    bg, bar = colors["bg"], colors["bar"]
    inb, outb = colors["in"], colors["out"]
    text, accent = colors["text"], colors["accent"]
    reply = colors["reply"]

    A = lambda k: max(0, min(255, round(255 * (1 - alphas.get(k, 0) / 100.0))))
    a_bar, a_in, a_out = A("bar"), A("in"), A("out")
    a_text, a_acc, a_reply = A("text"), A("accent"), A("reply")

    dark = luminance(bg) < 0.5
    S = 2                                    # supersampling
    W, H = 480 * S, 980 * S
    SB, BAR, INP = 28 * S, 56 * S, 62 * S    # status / action / input heights
    top, bot = SB + BAR, H - INP

    # ---- derived tones (darker, never lighter) ----
    bar_text = readable_on(bar)
    bar_icon = mix(bar_text, bar, 0.15)
    sb_col = mix(bar, (0, 0, 0), 0.22 if dark else 0.06)
    in_text = ensure_contrast(text, inb)
    out_text = ensure_contrast(text, outb)
    in_time = mix(in_text, inb, 0.35)
    out_time = mix(out_text, outb, 0.25)
    reply_in = ensure_contrast(reply, inb)
    reply_msg = mix(reply_in, inb, 0.15)
    on_acc = readable_on(accent)
    acc_bar = ensure_contrast(accent, bar)
    acc_in = ensure_contrast(accent, inb)
    gray2 = mix(text, bg, 0.45)
    gray3 = mix(text, bg, 0.55)
    divider = mix(bg, (0, 0, 0), 0.40) if dark else mix(bg, (0, 0, 0), 0.14)
    fab = mix(accent, (0, 0, 0), 0.32 if dark else 0.18)   # darker circle buttons
    on_fab = readable_on(fab)

    # ---- fonts ----
    f_sb = _font(13 * S)
    f_name = _font(17 * S, bold=True)
    f_status = _font(13 * S)
    f_text = _font(16 * S)
    f_time = _font(11 * S)
    f_chip = _font(13 * S)
    f_reply = _font(12 * S)
    f_reply_b = _font(12 * S, bold=True)
    f_av = _font(20 * S, bold=True)
    f_badge = _font(11 * S, bold=True)

    # ---- base: wallpaper in the chat area ----
    if wall_bytes:
        chat = _cover(Image.open(io.BytesIO(wall_bytes)), W, bot - top)
        tint = Image.new("RGBA", (W, bot - top),
                         (0, 0, 0, 100) if dark else (255, 255, 255, 110))
        chat = Image.alpha_composite(chat.convert("RGBA"), tint).convert("RGB")
    else:
        wf = wall_flat if wall_flat else mix(bg, (0, 0, 0), 0.25 if dark else 0.05)
        chat = Image.new("RGB", (W, bot - top), wf)
    img = Image.new("RGB", (W, H), bg)
    img.paste(chat, (0, top))
    img = img.convert("RGBA")

    # ================= PASS A — surfaces (per-section alpha) =================
    ov = Image.new("RGBA", (W, H), (0, 0, 0, 0))
    d = ImageDraw.Draw(ov)

    # status bar + action bar
    d.rectangle([0, 0, W, SB], fill=sb_col + (a_bar,))
    d.rectangle([0, SB, W, top], fill=bar + (a_bar,))

    # "Today" date chip — darker than bg, semi-transparent
    chip_txt = "Today"
    cw = int(d.textlength(chip_txt, font=f_chip) + 34 * S)
    ch = 28 * S
    ccy = top + 20 * S
    d.rounded_rectangle([(W - cw) // 2, ccy, (W + cw) // 2, ccy + ch],
                        radius=ch // 2,
                        fill=(mix(bg, (0, 0, 0), 0.25) if dark
                              else mix(bg, (0, 0, 0), 0.05)) + (200,))

    # ---- incoming bubble (with reply block inside) ----
    p, R = 11 * S, 17 * S
    r_un, r_msg = "Alex", "Check this out!"
    rw = int(max(d.textlength(r_un, font=f_reply_b),
                 d.textlength(r_msg, font=f_reply)) + 11 * S)
    rh = 36 * S
    mt1, t1 = "Hi! How's the theme?", "14:32"
    mw1 = d.textlength(mt1, font=f_text)
    tw1 = d.textlength(t1, font=f_time)
    bw1 = int(max(mw1, rw, tw1 + 44 * S) + 2 * p)
    bh1 = p + rh + 7 * S + f_text.size + 8 * S + f_time.size + p
    bx1, by1 = 14 * S, top + 66 * S
    d.rounded_rectangle([bx1 + 2 * S, by1 + 4 * S,
                         bx1 + bw1 + 2 * S, by1 + bh1 + 4 * S],
                        radius=R, fill=(0, 0, 0, 45 if dark else 26))
    d.rounded_rectangle([bx1, by1, bx1 + bw1, by1 + bh1], radius=R,
                        fill=inb + (a_in,), corners=(True, True, True, False))

    # ---- outgoing bubble ----
    mt2, t2 = "LOOKS GREAT", "14:33"
    mw2 = d.textlength(mt2, font=f_text)
    tw2 = d.textlength(t2, font=f_time)
    bw2 = int(mw2 + 30 * S + 46 * S + 2 * p)          # text + 🔥 + ✓✓time
    bh2 = p + f_text.size + 8 * S + f_time.size + p
    bx2, by2 = W - bw2 - 14 * S, by1 + bh1 + 14 * S
    d.rounded_rectangle([bx2 + 2 * S, by2 + 4 * S,
                         bx2 + bw2 + 2 * S, by2 + bh2 + 4 * S],
                        radius=R, fill=(0, 0, 0, 45 if dark else 26))
    d.rounded_rectangle([bx2, by2, bx2 + bw2, by2 + bh2], radius=R,
                        fill=outb + (a_out,), corners=(True, True, False, True))

    # ---- scroll-down FAB (darker circle) ----
    fr = 22 * S
    fcx, fcy = W - 46 * S, bot - 62 * S
    d.ellipse([fcx - fr, fcy - fr, fcx + fr, fcy + fr], fill=fab + (235,))

    # ---- input bar + pill + send button ----
    d.rectangle([0, bot, W, H], fill=bg + (255,))
    d.line([(0, bot), (W, bot)], fill=divider + (255,), width=S)
    py0, py1 = bot + 9 * S, bot + 53 * S
    d.rounded_rectangle([12 * S, py0, W - 78 * S, py1],
                        radius=(py1 - py0) // 2,
                        fill=(mix(bg, (0, 0, 0), 0.28) if dark
                              else mix(bg, (0, 0, 0), 0.08)) + (255,))
    scx, scy, sr = W - 40 * S, bot + 31 * S, 21 * S
    d.ellipse([scx - sr, scy - sr, scx + sr, scy + sr], fill=accent + (a_acc,))

    img = Image.alpha_composite(img, ov)

    # ================= PASS B — content (text & icons) =================
    ov2 = Image.new("RGBA", (W, H), (0, 0, 0, 0))
    d = ImageDraw.Draw(ov2)

    # ---- status bar: clock / signal / battery ----
    d.text((22 * S, (SB - f_sb.size) // 2), "9:41", font=f_sb, fill=bar_text)
    _battery(d, W - 52 * S, SB // 2, 16 * S, bar_icon)
    _signal(d, W - 106 * S, SB // 2, 16 * S, bar_icon)

    # ---- action bar: back / avatar / name / online / dots ----
    cy = SB + BAR // 2
    d.line([(38 * S, cy - 12 * S), (24 * S, cy)], fill=bar_icon, width=3 * S)
    d.line([(24 * S, cy), (38 * S, cy + 12 * S)], fill=bar_icon, width=3 * S)
    acx, ar = 62 * S, 17 * S
    d.ellipse([acx - ar, cy - ar, acx + ar, cy + ar], fill=accent)
    lw = d.textlength("C", font=f_av)
    d.text((acx - lw / 2, cy - f_av.size * 0.62), "C", font=f_av, fill=on_acc)
    d.text((90 * S, SB + 9 * S), "Chat", font=f_name, fill=bar_text)
    d.text((90 * S, SB + 31 * S), "online", font=f_status, fill=acc_bar)
    for i in range(3):
        dy = cy - 9 * S + i * 9 * S
        d.ellipse([(W - 27 * S), dy, (W - 22 * S), dy + 5 * S], fill=bar_icon)

    # ---- date chip text ----
    ctw = d.textlength(chip_txt, font=f_chip)
    d.text(((W - ctw) // 2, ccy + (ch - f_chip.size) // 2), chip_txt,
           font=f_chip, fill=text + (235,))

    # ---- incoming bubble content (reply block + message) ----
    rl_x = bx1 + p
    d.rectangle([rl_x, by1 + p, rl_x + 3 * S, by1 + p + rh], fill=acc_in)
    d.text((rl_x + 9 * S, by1 + p - 1 * S), r_un, font=f_reply_b,
           fill=reply_in + (a_reply,))
    d.text((rl_x + 9 * S, by1 + p + 16 * S), r_msg, font=f_reply,
           fill=reply_msg + (a_reply,))
    d.text((bx1 + p, by1 + p + rh + 7 * S), mt1, font=f_text,
           fill=in_text + (a_text,))
    d.text((bx1 + bw1 - p - tw1, by1 + bh1 - p - f_time.size), t1,
           font=f_time, fill=in_time)

    # ---- outgoing bubble content ----
    d.text((bx2 + p, by2 + p), mt2, font=f_text, fill=out_text + (a_text,))
    _flame(d, bx2 + p + mw2 + 16 * S, by2 + p + f_text.size * 0.55, 26 * S)
    tx2 = bx2 + bw2 - p
    _checks(d, tx2 - tw2 - 30 * S, by2 + bh2 - p - f_time.size, 13 * S,
            mix(out_text, outb, 0.20))
    d.text((tx2 - tw2, by2 + bh2 - p - f_time.size), t2, font=f_time, fill=out_time)

    # ---- gray centered hint ----
    hint = "You can save and apply it"
    hw = d.textlength(hint, font=f_status)
    d.text(((W - hw) // 2, by2 + bh2 + 28 * S), hint, font=f_status, fill=gray2)

    # ---- FAB: chevron + unread badge ----
    _chevron_down(d, fcx, fcy, 24 * S, on_fab)
    br_ = 11 * S
    bcx, bcy = fcx + fr - 6 * S, fcy - fr + 6 * S
    d.ellipse([bcx - br_, bcy - br_, bcx + br_, bcy + br_], fill=accent)
    bw_t = d.textlength("3", font=f_badge)
    d.text((bcx - bw_t / 2, bcy - f_badge.size * 0.60), "3",
           font=f_badge, fill=on_acc)

    # ---- input bar: paperclip / hint / mic / send arrow ----
    pv = (py0 + py1) // 2
    _paperclip(d, 34 * S, pv, 20 * S, gray3)
    d.text((58 * S, pv - f_status.size * 0.55), "Message...",
           font=f_status, fill=mix(text, bg, 0.45))
    _mic(d, W - 98 * S, pv, 20 * S, gray3)
    hh = 20 * S
    d.polygon([(scx - hh * 0.38, scy - hh * 0.5),
               (scx - hh * 0.38, scy + hh * 0.5),
               (scx + hh * 0.55, scy)], fill=on_acc)

    img = Image.alpha_composite(img, ov2).convert("RGB")

    # ---- downscale & save ----
    img = img.resize((W // S, H // S), Image.Resampling.LANCZOS)
    buf = io.BytesIO()
    buf.name = "preview.png"
    img.save(buf, "PNG")
    buf.seek(0)
    return buf
