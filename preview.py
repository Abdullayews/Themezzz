import io
import math
from PIL import Image, ImageDraw, ImageFilter

from colors import readable_on, mix, ensure_contrast, luminance


# ---------- Vector Drawing Helpers ----------

def _draw_status_bar(d, pw, text_color, body_color):
    """Draws realistic phone status bar (Time, Camera, Battery, Wifi, Signal)."""
    col = mix(text_color, body_color, 0.35) + (220,)
    # Notch / Camera punch hole
    d.ellipse([pw // 2 - 6, 12, pw // 2 + 6, 24], fill=(0, 0, 0, 200))
    # Time placeholder line / dots
    d.rounded_rectangle([24, 14, 64, 22], radius=3, fill=col)
    # Signal bars
    for i in range(4):
        h = 3 + i * 2.5
        d.rectangle([pw - 65 + i * 5, 22 - h, pw - 62 + i * 5, 22], fill=col)
    # Wifi icon arc approximation
    d.arc([pw - 42, 12, pw - 30, 24], start=210, end=330, fill=col, width=2)
    # Battery icon
    d.rounded_rectangle([pw - 26, 13, pw - 10, 23], radius=2, outline=col, width=1)
    d.rectangle([pw - 24, 15, pw - 14, 21], fill=col)
    d.rectangle([pw - 9, 15, pw - 8, 21], fill=col)


def _draw_back_arrow(d, x, y, size, color):
    d.line([(x + size, y), (x + 2, y + size // 2)], fill=color, width=3)
    d.line([(x + 2, y + size // 2), (x + size, y + size)], fill=color, width=3)


def _draw_search_icon(d, x, y, r, color):
    d.ellipse([x, y, x + r, y + r], outline=color, width=2)
    d.line([(x + r - 2, y + r - 2), (x + r + 5, y + r + 5)], fill=color, width=2)


def _draw_more_dots(d, x, y, color):
    for i in range(3):
        d.ellipse([x, y + i * 6, x + 3, y + i * 6 + 3], fill=color)


def _draw_hamburger(d, x, y, w, color):
    for i in range(3):
        d.rounded_rectangle([x, y + i * 6, x + w, y + i * 6 + 2], radius=1, fill=color)


def _draw_checkmarks(d, x, y, color):
    d.line([(x, y + 4), (x + 3, y + 7), (x + 8, y + 1)], fill=color, width=2)
    d.line([(x + 4, y + 4), (x + 7, y + 7), (x + 12, y + 1)], fill=color, width=2)


def _draw_clip_icon(d, x, y, color):
    d.arc([x, y, x + 10, y + 10], 180, 0, fill=color, width=2)
    d.line([(x, y + 5), (x, y + 14)], fill=color, width=2)
    d.line([(x + 10, y + 5), (x + 10, y + 12)], fill=color, width=2)
    d.arc([x + 3, y + 9], 0, 180, fill=color, width=2)


def _draw_pencil_icon(d, cx, cy, size, color):
    d.line([(cx - size // 2, cy + size // 2), (cx + size // 2, cy - size // 2)], fill=color, width=3)
    d.polygon([(cx + size // 2 - 1, cy - size // 2 - 3), (cx + size // 2 + 3, cy - size // 2 + 1), (cx + size // 2 + 2, cy - size // 2 - 2)], fill=color)


def _cover(img, w, h):
    """Center-crop image to fully cover w×h maintaining aspect ratio."""
    iw, ih = img.size
    scale = max(w / iw, h / ih)
    img = img.resize((int(iw * scale) + 1, int(ih * scale) + 1), Image.Resampling.LANCZOS)
    x, y = (img.width - w) // 2, (img.height - h) // 2
    return img.crop((x, y, x + w, y + h))


# ---------- Main Renderer ----------

def render_preview(colors, alphas, wall_bytes, wall_flat):
    """
    Pixel-perfect dual-phone Telegram UI renderer.
    """
    bg, bar = colors["bg"], colors["bar"]
    inb, outb = colors["in"], colors["out"]
    text, accent = colors["text"], colors["accent"]
    reply = colors["reply"]

    get_alpha = lambda k: max(0, min(255, round(255 * (1 - alphas.get(k, 0) / 100.0))))
    a_bar = get_alpha("bar")
    a_in, a_out = get_alpha("in"), get_alpha("out")
    a_text = get_alpha("text")
    a_acc = get_alpha("accent")
    a_reply = get_alpha("reply")

    dark = luminance(bg) < 0.5

    # Resolution & Supersampling (2x for ultra sharp edges)
    S = 2
    W, H = 1000 * S, 1000 * S

    # Dynamic Canvas Backdrop
    outer_bg = mix(accent, (0, 0, 0) if dark else (255, 255, 255), 0.6 if dark else 0.35)
    canvas = Image.new("RGBA", (W, H), outer_bg + (255,))

    # Phone Frame Dimensions
    pw, ph = 410 * S, 850 * S
    py = (H - ph) // 2
    px1, px2 = 55 * S, 535 * S
    p_radius = 44 * S

    phone_body_1 = mix(bg, (0, 0, 0), 0.35 if dark else 0.08)
    phone_body_2 = mix(bg, (0, 0, 0), 0.30 if dark else 0.10)

    # ================= LEFT PHONE: CHAT VIEW =================
    phone1_img = Image.new("RGBA", (pw, ph), (0, 0, 0, 0))
    p1_d = ImageDraw.Draw(phone1_img)

    # Frame Shell
    p1_d.rounded_rectangle([0, 0, pw, ph], radius=p_radius, fill=phone_body_1 + (255,))

    # Chat Wallpaper Layer
    chat_h = ph - (116 * S)
    chat_y = 60 * S

    if wall_bytes:
        w_img = _cover(Image.open(io.BytesIO(wall_bytes)), pw, chat_h)
        tint = Image.new("RGBA", (pw, chat_h), (0, 0, 0, 100) if dark else (255, 255, 255, 90))
        w_img = Image.alpha_composite(w_img.convert("RGBA"), tint)
        phone1_img.paste(w_img, (0, chat_y))
    else:
        wf = wall_flat if wall_flat else mix(bg, (0, 0, 0), 0.15 if dark else 0.03)
        glow_img = Image.new("RGBA", (pw, chat_h), wf + (255,))
        glow_d = ImageDraw.Draw(glow_img)
        glow_col = mix(accent, (255, 255, 255), 0.2)
        glow_d.ellipse([pw // 2 - 140 * S, chat_h // 2 - 140 * S,
                        pw // 2 + 140 * S, chat_h // 2 + 140 * S], fill=glow_col + (40,))
        phone1_img.paste(glow_img, (0, chat_y))

    p1_d = ImageDraw.Draw(phone1_img)

    # Status Bar
    _draw_status_bar(p1_d, pw, text, phone_body_1)

    # Action Bar (Header)
    p1_d.rectangle([0, 30 * S, pw, 86 * S], fill=bar + (a_bar,))
    _draw_back_arrow(p1_d, 16 * S, 51 * S, 14 * S, ensure_contrast(text, bar) + (a_text,))
    p1_d.ellipse([46 * S, 44 * S, 82 * S, 80 * S], fill=accent + (a_acc,))
    p1_d.rounded_rectangle([94 * S, 49 * S, 210 * S, 59 * S], radius=4 * S, fill=ensure_contrast(text, bar) + (a_text,))
    p1_d.rounded_rectangle([94 * S, 65 * S, 150 * S, 72 * S], radius=3 * S, fill=mix(accent, bar, 0.3) + (a_acc,))
    _draw_search_icon(p1_d, pw - 68 * S, 51 * S, 12 * S, ensure_contrast(text, bar) + (a_text,))
    _draw_more_dots(p1_d, pw - 30 * S, 51 * S, ensure_contrast(text, bar) + (a_text,))

    # Date Pill
    p1_d.rounded_rectangle([pw // 2 - 40 * S, 98 * S, pw // 2 + 40 * S, 116 * S], radius=9 * S, fill=mix(bg, (0, 0, 0), 0.4) + (160,))
    p1_d.rounded_rectangle([pw // 2 - 25 * S, 104 * S, pw // 2 + 25 * S, 110 * S], radius=3 * S, fill=(255, 255, 255, 200))

    # 1. Incoming Message with Reply Block
    p1_d.rounded_rectangle([16 * S, 130 * S, 250 * S, 185 * S], radius=14 * S, fill=inb + (a_in,))
    p1_d.rounded_rectangle([26 * S, 138 * S, 240 * S, 162 * S], radius=4 * S, fill=mix(reply, inb, 0.85) + (40,))
    p1_d.rectangle([26 * S, 138 * S, 29 * S, 162 * S], fill=reply + (a_reply,))
    p1_d.rounded_rectangle([36 * S, 142 * S, 120 * S, 149 * S], radius=3 * S, fill=reply + (a_reply,))
    p1_d.rounded_rectangle([36 * S, 152 * S, 210 * S, 158 * S], radius=3 * S, fill=mix(text, inb, 0.3) + (a_text,))
    p1_d.rounded_rectangle([26 * S, 168 * S, 190 * S, 176 * S], radius=4 * S, fill=ensure_contrast(text, inb) + (a_text,))
    p1_d.rounded_rectangle([205 * S, 172 * S, 240 * S, 178 * S], radius=3 * S, fill=mix(text, inb, 0.5) + (a_text,))

    # 2. Outgoing Text Message
    p1_d.rounded_rectangle([140 * S, 200 * S, 394 * S, 248 * S], radius=14 * S, fill=outb + (a_out,))
    p1_d.rounded_rectangle([154 * S, 212 * S, 370 * S, 221 * S], radius=4 * S, fill=ensure_contrast(text, outb) + (a_text,))
    p1_d.rounded_rectangle([154 * S, 226 * S, 280 * S, 234 * S], radius=4 * S, fill=ensure_contrast(text, outb) + (a_text,))
    _draw_checkmarks(p1_d, 370 * S, 230 * S, ensure_contrast(text, outb) + (a_text,))

    # 3. Outgoing Voice Note Bubble
    p1_d.rounded_rectangle([110 * S, 265 * S, 394 * S, 335 * S], radius=18 * S, fill=outb + (a_out,))
    # Play Button Circle + Triangle
    p1_d.ellipse([124 * S, 277 * S, 168 * S, 321 * S], fill=accent + (a_acc,))
    p1_d.polygon([(143 * S, 292 * S), (143 * S, 306 * S), (154 * S, 299 * S)], fill=readable_on(accent) + (255,))
    # Audio Waveform Vertical Bars
    wave_x = 180 * S
    waveform = [6, 14, 10, 22, 26, 16, 20, 12, 24, 18, 14, 8, 16, 10, 6]
    for i, h_val in enumerate(waveform):
        bar_col = ensure_contrast(text, outb) if i < 7 else mix(text, outb, 0.5)
        p1_d.rounded_rectangle([wave_x + i * 13 * S, 296 * S - h_val * S // 2,
                                wave_x + i * 13 * S + 5 * S, 296 * S + h_val * S // 2],
                               radius=2 * S, fill=bar_col + (a_text,))
    p1_d.rounded_rectangle([180 * S, 316 * S, 225 * S, 323 * S], radius=3 * S, fill=mix(text, outb, 0.4) + (a_text,))
    _draw_checkmarks(p1_d, 370 * S, 316 * S, ensure_contrast(text, outb) + (a_text,))

    # 4. Incoming Message with Avatar
    p1_d.ellipse([14 * S, 355 * S, 46 * S, 387 * S], fill=mix(accent, (255, 0, 0), 0.3) + (230,))
    p1_d.rounded_rectangle([54 * S, 350 * S, 280 * S, 405 * S], radius=14 * S, fill=inb + (a_in,))
    p1_d.rounded_rectangle([66 * S, 360 * S, 140 * S, 368 * S], radius=3 * S, fill=reply + (a_reply,))
    p1_d.rounded_rectangle([66 * S, 374 * S, 250 * S, 383 * S], radius=4 * S, fill=ensure_contrast(text, inb) + (a_text,))
    p1_d.rounded_rectangle([66 * S, 388 * S, 180 * S, 395 * S], radius=3 * S, fill=mix(text, inb, 0.3) + (a_text,))

    # 5. Outgoing Short Bubble
    p1_d.rounded_rectangle([210 * S, 420 * S, 394 * S, 460 * S], radius=14 * S, fill=outb + (a_out,))
    p1_d.rounded_rectangle([224 * S, 434 * S, 340 * S, 444 * S], radius=4 * S, fill=ensure_contrast(text, outb) + (a_text,))
    _draw_checkmarks(p1_d, 370 * S, 440 * S, ensure_contrast(text, outb) + (a_text,))

    # Bottom Input Bar Container
    p1_d.rectangle([0, ph - 60 * S, pw, ph], fill=bg + (255,))
    p1_d.rounded_rectangle([12 * S, ph - 52 * S, pw - 62 * S, ph - 10 * S], radius=21 * S, fill=mix(bg, text, 0.12) + (255,))
    # Emoji Icon Placeholder
    p1_d.ellipse([24 * S, ph - 42 * S, 44 * S, ph - 22 * S], outline=mix(text, bg, 0.4) + (a_text,), width=2)
    p1_d.rounded_rectangle([56 * S, ph - 36 * S, 180 * S, ph - 26 * S], radius=4 * S, fill=mix(text, bg, 0.4) + (a_text,))
    # Clip Icon
    _draw_clip_icon(p1_d, pw - 90 * S, ph - 42 * S, mix(text, bg, 0.4) + (a_text,))
    # Circular Action Button (Mic/Send)
    p1_d.ellipse([pw - 52 * S, ph - 52 * S, pw - 10 * S, ph - 10 * S], fill=accent + (a_acc,))
    p1_d.ellipse([pw - 36 * S, ph - 38 * S, pw - 26 * S, ph - 20 * S], fill=readable_on(accent) + (255,))

    # ================= RIGHT PHONE: CHAT LIST VIEW =================
    phone2_img = Image.new("RGBA", (pw, ph), (0, 0, 0, 0))
    p2_d = ImageDraw.Draw(phone2_img)

    # Frame Shell
    p2_d.rounded_rectangle([0, 0, pw, ph], radius=p_radius, fill=phone_body_2 + (255,))

    # Status Bar
    _draw_status_bar(p2_d, pw, text, phone_body_2)

    # Header Bar
    p2_d.rectangle([0, 30 * S, pw, 86 * S], fill=bar + (a_bar,))
    _draw_hamburger(p2_d, 18 * S, 51 * S, 18 * S, ensure_contrast(text, bar) + (a_text,))
    p2_d.rounded_rectangle([56 * S, 51 * S, 170 * S, 65 * S], radius=5 * S, fill=ensure_contrast(text, bar) + (a_text,))
    _draw_search_icon(p2_d, pw - 34 * S, 51 * S, 13 * S, ensure_contrast(text, bar) + (a_text,))

    # Chat List Rows
    row_y_start = 90 * S
    row_height = 76 * S

    for i in range(8):
        ry = row_y_start + i * row_height

        # Avatar Circle
        av_col = mix(accent, (255, 255, 255) if dark else (0, 0, 0), (i % 4) * 0.18)
        p2_d.ellipse([16 * S, ry + 10 * S, 68 * S, ry + 62 * S], fill=av_col + (240,))

        # Name & Message Skeletons
        name_w = 70 * S + ((i * 43) % 90) * S
        msg_w = 110 * S + ((i * 61) % 130) * S

        p2_d.rounded_rectangle([82 * S, ry + 18 * S, 82 * S + name_w, ry + 29 * S], radius=5 * S, fill=text + (a_text,))
        p2_d.rounded_rectangle([82 * S, ry + 39 * S, 82 * S + msg_w, ry + 49 * S], radius=4 * S, fill=mix(text, bg, 0.45) + (a_text,))

        # Time Tag & Status Badges
        p2_d.rounded_rectangle([pw - 58 * S, ry + 18 * S, pw - 18 * S, ry + 26 * S], radius=3 * S, fill=mix(text, bg, 0.5) + (a_text,))

        if i % 3 == 1:
            # Unread Badge
            p2_d.ellipse([pw - 40 * S, ry + 36 * S, pw - 18 * S, ry + 58 * S], fill=accent + (a_acc,))
        elif i % 3 == 2:
            # Read Checkmarks
            _draw_checkmarks(p2_d, pw - 38 * S, ry + 38 * S, mix(accent, text, 0.4) + (a_acc,))

        # Row Divider Line
        p2_d.line([(82 * S, ry + row_height), (pw, ry + row_height)], fill=mix(text, bg, 0.15) + (40,))

    # Floating Action Button (FAB)
    fab_col = mix(accent, (0, 0, 0), 0.15 if dark else 0.05)
    p2_d.ellipse([pw - 76 * S, ph - 90 * S, pw - 16 * S, ph - 30 * S], fill=fab_col + (245,))
    _draw_pencil_icon(p2_d, pw - 46 * S, ph - 60 * S, 14 * S, readable_on(fab_col) + (255,))

    # ================= COMPOSITING & SHADOWS =================
    shadow_img = Image.new("RGBA", (pw + 30 * S, ph + 30 * S), (0, 0, 0, 0))
    sh_d = ImageDraw.Draw(shadow_img)
    sh_d.rounded_rectangle([15 * S, 15 * S, pw + 15 * S, ph + 15 * S], radius=p_radius, fill=(0, 0, 0, 95 if dark else 50))
    shadow_blur = shadow_img.filter(ImageFilter.GaussianBlur(16 * S))

    # Paste shadows & phone surfaces onto backdrop canvas
    canvas.paste(shadow_blur, (px1 - 15 * S, py - 10 * S), shadow_blur)
    canvas.paste(phone1_img, (px1, py), phone1_img)

    canvas.paste(shadow_blur, (px2 - 15 * S, py - 10 * S), shadow_blur)
    canvas.paste(phone2_img, (px2, py), phone2_img)

    # Downscale supersampled canvas to target resolution
    final_img = canvas.resize((W // S, H // S), Image.Resampling.LANCZOS).convert("RGB")
    buf = io.BytesIO()
    buf.name = "preview.png"
    final_img.save(buf, "PNG")
    buf.seek(0)
    return buf
