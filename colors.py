import colorsys
import io
import math

from PIL import Image

AV = ["Blue", "Cyan", "Green", "Orange", "Pink", "Red", "Violet"]
BLACK = (0, 0, 0)

# Safety net ONLY for undecodable files: zero-saturation ramp (no chroma).
# Never used for real images — extract_palette derives everything from the pic.
_ACHROMATIC = ["#101010", "#2c2c2c", "#4d4d4d", "#707070", "#969696", "#bcbcbc"]

# Semantic target hues (hue position only — saturation & lightness always
# come from the picture's own colors):
_HUE_LINK, _HUE_ERROR, _HUE_SUCCESS, _HUE_FLAME = 0.588, 0.02, 0.33, 0.07


# ---------- Color math (WCAG) ----------

def rgb_to_hex(rgb):
    return f"#{rgb[0]:02x}{rgb[1]:02x}{rgb[2]:02x}"


def hex_to_rgb(hex_str):
    hex_clean = hex_str.lstrip("#")
    if len(hex_clean) == 3:
        hex_clean = "".join(c * 2 for c in hex_clean)
    if len(hex_clean) != 6:
        raise ValueError(f"Invalid hex color string: {hex_str}")
    return tuple(int(hex_clean[i:i + 2], 16) for i in (0, 2, 4))


def luminance(rgb):
    r, g, b = [c / 255.0 for c in rgb[:3]]
    r = r / 12.92 if r <= 0.04045 else ((r + 0.055) / 1.055) ** 2.4
    g = g / 12.92 if g <= 0.04045 else ((g + 0.055) / 1.055) ** 2.4
    b = b / 12.92 if b <= 0.04045 else ((b + 0.055) / 1.055) ** 2.4
    return 0.2126 * r + 0.7152 * g + 0.0722 * b


def mix(c1, c2, weight=0.5):
    w = max(0.0, min(1.0, float(weight)))
    return tuple(max(0, min(255, round(c1[i] * w + c2[i] * (1.0 - w))))
                 for i in range(3))


def contrast_ratio(c1, c2):
    l1, l2 = luminance(c1), luminance(c2)
    return (max(l1, l2) + 0.05) / (min(l1, l2) + 0.05)


def readable_on(bg):
    return (255, 255, 255) if luminance(bg) < 0.45 else (0, 0, 0)


def ensure_contrast(fg, bg, min_ratio=4.5):
    if contrast_ratio(fg, bg) >= min_ratio:
        return fg
    target = readable_on(bg)
    for step in range(1, 11):
        step_fg = mix(target, fg, step / 10.0)
        if contrast_ratio(step_fg, bg) >= min_ratio:
            return step_fg
    return target


def saturation(rgb):
    return colorsys.rgb_to_hls(rgb[0] / 255, rgb[1] / 255, rgb[2] / 255)[2]


def _hls(rgb):
    return colorsys.rgb_to_hls(rgb[0] / 255, rgb[1] / 255, rgb[2] / 255)


def _to_rgb(h, l, s):
    r, g, b = colorsys.hls_to_rgb(h, l, s)
    return (int(round(r * 255)), int(round(g * 255)), int(round(b * 255)))


def set_lightness(rgb, l):
    h, _, s = _hls(rgb)
    return _to_rgb(h, l, s)


def clamp_lightness(rgb, min_l=None, max_l=None):
    h, l, s = _hls(rgb)
    if min_l is not None and l < min_l:
        l = min_l
    if max_l is not None and l > max_l:
        l = max_l
    return _to_rgb(h, l, s)


def _hue_deg(rgb):
    return (_hls(rgb)[0] % 1.0) * 360.0


def _in_hue_deg(rgb, lo, hi):
    """True if hue is in [lo, hi] degrees (wraps across 0)."""
    hd = _hue_deg(rgb)
    if lo <= hi:
        return lo <= hd <= hi
    return hd >= lo or hd <= hi


def _pick_hue(pal, lo, hi):
    """Most saturated palette color whose hue is in range, else None."""
    matches = [c for c in pal if _in_hue_deg(c, lo, hi)]
    return max(matches, key=saturation) if matches else None


def _shift_hue(rgb, hue):
    """Move hue, KEEP the picture color's own saturation & lightness."""
    h, l, s = _hls(rgb)
    return _to_rgb(hue, l, s)


# ---------- Palette extraction (all colors from the pic) ----------

