import io

from PIL import Image, ImageDraw, ImageFont

from colors import M3Palette, hex_to_rgb, OUT_TONES


# ---------- Font ----------

def _font(size):
    for path in (
        "/usr/share/fonts/truetype/roboto/unhinted/Roboto-Medium.ttf",
        "/usr/share/fonts/truetype/roboto/Roboto-Medium.ttf",
        "/usr/share/fonts/truetype/roboto/unhinted/Roboto-Regular.ttf",
        "/usr/share/fonts/truetype/roboto/Roboto-Regular.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf",
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
    """Crop image to fully cover w×h (center crop)."""
    iw, ih = img.size
    scale = max(w / iw, h / ih)
    img = img.resize((int(iw * scale) + 1, int(ih * scale) + 1),
                     Image.Resampling.LANCZOS)
    x, y = (img.width - w) // 2, (img.height - h) // 2
    return img.crop((x, y, x + w, y + h))


def _mix(c1, c2, k):
    return tuple(int(a + (b - a) * k) for a, b in zip(c1, c2))


# ---------- Icon helpers (drawn with primitives) ----------

def _icon_back(d, x, y, size, col):
    """← back arrow"""
    s = size
    d.line([(x + s * 0.65, y), (x, y + s * 0.5), (x + s * 0.65, y + s)],
           fill=col, width=max(2, int(s * 0.14)))


def _icon_attach(d, cx, cy, s, col):
    """📎 paperclip"""
    r = s * 0.28
    d.arc([cx - r, cy - r, cx + r, cy + r], 90, 270, fill=col,
          width=max(2, int(s * 0.09)))
    d.arc([cx - r, cy - r * 1.8, cx + r, cy + r * 0.2], 270, 90, fill=col,
          width=max(2, int(s * 0.09)))
    d.line([(cx, cy - r), (cx, cy + r)], fill=col, width=max(2, int(s * 0.09)))


def _icon_mic(d, cx, cy, s, col):
    """🎤 microphone"""
    r = s * 0.16
    d.rounded_rectangle([cx - r, cy - s * 0.35, cx + r, cy + s * 0.1],
                        radius=r, fill=col)
    d.arc([cx - s * 0.24, cy - s * 0.1, cx + s * 0.24, cy + s * 0.4],
          0, 180, fill=col, width=max(2, int(s * 0.08)))
    d.line([(cx, cy + s * 0.4), (cx, cy + s * 0.5)], fill=col,
           width=max(2, int(s * 0.08)))
    d.line([(cx - s * 0.15, cy + s * 0.5), (cx + s * 0.15, cy + s * 0.5)],
           fill=col, width=max(2, int(s * 0.08)))


def _icon_send(d, cx, cy, s, col):
    """➤ send triangle"""
    h = s * 0.5
    d.polygon([(cx - h * 0.5, cy - h), (cx - h * 0.5, cy + h), (cx + h, cy)],
              fill=col)


def _icon_emoji(d, cx, cy, s, col):
    """😊 smiley"""
    r = s * 0.3
    d.ellipse([cx - r, cy - r, cx + r, cy + r], outline=col,
              width=max(2, int(s * 0.08)))
    er = r * 0.15
    d.ellipse([cx - r * 0.45 - er, cy - r * 0.35 - er,
               cx - r * 0.45 + er, cy - r * 0.35 + er], fill=col)
    d.ellipse([cx + r * 0.45 - er, cy - r * 0.35 - er,
               cx + r * 0.45 + er, cy - r * 0.35 + er], fill=col)
    d.arc([cx - r * 0.55, cy - r * 0.2, cx + r * 0.55, cy + r * 0.55],
          15, 165, fill=col, width=max(2, int(s * 0.08)))


def _icon_mute(d, cx, cy, s, col):
    """🔕 muted speaker"""
    d.polygon([(cx - s * 0.4, cy - s * 0.12), (cx - s * 0.15, cy - s * 0.12),
               (cx + s * 0.1, cy - s * 0.35), (cx + s * 0.1, cy + s * 0.35),
               (cx - s * 0.15, cy + s * 0.12), (cx - s * 0.4, cy + s * 0.12)],
              fill=col)
    d.line([(cx + s * 0.25, cy - s * 0.2), (cx + s * 0.45, cy + s * 0.2)],
           fill=col, width=max(2, int(s * 0.07)))
    d.line([(cx + s * 0.45, cy - s * 0.2), (cx + s * 0.25, cy + s * 0.2)],
           fill=col, width=max(2, int(s * 0.07)))


def _draw_checks(d, x, y, s, col):
    """✓✓ double check"""
    w = max(2, int(s * 0.12))
    d.line([(x, y + s * 0.35), (x + s * 0.3, y + s * 0.7)], fill=col, width=w)
    d.line([(x + s * 0.3, y + s * 0.7), (x + s * 0.7, y)], fill=col, width=w)
    d.line([(x + s * 0.45, y + s * 0.35), (x + s * 0.75, y + s * 0.7)], fill=col, width=w)
    d.line([(x + s * 0.75, y + s * 0.7), (x + s * 1.15, y)], fill=col, width=w)


def _draw_avatar(d, cx, cy, r, col_bg, col_txt, letter, f):
    d.ellipse([cx - r, cy - r, cx + r, cy + r], fill=col_bg)
    tw = d.textlength(letter, font=f)
    d.text((cx - tw / 2, cy - r * 0.55), letter, font=f, fill=col_txt)


# ---------- Main preview ----------

def render_preview(seed_hex, style, alpha_pct, role, swatches, sel, wall_bytes):
    """1:1 Telegram Android chat screen preview."""
    p = M3Palette(seed_hex)
    dark = style == "dark"
    S = 2                          # supersampling
    W, H = 480, 860                # final output size

    # --- Color tokens ---
    N  = lambda t: p.neutral(t)
    NV = lambda t: p.neutral_variant(t)
    P  = lambda t: p.primary(t)
    out_fn = getattr(p, role)
    out_t = OUT_TONES[role][0 if dark else 1]

    c_bar        = N(18 if dark else 98)       # action bar
    c_bar_title  = N(96 if dark else 8)
    c_bar_sub    = N(55 if dark else 48)
    c_bar_icon   = N(85 if dark else 25)
    c_input_bg   = N(14 if dark else 97)       # input bar
    c_input_hint = N(45 if dark else 42)
    c_input_icon = N(55 if dark else 38)
    c_send_bg    = P(62 if dark else 42)
    c_send_ic    = N(10 if dark else 100)
    c_bubble_in  = NV(24 if dark else 93)
    c_bubble_out = out_fn(out_t)
    c_txt_in     = N(95 if dark else 12)
    c_txt_out    = N(98 if dark else 100)
    c_time_in    = N(55 if dark else 48)
    c_time_out   = N(75 if dark else 80)
    c_check_out  = N(80 if dark else 85)
    c_chip_bg    = N(35 if dark else 55)
    c_chip_txt   = N(92 if dark else 98)
    c_wall_flat  = N(8 if dark else 90)
    c_panel_bg   = N(16 if dark else 96)
    c_panel_txt  = N(70 if dark else 40)
    c_sel_ring   = P(70 if dark else 50)
    c_sw_border  = N(40 if dark else 70)

    # --- Fonts ---
    f_name  = _font(17 * S)     # action bar title
    f_sub   = _font(13 * S)     # action bar subtitle
    f_bub   = _font(16 * S)     # bubble text
    f_time  = _font(12 * S)     # timestamp
    f_chip  = _font(13 * S)     # date chip
    f_hint  = _font(15 * S)     # input hint
    f_lbl   = _font(13 * S)     # swatch label
    f_info  = _font(13 * S)     # panel info
    f_av    = _font(20 * S)     # avatar letter

    # ============ CANVAS ============
    WW, HH = W * S, H * S
    BAR_H   = 56 * S             # action bar height
    INPUT_H = 52 * S             # input bar height
    PANEL_H = 150 * S            # swatches panel height
    CHAT_TOP = BAR_H
    CHAT_BOT = HH - INPUT_H - PANEL_H

    # ---- Chat wallpaper ----
    if wall_bytes:
        base = _cover(Image.open(io.BytesIO(wall_bytes)),
                      WW, CHAT_BOT - CHAT_TOP).convert("RGBA")
        tint_col = (0, 0, 0, 100) if dark else (255, 255, 255, 110)
        tint = Image.new("RGBA", base.size, tint_col)
        wall = Image.alpha_composite(base, tint).convert("RGB")
    else:
        wall = Image.new("RGB", (WW, CHAT_BOT - CHAT_TOP), c_wall_flat)

    # Full canvas
    img = Image.new("RGB", (WW, HH), c_wall_flat)
    img.paste(wall, (0, BAR_H))
    d = ImageDraw.Draw(img)

    # ---- Action bar ----
    d.rectangle([0, 0, WW, BAR_H], fill=c_bar)
    # subtle divider under bar
    d.line([(0, BAR_H), (WW, BAR_H)], fill=N(24 if dark else 90), width=S)

    # Back arrow
    _icon_back(d, 10 * S, 14 * S, 26 * S, c_bar_icon)
    # Avatar
    av_cx, av_cy, av_r = 52 * S, BAR_H // 2, 17 * S
    _draw_avatar(d, av_cx, av_cy, av_r, P(55 if dark else 45), N(10 if dark else 100),
                 "T", f_av)
    # Name + status
    d.text((78 * S, 9 * S), "Telegram", font=f_name, fill=c_bar_title)
    d.text((78 * S, 30 * S), "online", font=f_sub, fill=P(65 if dark else 45))
    # Menu dots (⋮)
    for i in range(3):
        d.ellipse([(WW - 22 * S) , (BAR_H // 2 - 8 * S + i * 8 * S),
                   (WW - 18 * S), (BAR_H // 2 - 4 * S + i * 8 * S)],
                  fill=c_bar_icon)

    # ---- Date chip ----
    alpha = round(255 * (1 - alpha_pct / 100))
    ov = Image.new("RGBA", (WW, HH), (0, 0, 0, 0))
    od = ImageDraw.Draw(ov)

    chip_w = d.textlength("Today", font=f_chip) + 32 * S
    chip_h = 26 * S
    chip_x = (WW - chip_w) // 2
    chip_y = CHAT_TOP + 14 * S
    od.rounded_rectangle([chip_x, chip_y, chip_x + chip_w, chip_y + chip_h],
                         radius=chip_h // 2, fill=c_chip_bg + (160,))

    # ---- Message bubbles ----
    pad_x, pad_y = 12 * S, 8 * S
    line_h = 22 * S
    time_w = 62 * S

    def draw_msg(text_lines, fill_rgb, out, y, time_str, read=True):
        """Draw a Telegram-style bubble. Returns bottom y."""
        nonlocal od
        bubble_alpha = alpha if alpha_pct > 0 else 255
        txt_h = len(text_lines) * line_h
        max_tw = max(d.textlength(t, font=f_bub) for t in text_lines)
        bw = max(max_tw, time_w) + pad_x * 2
        bh = txt_h + pad_y * 2 + 14 * S
        bx = WW - bw - 14 * S if out else 14 * S
        r = 16 * S

        # Rounded rect with one small corner (Telegram style)
        corners = [r, r, r, 2 * S] if out else [r, r, 2 * S, r]
        od.rounded_rectangle([bx, y, bx + bw, y + bh], radius=r,
                             fill=fill_rgb + (bubble_alpha,))
        return bx, y, bw, bh, text_lines

    def draw_msg_text(b, out, time_str, read=True):
        nonlocal d
        bx, by, bw, bh, lines = b
        txt_col = c_txt_out if out else c_txt_in
        time_col = c_time_out if out else c_time_in
        check_col = c_check_out if out else c_time_in

        ty = by + pad_y
        for t in lines:
            d.text((bx + pad_x, ty), t, font=f_bub, fill=txt_col)
            ty += line_h

        # Time + checks (bottom-right)
        ty2 = by + bh - pad_y - 12 * S
        tx = bx + bw - pad_x
        if out:
            _draw_checks(d, tx - 48 * S, ty2, 14 * S, check_col)
            d.text((tx - d.textlength(time_str, font=f_time), ty2),
                   time_str, font=f_time, fill=time_col)
        else:
            d.text((tx - d.textlength(time_str, font=f_time), ty2),
                   time_str, font=f_time, fill=time_col)

    y = chip_y + chip_h + 18 * S
    b1 = draw_msg(["Hey! Check out my new", "Material You theme 👀"], c_bubble_in,
                  False, y, "14:32")
    draw_msg_text(b1, False, "14:32")

    y2 = b1[1] + b1[3] + 10 * S
    b2 = draw_msg(["Looks amazing! 🔥"], c_bubble_out, True, y2, "14:33")
    draw_msg_text(b2, True, "14:33")

    # Composite transparent layer over image
    img = Image.alpha_composite(img.convert("RGBA"), ov).convert("RGB")
    d = ImageDraw.Draw(img)
    # Date chip text (after composite so it's on top)
    chip_tw = d.textlength("Today", font=f_chip)
    d.text(((WW - chip_tw) // 2, chip_y + 5 * S), "Today",
           font=f_chip, fill=c_chip_txt)

    # ---- Swatches panel ----
    py0 = HH - PANEL_H - INPUT_H
    d.rectangle([0, py0, WW, HH - INPUT_H], fill=c_panel_bg)
    d.line([(0, py0), (WW, py0)], fill=N(24 if dark else 88), width=S)

    info_txt = f"{style.capitalize()}  •  {alpha_pct}% transparent  •  {seed_hex}  •  {role}"
    d.text((18 * S, py0 + 10 * S), info_txt, font=f_info, fill=c_panel_txt)

    # Swatch circles
    n = len(swatches)
    r_sw = 24 * S
    gap = 16 * S
    total_w = n * (r_sw * 2 + gap) - gap
    sx0 = (WW - total_w) // 2
    sy = py0 + 55 * S
    for i, hexcol in enumerate(swatches):
        ccx = sx0 + i * (r_sw * 2 + gap) + r_sw
        if i == sel:
            d.ellipse([ccx - r_sw - 6 * S, sy - r_sw - 6 * S,
                       ccx + r_sw + 6 * S, sy + r_sw + 6 * S],
                      outline=c_sel_ring, width=3 * S)
        d.ellipse([ccx - r_sw, sy - r_sw, ccx + r_sw, sy + r_sw],
                  fill=hex_to_rgb(hexcol), outline=c_sw_border, width=2 * S)
        label = "C" if (i == n - 1 and n > 5) else str(i + 1)
        lw = d.textlength(label, font=f_lbl)
        d.text((ccx - lw / 2, sy + r_sw + 8 * S), label, font=f_lbl,
               fill=c_panel_txt)

    # Hint line under swatches
    hint = "Pick a color ↑  •  Type custom: #RRGGBB"
    hw = d.textlength(hint, font=f_info)
    d.text(((WW - hw) // 2, py0 + 118 * S), hint, font=f_info,
           fill=N(55 if dark else 55))

    # ---- Input bar ----
    iy = HH - INPUT_H
    d.rectangle([0, iy, WW, HH], fill=c_input_bg)
    d.line([(0, iy), (WW, iy)], fill=N(22 if dark else 88), width=S)

    # Attach icon
    _icon_attach(d, 28 * S, iy + INPUT_H // 2, 22 * S, c_input_icon)
    # Emoji icon
    _icon_emoji(d, 56 * S, iy + INPUT_H // 2, 22 * S, c_input_icon)
    # "Message" hint
    d.text((76 * S, iy + (INPUT_H - 15 * S) // 2), "Message",
           font=f_hint, fill=c_input_hint)
    # Send button
    btn_r = 19 * S
    btn_cx, btn_cy = WW - 30 * S, iy + INPUT_H // 2
    d.ellipse([btn_cx - btn_r, btn_cy - btn_r, btn_cx + btn_r, btn_cy + btn_r],
              fill=c_send_bg)
    _icon_send(d, btn_cx - 2 * S, btn_cy, 20 * S, c_send_ic)

    # ---- Downscale & save ----
    img = img.resize((W, H), Image.Resampling.LANCZOS)
    buf = io.BytesIO()
    buf.name = "preview.png"
    img.save(buf, "PNG")
    buf.seek(0)
    return buf
