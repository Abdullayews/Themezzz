import io

from PIL import Image, ImageDraw, ImageFont

from colors import luminance, readable_on, mix, ensure_contrast, rgb_to_hex

CHIPS = [("bg", "BG"), ("bar", "BAR"), ("in", "IN"),
         ("out", "OUT"), ("text", "TXT"), ("accent", "ACC")]


def _font(size):
    for path in ("/usr/share/fonts/truetype/roboto/unhinted/Roboto-Regular.ttf",
                 "/usr/share/fonts/truetype/roboto/Roboto-Regular.ttf",
                 "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf"):
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


def _checks(d, x, y, s, col):
    w = max(2, int(s * 0.12))
    d.line([(x, y + s * 0.35), (x + s * 0.3, y + s * 0.7)], fill=col, width=w)
    d.line([(x + s * 0.3, y + s * 0.7), (x + s * 0.7, y)], fill=col, width=w)
    d.line([(x + s * 0.45, y + s * 0.35), (x + s * 0.75, y + s * 0.7)], fill=col, width=w)
    d.line([(x + s * 0.75, y + s * 0.7), (x + s * 1.15, y)], fill=col, width=w)


def render_preview(colors, alphas, palette, active, sel, info,
                   wall_bytes, wall_flat):
    """1:1 Telegram chat mock. All colors come straight from user choices."""
    bg, bar = colors["bg"], colors["bar"]
    inb, outb = colors["in"], colors["out"]
    text, accent = colors["text"], colors["accent"]

    A = lambda k: round(255 * (1 - alphas.get(k, 0) / 100.0))
    in_text = ensure_contrast(text, inb)
    out_text = ensure_contrast(text, outb)
    on_acc = readable_on(accent)
    gray = mix(text, bg, 0.40)
    dark = luminance(bg) < 0.5
    panel_txt = readable_on(bg)

    S = 2
    W, H = 480, 900
    BAR_H, INPUT_H, PANEL_H = 56 * S, 52 * S, 176 * S
    chat_y1 = H * S - INPUT_H - PANEL_H

    # ---- Chat background ----
    if wall_bytes:
        chat = _cover(Image.open(io.BytesIO(wall_bytes)), W * S, chat_y1 - BAR_H)
    else:
        chat = Image.new("RGB", (W * S, chat_y1 - BAR_H), wall_flat)
    img = Image.new("RGB", (W * S, H * S), bg)
    img.paste(chat, (0, BAR_H))

    f_title, f_sub = _font(17 * S), _font(13 * S)
    f_bub, f_time = _font(16 * S), _font(12 * S)
    f_pinfo, f_plbl, f_pnum = _font(13 * S), _font(10 * S), _font(12 * S)

    # ---- Alpha layer: bar, chip, bubbles ----
    ov = Image.new("RGBA", (W * S, H * S), (0, 0, 0, 0))
    od = ImageDraw.Draw(ov)
    od.rectangle([0, 0, W * S, BAR_H], fill=bar + (A("bar"),))

    chip_w, chip_h = 150 * S, 40 * S
    cx, cy = (W * S - chip_w) // 2, BAR_H + 14 * S
    od.rounded_rectangle([cx, cy, cx + chip_w, cy + chip_h], radius=chip_h // 2,
                         fill=mix(bg, text, 0.45) + (160,))

    pad_x, pad_y, line_h = 12 * S, 9 * S, 22 * S

    def bubble(lines_txt, fill, alpha, out=False, y=0):
        w = max(od.textlength(t, font=f_bub) for t in lines_txt) + pad_x * 2 + 60 * S
        h = len(lines_txt) * line_h + pad_y * 2 + 14 * S
        x = (W * S - w - 14 * S) if out else 14 * S
        od.rounded_rectangle([x, y, x + w, y + h], radius=16 * S, fill=fill + (alpha,))
        return x, y, w, h, lines_txt

    y1 = cy + chip_h + 18 * S
    b1 = bubble(["Hey! Check out this", "theme I just made"], inb, A("in"), y=y1)
    y2 = y1 + b1[3] + 10 * S
    b2 = bubble(["Looks amazing! 🔥"], outb, A("out"), out=True, y=y2)

    # date chip text on overlay too (crisp over wallpaper)
    od.text(((W * S - od.textlength("Today", font=f_sub)) // 2, cy + 12 * S),
            "Today", font=f_sub, fill=text + (230,))

    img = Image.alpha_composite(img.convert("RGBA"), ov).convert("RGB")
    d = ImageDraw.Draw(img)

    # ---- Bar content ----
    bar_text = readable_on(bar)
    d.line([(14 * S, 22 * S), (14 * S, 34 * S), (24 * S, 28 * S)],
           fill=mix(bar_text, bar, 0.15), width=3 * S)
    d.ellipse([42 * S, 11 * S, 74 * S, 43 * S], fill=accent)
    d.text((56 * S - od.textlength("T", font=f_title) / 2, 13 * S), "T",
           font=f_title, fill=on_acc)
    d.text((84 * S, 8 * S), "Telegram", font=f_title, fill=bar_text)
    d.text((84 * S, 30 * S), "online", font=f_sub, fill=mix(bar_text, bar, 0.40))
    for i in range(3):
        d.ellipse([(W - 22) * S, (28 + i * 8 - 8) * S,
                   (W - 18) * S, (28 + i * 8 - 4) * S], fill=mix(bar_text, bar, 0.15))

    # ---- Bubble texts ----
    def texts(b, out, tcol, time_str):
        x, y, w, h, lines = b
        ty = y + pad_y
        for t in lines:
            d.text((x + pad_x, ty), t, font=f_bub, fill=tcol)
            ty += line_h
        ty2 = y + h - pad_y - 12 * S
        tx = x + w - pad_x
        if out:
            _checks(d, tx - 46 * S, ty2, 13 * S, mix(tcol, outb, 0.20))
        d.text((tx - d.textlength(time_str, font=f_time), ty2), time_str,
               font=f_time, fill=mix(tcol, outb if out else inb, 0.30))

    texts(b1, False, in_text, "14:32")
    texts(b2, True, out_text, "14:33")

    # ---- Input bar ----
    iy = H * S - INPUT_H - PANEL_H
    d.rectangle([0, iy, W * S, iy + INPUT_H], fill=mix(bg, text, 0.05))
    d.line([(0, iy), (W * S, iy)], fill=mix(bg, text, 0.12), width=S)
    ic = mix(text, bg, 0.30)
    # attach
    d.arc([20 * S, iy + 16 * S, 36 * S, iy + 32 * S], 90, 270, fill=ic, width=2 * S)
    d.text((72 * S, iy + 17 * S), "Message", font=f_sub, fill=gray)
    d.ellipse([(W - 44) * S, iy + 10 * S, (W - 10) * S, iy + 44 * S], fill=accent)
    d.polygon([(W - 36) * S, iy + 19 * S, (W - 36) * S, iy + 35 * S, (W - 22) * S, iy + 27 * S],
              fill=on_acc)

    # ---- Bottom panel: info + section chips + palette swatches ----
    py0 = iy + INPUT_H
    d.rectangle([0, py0, W * S, H * S], fill=bg)
    d.line([(0, py0), (W * S, py0)], fill=mix(bg, text, 0.12), width=S)
    d.text((16 * S, py0 + 8 * S), info, font=f_pinfo, fill=mix(text, bg, 0.25))

    # section chips
    chip_r = 12 * S
    gap = (W * S - 32 * S - 6 * chip_r * 2) / 5
    cy2 = py0 + 44 * S
    for i, (k, lbl) in enumerate(CHIPS):
        ccx = int(16 * S + chip_r + i * (chip_r * 2 + gap))
        col = colors[k]
        if k == active:
            d.ellipse([ccx - chip_r - 4 * S, cy2 - chip_r - 4 * S,
                       ccx + chip_r + 4 * S, cy2 + chip_r + 4 * S],
                      outline=accent, width=2 * S)
        d.ellipse([ccx - chip_r, cy2 - chip_r, ccx + chip_r, cy2 + chip_r], fill=col)
        lw = d.textlength(lbl, font=f_plbl)
        d.text((ccx - lw / 2, cy2 + chip_r + 5 * S), lbl, font=f_plbl, fill=gray)

    # palette swatches (selection row)
    n = len(palette)
    r_sw = 15 * S
    gap2 = (W * S - 40 * S - (n + 1) * r_sw * 2) / n
    sy = py0 + 108 * S
    x = 20 * S + r_sw
    # auto
    d.ellipse([x - r_sw, sy - r_sw, x + r_sw, sy + r_sw],
              fill=mix(bg, text, 0.18), outline=mix(text, bg, 0.4), width=2 * S)
    lw = d.textlength("A", font=f_pnum)
    d.text((x - lw / 2, sy - 8 * S), "A", font=f_pnum, fill=text)
    if sel["idx"] == -1 and not sel["custom"]:
        d.ellipse([x - r_sw - 4 * S, sy - r_sw - 4 * S, x + r_sw + 4 * S, sy + r_sw + 4 * S],
                  outline=accent, width=2 * S)
    for i, hexcol in enumerate(palette):
        x += r_sw * 2 + gap2
        from colors import hex_to_rgb
        if i == sel["idx"]:
            d.ellipse([x - r_sw - 4 * S, sy - r_sw - 4 * S,
                       x + r_sw + 4 * S, sy + r_sw + 4 * S], outline=accent, width=2 * S)
        d.ellipse([x - r_sw, sy - r_sw, x + r_sw, sy + r_sw], fill=hex_to_rgb(hexcol))
        lw = d.textlength(str(i + 1), font=f_pnum)
        d.text((x - lw / 2, sy - 8 * S), str(i + 1), font=f_pnum, fill=readable_on(hex_to_rgb(hexcol)))
    if sel["custom"]:
        x += r_sw * 2 + gap2
        d.ellipse([x - r_sw - 4 * S, sy - r_sw - 4 * S,
                   x + r_sw + 4 * S, sy + r_sw + 4 * S], outline=accent, width=2 * S)
        d.ellipse([x - r_sw, sy - r_sw, x + r_sw, sy + r_sw],
                  fill=hex_to_rgb(sel["custom"]))
        lw = d.textlength("C", font=f_pnum)
        d.text((x - lw / 2, sy - 8 * S), "C", font=f_pnum,
               fill=readable_on(hex_to_rgb(sel["custom"])))

    img = img.resize((W, H), Image.Resampling.LANCZOS)
    buf = io.BytesIO()
    buf.name = "preview.png"
    img.save(buf, "PNG")
    buf.seek(0)
    return buf
