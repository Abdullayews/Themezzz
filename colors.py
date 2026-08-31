import io
import math
import colorsys
from PIL import Image


# ---------- Color Utility & Math Helpers ----------

def rgb_to_hex(rgb):
    """Convert (R, G, B) tuple to #HEX string."""
    return f"#{rgb[0]:02x}{rgb[1]:02x}{rgb[2]:02x}"


def hex_to_rgb(hex_str):
    """Convert #HEX or HEX string to (R, G, B) tuple safely."""
    hex_clean = hex_str.lstrip("#")
    if len(hex_clean) == 3:
        hex_clean = "".join(c * 2 for c in hex_clean)
    if len(hex_clean) != 6:
        raise ValueError(f"Invalid hex color string: {hex_str}")
    return tuple(int(hex_clean[i:i + 2], 16) for i in (0, 2, 4))


def luminance(rgb):
    """
    Calculate relative WCAG 2.x sRGB luminance.
    Returns float in range [0.0, 1.0].
    """
    r, g, b = [c / 255.0 for c in rgb[:3]]
    r = r / 12.92 if r <= 0.04045 else ((r + 0.055) / 1.055) ** 2.4
    g = g / 12.92 if g <= 0.04045 else ((g + 0.055) / 1.055) ** 2.4
    b = b / 12.92 if b <= 0.04045 else ((b + 0.055) / 1.055) ** 2.4
    return 0.2126 * r + 0.7152 * g + 0.0722 * b


def mix(c1, c2, weight=0.5):
    """
    Linear RGB interpolation between two colors c1 and c2.
    weight=1.0 returns c1, weight=0.0 returns c2.
    """
    w = max(0.0, min(1.0, float(weight)))
    return tuple(
        max(0, min(255, round(c1[i] * w + c2[i] * (1.0 - w))))
        for i in range(3)
    )


def contrast_ratio(c1, c2):
    """Calculate WCAG contrast ratio between two colors [1.0 to 21.0]."""
    l1 = luminance(c1)
    l2 = luminance(c2)
    lighter = max(l1, l2)
    darker = min(l1, l2)
    return (lighter + 0.05) / (darker + 0.05)


def readable_on(bg):
    """Returns pure white (255, 255, 255) or black (0, 0, 0) for high readability."""
    return (255, 255, 255) if luminance(bg) < 0.45 else (0, 0, 0)


def ensure_contrast(fg, bg, min_ratio=4.5):
    """
    Adjusts fg toward white or black if contrast ratio against bg is below min_ratio.
    Guarantees accessible UI elements without abrupt color jumps.
    """
    if contrast_ratio(fg, bg) >= min_ratio:
        return fg

    target = readable_on(bg)
    step_fg = fg
    for step in range(1, 11):
        step_fg = mix(target, fg, step / 10.0)
        if contrast_ratio(step_fg, bg) >= min_ratio:
            return step_fg

    return target


# ---------- Image Palette Extraction ----------

def extract_palette(image_bytes, count=6):
    """
    Extracts distinct, visually vibrant, and balanced palette colors from input image.
    Uses Pillow quantization with luminance & saturation filtering to avoid duplicate shades.
    """
    img = Image.open(io.BytesIO(image_bytes))

    # Handle transparency by blending over neutral gray canvas
    if img.mode in ("RGBA", "LA") or (img.mode == "P" and "transparency" in img.info):
        bg_canvas = Image.new("RGB", img.size, (128, 128, 128))
        if img.mode != "RGBA":
            img = img.convert("RGBA")
        bg_canvas.paste(img, mask=img.getchannel("A"))
        img = bg_canvas
    else:
        img = img.convert("RGB")

    # Downscale image for fast processing
    img.thumbnail((256, 256), Image.Resampling.LANCZOS)

    # Quantize down to 24 colors to filter noise
    quantized = img.quantize(colors=24, method=Image.Quantizing.MEDIANCUT)
    palette_raw = quantized.getpalette()[:72]  # 24 * 3 RGB values
    color_counts = quantized.getcolors()

    if not color_counts:
        # Fallback default vibrant palette
        return [(41, 121, 255), (0, 230, 118), (255, 171, 0), (245, 0, 87), (101, 31, 255), (0, 184, 212)]

    # Sort colors by occurrence frequency
    color_counts.sort(key=lambda x: x[0], reverse=True)

    candidates = []
    for count_val, idx in color_counts:
        rgb = tuple(palette_raw[idx * 3:(idx + 1) * 3])
        # Skip near pure black or near pure white noise
        lum = luminance(rgb)
        if lum < 0.03 or lum > 0.97:
            continue
        candidates.append(rgb)

    if not candidates:
        candidates = [tuple(palette_raw[i * 3:(i + 1) * 3]) for i in range(min(count, 8))]

    # Filter out visually redundant colors using perceptual HSL distance
    selected = [candidates[0]]
    for cand in candidates[1:]:
        if len(selected) >= count:
            break

        # Check Euclidean distance in RGB space to avoid duplicates
        is_distinct = True
        for sel in selected:
            dist = math.sqrt(sum((cand[i] - sel[i]) ** 2 for i in range(3)))
            if dist < 45.0:  # Distance threshold
                is_distinct = False
                break

        if is_distinct:
            selected.append(cand)

    # If count not met, backfill with shifted hue variants
    while len(selected) < count:
        last = selected[-1]
        h, s, v = colorsys.rgb_to_hsv(last[0] / 255.0, last[1] / 255.0, last[2] / 255.0)
        h = (h + 0.15) % 1.0
        r, g, b = colorsys.hsv_to_rgb(h, max(0.4, s), max(0.5, v))
        selected.append((int(r * 255), int(g * 255), int(b * 255)))

    return selected[:count]


