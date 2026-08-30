import colorsys
import io

from PIL import Image

FALLBACK_PALETTE = ["#5b6bbf", "#8e5bbf", "#3f7fd6", "#b05b9e", "#4c8f6a"]


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
    """Tonal palette built from a seed color."""

    def __init__(self, seed_hex: str):
        r, g, b = hex_to_rgb(seed_hex)
        h, l, s = colorsys.rgb_to_hls(r / 255, g / 255, b / 255)
        self.h = h
        self.s = min(1.0, max(0.22, s * 1.15))

    def _tone(self, hue: float, sat: float, tone: int):
        tone = max(0, min(100, tone))
        s = min(1.0, sat)
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


# ---------- Color extraction (never raises) ----------

def extract_palette(data: bytes, count: int = 5) -> list:
    try:
        img = Image.open(io.BytesIO(data)).convert("RGB")
        img.thumbnail((160, 160))
        q = img.quantize(colors=9)
        counts = q.convert("RGB").getcolors(160 * 160) or []

        cands = []
        for cnt, (r, g, b) in counts:
            h, l, s = colorsys.rgb_to_hls(r / 255, g / 255, b / 255)
            cands.append((cnt, h, l, s, (r, g, b)))

        def pick(strict):
            out = []
            for cnt, h, l, s, rgb in sorted(cands, key=lambda x: -x[0]):
                if strict and (l < 0.10 or l > 0.90):
                    continue
                if any(abs(h - h2) < 0.07 and abs(l - l2) < 0.18
                       for _, h2, l2, _, _ in out):
                    continue
                out.append((cnt, h, l, s, rgb))
                if len(out) >= count:
                    break
            return out

        res = pick(True) or pick(False)
        if res:
            res.sort(key=lambda x: -(x[0] * (0.3 + x[3])))
            return ["#%02x%02x%02x" % rgb for *_, rgb in res]
    except Exception:
        pass
    return list(FALLBACK_PALETTE)


# ---------- Wallpaper ----------

def prepare_wallpaper(data: bytes, max_side: int = 1080, quality: int = 75) -> bytes:
    """Downscale the image to be embedded into the .attheme."""
    img = Image.open(io.BytesIO(data)).convert("RGB")
    img.thumbnail((max_side, max_side), Image.Resampling.LANCZOS)
    buf = io.BytesIO()
    img.save(buf, "JPEG", quality=quality)
    return buf.getvalue()


# ---------- .attheme generator ----------

# Keys that follow the user's transparency slider
ALPHA_KEYS = {
    "chat_inBubble", "chat_outBubble",
    "chat_outBubbleGradient", "chat_outBubbleGradient2", "chat_outBubbleGradient3",
}

# Selected bubbles stay more opaque so the selection is clearly visible
SELECTED_KEYS = {"chat_inBubbleSelected", "chat_outBubbleSelected"}

# Overlay keys: alpha < 255 means BLEND amount, not transparency (glossary §4)
FIXED_ALPHA = {
    "chat_serviceBackground": 102,       # "Today" chip → 40%
    "chat_selectedBackground": 100,      # long-press selection overlay
    "listSelectorSDK21": 60,             # ripple effect
    "windowBackgroundWhiteLinkSelection": 60,
    "actionBarDefaultSelector": 70,
    "dialogButtonSelector": 70,
}

ERROR_RGB = {"dark": (242, 184, 181), "light": (179, 38, 30)}   # M3 error tones
GREEN_RGB = {"dark": (110, 216, 140), "light": (46, 125, 50)}

OUT_TONES = {"primary": (34, 40), "secondary": (36, 42), "tertiary": (34, 40)}


