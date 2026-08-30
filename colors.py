import colorsys
import io

from PIL import Image


# ---------- Helpers ----------

def hex_to_rgb(h: str):
    h = h.lstrip("#")
    return tuple(int(h[i:i + 2], 16) for i in (0, 2, 4))


def signed_argb(r: int, g: int, b: int, a: int = 255) -> int:
    """Telegram .attheme colors are signed 32-bit ARGB decimals."""
    v = (a << 24) | (r << 16) | (g << 8) | b
    return v - 0x100000000 if v > 0x7FFFFFFF else v


# ---------- Material 3 palette ----------

class M3Palette:
    """Tonal palette built from a seed color (chroma factor controls saturation)."""

    def __init__(self, seed_hex: str, chroma: float = 1.0):
        r, g, b = hex_to_rgb(seed_hex)
        h, l, s = colorsys.rgb_to_hls(r / 255, g / 255, b / 255)
        self.h = h
        self.s = min(1.0, max(0.22, s * 1.15))
        self.k = chroma

    def _tone(self, hue: float, sat: float, tone: int):
        tone = max(0, min(100, tone))
        s = min(1.0, sat * self.k)
        # Reduce chroma near extreme tones (mimics M3 behavior)
        if tone <= 12:
            s *= 0.55 + (tone / 12) * 0.45
        if tone >= 88:
            s *= 0.55 + ((100 - tone) / 12) * 0.45
        r, g, b = colorsys.hls_to_rgb(hue, tone / 100, s)
        return int(r * 255), int(g * 255), int(b * 255)

    def primary(self, t):         return self._tone(self.h, self.s * 0.85, t)
    def secondary(self, t):       return self._tone(self.h, self.s * 0.30, t)
    def tertiary(self, t):        return self._tone((self.h + 0.16) % 1, self.s * 0.55, t)
    def neutral(self, t):         return self._tone(self.h, self.s * 0.07, t)
    def neutral_variant(self, t): return self._tone(self.h, self.s * 0.18, t)


# ---------- Color extraction from an image ----------

def extract_palette(data: bytes, count: int = 5) -> list:
    img = Image.open(io.BytesIO(data)).convert("RGB")
    img.thumbnail((160, 160))
    q = img.quantize(colors=9)
    counts = q.convert("RGB").getcolors(160 * 160) or []

    cands = []
    for cnt, (r, g, b) in counts:
        h, l, s = colorsys.rgb_to_hls(r / 255, g / 255, b / 255)
        cands.append((cnt, h, l, s, (r, g, b)))

    def pick(avoid_extremes=True):
        out = []
        for cnt, h, l, s, rgb in sorted(cands, key=lambda x: -x[0]):
            if avoid_extremes and (l < 0.10 or l > 0.90):
                continue  # skip near-black / near-white
            if any(abs(h - h2) < 0.07 and abs(l - l2) < 0.18 for _, h2, l2, _, _ in out):
                continue  # skip similar colors
            out.append((cnt, h, l, s, rgb))
            if len(out) >= count:
                break
        return out

    res = pick() or pick(avoid_extremes=False)
    # Sort by frequency + saturation → first color is the best seed
    res.sort(key=lambda x: -(x[0] * (0.3 + x[3])))
    return ["#%02x%02x%02x" % rgb for *_, rgb in res]


# ---------- Wallpaper preparation ----------

def prepare_wallpaper(data: bytes, max_side: int = 1440, quality: int = 82) -> bytes:
    """Downscale the image to be embedded into the .attheme (keeps file size sane)."""
    img = Image.open(io.BytesIO(data)).convert("RGB")
    img.thumbnail((max_side, max_side), Image.LANCZOS)
    buf = io.BytesIO()
    img.save(buf, "JPEG", quality=quality)
    return buf.getvalue()


# ---------- .attheme generator ----------

ALPHA_KEYS = {"chat_inBubble", "chat_outBubble"}
OUT_TONES = {"primary": (34, 40), "secondary": (36, 42), "tertiary": (34, 40)}


