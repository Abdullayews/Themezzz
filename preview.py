import io

from PIL import Image, ImageDraw, ImageFont

from colors import hex_to_rgb

W = (255, 255, 255)

CHIPS = [("bg", "BG"), ("bar", "BAR"), ("in", "IN"), ("out", "OUT"),
         ("link", "LINK"), ("accent", "ACC"), ("wall", "WALL")]


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


def render_preview(cats, wall, wall_mode, wall_bytes, swatches, active):
    """1:1 Telegram chat mock — only colors the user actually picked."""
    C = {k: hex_to_rgb(cats[k]["hex"]) for k in cats}
    A = {k: round(255 * (1 - cats[k]["alpha"] / 100)) for k in cats}
    bg, bar, inb, outb = C["bg"], C["bar"], C["in"], C["out"]
    link, acc = C["link"], C["accent"]

    S = 2
    WD, HT = 480, 940
    BAR_H, INP_H, PAN_H = 56 * S, 52 * S, 196 * S
    chat_b = HT * S - INP_H - PAN_H
    iy = chat_b
    py0 = chat_b + INP_H

    # ---- wallpaper strip (user's image as-is, or flat color + its alpha) ----
    if wall_mode == "image" and wall_bytes:
        strip = _cover(Image.open(io.BytesIO(wall_bytes)), WD * S, chat_b)
    else:
        strip = Image.new("RGB", (WD * S, chat_b), bg)
        wa = round(255 * (1 - wall["alpha"] / 100))
        fov = Image.new("RGBA", (WD * S, chat_b),
                        hex_to_rgb(wall["hex"]) + (wa,))
        strip = Image.alpha_composite(strip.convert("RGBA"), fov).convert("RGB")

    # ---- alpha overlay: bar, date chip, bubbles (chat_b ölçüsü — uyğundur) ----
    ov = Image.new("RGBA", (WD * S, chat_b), (0, 0, 0, 0))
    od = ImageDraw.Draw(ov)
    od.rectangle([0, 0, WD * S, BAR_H], fill=bar + (A["bar"],))

    f_title, f_sub = _font(17 * S), _font(13 * S)
    f_bub, f_time = _font(16 * S), _font(12 * S)
    f_info, f_lbl = _font(13 * S), _font(10 * S)

    chip_w, chip_h = 150 * S, 40 * S
    cx, cy = (WD * S - chip_w) // 2, BAR_H + 14 * S
    od.rounded_rectangle([cx, cy, cx + chip_w, cy + chip_h], radius=chip_h // 2,
                         fill=bg + (max(A["bg"], 160),))

    pad_x, pad_y, line_h = 12 * S, 9 * S, 22 * S

    def bubble(lines_txt, fill, alpha, out=False, y=0):
        w = max(od.textlength(t, font=f_bub) for t in lines_txt) + pad_x * 2 + 60 * S
        h = len(lines_txt) * line_h + pad_y * 2 + 14 * S
        x = (WD * S - w - 14 * S) if out else 14 * S
        od.rounded_rectangle([x, y, x + w, y + h], radius=16 * S,
                             fill=fill + (alpha,))
        return x, y, w, h, lines_txt

    y1 = cy + chip_h + 18 * S
    b1 = bubble(["Hey! Check this out —", "all text is white now"],
                inb, A["in"], y=y1)
    y2 = y1 + b1[3] + 10 * S
    b2 = bubble(["Nice! 🔥", "t.me/addtheme/mine"], outb, A["out"], out=True, y=y2)

    strip = Image.alpha_composite(strip.convert("RGBA"), ov).convert("RGB")
    d = ImageDraw.Draw(strip)

    # bar content (white — rule)
    d.line([(14 * S, 22 * S), (14 * S, 34 * S), (24 * S, 28 * S)], fill=W, width=3 * S)
    d.ellipse([42 * S, 11 * S, 74 * S, 43 * S], fill=acc)
    tw = d.textlength("T", font=f_title)
    d.text((58 * S - tw / 2, 13 * S), "T", font=f_title, fill=W)
    d.text((84 * S, 8 * S), "Telegram", font=f_title, fill=W)
    d.text((84 * S, 30 * S), "online", font=f_sub, fill=W)
    for i in range(3):
        d.ellipse([(WD - 22) * S, (24 + i * 8) * S,
                   (WD - 18) * S, (28 + i * 8) * S], fill=W)
    d.text(((WD * S - d.textlength("Today", font=f_sub)) // 2, cy + 12 * S),
           "Today", font=f_sub, fill=W)

    # bubble content (white text; the link line is the ONLY colored text)
    def texts(b, out, time_str, link_line=None):
        x, y, w, h, lines = b
        ty = y + pad_y
        for t in lines:
            col = link if (link_line and t == link_line) else W
            d.text((x + pad_x, ty), t, font=f_bub, fill=col)
            ty += line_h
        ty2 = y + h - pad_y - 12 * S
        tx = x + w - pad_x
        if out:
            _checks(d, tx - 46 * S, ty2, 13 * S, W)
        d.text((tx - d.textlength(time_str, font=f_time), ty2), time_str,
               font=f_time, fill=W)

    texts(b1, False, "14:32")
    texts(b2, True, "14:33", link_line="t.me/addtheme/mine")

    # ---- assemble canvas ----
    img = Image.new("RGB", (WD * S, HT * S), bg)
    img.paste(strip, (0, 0))

    # ✅ FIX: overlay tam kətan ölçüsündə — yalnız input bar bölgəsi doldurulur
    ov2 = Image.new("RGBA", (WD * S, HT * S), (0, 0, 0, 0))
    ImageDraw.Draw(ov2).rectangle([0, iy, WD * S, iy + INP_H],
                                  fill=bg + (A["bg"],))
    img = Image.alpha_composite(img.convert("RGBA"), ov2).convert("RGB")
    d = ImageDraw.Draw(img)

    # input bar content (white — rule)
    d.arc([20 * S, iy + 16 * S, 36 * S, iy + 32 * S], 90, 270, fill=W, width=2 * S)
    d.text((72 * S, iy + 17 * S), "Message", font=f_sub, fill=W)
    d.ellipse([(WD - 44) * S, iy + 10 * S, (WD - 10) * S, iy + 44 * S], fill=acc)
    d.polygon([(WD - 36) * S, iy + 19 * S, (WD - 36) * S, iy + 35 * S,
               (WD - 22) * S, iy + 27 * S], fill=W)

    # ---- editor panel (opaque) ----
    d.rectangle([0, py0, WD * S, HT * S], fill=bg)
    soft = tuple(int(c + (255 - c) * 0.25) for c in bg)   # bg + 25% white
    d.line([(0, iy), (WD * S, iy)], fill=soft, width=S)
    d.line([(0, py0), (WD * S, py0)], fill=soft, width=S)

    show_alpha = active in ("bg", "bar", "in", "out", "accent") or \
                 (active == "wall" and wall_mode == "flat")
    a = cats.get(active, wall)
    a_txt = f" · {a['alpha']}%" if (show_alpha and a.get("alpha")) else ""
    d.text((16 * S, py0 + 8 * S),
           f"Editing: {active}  {a['hex']}{a_txt}", font=f_info, fill=W)

    # category chips
    r = 13 * S
    gap = (WD * S - 32 * S - 7 * 2 * r) / 6
    cy2 = py0 + 46 * S
    for i, (k, lbl) in enumerate(CHIPS):
        ccx = int(16 * S + r + i * (2 * r + gap))
        if k == "wall" and wall_mode == "image" and wall_bytes:
            thumb = _cover(Image.open(io.BytesIO(wall_bytes)), 2 * r, 2 * r)
            mask = Image.new("L", (2 * r, 2 * r), 0)
            ImageDraw.Draw(mask).ellipse([0, 0, 2 * r, 2 * r], fill=255)
            img.paste(thumb, (ccx - r, cy2 - r), mask)
            d = ImageDraw.Draw(img)
        else:
            col = hex_to_rgb(wall["hex"]) if k == "wall" else C[k]
            d.ellipse([ccx - r, cy2 - r, ccx + r, cy2 + r], fill=col)
        if k == active:
            d.ellipse([ccx - r - 4 * S, cy2 - r - 4 * S,
                       ccx + r + 4 * S, cy2 + r + 4 * S], outline=acc, width=2 * S)
        lw = d.textlength(lbl, font=f_lbl)
        d.text((ccx - lw / 2, cy2 + r + 5 * S), lbl, font=f_lbl, fill=W)

    # photo swatches (suggestions — tap to apply)
    n = min(len(swatches), 6)
    r_sw = 15 * S
    gap2 = (WD * S - 40 * S - n * 2 * r_sw) / max(1, n - 1)
    sy = py0 + 118 * S
    cur = a["hex"]
    for i in range(n):
        x = int(20 * S + r_sw + i * (2 * r_sw + gap2))
        col = hex_to_rgb(swatches[i])
        if swatches[i] == cur:
            d.ellipse([x - r_sw - 4 * S, sy - r_sw - 4 * S,
                       x + r_sw + 4 * S, sy + r_sw + 4 * S],
                      outline=acc, width=2 * S)
        d.ellipse([x - r_sw, sy - r_sw, x + r_sw, sy + r_sw], fill=col)
        num = str(i + 1)
        lw = d.textlength(num, font=f_lbl)
        lum = (0.2126 * col[0] + 0.7152 * col[1] + 0.0722 * col[2]) / 255
        d.text((x - lw / 2, sy - 6 * S), num, font=f_lbl,
               fill=(20, 20, 20) if lum > 0.5 else W)

    d.text((16 * S, py0 + 158 * S),
           "Swatches from your photo — tap one to apply to the selected part",
           font=f_lbl, fill=W)

    img = img.resize((WD, HT), Image.Resampling.LANCZOS)
    buf = io.BytesIO()
    buf.name = "preview.png"
    img.save(buf, "PNG")
    buf.seek(0)
    return buf
