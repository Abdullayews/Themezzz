import io

from PIL import Image, ImageDraw, ImageFont

from colors import M3Palette, hex_to_rgb, OUT_TONES


def _font(sz: int):
    for path in (
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        "/usr/share/fonts/TTF/DejaVuSans.ttf",
        "/usr/share/fonts/dejavu/DejaVuSans.ttf",
    ):
        try:
            return ImageFont.truetype(path, sz)
        except Exception:
            continue
    try:
        return ImageFont.load_default(size=sz)
    except TypeError:
        return ImageFont.load_default()


def _cover(img, w, h):
    """Crop image to fully cover the w×h area."""
    iw, ih = img.size
    scale = max(w / iw, h / ih)
    img = img.resize((int(iw * scale) + 1, int(ih * scale) + 1), Image.LANCZOS)
    x, y = (img.width - w) // 2, (img.height - h) // 2
    return img.crop((x, y, x + w, y + h))


def render_preview(seed_hex, style, alpha_pct, role, chroma, contrast,
                   swatches, sel, info_text, wall_bytes):
    p = M3Palette(seed_hex, chroma)
    dark = style == "dark"
    S = 2                       # supersampling → sharp text
    W, H = 460, 820

    sh = (-4 * contrast) if dark else (4 * contrast)
    N = lambda t: p.neutral(t + (sh if t <= 60 else 0))
    NV = lambda t: p.neutral_variant(t + (sh if t <= 60 else 0))
    P = lambda t: p.primary(t)
    out_fn = getattr(p, role)
    out_t = OUT_TONES[role][0 if dark else 1]

    # ---- Background ----
    if wall_bytes:
        base = _cover(Image.open(io.BytesIO(wall_bytes)), W * S, H * S).convert("RGBA")
        tint = Image.new("RGBA", base.size,
                         (0, 0, 0, 130) if dark else (255, 255, 255, 135))
        img = Image.alpha_composite(base, tint).convert("RGB")
    else:
        img = Image.new("RGB", (W * S, H * S))
        top, bot = (N(7), N(18)) if dark else (N(86), N(95))
        d = ImageDraw.Draw(img)
        for y in range(H * S):
            k = y / (H * S)
            d.line([(0, y), (W * S, y)],
                   fill=tuple(int(a + (b - a) * k) for a, b in zip(top, bot)))

    f_title, f_text, f_small = _font(30 * S), _font(23 * S), _font(18 * S)
    d = ImageDraw.Draw(img)

    # ---- Action bar ----
    d.rectangle([0, 0, W * S, 92 * S], fill=N(20 if dark else 97))
    d.text((26 * S, 16 * S), "Material You", font=f_title, fill=N(96 if dark else 8))
    d.text((26 * S, 58 * S), "theme preview", font=f_small, fill=N(58 if dark else 48))
    d.ellipse([(W - 66) * S, 26 * S, (W - 26) * S, 66 * S], fill=P(65 if dark else 45))
    # Small circle showing the current seed color
    d.ellipse([(W - 100) * S, 34 * S, (W - 72) * S, 62 * S],
              fill=hex_to_rgb(seed_hex), outline=N(100 if dark else 0), width=2 * S)

    # ---- Transparent layer: date chip + bubbles ----
    alpha = round(255 * (1 - alpha_pct / 100))
    ov = Image.new("RGBA", (W * S, H * S), (0, 0, 0, 0))
    od = ImageDraw.Draw(ov)

    chip_w, chip_h = 150 * S, 40 * S
    cx, cy = (W * S - chip_w) // 2, 122 * S
    od.rounded_rectangle([cx, cy, cx + chip_w, cy + chip_h],
                         radius=chip_h // 2, fill=N(45) + (102,))

    def add_bubble(lines_text, fill, out=False, y=190 * S):
        pad, lh = 18 * S, 32 * S
        w = max(d.textlength(t, font=f_text) for t in lines_text) + pad * 2
        h = len(lines_text) * lh + pad * 2 + 26 * S
        x = (W * S - w - 26 * S) if out else 26 * S
        od.rounded_rectangle([x, y, x + w, y + h], radius=26 * S, fill=fill + (alpha,))
        return x, y, w, h, pad, lh, lines_text

    b1 = add_bubble(["Hi! Building your", "new theme"], NV(24 if dark else 92))
    b2 = add_bubble(["Looks great!"], out_fn(out_t), out=True, y=400 * S)

    img = Image.alpha_composite(img.convert("RGBA"), ov).convert("RGB")
    d = ImageDraw.Draw(img)

    def draw_texts(b, tcol, timecol, time):
        x, y, w, h, pad, lh, lines = b
        ty = y + pad - 4 * S
        for t in lines:
            d.text((x + pad, ty), t, font=f_text, fill=tcol)
            ty += lh
        tw = d.textlength(time, font=f_small)
        d.text((x + w - pad - tw, y + h - pad - 24 * S), time, font=f_small, fill=timecol)

    draw_texts(b1, N(95 if dark else 12), N(60 if dark else 50), "14:32")
    draw_texts(b2, N(98 if dark else 100), N(80 if dark else 60), "14:33")
    d.text(((W * S - d.textlength("Today", font=f_small)) // 2, cy + 9 * S),
           "Today", font=f_small, fill=N(100))

    # ---- Settings panel with color swatches ----
    panel_y0, panel_y1 = 556 * S, 716 * S
    d.rounded_rectangle([16 * S, panel_y0, (W - 16) * S, panel_y1],
                        radius=22 * S, fill=N(16 if dark else 96))

    # Info text (greedy word wrap, max 3 lines)
    max_w = (W - 64) * S
    words, info_lines, cur = info_text.split(" "), [], ""
    for w_ in words:
        t = (cur + " " + w_).strip()
        if d.textlength(t, font=f_small) <= max_w:
            cur = t
        else:
            if cur:
                info_lines.append(cur)
            cur = w_
    if cur:
        info_lines.append(cur)
    ty = panel_y0 + 12 * S
    for ln in info_lines[:3]:
        d.text((32 * S, ty), ln, font=f_small, fill=N(72 if dark else 42))
        ty += 26 * S

    # Swatch circles (numbered; "C" = custom color)
    n = len(swatches)
    r = 22 * S
    spacing = (W * S - 120 * S) / max(1, n - 1)
    x0 = 60 * S
    cyc = panel_y1 - 58 * S
    for i, hexcol in enumerate(swatches):
        ccx = int(x0 + i * spacing)
        if i == sel:
            d.ellipse([ccx - r - 7 * S, cyc - r - 7 * S, ccx + r + 7 * S, cyc + r + 7 * S],
                      outline=N(100 if dark else 10), width=3 * S)
        d.ellipse([ccx - r, cyc - r, ccx + r, cyc + r], fill=hex_to_rgb(hexcol),
                  outline=N(35 if dark else 75), width=2 * S)
        label = "C" if (i == n - 1 and n > 5) else str(i + 1)
        tw = d.textlength(label, font=f_small)
        d.text((ccx - tw / 2, cyc + r + 4 * S), label, font=f_small,
               fill=N(85 if dark else 20))

    # ---- Message input panel ----
    d.rectangle([0, H * S - 96 * S, W * S, H * S], fill=N(15 if dark else 96))
    d.text((70 * S, H * S - 58 * S), "Write a message...", font=f_text,
           fill=N(50 if dark else 45))
    d.ellipse([(W - 88) * S, (H - 80) * S, (W - 24) * S, (H - 16) * S],
              fill=P(65 if dark else 45))
    d.polygon([(W - 71) * S, (H - 64) * S, (W - 71) * S, (H - 32) * S,
               (W - 43) * S, (H - 48) * S],
              fill=N(10) if dark else N(100))

    img = img.resize((W, H), Image.LANCZOS)
    buf = io.BytesIO()
    img.save(buf, "PNG")
    buf.seek(0)
    return buf
