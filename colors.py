import colorsys
import io

from PIL import Image

FALLBACK_PALETTE = ["#23272e", "#333a45", "#4a90d9", "#7e57c2", "#26a69a", "#ef6c61"]

AV = ["Blue", "Cyan", "Green", "Orange", "Pink", "Red", "Violet"]

# Links keep Telegram's classic blue in every theme — never themed.
# Change this one constant if you prefer another shade (e.g. #2678b6).
LINK_BLUE = (0x27, 0x82, 0xE9)


# ---------- Basic helpers ----------

def hex_to_rgb(h: str):
    h = h.lstrip("#")
    return tuple(int(h[i:i + 2], 16) for i in (0, 2, 4))


def rgb_to_hex(c) -> str:
    return "#%02x%02x%02x" % tuple(c)


def signed_argb(r: int, g: int, b: int, a: int = 255) -> int:
    """Telegram .attheme colors are signed 32-bit ARGB decimals."""
    v = (a << 24) | (r << 16) | (g << 8) | b
    return v - 0x100000000 if v > 0x7FFFFFFF else v


def luminance(rgb) -> float:
    r, g, b = rgb
    return (0.2126 * r + 0.7152 * g + 0.0722 * b) / 255


def saturation(rgb) -> float:
    _, _, s = colorsys.rgb_to_hls(rgb[0] / 255, rgb[1] / 255, rgb[2] / 255)
    return s


def readable_on(bg_rgb):
    """Near-white or near-black depending on the background."""
    return (255, 255, 255) if luminance(bg_rgb) < 0.5 else (22, 24, 29)


def mix(c1, c2, k):
    return tuple(int(round(a + (b - a) * k)) for a, b in zip(c1, c2))


def ensure_contrast(rgb, bg_rgb, min_diff=0.25):
    """If the color is unreadable on bg, push it toward the readable extreme."""
    if abs(luminance(rgb) - luminance(bg_rgb)) < min_diff:
        return mix(rgb, readable_on(bg_rgb), 0.55)
    return rgb


def _hls(rgb):
    return colorsys.rgb_to_hls(rgb[0] / 255, rgb[1] / 255, rgb[2] / 255)


def _to_rgb(h, l, s):
    r, g, b = colorsys.hls_to_rgb(h, l, s)
    return (int(round(r * 255)), int(round(g * 255)), int(round(b * 255)))


def set_lightness(rgb, l):
    """Keep hue & saturation, force lightness — guarantees dark/light colors."""
    h, _, s = _hls(rgb)
    return _to_rgb(h, l, s)


def clamp_lightness(rgb, min_l=None, max_l=None):
    h, l, s = _hls(rgb)
    if min_l is not None and l < min_l:
        l = min_l
    if max_l is not None and l > max_l:
        l = max_l
    return _to_rgb(h, l, s)


# ---------- Color extraction (never raises) ----------

def extract_palette(data: bytes, count: int = 6) -> list:
    """6 suggested colors from the user's image."""
    try:
        img = Image.open(io.BytesIO(data)).convert("RGB")
        img.thumbnail((160, 160))
        q = img.quantize(colors=10)
        counts = q.convert("RGB").getcolors(160 * 160) or []

        cands = []
        for cnt, (r, g, b) in counts:
            h, l, s = colorsys.rgb_to_hls(r / 255, g / 255, b / 255)
            cands.append((cnt, h, l, s, (r, g, b)))

        def pick(strict):
            out = []
            for cnt, h, l, s, rgb in sorted(cands, key=lambda x: -x[0]):
                if strict and (l < 0.10 or l > 0.90):
                    continue  # skip near-black / near-white
                if any(abs(h - h2) < 0.07 and abs(l - l2) < 0.18
                       for _, h2, l2, _, _ in out):
                    continue  # skip similar colors
                out.append((cnt, h, l, s, rgb))
                if len(out) >= count:
                    break
            return out

        res = pick(True) or pick(False)
        if res:
            res.sort(key=lambda x: -(x[0] * (0.3 + x[3])))
            return [rgb_to_hex(rgb) for *_, rgb in res]
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


