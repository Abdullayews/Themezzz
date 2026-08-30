import colorsys
import io

from PIL import Image

WHITE = (255, 255, 255)
GREEN = (98, 195, 122)    # fixed semantic icons only (call log, added sticker)
RED = (232, 95, 88)

AV = ["Blue", "Cyan", "Green", "Orange", "Pink", "Red", "Violet"]

FALLBACK_SWATCHES = ["#17212b", "#2b5278", "#6ab3f3", "#26a69a", "#ef6c61", "#7e57c2"]

# Starting preset (Telegram dark) — visible & editable; the user overrides freely.
DEFAULT_CATS = {
    "bg":     {"hex": "#17212b", "alpha": 0},
    "bar":    {"hex": "#17212b", "alpha": 0},
    "in":     {"hex": "#182533", "alpha": 0},
    "out":    {"hex": "#2b5278", "alpha": 0},
    "link":   {"hex": "#6ab3f3", "alpha": 0},
    "accent": {"hex": "#6ab3f3", "alpha": 0},
}
DEFAULT_WALL = {"hex": "#0e1621", "alpha": 0}


# ---------- Helpers ----------

def hex_to_rgb(h: str):
    h = h.lstrip("#")
    return tuple(int(h[i:i + 2], 16) for i in (0, 2, 4))


def rgb_to_hex(c) -> str:
    return "#%02x%02x%02x" % tuple(c)


def signed_argb(r: int, g: int, b: int, a: int = 255) -> int:
    v = (a << 24) | (r << 16) | (g << 8) | b
    return v - 0x100000000 if v > 0x7FFFFFFF else v


# ---------- Swatch suggestions from photo (never auto-applied) ----------

def extract_palette(data: bytes, count: int = 6) -> list:
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
            return [rgb_to_hex(rgb) for *_, rgb in res]
    except Exception:
        pass
    return list(FALLBACK_SWATCHES)


# ---------- Wallpaper (used exactly as the user sent it) ----------

def prepare_wallpaper(data: bytes, max_side: int = 1080) -> bytes:
    img = Image.open(io.BytesIO(data)).convert("RGB")
    img.thumbnail((max_side, max_side), Image.Resampling.LANCZOS)
    buf = io.BytesIO()
    img.save(buf, "PNG", optimize=True)
    return buf.getvalue()


# ---------- .attheme generator ----------
# Rule: every color below is either (a) the user's pick for that category,
# (b) WHITE (owner's rule: all text white, links excepted), or
# (c) a fixed semantic icon color. No derivation, no blending, no M3.