def build_attheme(seed_hex: str, style: str, alpha_pct: int,
                  role: str = "primary", wallpaper: bytes | None = None) -> bytes:
    p = M3Palette(seed_hex)
    dark = style == "dark"
    err = ERROR_RGB["dark" if dark else "light"]
    grn = GREEN_RGB["dark" if dark else "light"]

    N   = lambda t: p.neutral(t)
    NV  = lambda t: p.neutral_variant(t)
    P   = lambda t: p.primary(t)
    SEC = p.secondary
    TER = p.tertiary
    out_fn = getattr(p, role)
    out_t = OUT_TONES[role][0 if dark else 1]

    mapping = {
        # ===== A. Backgrounds & Action bar =====
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
        "actionBarDefaultSubmenuBackground": N(22 if dark else 100),
        "actionBarDefaultSubmenuItem": N(95 if dark else 10),
        "actionBarDefaultSubmenuItemIcon": N(85 if dark else 30),
        "actionBarTabLine": P(70 if dark else 40),
        "actionBarTabActiveText": P(80 if dark else 40),
        "actionBarTabText": N(65 if dark else 50),
        "actionBarActionModeDefault": N(28 if dark else 92),
        "actionBarActionModeDefaultIcon": N(90 if dark else 25),
        "actionBarActionModeDefaultTitle": N(95 if dark else 10),

        # ===== B. Text colors =====
        "windowBackgroundWhiteBlackText": N(95 if dark else 10),
        "windowBackgroundWhiteGrayText1": N(68 if dark else 45),
        "windowBackgroundWhiteGrayText2": N(62 if dark else 48),
        "windowBackgroundWhiteGrayText3": N(58 if dark else 50),
        "windowBackgroundWhiteGrayText4": N(60 if dark else 45),
        "windowBackgroundWhiteGrayText5": N(50 if dark else 55),
        "windowBackgroundWhiteGrayText6": N(62 if dark else 45),
        "windowBackgroundWhiteGrayText7": N(42 if dark else 62),   # disabled
        "windowBackgroundWhiteGrayText8": N(62 if dark else 45),
        "windowBackgroundWhiteRedText1": err,
        "windowBackgroundWhiteRedText2": err,
        "windowBackgroundWhiteRedText3": err,
        "windowBackgroundWhiteRedText4": err,
        "windowBackgroundWhiteRedText5": err,
        "windowBackgroundWhiteRedText6": err,
        "windowBackgroundWhiteGreenText": grn,
        "windowBackgroundWhiteBlueText": P(70 if dark else 40),
        "windowBackgroundWhiteBlueText2": P(70 if dark else 40),
        "windowBackgroundWhiteBlueText3": P(70 if dark else 40),
        "windowBackgroundWhiteBlueText4": P(70 if dark else 40),
        "windowBackgroundWhiteBlueHeader": P(75 if dark else 38),
        "windowBackgroundWhiteValueText": P(75 if dark else 35),
        "windowBackgroundWhiteLinkSelection": P(60 if dark else 80),
        "windowBackgroundWhiteHintText": N(50 if dark else 48),
        "windowBackgroundWhiteInputField": N(35 if dark else 78),
        "windowBackgroundWhiteInputFieldActivated": P(65 if dark else 45),
        "windowBackgroundWhiteIcon": N(60 if dark else 38),
        "windowBackgroundWhiteGrayIcon": N(55 if dark else 42),
        "graySection": N(14 if dark else 94),
        "graySectionText": N(60 if dark else 45),

        # ===== C. Chats list =====
        "chats_name": N(95 if dark else 10),
        "chats_nameIcon": N(60 if dark else 45),
        "chats_message": N(70 if dark else 45),
        "chats_date": N(50 if dark else 55),
        "chats_muteIcon": N(50 if dark else 55),
        "chats_draft": err,
        "chats_unreadCounter": P(65 if dark else 40),
        "chats_unreadCounterText": N(10 if dark else 100),
        "chats_unreadCounterMuted": N(40 if dark else 70),
        "chats_unreadCounterMutedText": N(90 if dark else 100),
        "chats_menuBackground": N(12 if dark else 98),
        "chats_menuTopShadow": N(20 if dark else 85),
        "chats_menuName": N(95 if dark else 10),
        "chats_actionBackground": P(55 if dark else 45),
        "chats_actionIcon": N(10 if dark else 100),
        "chats_actionPressedBackground": P(45 if dark else 55),
        "chats_sentCheck": P(60 if dark else 50),
        "chats_sentReadCheck": P(70 if dark else 40),
        "chats_verifiedBackground": P(60 if dark else 45),
        "chats_onlineCircle": grn,

        # ===== D. Chat bubbles =====
        "chat_inBubble": NV(24 if dark else 93),
        "chat_outBubble": out_fn(out_t),
        "chat_inBubbleSelected": NV(20 if dark else 89),
        "chat_outBubbleSelected": out_fn(out_t - 6),
        "chat_inBubbleShadow": N(0 if dark else 100),
        "chat_outBubbleShadow": N(0 if dark else 100),
        "chat_outBubbleGradient": out_fn(out_t),
        "chat_outBubbleGradient2": out_fn(out_t),
        "chat_outBubbleGradient3": out_fn(out_t),
        "chat_messageTextIn": N(95 if dark else 12),
        "chat_messageTextOut": N(98 if dark else 100),
        "chat_messageTextInSelected": N(100 if dark else 10),
        "chat_messageTextOutSelected": N(100 if dark else 100),
        "chat_messageLinkIn": P(70 if dark else 40),
        "chat_messageLinkOut": P(80 if dark else 30),
        "chat_inTimeText": N(62 if dark else 50),
        "chat_outTimeText": N(85 if dark else 90),
        "chat_inTimeSelectedText": N(78 if dark else 10),
        "chat_outTimeSelectedText": N(90 if dark else 100),
        "chat_outSentCheck": out_fn(65 if dark else 55),
        "chat_outSentCheckRead": out_fn(75 if dark else 45),
        "chat_outSentCheckSelected": out_fn(75 if dark else 65),
        "chat_outSentCheckReadSelected": out_fn(85 if dark else 55),
        "chat_mediaSentCheck": out_fn(65 if dark else 55),
        "chat_selectedBackground": N(28 if dark else 60),
        "chat_serviceText": N(95 if dark else 100),
        "chat_serviceBackground": N(45),

        # ===== E. Reply block =====
        "chat_inReplyLine": P(70 if dark else 40),
        "chat_outReplyLine": out_fn(80 if dark else 85),
        "chat_inReplyNameText": P(70 if dark else 35),
        "chat_outReplyNameText": out_fn(80 if dark else 20),
        "chat_inReplyMessageText": N(75 if dark else 35),
        "chat_outReplyMessageText": N(85 if dark else 90),

        # ===== F. Message panel & emoji & bot keyboard =====
        "chat_messagePanelBackground": N(13 if dark else 96),
        "chat_messagePanelText": N(95 if dark else 10),
        "chat_messagePanelHint": N(50 if dark else 50),
        "chat_messagePanelIcons": N(60 if dark else 40),
        "chat_messagePanelSend": P(70 if dark else 40),
        "chat_messagePanelVoicePressed": P(45 if dark else 75),
        "chat_emojiPanelBackground": N(12 if dark else 97),
        "chat_emojiPanelEmptyText": N(50 if dark else 45),
        "chat_emojiPanelIcon": N(50 if dark else 42),
        "chat_emojiPanelIconSelected": P(65 if dark else 45),
        "chat_emojiPanelBackspace": N(55 if dark else 45),
        "chat_botKeyboardButtonText": P(70 if dark else 40),
        "chat_botKeyboardButtonBackground": NV(22 if dark else 90),
        "chat_botKeyboardButtonBackgroundPressed": NV(28 if dark else 84),

        # ===== G. Attach panel (each tile gets its own M3 role) =====
        "chat_attachGalleryBackground": P(30 if dark else 88),
        "chat_attachGalleryIcon": N(95 if dark else 12),
        "chat_attachVideoBackground": TER(32 if dark else 88),
        "chat_attachVideoIcon": N(95 if dark else 12),
        "chat_attachMusicBackground": SEC(34 if dark else 88),
        "chat_attachMusicIcon": N(95 if dark else 12),
        "chat_attachFileBackground": NV(30 if dark else 85),
        "chat_attachFileIcon": N(95 if dark else 12),

        # ===== H. Dialogs =====
        "dialogBackground": N(18 if dark else 100),
        "dialogTextBlack": N(95 if dark else 10),
        "dialogButton": P(70 if dark else 40),
        "dialogButtonSelector": N(30 if dark else 85),
        "dialogCheckboxSquareUnchecked": N(45 if dark else 75),
        "dialogCheckboxSquareBackground": P(60 if dark else 45),
        "dialogCheckboxSquareCheck": N(10 if dark else 100),
        "dialogProgressCircle": P(70 if dark else 45),
        "dialogLineProgress": P(70 if dark else 45),
        "dialogLineProgressBackground": N(30 if dark else 85),
        "dialogRoundCheckBox": P(50 if dark else 45),
        "dialogRoundCheckBoxCheck": N(10 if dark else 100),

        # ===== I. Controls =====
        "switchTrack": NV(30 if dark else 75),
        "switchTrackChecked": P(50 if dark else 45),
        "switchThumb": N(75 if dark else 100),
        "switchThumbChecked": N(95 if dark else 100),
        "checkboxSquareUnchecked": N(45 if dark else 70),
        "checkboxSquareBackground": P(60 if dark else 45),
        "checkboxSquareCheck": N(10 if dark else 100),
        "checkboxSquareDisabled": N(35 if dark else 80),
        "radioBackground": N(45 if dark else 70),
        "radioBackgroundChecked": P(65 if dark else 45),
        "progressCircle": P(70 if dark else 45),

        # ===== J. Lists & misc =====
        "listSelectorSDK21": N(30 if dark else 70),
        "divider": N(18 if dark else 88),
        "stickers_menu": N(60 if dark else 40),
        "stickers_menuSelector": N(30 if dark else 85),
        "fastScrollInactive": N(45 if dark else 65),
        "fastScrollActive": P(65 if dark else 45),
        "fastScrollText": N(95 if dark else 10),

        # ===== K. Player =====
        "player_progress": P(70 if dark else 45),
        "player_progressBackground": N(35 if dark else 80),
        "player_time": N(70 if dark else 40),
        "player_button": N(80 if dark else 30),
        "player_buttonActive": P(70 if dark else 45),
        "player_actionBarItems": N(90 if dark else 20),
        "inappPlayerBackground": N(20 if dark else 97),
        "inappPlayerPlayPause": N(95 if dark else 20),
        "inappPlayerPerformer": N(65 if dark else 45),
        "inappPlayerTitle": N(95 if dark else 10),
        "inappPlayerClose": N(70 if dark else 40),

        # ===== L. Calls & featured stickers =====
        "calls_callReceivedGreenIcon": grn,
        "calls_callReceivedRedIcon": err,
        "featuredStickers_addedIcon": grn,
        "featuredStickers_addButton": P(65 if dark else 45),
        "featuredStickers_buttonText": N(10 if dark else 100),
        "featuredStickers_buttonProgress": P(70 if dark else 50),

        "chat_status": P(70 if dark else 40),
    }

    alpha = round(255 * (1 - alpha_pct / 100))
    sel_alpha = max(alpha, 200)   # selected bubbles must stay clearly visible

    lines = []
    for key, (r, g, b) in mapping.items():
        if key in ALPHA_KEYS:
            a = alpha                      # 🫧 user's transparency slider
        elif key in SELECTED_KEYS:
            a = sel_alpha                  # selection state stays readable
        else:
            a = FIXED_ALPHA.get(key, 255)  # overlays blend, rest opaque
        lines.append(f"{key}={signed_argb(r, g, b, a)}")

    # Background: flat color OR the user's image (hex-embedded)
    if wallpaper:
        lines.append("chat_wallpaper=-1")
        hx = wallpaper.hex()
        for i in range(0, len(hx), 1024):
            lines.append(hx[i:i + 1024])
    else:
        r, g, b = N(7 if dark else 92)
        lines.append(f"chat_wallpaper={signed_argb(r, g, b, 255)}")

    return ("\n".join(lines) + "\n").encode("utf-8")