# ---------- Section resolution (mode-driven auto, no M3) ----------

def resolve_theme(palette: list, sections: dict, mode: str = "dark") -> dict:
    """
    Auto values are driven by dark/light MODE:
      dark  → truly dark bg, WHITE text, vivid accent
      light → truly light bg, near-black text, deep accent
    Manual picks (idx / custom hex) always win.
    """
    dark = mode == "dark"
    pal = [hex_to_rgb(h) for h in (palette or FALLBACK_PALETTE)]

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

    # --- auto colors, forced to proper dark/light (fixes "too light") ---
    auto_bg = set_lightness(darkest, 0.07) if dark else set_lightness(lightest, 0.96)
    auto_text = (255, 255, 255) if dark else (22, 24, 29)
    auto_accent = clamp_lightness(most_sat, min_l=0.62) if dark \
        else clamp_lightness(most_sat, max_l=0.42)
    auto_bar = auto_bg
    auto_in = mix(auto_bg, auto_text, 0.10)
    auto_out = auto_accent

    res = {}
    res["bg"] = chosen("bg") or auto_bg
    res["text"] = chosen("text") or auto_text
    res["accent"] = chosen("accent") or auto_accent
    res["bar"] = chosen("bar") or auto_bar
    res["in"] = chosen("in") or auto_in
    res["out"] = chosen("out") or auto_out
    return res


def resolve_wall(palette: list, wall_idx, wall_custom, mode: str = "dark"):
    """Flat wallpaper color (used when the image is not selected)."""
    if wall_custom:
        return hex_to_rgb(wall_custom)
    pal = [hex_to_rgb(h) for h in (palette or FALLBACK_PALETTE)]
    if isinstance(wall_idx, int) and 0 <= wall_idx < len(pal):
        return pal[wall_idx]
    if mode == "dark":
        return set_lightness(min(pal, key=luminance), 0.06)
    return set_lightness(max(pal, key=luminance), 0.97)


# ---------- .attheme generator ----------