def build_attheme(cats: dict, wallpaper: bytes | None = None,
                  wall_hex: str = "#0e1621", wall_alpha: int = 0) -> bytes:
    C = {k: hex_to_rgb(cats[k]["hex"]) for k in cats}
    A = {k: round(255 * (1 - cats[k]["alpha"] / 100)) for k in cats}
    bg, bar, inb, outb = C["bg"], C["bar"], C["in"], C["out"]
    link, acc = C["link"], C["accent"]
    a_bg, a_bar, a_in, a_out, a_acc = (A["bg"], A["bar"], A["in"],
                                       A["out"], A["accent"])
    W = WHITE

    M = {}

    def put(keys, rgb, alpha=255):
        if isinstance(keys, str):
            keys = [keys]
        for k in keys:
            M[k] = (rgb, alpha)

    # ================= BG category =================
    put(["windowBackgroundWhite", "windowBackgroundGray", "windowBackgroundBlack",
         "graySection", "dialogBackground", "dialogBackgroundGray",
         "chats_menuBackground", "chats_pinnedOverlay",
         "chat_messagePanelBackground", "chat_messagePanelVoiceBackground",
         "chat_emojiPanelBackground", "chat_stickersHintPanel",
         "chat_botKeyboardButtonBackground", "chat_recordedVoiceBackground",
         "chat_topPanelBackground", "player_background", "player_placeholderBackground",
         "inappPlayerBackground", "profile_actionBackground",
         "profile_actionPressedBackground", "contacts_inviteBackground",
         "files_folderIconBackground", "musicPicker_buttonBackground"],
        bg, a_bg)
    put(["switchTrack", "switch2Track"], bg, 150)
    put(["chat_serviceBackground", "chat_serviceBackgroundSelected"],
        bg, max(a_bg, 160))

    # ================= Bar category =================
    put(["actionBarDefault", "actionBarDefaultArchived",
         "actionBarActionModeDefault", "actionBarActionModeDefaultTop",
         "player_actionBar", "player_actionBarTop",
         "actionBarDefaultSubmenuBackground", "sharedMedia_actionMode",
         "returnToCallBackground"] + [f"avatar_backgroundActionBar{c}" for c in AV],
        bar, a_bar)

    # ================= In bubble category =================
    put(["chat_inBubble", "chat_inBubbleSelected",
         "chat_inFileBackground", "chat_inFileBackgroundSelected",
         "chat_inLocationBackground", "chat_inContactBackground",
         "chat_inLoaderPhoto", "chat_inLoaderPhotoSelected"], inb, a_in)

    # ================= Out bubble category =================
    put(["chat_outBubble", "chat_outBubbleSelected",
         "chat_outBubbleGradient", "chat_outBubbleGradient2",
         "chat_outBubbleGradient3",
         "chat_outFileBackground", "chat_outFileBackgroundSelected",
         "chat_outLocationBackground", "chat_outContactBackground",
         "chat_outLoaderPhoto", "chat_outLoaderPhotoSelected"], outb, a_out)

    # ================= Accent category =================
    put(["actionBarTabLine",
         "chats_unreadCounter", "chats_verifiedBackground",
         "chats_archivePinBackground", "chats_actionBackground",
         "chats_actionPressedBackground",
         "chat_attachGalleryBackground", "chat_attachVideoBackground",
         "chat_attachAudioBackground", "chat_attachFileBackground",
         "chat_attachContactBackground", "chat_attachLocationBackground",
         "chat_attachHideBackground", "chat_attachSendBackground",
         "chat_attachMediaBanBackground",
         "undo_background", "picker_badge", "picker_enabledButton",
         "dialogBadgeBackground", "dialogFloatingButton",
         "dialogFloatingButtonPressed",
         "checkbox", "checkboxSquareBackground",
         "dialogCheckboxSquareBackground",
         "radioBackgroundChecked", "dialogRadioBackgroundChecked",
         "dialogRoundCheckBox",
         "switchTrackChecked", "switch2TrackChecked",
         "dialogLineProgress", "dialogProgressCircle", "progressCircle",
         "contextProgressInner1", "contextProgressOuter1",
         "featuredStickers_addButton", "featuredStickers_addButtonPressed",
         "location_sendLocationBackground", "location_sendLiveLocationBackground",
         "location_placeLocationBackground", "location_liveLocationProgress",
         "chat_messagePanelVoicePressed", "chat_messagePanelSend",
         "chat_goDownButton", "chat_goDownButtonCounterBackground",
         "fastScrollActive", "windowBackgroundWhiteInputFieldActivated",
         "dialogInputFieldActivated", "player_progress",
         "chat_emojiPanelBadgeBackground", "chat_emojiPanelIconSelected",
         "chat_emojiPanelMasksIconSelected", "chat_emojiPanelNewTrending",
         "chat_selectedBackground", "chat_textSelectBackground",
         "musicPicker_checkbox", "groupcreate_checkbox",
         "groupcreate_spanBackground", "groupcreate_cursor"],
        acc, a_acc)
    put(["chat_inReplyLine", "chat_outReplyLine", "chat_stickerReplyLine"], acc)

    # ================= Link category (the ONLY colored text) =================
    put(["chat_messageLinkIn", "chat_messageLinkOut",
         "windowBackgroundWhiteLinkText", "dialogTextLink", "chat_serviceLink"],
        link)
    put(["windowBackgroundWhiteLinkSelection", "dialogLinkSelection",
         "chat_linkSelectBackground"], link, 60)

    # ================= EVERYTHING TEXTUAL → WHITE =================
    put(["windowBackgroundWhiteBlackText",
         "windowBackgroundWhiteGrayText", "windowBackgroundWhiteGrayText2",
         "windowBackgroundWhiteGrayText3", "windowBackgroundWhiteGrayText4",
         "windowBackgroundWhiteGrayText8",
         "windowBackgroundWhiteRedText", "windowBackgroundWhiteRedText2",
         "windowBackgroundWhiteRedText3", "windowBackgroundWhiteRedText4",
         "windowBackgroundWhiteRedText5", "windowBackgroundWhiteRedText6",
         "windowBackgroundWhiteGreenText2",
         "windowBackgroundWhiteBlueText", "windowBackgroundWhiteBlueText3",
         "windowBackgroundWhiteBlueText4", "windowBackgroundWhiteBlueText6",
         "windowBackgroundWhiteBlueText7", "windowBackgroundWhiteBlueHeader",
         "windowBackgroundWhiteValueText", "windowBackgroundWhiteHintText",
         "windowBackgroundWhiteIcon", "windowBackgroundWhiteGrayIcon",
         "emptyListPlaceholder", "fastScrollText",
         "dialogTextBlack", "dialogTextGray", "dialogTextGray2", "dialogTextRed",
         "dialogTextBlue", "dialogTextBlue2", "dialogTextBlue3", "dialogTextBlue4",
         "dialogButton", "dialogIcon", "dialogSearchIcon", "dialogSearchHint",
         "dialogSearchText",
         "chats_name", "chats_nameIcon", "chats_nameArchived", "chats_nameMessage",
         "chats_nameMessage_threeLines", "chats_nameMessageArchived",
         "chats_nameMessageArchived_threeLines", "chats_secretName",
         "chats_message", "chats_message_threeLines", "chats_date", "chats_draft",
         "chats_muteIcon", "chats_pinnedIcon", "chats_secretIcon",
         "chats_mentionIcon", "chats_attachMessage", "chats_actionMessage",
         "chats_menuName", "chats_menuPhone", "chats_menuPhoneCats",
         "chats_menuCloud", "chats_menuCloudBackgroundCats",
         "chats_menuItemText", "chats_menuItemIcon", "chats_menuItemCheck",
         "chats_archiveText", "chats_archiveIcon",
         "chats_sentCheck", "chats_sentClock", "chats_sentReadCheck",
         "chats_sentError", "chats_sentErrorIcon",
         "chat_messageTextIn", "chat_messageTextOut",
         "chat_inTimeText", "chat_inTimeSelectedText",
         "chat_outTimeText", "chat_outTimeSelectedText",
         "chat_outSentCheck", "chat_outSentCheckSelected",
         "chat_outSentCheckRead", "chat_outSentCheckReadSelected",
         "chat_outSentClock", "chat_outSentClockSelected",
         "chat_mediaSentCheck", "chat_mediaTimeText",
         "chat_mediaLoaderPhotoIcon", "chat_mediaLoaderPhotoIconSelected",
         "chat_inAudioDurationText", "chat_inAudioDurationSelectedText",
         "chat_inAudioPerfomerText", "chat_inAudioPerfomerSelectedText",
         "chat_inAudioPerformerSelectedText", "chat_inAudioTitleText",
         "chat_inContactNameText", "chat_inContactPhoneText",
         "chat_inFileInfoText", "chat_inFileInfoSelectedText",
         "chat_inFileNameText", "chat_inForwardedNameText",
         "chat_inPreviewInstantText", "chat_inPreviewInstantSelectedText",
         "chat_inReplyMediaMessageText", "chat_inReplyMediaMessageSelectedText",
         "chat_inReplyMessageText", "chat_inReplyNameText",
         "chat_inSiteNameText", "chat_inVenueInfoText",
         "chat_inVenueInfoSelectedText", "chat_inVenueNameText",
         "chat_inViaBotNameText", "chat_inViews", "chat_inViewsSelected",
         "chat_inMenu", "chat_inMenuSelected", "chat_inInstant",
         "chat_inInstantSelected", "chat_inSentClock", "chat_inSentClockSelected",
         "chat_inFileIcon", "chat_inFileSelectedIcon", "chat_inContactIcon",
         "chat_inPreviewLine", "chat_inLoader", "chat_inLoaderSelected",
         "chat_inLoaderPhotoIcon", "chat_inLoaderPhotoIconSelected",
         "chat_inLocationIcon",
         "chat_inAudioSeekbarFill", "chat_inVoiceSeekbarFill",
         "chat_inAudioProgress", "chat_inAudioSelectedProgress",
         "chat_inFileProgress", "chat_inFileProgressSelected",
         "chat_outAudioDurationText", "chat_outAudioDurationSelectedText",
         "chat_outAudioPerfomerText", "chat_outAudioPerformerSelectedText",
         "chat_outAudioTitleText", "chat_outContactNameText",
         "chat_outContactPhoneText", "chat_outFileInfoText",
         "chat_outFileInfoSelectedText", "chat_outFileNameText",
         "chat_outForwardedNameText", "chat_outPreviewInstantText",
         "chat_outPreviewInstantSelectedText",
         "chat_outReplyMediaMessageText", "chat_outReplyMediaMessageSelectedText",
         "chat_outReplyMessageText", "chat_outReplyNameText",
         "chat_outSiteNameText", "chat_outVenueInfoText",
         "chat_outVenueInfoSelectedText", "chat_outVenueNameText",
         "chat_outViaBotNameText", "chat_outViews", "chat_outViewsSelected",
         "chat_outMenu", "chat_outMenuSelected", "chat_outInstant",
         "chat_outInstantSelected", "chat_outFileIcon", "chat_outFileSelectedIcon",
         "chat_outContactIcon", "chat_outPreviewLine", "chat_outLoader",
         "chat_outAudioSeekbarFill", "chat_outVoiceSeekbarFill",
         "chat_outAudioProgress", "chat_outAudioSelectedProgress",
         "chat_outFileProgress", "chat_outFileProgressSelected",
         "chat_outLoaderPhotoIcon", "chat_outLoaderPhotoIconSelected",
         "chat_outLocationIcon",
         "chat_messagePanelText", "chat_messagePanelHint",
         "chat_messagePanelIcons", "chat_messagePanelCancelInlineBot",
         "chat_messagePanelVoiceDelete", "chat_messagePanelVoiceDuration",
         "chat_replyPanelClose", "chat_replyPanelIcons", "chat_replyPanelLine",
         "chat_replyPanelMessage", "chat_replyPanelName",
         "chat_searchPanelIcons", "chat_searchPanelText", "chat_fieldOverlayText",
         "chat_editDoneIcon", "chat_muteIcon", "chat_lockIcon", "chat_reportSpam",
         "chat_sentError", "chat_sentErrorIcon",
         "chat_serviceText", "chat_serviceIcon",
         "chat_unreadMessagesStartText",
         "chat_stickerReplyMessageText", "chat_stickerReplyNameText",
         "chat_stickerViaBotNameText",
         "chat_topPanelClose", "chat_topPanelLine", "chat_topPanelMessage",
         "chat_topPanelTitle", "chat_secretTimerText", "chat_secretTimeText",
         "chat_recordTime", "chat_recordVoiceCancel", "chat_recordedVoiceDot",
         "chat_recordedVoicePlayPause", "chat_recordedVoicePlayPausePressed",
         "chat_recordedVoiceProgress", "chat_recordedVoiceProgressInner",
         "chat_botButtonText", "chat_botKeyboardButtonText",
         "chat_botSwitchToInlineText", "chat_botProgress",
         "chat_adminText", "chat_adminSelectedText", "chat_addContact",
         "chat_status", "chat_previewDurationText", "chat_previewGameText",
         "chat_emojiPanelEmptyText", "chat_emojiPanelBackspace",
         "chat_emojiPanelIcon", "chat_emojiPanelTrendingTitle",
         "chat_emojiPanelTrendingDescription", "chat_emojiPanelStickerSetName",
         "chat_emojiPanelStickerSetNameIcon", "chat_emojiPanelMasksIcon",
         "chat_emojiPanelBadgeText",
         "player_time", "player_button", "player_buttonActive",
         "player_actionBarSubtitle", "inappPlayerTitle", "inappPlayerPerformer",
         "inappPlayerPlayPause", "inappPlayerClose",
         "profile_title", "profile_adminIcon", "profile_creatorIcon",
         "profile_actionIcon",
         "contacts_inviteText", "files_folderIcon", "files_iconText",
         "musicPicker_buttonIcon", "musicPicker_checkboxCheck",
         "groupcreate_hintText", "groupcreate_sectionText",
         "groupcreate_onlineText", "groupcreate_offlineText",
         "groupcreate_checkboxCheck", "graySectionText", "stickers_menu",
         "chats_actionIcon", "chats_verifiedCheck", "picker_badgeText",
         "dialogBadgeText", "checkboxCheck", "checkboxSquareCheck",
         "dialogCheckboxSquareCheck", "dialogRoundCheckBoxCheck",
         "dialogFloatingIcon", "featuredStickers_buttonText",
         "chat_attachGalleryIcon", "chat_attachVideoIcon",
         "chat_attachAudioIcon", "chat_attachFileIcon",
         "chat_attachContactIcon", "chat_attachLocationIcon",
         "chat_attachHideIcon", "chat_attachSendIcon", "chat_attachMediaBanText",
         "chat_attachCameraIcon1", "chat_attachCameraIcon2", "chat_attachCameraIcon3",
         "chat_attachCameraIcon4", "chat_attachCameraIcon5", "chat_attachCameraIcon6",
         "location_sendLocationIcon", "chat_goDownButtonIcon",
         "chat_goDownButtonCounter", "undo_cancelColor", "undo_infoColor",
         "changephoneinfo_image", "sessions_devicesImage",
         "key_sheet_other", "key_sheet_scrollUp",
         "key_chat_messagePanelVoiceLock", "switch2Check", "switchThumb",
         "switchThumbChecked", "switch2Thumb", "switch2ThumbChecked"], W)

    # white-on-white glyphs for unchecked/neutral controls & separators
    put(["checkboxSquareUnchecked", "dialogCheckboxSquareUnchecked",
         "radioBackground", "dialogRadioBackground"], W, 90)
    put("checkboxSquareDisabled", W, 50)
    put(["divider", "dialogGrayLine", "dialogShadowLine",
         "chat_emojiPanelShadowLine", "chats_menuTopShadow"], W, 40)
    put(["windowBackgroundWhiteInputField", "dialogInputField"], W, 80)
    put(["chat_inAudioSeekbar", "chat_inAudioSeekbarSelected",
         "chat_inVoiceSeekbar", "chat_inVoiceSeekbarSelected",
         "chat_outAudioSeekbar", "chat_outAudioSeekbarSelected",
         "chat_outVoiceSeekbar", "chat_outVoiceSeekbarSelected"], W, 60)
    put(["listSelectorSDK21", "stickers_menuSelector",
         "chat_emojiPanelIconSelector", "dialogButtonSelector",
         "actionBarDefaultSelector", "actionBarActionModeDefaultSelector",
         "actionBarWhiteSelector", "player_actionBarSelector",
         "chats_tabletSelectedOverlay", "dialogScrollGlow",
         "fastScrollInactive", "player_placeholder",
         "chat_emojiSearchBackground", "dialogSearchBackground",
         "chats_unreadCounterMuted", "dialogLineProgressBackground",
         "player_progressBackground", "chat_botKeyboardButtonBackgroundPressed",
         "picker_disabledButton"] +
        [f"avatar_actionBarSelector{c}" for c in AV], W, 60)
    put(["actionBarDefaultTitle", "actionBarDefaultArchivedTitle",
         "actionBarDefaultSearch", "actionBarDefaultArchivedSearch",
         "actionBarDefaultSearchPlaceholder",
         "actionBarDefaultSearchArchivedPlaceholder",
         "actionBarDefaultSubtitle", "actionBarActionModeDefaultTitle",
         "actionBarDefaultIcon", "actionBarDefaultArchivedIcon",
         "actionBarActionModeDefaultIcon", "player_actionBarItems",
         "player_actionBarTitle", "actionBarTabText",
         "actionBarTabActiveText", "actionBarDefaultSubmenuItem",
         "actionBarDefaultSubmenuItemIcon"] +
        [f"avatar_actionBarIcon{c}" for c in AV] +
        [f"avatar_nameInMessage{c}" for c in AV] +
        [f"avatar_subtitleInProfile{c}" for c in AV] +
        ["avatar_text"], W)

    # avatars follow the accent
    put([f"avatar_background{c}" for c in AV] +
        ["avatar_backgroundSaved", "avatar_backgroundArchived",
         "avatar_backgroundArchivedHidden", "avatar_backgroundGroupCreateSpanBlue"] +
        [f"avatar_backgroundInProfile{c}" for c in AV], acc, a_acc)

    # fixed semantic icons (not text — status colors stay meaningful)
    put("calls_callReceivedGreenIcon", GREEN)
    put("calls_callReceivedRedIcon", RED)
    put("featuredStickers_addedIcon", GREEN)

    # ================= serialize =================
    lines = [f"{k}={signed_argb(rgb[0], rgb[1], rgb[2], a)}"
             for k, (rgb, a) in M.items()]

    if wallpaper:
        # image wallpaper: sent by the user, embedded unchanged (guide format)
        text = "\n".join(lines) + "\nchat_wallpaper=-1"
        return text.encode("utf-8") + b"\nWPS" + wallpaper + b"WPE"

    # flat wallpaper: user's color + its own transparency
    r, g, b = hex_to_rgb(wall_hex)
    a = round(255 * (1 - wall_alpha / 100))
    lines.append(f"chat_wallpaper={signed_argb(r, g, b, a)}")
    return ("\n".join(lines) + "\n").encode("utf-8")
