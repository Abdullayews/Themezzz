import io

from PIL import Image, ImageDraw, ImageFont

from colors import readable_on, mix, ensure_contrast


def _font(size):
    for path in (
        "/usr/share/fonts/truetype/roboto/unhinted/Roboto-Regular.ttf",
        "/usr/share/fonts/truetype/roboto/Roboto-Regular.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
    ):
        try:
            return ImageFont.truetype(path, size)
        except Exception:
            continue
    try:
        return ImageFont.load_default(size=size)
    except TypeError:
        return ImageFont.load_default()


def _cover(img, w, h):
    iw, ih = img.size
    scale = max(w / iw, h / ih)
    img = img.resize((int(iw * scale) + 1, int(ih * scale) + 1),
                     Image.Resampling.LANCZOS)
    x, y = (img.width - w) // 2, (img.height - h) // 2
    return img.crop((x, y, x + w, y + h))


def _flame(d, cx, cy, h):
    """🔥 drawn manually (no emoji font on server)."""
    outer = (255, 109, 0)
    inner = (255, 214, 0)
    d.polygon([(cx - 0.30 * h, cy + 0.18 * h), (cx, cy - 0.50 * h),
               (cx + 0.30 * h, cy + 0.18 * h)], fill=outer)
    d.ellipse([cx - 0.32 * h, cy - 0.05 * h, cx + 0.32 * h, cy + 0.45 * h], fill=outer)
    d.polygon([(cx - 0.14 * h, cy + 0.22 * h), (cx, cy - 0.12 * h),
               (cx + 0.14 * h, cy + 0.22 * h)], fill=inner)
    d.ellipse([cx - 0.15 * h, cy + 0.12 * h, cx + 0.15 * h, cy + 0.42 * h], fill=inner)


def render_preview(colors, alphas, wall_bytes, wall_flat):
    """1:1 copy of the classic Telegram theme-preview chat screen (English)."""
    bg, bar = colors["bg"], colors["bar"]
    inb, outb = colors["in"], colors["out"]
    text, accent = colors["text"], colors["accent"]

    A = lambda k: max(0, min(255, round(255 * (1 - alphas.get(k, 0) / 100.0))))

    S = 2
    W, H = 480, 900
    BAR_H, INPUT_H = 56 * S, 56 * S
    chat_y1 = H * S - INPUT_H

    # ---- wallpaper / canvas ----
    if wall_bytes:
        chat = _cover(Image.open(io.BytesIO(wall_bytes)), W * S, chat_y1 - BAR_H)
    else:
        chat = Image.new("RGB", (W * S, chat_y1 - BAR_H), wall_flat)
    img = Image.new("RGB", (W * S, H * S), bg)
    img.paste(chat, (0, BAR_H))

    f_title = _font(17 * S)
    f_sub   = _font(13 * S)
    f_bub   = _font(16 * S)
    f_av    = _font(20 * S)

    in_text = ensure_contrast(text, inb)
    out_text = ensure_contrast(text, outb)

    # ---- transparent layer: bar, bubbles, send button ----
    ov = Image.new("RGBA", (W * S, H * S), (0, 0, 0, 0))
    od = ImageDraw.Draw(ov)
    od.rectangle([0, 0, W * S, BAR_H], fill=bar + (A("bar"),))

    pad_x, pad_y = 12 * S, 8 * S
    ta = A("text")

    def bubble(txt, fill, alpha, tcol, out, y, flame=False):
        tw = od.textlength(txt, font=f_bub)
        flame_w = 32 * S if flame else 0
        bw = int(tw + flame_w + pad_x * 2)
        bh = int(f_bub.size + pad_y * 2)
        x = int((W * S - bw - 14 * S) if out else 14 * S)
        corners = (1, 1, 0, 1) if out else (1, 1, 1, 0)
        radius = min(16 * S, bh // 2 - 1, bw // 2 - 1)
        od.rounded_rectangle([x, y, x + bw, y + bh], radius=radius,
                             fill=fill + (alpha,), corners=corners)
        od.text((x + pad_x, y + pad_y), txt, font=f_bub, fill=tcol + (ta,))
        return x, y, bw, bh, tw

    y1 = BAR_H + 48 * S
    b1 = bubble("Hi! How's the theme?", inb, A("in"), in_text, False, y1)
    y2 = y1 + b1[3] + 12 * S
    b2 = bubble("Looks great", outb, A("out"), out_text, True, y2, flame=True)

    # send button (respects accent transparency)
    scx, scy, sr = W * S - 44 * S, chat_y1 + INPUT_H // 2, 21 * S
    od.ellipse([scx - sr, scy - sr, scx + sr, scy + sr],
               fill=accent + (A("accent"),))

    img = Image.alpha_composite(img.convert("RGBA"), ov).convert("RGB")
    d = ImageDraw.Draw(img)

    # ---- action bar content ----
    bar_text = readable_on(bar)
    acx, acy, ar = 32 * S, BAR_H // 2, 16 * S
    d.ellipse([acx - ar, acy - ar, acx + ar, acy + ar], fill=accent)
    d.text((acx, acy), "C", font=f_av, fill=readable_on(accent), anchor="mm")
    d.text((58 * S, 8 * S), "Chat", font=f_title, fill=bar_text)
    d.text((58 * S, 34 * S), "online", font=f_sub, fill=ensure_contrast(accent, bar))

    # ---- flame emoji (drawn after text) ----
    fx = b2[0] + pad_x + b2[4] + 16 * S
    fy = y2 + pad_y + f_bub.size * 0.45
    _flame(d, fx, fy, 26 * S)

    # ---- gray hint ----
    hint = "You can save and apply it"
    hw = d.textlength(hint, font=f_sub)
    d.text(((W * S - hw) // 2, y2 + b2[3] + 28 * S), hint,
           font=f_sub, fill=mix(text, bg, 0.35))

    # ---- input bar ----
    iy = chat_y1
    d.rectangle([0, iy, W * S, iy + INPUT_H], fill=mix(bg, text, 0.05))
    d.line([(0, iy), (W * S, iy)], fill=mix(bg, text, 0.12), width=S)
    fy0, fy1 = iy + 12 * S, iy + INPUT_H - 12 * S
    d.rounded_rectangle([46 * S, fy0, W * S - 84 * S, fy1],
                        radius=(fy1 - fy0) // 2, fill=mix(bg, text, 0.10))
    d.text((62 * S, iy + (INPUT_H - f_sub.size) // 2 - 2 * S), "Message...",
           font=f_sub, fill=mix(text, bg, 0.45))
    on_acc = readable_on(accent)
    hh = 20 * S
    d.polygon([(scx - hh * 0.4, scy - hh * 0.5), (scx - hh * 0.4, scy + hh * 0.5),
               (scx + hh * 0.6, scy)], fill=on_acc)

    img = img.resize((W, H), Image.Resampling.LANCZOS)
    buf = io.BytesIO()
    buf.name = "preview.png"
    img.save(buf, "PNG")
    buf.seek(0)
    return buf