def build_attheme(seed_hex: str, style: str, alpha_pct: int,
                  role: str = "primary", chroma: float = 1.0,
                  contrast: int = 0, wallpaper: bytes | None = None) -> bytes:
    p = M3Palette(seed_hex, chroma)
    dark = style == "dark"

    # Contrast: shift surface tones (dark → deeper, light → brighter)
    sh = (-4 * contrast) if dark else (4 * contrast)
    N = lambda t: p.neutral(t + (sh if t <= 60 else 0))
    NV = lambda t: p.neutral_variant(t + (sh if t <= 60 else 0))
    P = lambda t: p.primary(t)
    out_fn = getattr(p, role)
    out_t = OUT_TONES[role][0 if dark else 1]

    if dark:
        out_text = N(98)
        out_time = N(85)
        out_reply, out_line = out_fn(80), out_fn(80)
        link_in, link_out = P(70), P(80)
        sent, read = out_fn(65), out_fn(75)
    else:
        out_text = N(100)
        out_time = N(60)
        out_reply, out_line = out_fn(20), out_fn(85)
        link_in, link_out = P(40), P(30)
        sent, read = out_fn(55), out_fn(45)

    mapping = {
        "windowBackgroundWhite": N(10 if dark else 98),
        "windowBackgroundGray": N(6 if dark else 94),
        "windowBackgroundBlack": N(4 if dark else 100),
        "actionBarDefault": N(16 if dark else 97),
        "actionBarDefaultTitle": N(95 if dark else 10),
        "actionBarDefaultSubtitle": N(70 if dark else 45),
        "actionBarDefaultIcon": N(85 if dark else 30),
        "actionBarDefaultSelector": N(28 if dark else 88),
        "actionBarDefaultSearch": N(95 if dark else 10),
        "actionBarDefaultSearchPlaceholder": N(55 if dark else 50),
        "actionBarTabLine": P(70 if dark else 40),
        "actionBarTabActiveText": P(80 if dark else 40),
        "actionBarTabText": N(65 if dark else 50),
        "divider": N(18 if dark else 88),
        "chats_name": N(95 if dark else 10),
        "chats_message": N(70 if dark else 45),
        "chats_date": N(50 if dark else 55),
        "chats_muteIcon": N(50 if dark else 55),
        "chats_unreadCounter": P(65 if dark else 40),
        "chats_unreadCounterText": N(10 if dark else 100),
        "chats_unreadCounterMuted": N(40 if dark else 70),
        "chats_sentCheck": P(60 if dark else 50),
        "chats_sentReadCheck": P(70 if dark else 40),
        "chat_messagePanelBackground": N(13 if dark else 96),
        "chat_messagePanelText": N(95 if dark else 10),
        "chat_messagePanelHint": N(50 if dark else 50),
        "chat_messagePanelIcons": N(60 if dark else 40),
        "chat_messagePanelSend": P(70 if dark else 40),
        "chat_messagePanelVoicePressed": P(45 if dark else 75),
        "chat_inBubble": NV(22 if dark else 92),
        "chat_outBubble": out_fn(out_t),
        "chat_messageTextIn": N(95 if dark else 12),
        "chat_messageTextOut": out_text,
        "chat_messageLinkIn": link_in,
        "chat_messageLinkOut": link_out,
        "chat_inTimeText": N(62 if dark else 50),
        "chat_outTimeText": out_time,
        "chat_inBubbleShadow": N(0 if dark else 100),
        "chat_outBubbleShadow": N(0 if dark else 100),
        "chat_outSentCheck": sent,
        "chat_outSentReadCheck": read,
        "chat_serviceText": N(95 if dark else 100),
        "chat_serviceBackground": N(45),
        "chat_inReplyNameText": P(70 if dark else 35),
        "chat_outReplyNameText": out_reply,
        "chat_inReplyLine": P(70 if dark else 40),
        "chat_outReplyLine": out_line,
        "chat_status": P(70 if dark else 40),
        "switchTrack": NV(30 if dark else 70),
        "switchTrackChecked": P(50 if dark else 45),
        "radioBackgroundChecked": P(70 if dark else 40),
        "checkboxCheck": P(70 if dark else 40),
    }

    alpha = round(255 * (1 - alpha_pct / 100))
    lines = []
    for key, (r, g, b) in mapping.items():
        if key in ALPHA_KEYS:
            a = alpha                      # 🫧 transparency slider applies here
        elif key == "chat_serviceBackground":
            a = 102                        # "Today" chip stays fixed at 40%
        else:
            a = 255
        lines.append(f"{key}={signed_argb(r, g, b, a)}")

    # Background: flat color OR the user's image (hex-embedded into .attheme)
    if wallpaper:
        lines.append("chat_wallpaper=-1")
        hx = wallpaper.hex()
        for i in range(0, len(hx), 1024):
            lines.append(hx[i:i + 1024])
    else:
        r, g, b = N(7 if dark else 92)
        lines.append(f"chat_wallpaper={signed_argb(r, g, b, 255)}")

    return ("\n".join(lines) + "\n").encode("utf-8")