def _palette_from_single(rgb, count):
    """Build variants from one image color (used as in-pic fallback)."""
    h, l, s = _hls(rgb)
    out = [rgb]
    if s < 0.08:
        # achromatic image → achromatic ramp of its own lightness
        for i in range(1, count):
            ll = max(0.04, min(0.96, l + (i - count // 2) * 0.14))
            out.append(_to_rgb(h, ll, 0.0))
        return out[:count]
    offsets = [-0.20, 0.12, 0.26, -0.08, 0.36, 0.18]
    for i in range(1, count):
        out.append(_to_rgb((h + i * 0.13) % 1.0,
                           max(0.08, min(0.92, l + offsets[(i - 1) % len(offsets)])),
                           s))
    return out[:count]


def extract_palette(image_bytes, count=6):
    """6 distinct colors from the image. Never raises, never static."""
    try:
        img = Image.open(io.BytesIO(image_bytes))
        if img.mode in ("RGBA", "LA") or \
                (img.mode == "P" and "transparency" in img.info):
            bg_canvas = Image.new("RGB", img.size, (128, 128, 128))
            if img.mode != "RGBA":
                img = img.convert("RGBA")
            bg_canvas.paste(img, mask=img.getchannel("A"))
            img = bg_canvas
        else:
            img = img.convert("RGB")
    except Exception:
        return list(_ACHROMATIC[:count])   # undecodable file only

    try:
        avg = img.resize((1, 1), Image.Resampling.LANCZOS).getpixel((0, 0))
        avg = tuple(avg[:3])
    except Exception:
        avg = None

    try:
        img.thumbnail((256, 256), Image.Resampling.LANCZOS)
        quantized = img.quantize(colors=24, method=Image.Quantizing.MEDIANCUT)
        palette_raw = quantized.getpalette()[:72]
        color_counts = quantized.getcolors() or []

        candidates = []
        for _, idx in sorted(color_counts, key=lambda x: x[0], reverse=True):
            rgb = tuple(palette_raw[idx * 3:(idx + 1) * 3])
            lum = luminance(rgb)
            if lum < 0.03 or lum > 0.97:
                continue
            candidates.append(rgb)

        if not candidates and avg:
            return [rgb_to_hex(c) for c in _palette_from_single(avg, count)]

        selected = [candidates[0]]
        for cand in candidates[1:]:
            if len(selected) >= count:
                break
            if all(math.sqrt(sum((cand[i] - sel[i]) ** 2 for i in range(3))) >= 45.0
                   for sel in selected):
                selected.append(cand)

        while len(selected) < count:          # backfill: rotate the pic's own hues
            h, s, v = colorsys.rgb_to_hsv(*[c / 255.0 for c in selected[-1]])
            r, g, b = colorsys.hsv_to_rgb((h + 0.15) % 1.0, max(0.4, s), max(0.5, v))
            selected.append((int(r * 255), int(g * 255), int(b * 255)))

        return [rgb_to_hex(c) for c in selected[:count]]
    except Exception:
        if avg:
            return [rgb_to_hex(c) for c in _palette_from_single(avg, count)]
        return list(_ACHROMATIC[:count])


# ---------- Wallpaper ----------

def prepare_wallpaper(data: bytes, max_side: int = 1080, quality: int = 75) -> bytes:
    img = Image.open(io.BytesIO(data)).convert("RGB")
    img.thumbnail((max_side, max_side), Image.Resampling.LANCZOS)
    buf = io.BytesIO()
    img.save(buf, "JPEG", quality=quality)
    return buf.getvalue()


# ---------- Semantic colors, derived from the pic ----------

def _semantic(pal, bg, dark, lo, hi, hue, lmin, lmax):
    """Pic color with matching hue, else hue-shifted donor; legibility-clamped."""
    base = _pick_hue(pal, lo, hi)
    if base is None:
        donor = max(pal, key=saturation) if pal else bg
        base = _shift_hue(donor, hue)
    base = clamp_lightness(base, min_l=lmin if dark else None,
                           max_l=None if dark else lmax)
    return ensure_contrast(base, bg, 3.0)


def derive_semantics(pal, bg, dark):
    """link / error / success / flame / shadow — every value from the pic."""
    pal = list(pal) or [bg]
    return {
        "link":    _semantic(pal, bg, dark, 195, 265, _HUE_LINK,    0.42, 0.55),
        "error":   _semantic(pal, bg, dark, 330, 25,  _HUE_ERROR,   0.45, 0.50),
        "success": _semantic(pal, bg, dark, 90,  150, _HUE_SUCCESS, 0.42, 0.52),
        "flame":   _semantic(pal, bg, dark, 10,  50,  _HUE_FLAME,   0.50, 0.60),
        "shadow":  mix(bg, BLACK, 0.5 if dark else 0.15),
    }


# ---------- Section resolution ----------

def resolve_theme(palette: list, sections: dict, mode: str = "dark") -> dict:
    dark = mode == "dark"
    pal = [hex_to_rgb(h) for h in (palette or _ACHROMATIC)]

    def chosen(sec):
        s = sections.get(sec) or {}
        if s.get("custom"):
            return hex_to_rgb(s["custom"])
        i = s.get("idx", -1)
        if isinstance(i, int) and 0 <= i < len(pal):
            return pal[i]
        return None

    darkest = min(pal, key=luminance)
    lightest = max(pal, key=luminance)
    most_sat = max(pal, key=saturation)

    auto_bg = set_lightness(darkest, 0.07) if dark else set_lightness(lightest, 0.96)
    auto_accent = clamp_lightness(most_sat, min_l=0.62) if dark \
        else clamp_lightness(most_sat, max_l=0.42)

    res = {}
    res["bg"] = chosen("bg") or auto_bg
    res["text"] = chosen("text") or ((255, 255, 255) if dark else (22, 24, 29))
    res["accent"] = chosen("accent") or auto_accent
    res["bar"] = chosen("bar") or res["bg"]
    res["in"] = chosen("in") or mix(res["bg"], BLACK, 0.12 if dark else 0.05)
    res["out"] = chosen("out") or res["accent"]
    res["reply"] = chosen("reply") or res["text"]
    res.update(derive_semantics(pal, res["bg"], dark))
    return res


def resolve_wall(palette: list, wall_idx, wall_custom, mode: str = "dark"):
    if wall_custom:
        return hex_to_rgb(wall_custom)
    pal = [hex_to_rgb(h) for h in (palette or _ACHROMATIC)]
    if isinstance(wall_idx, int) and 0 <= wall_idx < len(pal):
        return pal[wall_idx]
    if mode == "dark":
        return set_lightness(min(pal, key=luminance), 0.06)
    return set_lightness(max(pal, key=luminance), 0.97)


# ---------- .attheme serialization ----------

def _rgb_to_attheme_int(rgb, alpha=255):
    r, g, b = rgb[:3]
    a = max(0, min(255, int(alpha)))
    val = (a << 24) | ((r & 0xFF) << 16) | ((g & 0xFF) << 8) | (b & 0xFF)
    return val - 0x100000000 if val >= 0x80000000 else val


def build_attheme(colors: dict, alphas: dict,
                  wallpaper: bytes | None = None, wall_flat=None) -> bytes:
    bg, bar = colors["bg"], colors["bar"]
    inb, outb = colors["in"], colors["out"]
    text, accent = colors["text"], colors["accent"]
    reply = colors["reply"]
    link = colors.get("link") or _shift_hue(accent, _HUE_LINK)
    error = colors.get("error") or _shift_hue(accent, _HUE_ERROR)
    success = colors.get("success") or _shift_hue(accent, _HUE_SUCCESS)
    shadow = colors.get("shadow") or mix(bg, BLACK, 0.5)

    get_alpha = lambda k: max(0, min(255, round(255 * (1 - alphas.get(k, 0) / 100.0))))
    a_bar = get_alpha("bar")
    a_in, a_out = get_alpha("in"), get_alpha("out")
    a_text, a_acc = get_alpha("text"), get_alpha("accent")
    a_reply = get_alpha("reply")

    dark = luminance(bg) < 0.5

    deep1 = mix(bg, BLACK, 0.18) if dark else mix(bg, BLACK, 0.04)
    deep2 = mix(bg, BLACK, 0.32) if dark else mix(bg, BLACK, 0.09)
    divider = mix(bg, BLACK, 0.45) if dark else mix(bg, BLACK, 0.14)

    bar_text = ensure_contrast(text, bar)
    bar_sub = (bar_text, 170) if dark else (mix(bar_text, BLACK, 0.35), 255)
    bar_icon = (bar_text, 190) if dark else (mix(bar_text, BLACK, 0.20), 255)

    in_text = ensure_contrast(text, inb)
    out_text = ensure_contrast(text, outb)
    in_time = (in_text, 170) if dark else (mix(in_text, BLACK, 0.35), 255)
    out_time = (out_text, 175) if dark else (mix(out_text, BLACK, 0.30), 255)

    acc_text = ensure_contrast(accent, bg)
    on_acc = readable_on(accent)
    acc_in = ensure_contrast(accent, inb)
    acc_out = ensure_contrast(accent, outb)
    reply_in = ensure_contrast(reply, inb)
    reply_out = ensure_contrast(reply, outb)

    gray1 = (text, 205) if dark else (mix(text, BLACK, 0.18), 255)
    gray2 = (text, 170) if dark else (mix(text, BLACK, 0.35), 255)
    gray3 = (text, 140) if dark else (mix(text, BLACK, 0.50), 255)

    thumb = readable_on(bg)
    in_sel = mix(inb, BLACK, 0.12)
    out_sel = mix(outb, BLACK, 0.12)
    in_deep = mix(inb, BLACK, 0.18)
    out_deep = mix(outb, BLACK, 0.18)
    fab = mix(accent, BLACK, 0.32 if dark else 0.18)
    fab_pressed = mix(fab, BLACK, 0.15)
    on_fab = readable_on(fab)

    M = {}

    def put(keys, rgb, alpha=255):
        if isinstance(keys, str):
            keys = [keys]
        for k in keys:
            M[k] = (rgb, alpha)

    # ===== Links — the pic's own blue (or accent hue-shifted to blue) =====
    put(["windowBackgroundWhiteLinkText", "dialogTextLink", "chat_messageLinkIn",
         "chat_messageLinkOut", "chat_serviceLink"], link)

    # ===== Surfaces = bg (opaque — drawer & forward stay solid) =====
    put(["windowBackgroundWhite", "windowBackgroundGray", "windowBackgroundBlack",
         "dialogBackground", "dialogBackgroundGray", "graySection",
         "actionBarDefaultSubmenuBackground", "chats_menuBackground",
         "chats_pinnedOverlay", "chats_archiveBackground",
         "chat_emojiPanelBackground", "chat_stickersHintPanel",
         "chat_topPanelBackground", "chat_messagePanelBackground",
         "chat_messagePanelVoiceBackground", "chat_recordedVoiceBackground",
         "contacts_inviteBackground", "musicPicker_buttonBackground",
         "chat_unreadMessagesStartBackground", "player_background",
         "inappPlayerBackground", "profile_actionBackground",
         "sharedMedia_actionMode", "returnToCallBackground",
         "player_actionBar", "player_actionBarTop",
         "key_chat_messagePanelVoiceLockBackground", "windowBackgroundGrayShadow"],
        bg, 255)

    # ===== Elevation = darker, never lighter =====
    put(["files_folderIconBackground", "chat_secretTimerBackground"], deep2, 220)
    put(["chat_goDownButton", "chat_botKeyboardButtonBackground"], deep2, 230)
    put("chat_botKeyboardButtonBackgroundPressed", mix(deep2, BLACK, 0.20))
    put("chat_goDownButtonShadow", shadow, 80)
    put("chat_messagePanelShadow", deep2, 120)
    put(["key_chat_messagePanelVoiceLockShadow", "ChatShadow"], shadow, 80)
    put("chats_menuTopShadow", deep1, 60)

    # ===== Top bar =====
    put(["actionBarDefault", "actionBarDefaultArchived",
         "actionBarActionModeDefaultTop"], bar, a_bar)
    put([f"avatar_backgroundActionBar{c}" for c in AV], bar)
    put("actionBarActionModeDefault", mix(bar, accent, 0.20), a_bar)
    put(["actionBarDefaultTitle", "actionBarDefaultArchivedTitle",
         "actionBarDefaultSearch", "actionBarDefaultArchivedSearch",
         "actionBarActionModeDefaultTitle", "player_actionBarTitle"], bar_text)
    put(["actionBarDefaultSubtitle", "actionBarDefaultSearchPlaceholder",
         "actionBarDefaultSearchArchivedPlaceholder", "actionBarTabText",
         "player_actionBarSubtitle"], bar_sub[0], bar_sub[1])
    put(["actionBarDefaultIcon", "actionBarDefaultArchivedIcon",
         "actionBarActionModeDefaultIcon", "player_actionBarItems"] +
        [f"avatar_actionBarIcon{c}" for c in AV], bar_icon[0], bar_icon[1])
    put(["actionBarDefaultSelector", "actionBarActionModeDefaultSelector",
         "actionBarWhiteSelector", "player_actionBarSelector"] +
        [f"avatar_actionBarSelector{c}" for c in AV], text, 30 if dark else 25)
    put("actionBarTabLine", accent)
    put("actionBarTabActiveText", ensure_contrast(accent, bar))
    put(["actionBarDefaultSubmenuItem", "chats_menuItemText"], text)
    put(["actionBarDefaultSubmenuItemIcon", "chats_menuItemIcon"],
        gray2[0], gray2[1])

    # ===== Avatars =====
    put([f"avatar_background{c}" for c in AV] +
        ["avatar_backgroundSaved", "avatar_backgroundArchived",
         "avatar_backgroundGroupCreateSpanBlue"], accent)
    put([f"avatar_nameInMessage{c}" for c in AV], acc_text)
    put([f"avatar_subtitleInProfile{c}" for c in AV], gray2[0], gray2[1])
    put([f"avatar_backgroundInProfile{c}" for c in AV], mix(accent, BLACK, 0.15))
    put("avatar_text", on_acc)

    # ===== Incoming bubble =====
    put("chat_inBubble", inb, a_in)
    put("chat_inBubbleSelected", in_sel, max(a_in, 180))
    put("chat_inBubbleShadow", shadow)
    put("chat_messageTextIn", in_text, a_text)
    put("chat_inTimeText", in_time[0], in_time[1])
    put("chat_inTimeSelectedText", in_text)
    put("chat_inReplyLine", acc_in)
    put(["chat_inAudioDurationText", "chat_inAudioDurationSelectedText",
         "chat_inAudioTitleText", "chat_inFileNameText", "chat_inFileInfoText",
         "chat_inFileInfoSelectedText", "chat_inContactNameText",
         "chat_inContactPhoneText", "chat_inForwardedNameText",
         "chat_inSiteNameText", "chat_inViaBotNameText",
         "chat_inPreviewInstantText", "chat_inPreviewInstantSelectedText",
         "chat_inViews", "chat_inViewsSelected", "chat_inMenu",
         "chat_inMenuSelected", "chat_inInstant", "chat_inInstantSelected",
         "chat_inFileIcon", "chat_inFileSelectedIcon", "chat_inContactIcon",
         "chat_inPreviewLine", "chat_inLoader", "chat_inSentClock",
         "chat_inSentClockSelected", "chat_inVenueNameText",
         "chat_inLocationIcon", "chat_inAudioPerfomerText",
         "chat_inAudioPerformerSelectedText"], in_text, a_text)
    put(["chat_inVenueInfoText", "chat_inVenueInfoSelectedText"],
        in_time[0], in_time[1])
    put(["chat_inAudioSeekbar", "chat_inVoiceSeekbar", "chat_inAudioSeekbarSelected",
         "chat_inVoiceSeekbarSelected"], in_deep)
    put(["chat_inAudioSeekbarFill", "chat_inVoiceSeekbarFill"], in_text)
    put(["chat_inAudioProgress", "chat_inAudioSelectedProgress",
         "chat_inFileProgress", "chat_inFileProgressSelected"], in_text)
    put(["chat_inFileBackground", "chat_inFileBackgroundSelected",
         "chat_inLocationBackground", "chat_inLoaderPhoto",
         "chat_inLoaderPhotoSelected", "chat_inContactBackground"], in_deep)
    put(["chat_inLoaderPhotoIcon", "chat_inLoaderPhotoIconSelected"], in_text)
    put("chat_inLoaderSelected", in_text, 140)

    # ===== Outgoing bubble =====
    put("chat_outBubble", outb, a_out)
    put("chat_outBubbleSelected", out_sel, max(a_out, 180))
    put(["chat_outBubbleGradient", "chat_outBubbleGradient2",
         "chat_outBubbleGradient3"], outb, a_out)
    put("chat_outBubbleShadow", shadow)
    put("chat_messageTextOut", out_text, a_text)
    put("chat_outTimeText", out_time[0], out_time[1])
    put("chat_outTimeSelectedText", out_text)
    put("chat_outReplyLine", acc_out)
    put(["chat_outAudioDurationText", "chat_outAudioDurationSelectedText",
         "chat_outAudioTitleText", "chat_outFileNameText", "chat_outFileInfoText",
         "chat_outFileInfoSelectedText", "chat_outContactNameText",
         "chat_outContactPhoneText", "chat_outForwardedNameText",
         "chat_outSiteNameText", "chat_outViaBotNameText",
         "chat_outPreviewInstantText", "chat_outPreviewInstantSelectedText",
         "chat_outViews", "chat_outViewsSelected", "chat_outMenu",
         "chat_outMenuSelected", "chat_outInstant", "chat_outInstantSelected",
         "chat_outFileIcon", "chat_outFileSelectedIcon", "chat_outContactIcon",
         "chat_outPreviewLine", "chat_outLoader", "chat_outSentClock",
         "chat_outSentClockSelected", "chat_outVenueNameText",
         "chat_outAudioPerfomerText", "chat_outAudioPerformerSelectedText",
         "chat_outLocationIcon"], out_text, a_text)
    put(["chat_outVenueInfoText", "chat_outVenueInfoSelectedText"],
        out_time[0], out_time[1])
    put(["chat_outSentCheck", "chat_outSentCheckSelected",
         "chat_outSentCheckRead", "chat_outSentCheckReadSelected"], out_text)
    put("chat_mediaSentCheck", out_text)
    put(["chat_outAudioSeekbar", "chat_outVoiceSeekbar",
         "chat_outAudioSeekbarSelected", "chat_outVoiceSeekbarSelected"], out_deep)
    put(["chat_outAudioSeekbarFill", "chat_outVoiceSeekbarFill"], out_text)
    put(["chat_outAudioProgress", "chat_outAudioSelectedProgress",
         "chat_outFileProgress", "chat_outFileProgressSelected"], out_text)
    put(["chat_outFileBackground", "chat_outFileBackgroundSelected",
         "chat_outLocationBackground", "chat_outLoaderPhoto",
         "chat_outLoaderPhotoSelected", "chat_outContactBackground"], out_deep)
    put(["chat_outLoaderPhotoIcon", "chat_outLoaderPhotoIconSelected"], out_text)
    put("chat_outLoaderSelected", out_text, 140)

    # ===== Reply (tag) block =====
    put(["chat_inReplyNameText", "chat_inReplyMessageText",
         "chat_inReplyMediaMessageText",
         "chat_inReplyMediaMessageSelectedText"], reply_in, a_reply)
    put(["chat_outReplyNameText", "chat_outReplyMessageText",
         "chat_outReplyMediaMessageText",
         "chat_outReplyMediaMessageSelectedText"], reply_out, a_reply)
    put(["chat_stickerReplyNameText", "chat_stickerReplyMessageText"],
        ensure_contrast(reply, bg), a_reply)

    # ===== Text / UI =====
    put(["windowBackgroundWhiteBlackText", "chats_name", "chats_nameMessage",
         "chats_nameArchived", "chats_nameMessageArchived",
         "chats_secretName", "chats_menuName", "dialogTextBlack",
         "emptyListPlaceholder", "fastScrollText", "chat_messagePanelText",
         "profile_title", "chats_attachMessage", "dialogSearchText",
         "chat_fieldOverlayText", "chat_topPanelMessage", "chat_topPanelTitle",
         "chat_secretTimerText", "groupcreate_sectionText",
         "chats_message_threeLines", "chats_nameMessage_threeLines",
         "chats_nameMessageArchived_threeLines"], text, a_text)
    put(["chats_message", "chats_actionMessage", "dialogTextGray", "player_time",
         "inappPlayerPerformer", "chats_menuPhone",
         "windowBackgroundWhiteGrayText1"], gray1[0], gray1[1])
    put(["chats_date", "chats_muteIcon", "chats_pinnedIcon", "chats_secretIcon",
         "chats_mentionIcon", "chats_archiveIcon", "chat_muteIcon",
         "chat_lockIcon", "chat_messagePanelHint",
         "windowBackgroundWhiteGrayText2", "windowBackgroundWhiteGrayText3",
         "windowBackgroundWhiteGrayText4", "windowBackgroundWhiteGrayText8",
         "dialogTextGray2", "chat_emojiPanelEmptyText",
         "chat_emojiPanelBackspace", "chat_emojiPanelIcon",
         "chat_emojiPanelTrendingTitle", "fastScrollInactive",
         "inappPlayerClose", "chats_menuCloud", "chats_menuPhoneCats",
         "chats_menuCloudBackgroundCats", "groupcreate_hintText",
         "groupcreate_offlineText", "chat_previewDurationText",
         "chat_previewGameText", "sessions_devicesImage",
         "changephoneinfo_image", "key_sheet_other", "key_sheet_scrollUp",
         "chat_unreadMessagesStartArrowIcon"], gray2[0], gray2[1])
    put(["windowBackgroundWhiteGrayText", "windowBackgroundWhiteGrayIcon",
         "windowBackgroundWhiteIcon", "stickers_menu",
         "chat_emojiPanelStickerSetName", "dialogIcon", "dialogSearchIcon",
         "dialogSearchHint", "chat_searchPanelIcons",
         "chat_messagePanelIcons", "chat_messagePanelVoiceDelete",
         "chat_messagePanelVoiceDuration", "chat_messagePanelCancelInlineBot",
         "chat_replyPanelIcons", "chat_recordTime", "chat_recordVoiceCancel",
         "chat_topPanelClose", "chat_topPanelLine", "chat_secretTimeText",
         "windowBackgroundWhiteHintText"], gray3[0], gray3[1])
    put(["divider", "dialogGrayLine", "dialogShadowLine",
         "chat_emojiPanelShadowLine", "windowBackgroundWhiteInputField"],
        divider)
    put("windowBackgroundWhiteInputFieldActivated", accent)

    # ===== Accent =====
    put(["windowBackgroundWhiteBlueText", "windowBackgroundWhiteBlueText2",
         "windowBackgroundWhiteBlueText3", "windowBackgroundWhiteBlueText4",
         "windowBackgroundWhiteBlueText6", "windowBackgroundWhiteBlueText7",
         "windowBackgroundWhiteBlueHeader", "windowBackgroundWhiteValueText",
         "dialogTextBlue", "dialogTextBlue2", "dialogTextBlue3",
         "dialogTextBlue4", "chat_status", "chat_addContact",
         "chat_adminText", "chat_adminSelectedText", "chat_botSwitchToInlineText",
         "dialogInputFieldActivated", "groupcreate_cursor",
         "groupcreate_onlineText", "chat_messagePanelSend",
         "chat_unreadMessagesStartText", "chat_editDoneIcon", "chats_sentCheck",
         "chats_sentClock", "chats_sentReadCheck",
         "sharedMedia_startStopLoadIcon", "PreviewBack",
         "PreviewBackLinear"], acc_text, a_acc)
    put(["chats_unreadCounter", "chats_verifiedBackground",
         "chats_archivePinBackground",
         "chat_attachGalleryBackground", "chat_attachVideoBackground",
         "chat_attachAudioBackground", "chat_attachFileBackground",
         "chat_attachContactBackground", "chat_attachLocationBackground",
         "chat_attachHideBackground", "chat_attachSendBackground",
         "chat_attachMediaBanBackground", "undo_background", "picker_badge",
         "dialogBadgeBackground", "checkbox",
         "checkboxSquareBackground", "dialogCheckboxSquareBackground",
         "radioBackgroundChecked", "dialogRadioBackgroundChecked",
         "dialogRoundCheckBox", "switchTrackChecked", "switch2TrackChecked",
         "dialogLineProgress", "dialogProgressCircle", "progressCircle",
         "contextProgressInner1", "contextProgressOuter1",
         "featuredStickers_addButton", "location_sendLocationBackground",
         "location_sendLiveLocationBackground",
         "location_placeLocationBackground", "chat_messagePanelVoicePressed",
         "chat_botProgress"], accent, a_acc)
    put(["chats_unreadCounterText", "chats_verifiedCheck", "picker_badgeText",
         "dialogBadgeText", "checkboxCheck", "checkboxSquareCheck",
         "dialogCheckboxSquareCheck", "dialogRoundCheckBoxCheck",
         "featuredStickers_buttonText", "files_iconText",
         "undo_cancelColor", "undo_infoColor", "chat_attachGalleryIcon",
         "chat_attachVideoIcon", "chat_attachFileIcon",
         "chat_attachContactIcon", "chat_attachLocationIcon",
         "chat_attachHideIcon", "chat_attachSendIcon",
         "chat_attachMediaBanText", "chat_attachCameraIcon1",
         "chat_attachCameraIcon2", "chat_attachCameraIcon3",
         "chat_attachCameraIcon4", "chat_attachCameraIcon5",
         "chat_attachCameraIcon6", "location_sendLocationIcon",
         "musicPicker_checkboxCheck", "groupcreate_checkboxCheck",
         "chats_menuItemCheck"], on_acc)

    # ===== Circle buttons (FAB) =====
    put(["chats_actionBackground", "dialogFloatingButton",
         "chat_goDownButtonCounterBackground"], fab)
    put(["chats_actionPressedBackground", "dialogFloatingButtonPressed",
         "featuredStickers_addButtonPressed"], fab_pressed)
    put(["chats_actionIcon", "dialogFloatingIcon"], on_fab)

    # ===== Switches / checkboxes =====
    put(["switchTrack", "switch2Track"], deep2, 150)
    put(["switchThumb", "switchThumbChecked", "switch2Thumb",
         "switch2ThumbChecked"], thumb)
    put(["checkboxSquareUnchecked", "dialogCheckboxSquareUnchecked",
         "radioBackground", "dialogRadioBackground"], deep2)
    put("checkboxSquareDisabled", deep1)
    put(["switch2Check", "musicPicker_checkbox", "groupcreate_checkbox"], accent)

    # ===== Dialogs / selectors =====
    put("dialogButton", acc_text)
    put("dialogButtonSelector", accent, 60)
    put("dialogLineProgressBackground", deep1, 80)
    put("dialogSearchBackground", deep1, 90)
    put("dialogScrollGlow", gray2[0], gray2[1])
    put("dialogLinkSelection", link, 60)
    put("windowBackgroundWhiteLinkSelection", link, 60)
    put("listSelectorSDK21", text, 30 if dark else 25)
    put("stickers_menuSelector", text, 30 if dark else 25)
    put("chat_emojiPanelIconSelector", text, 30 if dark else 25)
    put("chat_emojiPanelStickerPackSelector", text, 40 if dark else 30)
    put("chat_selectedBackground", accent, 90)
    put("chat_textSelectBackground", accent, 100)
    put("chat_linkSelectBackground", link, 50)
    put("chats_tabletSelectedOverlay", text, 40 if dark else 30)

    # ===== Service chip =====
    put(["chat_serviceBackground", "chat_serviceBackgroundSelected"],
        deep1, 190)
    put(["chat_serviceText", "chat_serviceIcon"], text)

    # ===== Bots / emoji panel =====
    put("chat_botKeyboardButtonText", text)
    put("chat_emojiPanelBadgeBackground", accent)
    put("chat_emojiPanelBadgeText", on_acc)
    put(["chat_emojiPanelIconSelected", "chat_emojiPanelMasksIconSelected",
         "chat_emojiPanelMasksIcon"], accent)
    put("chat_emojiPanelNewTrending", error)
    put("chat_emojiPanelTrendingDescription", gray2[0], gray2[1])
    put(["chat_emojiPanelStickerSetNameIcon", "chat_stickerViaBotNameText",
         "chat_stickerReplyLine"], gray2[0], gray2[1])

    # ===== Player =====
    put("player_progress", accent)
    put("player_progressBackground", deep2)
    put(["player_button", "player_placeholder"], gray3[0], gray3[1])
    put("player_buttonActive", acc_text)
    put("inappPlayerTitle", text)
    put(["inappPlayerPlayPause", "inappPlayerClose"], acc_text)

    # ===== Red / green semantics — derived from the pic =====
    put(["chats_draft", "chat_reportSpam", "chat_sentError", "chats_sentError",
         "dialogTextRed", "windowBackgroundWhiteRedText",
         "windowBackgroundWhiteRedText2", "windowBackgroundWhiteRedText3",
         "windowBackgroundWhiteRedText4", "windowBackgroundWhiteRedText5",
         "windowBackgroundWhiteRedText6", "chat_sentErrorIcon",
         "chats_sentErrorIcon", "calls_callReceivedRedIcon"], error)
    put(["windowBackgroundWhiteGreenText2", "featuredStickers_addedIcon",
         "calls_callReceivedGreenIcon"], success)
    put("chats_unreadCounterMuted", gray2[0], gray2[1])
    put("chats_unreadCounterMutedText", text)

    # ===== Assemble =====
    lines = [f"{k}={_rgb_to_attheme_int(rgb, a)}" for k, (rgb, a) in M.items()]
    body = ("\n".join(lines) + "\n").encode("utf-8")

    if wallpaper:
        offset = 0
        header = b""
        for _ in range(6):
            header = f"wallpaperFileOffset={offset}\n".encode("utf-8")
            new_offset = len(header) + len(body)
            if new_offset == offset:
                break
            offset = new_offset
        return header + body + wallpaper

    r, g, b = wall_flat if wall_flat else deep2
    return body + f"chat_wallpaper={_rgb_to_attheme_int((r, g, b), 255)}\n".encode("utf-8")
