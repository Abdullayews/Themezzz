import io
from PIL import Image, ImageDraw, ImageFilter

from colors import readable_on, mix, ensure_contrast, luminance


# ---------- helpers ----------

def _cover(img, w, h):
    """Center-crop image to fully cover w×h maintaining aspect ratio."""
    iw, ih = img.size
    scale = max(w / iw, h / ih)
    img = img.resize((int(iw * scale) + 1, int(ih * scale) + 1),
                     Image.Resampling.LANCZOS)
    x, y = (img.width - w) // 2, (img.height - h) // 2
    return img.crop((x, y, x + w, y + h))


# ---------- main ----------

def render_preview(colors, alphas, wall_bytes, wall_flat):
    """
    Renders a dual-phone minimalist wireframe preview matching the red mockup aesthetic:
    - Left Phone: Chat View (chat bubbles, voice message waveform, status elements, reply block).
    - Right Phone: Dialogs/Chat List View (avatar circles, skeleton text bars, badges, FAB button).
    """
    bg, bar = colors["bg"], colors["bar"]
    inb, outb = colors["in"], colors["out"]
    text, accent = colors["text"], colors["accent"]
    reply = colors["reply"]

    # Transparency helper: converts percentage to 0-255 alpha value
    get_alpha = lambda k: max(0, min(255, round(255 * (1 - alphas.get(k, 0) / 100.0))))
    a_bar = get_alpha("bar")
    a_in, a_out = get_alpha("in"), get_alpha("out")
    a_text = get_alpha("text")
    a_acc = get_alpha("accent")
    a_reply = get_alpha("reply")

    dark = luminance(bg) < 0.5

    # Resolution & Supersampling (2x)
    S = 2
    W, H = 1000 * S, 1000 * S

    # Dynamic canvas background tint derived from theme colors
    outer_bg = mix(accent, (0, 0, 0) if dark else (255, 255, 255), 0.55 if dark else 0.35)
    canvas = Image.new("RGBA", (W, H), outer_bg + (255,))

    # Phone dimensions
    pw, ph = 410 * S, 830 * S
    py = (H - ph) // 2
    px1, px2 = 60 * S, 530 * S
    p_radius = 46 * S

    phone_body_1 = mix(bg, (0, 0, 0), 0.28 if dark else 0.06)
    phone_body_2 = mix(bg, (0, 0, 0), 0.22 if dark else 0.09)

    # ================= LEFT PHONE: CHAT VIEW =================
    phone1_img = Image.new("RGBA", (pw, ph), (0, 0, 0, 0))
    p1_d = ImageDraw.Draw(phone1_img)

    # Body frame
    p1_d.rounded_rectangle([0, 0, pw, ph], radius=p_radius, fill=phone_body_1 + (255,))

    # Chat area background
    chat_h = ph - 110 * S
    chat_y = 55 * S

    if wall_bytes:
        w_img = _cover(Image.open(io.BytesIO(wall_bytes)), pw, chat_h)
        tint = Image.new("RGBA", (pw, chat_h), (0, 0, 0, 110) if dark else (255, 255, 255, 110))
        w_img = Image.alpha_composite(w_img.convert("RGBA"), tint)
        phone1_img.paste(w_img, (0, chat_y))
    else:
        wf = wall_flat if wall_flat else mix(bg, (0, 0, 0), 0.18 if dark else 0.04)
        glow_img = Image.new("RGBA", (pw, chat_h), wf + (255,))
        glow_d = ImageDraw.Draw(glow_img)
        glow_col = mix(accent, (255, 255, 255), 0.25)
        glow_d.ellipse([pw // 2 - 150 * S, chat_h // 2 - 150 * S,
                        pw // 2 + 150 * S, chat_h // 2 + 150 * S], fill=glow_col + (45,))
        phone1_img.paste(glow_img, (0, chat_y))

    p1_d = ImageDraw.Draw(phone1_img)

    # Status Bar / Camera cutouts
    p1_d.ellipse([25 * S, 20 * S, 41 * S, 36 * S], fill=mix(text, phone_body_1, 0.4) + (200,))
    p1_d.rounded_rectangle([52 * S, 23 * S, 125 * S, 33 * S], radius=5 * S, fill=mix(text, phone_body_1, 0.4) + (200,))
    p1_d.ellipse([pw - 41 * S, 20 * S, pw - 25 * S, 36 * S], fill=mix(text, phone_body_1, 0.4) + (200,))

    # Top Action Bar elements
    p1_d.rounded_rectangle([25 * S, 72 * S, 35 * S, 102 * S], radius=2 * S, fill=mix(bar, text, 0.3) + (a_bar,))
    p1_d.rounded_rectangle([48 * S, 82 * S, 140 * S, 92 * S], radius=5 * S, fill=mix(bar, text, 0.3) + (a_bar,))

    # Incoming Bubble 1 with Reply block
    p1_d.rounded_rectangle([25 * S, 125 * S, 240 * S, 172 * S], radius=16 * S, fill=inb + (a_in,))
    p1_d.rectangle([35 * S, 135 * S, 38 * S, 162 * S], fill=reply + (a_reply,))
    p1_d.rounded_rectangle([46 * S, 138 * S, 145 * S, 146 * S], radius=4 * S, fill=ensure_contrast(text, inb) + (a_text,))
    p1_d.rounded_rectangle([46 * S, 151 * S, 200 * S, 158 * S], radius=3 * S, fill=mix(text, inb, 0.35) + (a_text,))

    # Outgoing Bubble 1
    p1_d.rounded_rectangle([145 * S, 215 * S, 385 * S, 258 * S], radius=16 * S, fill=outb + (a_out,))
    p1_d.rounded_rectangle([160 * S, 231 * S, 365 * S, 242 * S], radius=5 * S, fill=ensure_contrast(text, outb) + (a_text,))

    # Outgoing Voice Message Bubble with Waveform
    p1_d.rounded_rectangle([105 * S, 275 * S, 385 * S, 350 * S], radius=20 * S, fill=outb + (a_out,))
    p1_d.ellipse([120 * S, 290 * S, 165 * S, 335 * S], fill=mix(outb, (0, 0, 0), 0.22) + (a_out,))
    wave_x = 180 * S
    waveform = [8, 16, 10, 24, 28, 14, 20, 16, 26, 12, 18, 10, 6]
    for i, h_val in enumerate(waveform):
        p1_d.rounded_rectangle([wave_x + i * 14 * S, 312 * S - h_val * S // 2,
                                wave_x + i * 14 * S + 6 * S, 312 * S + h_val * S // 2],
                               radius=3 * S, fill=ensure_contrast(text, outb) + (a_text,))
    p1_d.rounded_rectangle([180 * S, 328 * S, 235 * S, 336 * S], radius=4 * S, fill=mix(text, outb, 0.35) + (a_text,))

    # Incoming Bubble 2
    p1_d.rounded_rectangle([25 * S, 365 * S, 275 * S, 420 * S], radius=18 * S, fill=inb + (a_in,))
    p1_d.ellipse([40 * S, 377 * S, 75 * S, 412 * S], fill=accent + (a_acc,))
    p1_d.rounded_rectangle([85 * S, 382 * S, 245 * S, 392 * S], radius=4 * S, fill=ensure_contrast(text, inb) + (a_text,))
    p1_d.rounded_rectangle([85 * S, 398 * S, 195 * S, 406 * S], radius=4 * S, fill=mix(text, inb, 0.35) + (a_text,))

    # Additional background chat bubbles
    p1_d.rounded_rectangle([170 * S, 435 * S, 385 * S, 475 * S], radius=16 * S, fill=outb + (a_out,))
    p1_d.rounded_rectangle([120 * S, 490 * S, 385 * S, 530 * S], radius=16 * S, fill=outb + (a_out,))

    # Bottom Input Bar
    p1_d.rounded_rectangle([25 * S, ph - 60 * S, 120 * S, ph - 30 * S], radius=15 * S, fill=mix(bg, text, 0.18) + (255,))
    p1_d.ellipse([pw - 60 * S, ph - 65 * S, pw - 20 * S, ph - 25 * S], fill=accent + (a_acc,))
    p1_d.ellipse([pw - 110 * S, ph - 65 * S, pw - 70 * S, ph - 25 * S], fill=mix(bg, text, 0.18) + (255,))

    # ================= RIGHT PHONE: CHAT LIST VIEW =================
    phone2_img = Image.new("RGBA", (pw, ph), (0, 0, 0, 0))
    p2_d = ImageDraw.Draw(phone2_img)

    # Body frame
    p2_d.rounded_rectangle([0, 0, pw, ph], radius=p_radius, fill=phone_body_2 + (255,))

    # Status Bar
    p2_d.ellipse([25 * S, 20 * S, 41 * S, 36 * S], fill=mix(text, phone_body_2, 0.4) + (200,))
    p2_d.rounded_rectangle([52 * S, 23 * S, 125 * S, 33 * S], radius=5 * S, fill=mix(text, phone_body_2, 0.4) + (200,))
    p2_d.ellipse([pw - 58 * S, 20 * S, pw - 42 * S, 36 * S], fill=mix(text, phone_body_2, 0.4) + (200,))
    p2_d.ellipse([pw - 36 * S, 20 * S, pw - 20 * S, 36 * S], fill=mix(text, phone_body_2, 0.4) + (200,))

    # Header Bar
    p2_d.ellipse([25 * S, 58 * S, 50 * S, 83 * S], fill=mix(bar, text, 0.3) + (a_bar,))
    p2_d.rounded_rectangle([65 * S, 66 * S, 175 * S, 76 * S], radius=5 * S, fill=mix(bar, text, 0.3) + (a_bar,))
    p2_d.ellipse([pw - 40 * S, 58 * S, pw - 28 * S, 70 * S], fill=mix(bar, text, 0.3) + (a_bar,))
    p2_d.ellipse([pw - 40 * S, 76 * S, pw - 28 * S, 88 * S], fill=mix(bar, text, 0.3) + (a_bar,))
    p2_d.ellipse([pw - 60 * S, 58 * S, pw - 48 * S, 70 * S], fill=mix(bar, text, 0.3) + (a_bar,))

    # Chat List Rows
    row_y_start = 105 * S
    row_height = 80 * S

    for i in range(8):
        ry = row_y_start + i * row_height

        # Avatar placeholder
        av_col = mix(accent, text, (i % 3) * 0.15)
        p2_d.ellipse([25 * S, ry, 75 * S, ry + 50 * S], fill=av_col + (230,))

        # Name and message skeleton pills
        name_w = 70 * S + ((i * 37) % 85) * S
        msg_w = 115 * S + ((i * 53) % 115) * S

        p2_d.rounded_rectangle([90 * S, ry + 10 * S, 90 * S + name_w, ry + 20 * S], radius=5 * S, fill=text + (a_text,))
        p2_d.rounded_rectangle([90 * S, ry + 28 * S, 90 * S + msg_w, ry + 37 * S], radius=4 * S, fill=mix(text, bg, 0.45) + (a_text,))

        # Unread status indicators
        if i % 3 == 1:
            p2_d.ellipse([pw - 38 * S, ry + 18 * S, pw - 26 * S, ry + 30 * S], fill=accent + (a_acc,))
        else:
            p2_d.rounded_rectangle([pw - 55 * S, ry + 14 * S, pw - 25 * S, ry + 22 * S], radius=4 * S, fill=mix(text, bg, 0.5) + (a_text,))

    # Floating Action Button (FAB)
    fab_col = mix(accent, (0, 0, 0), 0.22 if dark else 0.12)
    p2_d.ellipse([pw - 82 * S, ph - 92 * S, pw - 22 * S, ph - 32 * S], fill=fab_col + (240,))
    p2_d.rounded_rectangle([pw - 60 * S, ph - 66 * S, pw - 44 * S, ph - 58 * S], radius=4 * S, fill=readable_on(fab_col) + (255,))

    # ================= COMPOSITING & SHADOWS =================
    shadow_img = Image.new("RGBA", (pw + 30 * S, ph + 30 * S), (0, 0, 0, 0))
    sh_d = ImageDraw.Draw(shadow_img)
    sh_d.rounded_rectangle([15 * S, 15 * S, pw + 15 * S, ph + 15 * S], radius=p_radius, fill=(0, 0, 0, 90 if dark else 45))
    shadow_blur = shadow_img.filter(ImageFilter.GaussianBlur(18 * S))

    # Apply soft drop-shadows and paste phone bodies onto outer canvas
    canvas.paste(shadow_blur, (px1 - 15 * S, py - 10 * S), shadow_blur)
    canvas.paste(phone1_img, (px1, py), phone1_img)

    canvas.paste(shadow_blur, (px2 - 15 * S, py - 10 * S), shadow_blur)
    canvas.paste(phone2_img, (px2, py), phone2_img)

    # Downscale supersampled image for anti-aliased output
    final_img = canvas.resize((W // S, H // S), Image.Resampling.LANCZOS).convert("RGB")
    buf = io.BytesIO()
    buf.name = "preview.png"
    final_img.save(buf, "PNG")
    buf.seek(0)
    return buf