def build_attheme(colors: dict, alphas: dict,
                  wallpaper: bytes | None = None, wall_flat=None) -> bytes:
    """
    colors: bg, bar, in, out, text, accent (RGB tuples)
    alphas: per-section transparency 0..100
    """
    bg, bar = colors["bg"], colors["bar"]
    inb, outb = colors["in"], colors["out"]
    text, accent = colors["text"], colors["accent"]

    def A(k):
        return max(0, min(255, round(255 * (1 - alphas.get(k, 0) / 100.0))))

    a_bg, a_bar = A("bg"), A("bar")
    a_in, a_out = A("in"), A("out")
    a_text, a_acc = A("text"), A("accent")

    # ---- derived tones (all from user's colors) ----
    dark = luminance(bg) < 0.5
    bar_text = readable_on(bar)
    bar_sub = mix(bar_text, bar, 0.40)
    bar_icon = mix(bar_text, bar, 0.15)
    in_text = ensure_contrast(text, inb)
    out_text = ensure_contrast(text, outb)
    in_time = mix(in_text, inb, 0.35)
    out_time = mix(out_text, outb, 0.25)
    acc_text = ensure_contrast(accent, bg)
    on_acc = readable_on(accent)
    acc_in = ensure_contrast(accent, inb)
    acc_out = ensure_contrast(accent, outb)
    gray1 = mix(text, bg, 0.30)
    gray2 = mix(text, bg, 0.45)
    gray3 = mix(text, bg, 0.55)
    divider = mix(bg, text, 0.12)
    sel_ov = mix(bg, text, 0.10)
    surface = mix(bg, text, 0.04)
    surface2 = mix(bg, text, 0.06)
    surface3 = mix(bg, text, 0.10)
    shadow = (12, 12, 14) if dark else (255, 255, 255)
    red = (226, 88, 84) if dark else (211, 47, 47)
    green = (108, 203, 133) if dark else (46, 125, 50)
    in_sel = mix(inb, in_text, 0.12)
    out_sel = mix(outb, out_text, 0.12)

    M = {}

    def put(keys, rgb, alpha=255):
        if isinstance(keys, str):
            keys = [keys]
        for k in keys:
            M[k] = (rgb, alpha)

    # ===== Links — fixed Telegram blue, never themed =====
    put(["windowBackgroundWhiteLinkText", "dialogTextLink", "chat_messageLinkIn",
         "chat_messageLinkOut", "chat_serviceLink"], LINK_BLUE)

    # ===== Background / surfaces =====
    put("windowBackgroundWhite", bg, a_bg)
    put(["windowBackgroundGray", "dialogBackgroundGray"], surface2, a_bg)
    put(["windowBackgroundBlack", "chats_menuBackground", "chats_pinnedOverlay",
         "chat_emojiPanelBackground", "chat_stickersHintPanel",
         "chat_recordedVoiceBackground", "contacts_inviteBackground",
         "chat_topPanelBackground", "musicPicker_buttonBackground",
         "chats_archiveBackground", "chat_unreadMessagesStartBackground",
         "key_chat_messagePanelVoiceLockBackground"], surface, a_bg)
    put(["dialogBackground", "graySection", "player_background",
         "inappPlayerBackground", "profile_actionBackground",
         "actionBarDefaultSubmenuBackground", "sharedMedia_actionMode",
         "returnToCallBackground", "player_actionBar", "player_actionBarTop"],
        bg, a_bg)
    put(["chat_messagePanelBackground", "chat_messagePanelVoiceBackground"],
        surface2, a_bg)
    put(["chat_messagePanelShadow", "key_chat_messagePanelVoiceLockShadow",
         "ChatShadow", "chat_goDownButtonShadow"], shadow, 80)
    put(["files_folderIconBackground", "chat_secretTimerBackground"], surface3, 220)
    put("chat_goDownButton", surface3, 200)
    put("chats_menuTopShadow", surface3, 60)

    # ===== Top bar =====
    put(["actionBarDefault", "actionBarDefaultArchived",
         "actionBarActionModeDefaultTop"] +
        [f"avatar_backgroundActionBar{c}" for c in AV], bar, a_bar)
    put("actionBarActionModeDefault", mix(bar, accent, 0.20), a_bar)
    put(["actionBarDefaultTitle", "actionBarDefaultArchivedTitle",
         "actionBarDefaultSearch", "actionBarDefaultArchivedSearch",
         "actionBarActionModeDefaultTitle", "player_actionBarTitle"], bar_text)
    put(["actionBarDefaultSubtitle", "actionBarDefaultSearchPlaceholder",
         "actionBarDefaultSearchArchivedPlaceholder", "player_actionBarSubtitle",
         "actionBarTabText"], bar_sub)
    put(["actionBarDefaultIcon", "actionBarDefaultArchivedIcon",
         "actionBarActionModeDefaultIcon", "player_actionBarItems"] +
        [f"avatar_actionBarIcon{c}" for c in AV], bar_icon)
    put(["actionBarDefaultSelector", "actionBarActionModeDefaultSelector",
         "actionBarWhiteSelector", "player_actionBarSelector"] +
        [f"avatar_actionBarSelector{c}" for c in AV], sel_ov, 70)
    put("actionBarTabLine", accent)
    put("actionBarTabActiveText", ensure_contrast(accent, bar))
    put(["actionBarDefaultSubmenuItem", "chats_menuItemText"], text)
    put(["actionBarDefaultSubmenuItemIcon", "chats_menuItemIcon"], gray2)

    # ===== Avatars =====
    put([f"avatar_background{c}" for c in AV] +
        ["avatar_backgroundSaved", "avatar_backgroundArchived",
         "avatar_backgroundGroupCreateSpanBlue"], accent)
    put([f"avatar_nameInMessage{c}" for c in AV], acc_text)
    put([f"avatar_subtitleInProfile{c}" for c in AV], gray2)
    put([f"avatar_backgroundInProfile{c}" for c in AV], mix(accent, bg, 0.15))
    put("avatar_text", on_acc)

    # ===== Incoming bubble =====
    put("chat_inBubble", inb, a_in)
    put("chat_inBubbleSelected", in_sel, max(a_in, 170))
    put("chat_inBubbleShadow", shadow)
    put("chat_messageTextIn", in_text, a_text)
    put(["chat_inTimeText", "chat_inTimeSelectedText"], in_time)
    put(["chat_inReplyLine", "chat_inReplyNameText"], acc_in)
    put(["chat_inReplyMessageText", "chat_inReplyMediaMessageText",
         "chat_inReplyMediaMessageSelectedText"], mix(in_text, inb, 0.30))
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
    put(["chat_inVenueInfoText", "chat_inVenueInfoSelectedText"], in_time)
    put(["chat_inAudioSeekbar", "chat_inVoiceSeekbar", "chat_inAudioSeekbarSelected",
         "chat_inVoiceSeekbarSelected"], mix(in_text, inb, 0.55))
    put(["chat_inAudioSeekbarFill", "chat_inVoiceSeekbarFill"], in_text)
    put(["chat_inAudioProgress", "chat_inAudioSelectedProgress",
         "chat_inFileProgress", "chat_inFileProgressSelected"],
        mix(in_text, inb, 0.25))
    put(["chat_inFileBackground", "chat_inFileBackgroundSelected",
         "chat_inLocationBackground", "chat_inContactBackground",
         "chat_inLoaderPhoto", "chat_inLoaderPhotoSelected"],
        mix(inb, in_text, 0.06))
    put(["chat_inLoaderPhotoIcon", "chat_inLoaderPhotoIconSelected"], in_text)
    put("chat_inLoaderSelected", mix(in_text, inb, 0.45))

    # ===== Outgoing bubble =====
    put("chat_outBubble", outb, a_out)
    put("chat_outBubbleSelected", out_sel, max(a_out, 170))
    put(["chat_outBubbleGradient", "chat_outBubbleGradient2",
         "chat_outBubbleGradient3"], outb, a_out)
    put("chat_outBubbleShadow", shadow)
    put("chat_messageTextOut", out_text, a_text)
    put(["chat_outTimeText", "chat_outTimeSelectedText"], out_time)
    put(["chat_outReplyLine", "chat_outReplyNameText"], acc_out)
    put(["chat_outReplyMessageText", "chat_outReplyMediaMessageText",
         "chat_outReplyMediaMessageSelectedText"], mix(out_text, outb, 0.30))
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
         "chat_outLocationIcon", "chat_outAudioPerfomerText",
         "chat_outAudioPerformerSelectedText"], out_text, a_text)
    put(["chat_outVenueInfoText", "chat_outVenueInfoSelectedText"], out_time)
    put(["chat_outSentCheck", "chat_outSentCheckSelected"], mix(out_text, outb, 0.20))
    put(["chat_outSentCheckRead", "chat_outSentCheckReadSelected"],
        mix(out_text, outb, 0.08))
    put("chat_mediaSentCheck", mix(out_text, outb, 0.15))
    put(["chat_outAudioSeekbar", "chat_outVoiceSeekbar",
         "chat_outAudioSeekbarSelected", "chat_outVoiceSeekbarSelected"],
        mix(out_text, outb, 0.55))
    put(["chat_outAudioSeekbarFill", "chat_outVoiceSeekbarFill"], out_text)
    put(["chat_outAudioProgress", "chat_outAudioSelectedProgress",
         "chat_outFileProgress", "chat_outFileProgressSelected"],
        mix(out_text, outb, 0.25))
    put(["chat_outFileBackground", "chat_outFileBackgroundSelected",
         "chat_outLocationBackground", "chat_outContactBackground",
         "chat_outLoaderPhoto", "chat_outLoaderPhotoSelected"],
        mix(outb, out_text, 0.06))
    put(["chat_outLoaderPhotoIcon", "chat_outLoaderPhotoIconSelected"], out_text)
    put("chat_outLoaderSelected", mix(out_text, outb, 0.45))

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
         "windowBackgroundWhiteGrayText1"], gray1, a_text)
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
         "chat_unreadMessagesStartArrowIcon"], gray2, a_text)
    put(["windowBackgroundWhiteGrayText", "windowBackgroundWhiteGrayIcon",
         "windowBackgroundWhiteIcon", "stickers_menu",
         "chat_emojiPanelStickerSetName", "dialogIcon", "dialogSearchIcon",
         "dialogSearchHint", "chat_searchPanelIcons",
         "chat_messagePanelIcons", "chat_messagePanelVoiceDelete",
         "chat_messagePanelVoiceDuration", "chat_messagePanelCancelInlineBot",
         "chat_replyPanelIcons", "chat_recordTime", "chat_recordVoiceCancel",
         "chat_topPanelClose", "chat_topPanelLine", "chat_secretTimeText",
         "windowBackgroundWhiteHintText"], gray3, a_text)
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
         "chat_adminText", "chat_adminSelectedText", "chat_botButtonText",
         "chat_botSwitchToInlineText", "dialogInputFieldActivated",
         "groupcreate_cursor", "groupcreate_onlineText",
         "chat_messagePanelSend", "chat_unreadMessagesStartText",
         "chat_editDoneIcon", "chat_stickerReplyNameText", "chats_sentCheck",
         "chats_sentClock", "chats_sentReadCheck",
         "sharedMedia_startStopLoadIcon", "PreviewBack",
         "PreviewBackLinear"], acc_text, a_acc)
    put(["chats_unreadCounter", "chats_verifiedBackground",
         "chats_archivePinBackground", "chats_actionBackground",
         "chat_attachGalleryBackground", "chat_attachVideoBackground",
         "chat_attachAudioBackground", "chat_attachFileBackground",
         "chat_attachContactBackground", "chat_attachLocationBackground",
         "chat_attachHideBackground", "chat_attachSendBackground",
         "chat_attachMediaBanBackground", "undo_background", "picker_badge",
         "dialogBadgeBackground", "dialogFloatingButton", "checkbox",
         "checkboxSquareBackground", "dialogCheckboxSquareBackground",
         "radioBackgroundChecked", "dialogRadioBackgroundChecked",
         "dialogRoundCheckBox", "switchTrackChecked", "switch2TrackChecked",
         "dialogLineProgress", "dialogProgressCircle", "progressCircle",
         "contextProgressInner1", "contextProgressOuter1",
         "featuredStickers_addButton", "location_sendLocationBackground",
         "location_sendLiveLocationBackground",
         "location_placeLocationBackground", "chat_messagePanelVoicePressed",
         "chat_goDownButtonCounterBackground", "chat_botProgress"],
        accent, a_acc)
    put(["chats_unreadCounterText", "chats_verifiedCheck", "picker_badgeText",
         "dialogBadgeText", "checkboxCheck", "checkboxSquareCheck",
         "dialogCheckboxSquareCheck", "dialogRoundCheckBoxCheck",
         "dialogFloatingIcon", "featuredStickers_buttonText", "files_iconText",
         "undo_cancelColor", "undo_infoColor", "chat_attachGalleryIcon",
         "chat_attachVideoIcon", "chat_attachFileIcon",
         "chat_attachContactIcon", "chat_attachLocationIcon",
         "chat_attachHideIcon", "chat_attachSendIcon",
         "chat_attachMediaBanText", "chat_attachCameraIcon1",
         "chat_attachCameraIcon2", "chat_attachCameraIcon3",
         "chat_attachCameraIcon4", "chat_attachCameraIcon5",
         "chat_attachCameraIcon6", "location_sendLocationIcon",
         "musicPicker_checkboxCheck", "groupcreate_checkboxCheck",
         "chats_menuItemCheck", "chats_actionIcon"], on_acc)
    put(["chats_actionPressedBackground", "dialogFloatingButtonPressed",
         "featuredStickers_addButtonPressed"], mix(accent, on_acc, 0.25))

    # ===== Switches / checkboxes =====
    put(["switchTrack", "switch2Track"], mix(bg, text, 0.25), 140)
    put(["switchThumb", "switchThumbChecked", "switch2Thumb",
         "switch2ThumbChecked"], (252, 253, 255))
    put(["checkboxSquareUnchecked", "dialogCheckboxSquareUnchecked",
         "radioBackground", "dialogRadioBackground"], mix(bg, text, 0.30))
    put("checkboxSquareDisabled", mix(bg, text, 0.15))
    put(["switch2Check", "musicPicker_checkbox", "groupcreate_checkbox"], accent)

    # ===== Dialogs / selectors =====
    put("dialogButton", acc_text)
    put("dialogButtonSelector", accent, 60)
    put(["dialogLineProgressBackground", "dialogSearchBackground"],
        mix(bg, text, 0.15), 60)
    put("dialogScrollGlow", gray2)
    put("dialogLinkSelection", LINK_BLUE, 60)
    put("windowBackgroundWhiteLinkSelection", LINK_BLUE, 60)
    put("listSelectorSDK21", sel_ov, 60)
    put("stickers_menuSelector", sel_ov, 60)
    put("chat_emojiPanelIconSelector", sel_ov, 60)
    put("chat_emojiPanelStickerPackSelector", sel_ov, 60)
    put("chat_selectedBackground", accent, 90)
    put("chat_textSelectBackground", accent, 100)
    put("chat_linkSelectBackground", LINK_BLUE, 50)
    put("chats_tabletSelectedOverlay", sel_ov, 100)

    # ===== Service chip =====
    put(["chat_serviceBackground", "chat_serviceBackgroundSelected"],
        mix(bg, text, 0.45), 160)
    put(["chat_serviceText", "chat_serviceIcon"], text)

    # ===== Bots / emoji panel =====
    put("chat_botKeyboardButtonBackground", mix(bg, text, 0.08))
    put("chat_botKeyboardButtonBackgroundPressed", mix(bg, text, 0.16))
    put("chat_botKeyboardButtonText", text)
    put("chat_emojiPanelBadgeBackground", accent)
    put("chat_emojiPanelBadgeText", on_acc)
    put(["chat_emojiPanelIconSelected", "chat_emojiPanelMasksIconSelected",
         "chat_emojiPanelMasksIcon"], accent)
    put("chat_emojiPanelNewTrending", red)
    put("chat_emojiPanelTrendingDescription", gray2)
    put(["chat_emojiPanelStickerSetNameIcon", "chat_stickerViaBotNameText",
         "chat_stickerReplyLine", "chat_stickerReplyMessageText"], gray2)

    # ===== Player =====
    put("player_progress", accent)
    put("player_progressBackground", mix(bg, text, 0.20))
    put(["player_button", "player_placeholder"], mix(text, bg, 0.25))
    put("player_buttonActive", acc_text)
    put("inappPlayerTitle", text)
    put(["inappPlayerPlayPause", "inappPlayerClose"], acc_text)

    # ===== Red / green semantics =====
    put(["chats_draft", "chat_reportSpam", "chat_sentError", "chats_sentError",
         "dialogTextRed", "windowBackgroundWhiteRedText",
         "windowBackgroundWhiteRedText2", "windowBackgroundWhiteRedText3",
         "windowBackgroundWhiteRedText4", "windowBackgroundWhiteRedText5",
         "windowBackgroundWhiteRedText6", "chat_sentErrorIcon",
         "chats_sentErrorIcon", "calls_callReceivedRedIcon"], red)
    put(["windowBackgroundWhiteGreenText2", "featuredStickers_addedIcon",
         "calls_callReceivedGreenIcon"], green)
    put("chats_unreadCounterMuted", gray2)
    put("chats_unreadCounterMutedText", text)

    # ===== Wallpaper =====
    # image → all color lines, then "WPS", then raw image bytes
    # flat  → chat_wallpaper=<signed_argb color>
    lines = [f"{k}={signed_argb(rgb[0], rgb[1], rgb[2], a)}"
             for k, (rgb, a) in M.items()]
    text_part = ("\n".join(lines) + "\n").encode("utf-8")

    if wallpaper:
        return text_part + b"\nWPS\n" + wallpaper

    r, g, b = wall_flat if wall_flat else mix(bg, text, 0.02)
    return text_part + f"chat_wallpaper={signed_argb(r, g, b, 255)}\n".encode("utf-8")