# ---------- Telegram .attheme Serialization ----------

def _rgb_to_attheme_int(rgb, alpha=255):
    """
    Converts RGB/RGBA color to 32-bit signed ARGB integer used natively by Telegram (.attheme).
    Format: ARGB -> (A << 24) | (R << 16) | (G << 8) | B
    """
    r, g, b = rgb[:3]
    a = max(0, min(255, int(alpha)))
    val = (a << 24) | ((r & 0xFF) << 16) | ((g & 0xFF) << 8) | (b & 0xFF)
    # Convert unsigned 32-bit to signed 32-bit int
    if val >= 0x80000000:
        val -= 0x100000000
    return val


def generate_attheme(colors, alphas):
    """
    Generates full binary/text string content for standard Telegram .attheme files.
    Maps UI component state into comprehensive Telegram color key definitions.
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
    sec_text = mix(text, bg, 0.4)

    # Telegram key mapping table
    theme_keys = {
        # Window & Surfaces
        "windowBackgroundWhite": _rgb_to_attheme_int(bg),
        "windowBackgroundGray": _rgb_to_attheme_int(mix(bg, (0, 0, 0) if dark else (255, 255, 255), 0.05)),
        "dialogBackground": _rgb_to_attheme_int(bg),

        # Action Bar (Header)
        "actionBarDefault": _rgb_to_attheme_int(bar, a_bar),
        "actionBarDefaultTitle": _rgb_to_attheme_int(ensure_contrast(text, bar), a_text),
        "actionBarDefaultIcon": _rgb_to_attheme_int(ensure_contrast(text, bar), a_text),
        "actionBarDefaultSubtitle": _rgb_to_attheme_int(mix(ensure_contrast(text, bar), bar, 0.3), a_text),

        # Text & Labels
        "windowBackgroundWhiteBlackText": _rgb_to_attheme_int(text, a_text),
        "windowBackgroundWhiteGrayText": _rgb_to_attheme_int(sec_text, a_text),
        "windowBackgroundWhiteLinkText": _rgb_to_attheme_int(accent, a_acc),

        # Incoming Bubbles
        "chat_inBubble": _rgb_to_attheme_int(inb, a_in),
        "chat_inBubbleSelected": _rgb_to_attheme_int(mix(inb, accent, 0.15), a_in),
        "chat_inText": _rgb_to_attheme_int(ensure_contrast(text, inb), a_text),
        "chat_inTimeText": _rgb_to_attheme_int(mix(text, inb, 0.5), a_text),
        "chat_inReplyLine": _rgb_to_attheme_int(reply, a_reply),
        "chat_inReplyHeader": _rgb_to_attheme_int(reply, a_reply),
        "chat_inReplyNameText": _rgb_to_attheme_int(reply, a_reply),
        "chat_inReplyMessageText": _rgb_to_attheme_int(mix(text, inb, 0.3), a_text),

        # Outgoing Bubbles
        "chat_outBubble": _rgb_to_attheme_int(outb, a_out),
        "chat_outBubbleSelected": _rgb_to_attheme_int(mix(outb, accent, 0.15), a_out),
        "chat_outText": _rgb_to_attheme_int(ensure_contrast(text, outb), a_text),
        "chat_outTimeText": _rgb_to_attheme_int(mix(text, outb, 0.4), a_text),
        "chat_outReplyLine": _rgb_to_attheme_int(ensure_contrast(text, outb), a_text),
        "chat_outReplyHeader": _rgb_to_attheme_int(ensure_contrast(text, outb), a_text),
        "chat_outReplyNameText": _rgb_to_attheme_int(ensure_contrast(text, outb), a_text),
        "chat_outReplyMessageText": _rgb_to_attheme_int(mix(text, outb, 0.3), a_text),

        # Accents & Controls
        "chats_unreadCounter": _rgb_to_attheme_int(accent, a_acc),
        "chats_unreadCounterText": _rgb_to_attheme_int(readable_on(accent)),
        "chats_actionBackground": _rgb_to_attheme_int(accent, a_acc),
        "chats_actionIcon": _rgb_to_attheme_int(readable_on(accent)),
        "chats_name": _rgb_to_attheme_int(text, a_text),
        "chats_message": _rgb_to_attheme_int(sec_text, a_text),
        "chats_date": _rgb_to_attheme_int(sec_text, a_text),

        # Input Field & Attachments
        "chat_messagePanelBackground": _rgb_to_attheme_int(bg),
        "chat_messagePanelText": _rgb_to_attheme_int(text, a_text),
        "chat_messagePanelHint": _rgb_to_attheme_int(sec_text, a_text),
        "chat_messagePanelIcons": _rgb_to_attheme_int(mix(text, bg, 0.4), a_text),
        "chat_messagePanelSend": _rgb_to_attheme_int(accent, a_acc),
    }

    # Format into standard key=value lines for .attheme output
    lines = [f"{key}={val}" for key, val in theme_keys.items()]
    return "\n".join(lines).encode("utf-8")
