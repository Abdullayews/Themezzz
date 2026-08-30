"""Smoke test for the dependency upgrade. No bot token needed."""
import io

from PIL import Image, ImageDraw

from colors import extract_palette, prepare_wallpaper, build_attheme
from preview import render_preview

# 1. Fake "user photo"
img = Image.new("RGB", (640, 400), (52, 120, 198))
ImageDraw.Draw(img).rectangle([120, 80, 520, 320], fill=(240, 180, 40))
buf = io.BytesIO()
img.save(buf, "PNG")
raw = buf.getvalue()

# 2. Palette extraction
palette = extract_palette(raw)
assert palette and all(c.startswith("#") and len(c) == 7 for c in palette), palette

# 3. Wallpaper prep + live preview render
wall = prepare_wallpaper(raw)
png = render_preview(palette[0], "dark", 30, "primary", 1.0, 0,
                     palette, 0, "Dark | 30% transparent | smoke test", wall)
assert png.getvalue()[:8] == b"\x89PNG\r\n\x1a\n"

# 4. .attheme generation — with and without embedded wallpaper
with_wall = build_attheme(palette[0], "dark", 30, "primary", 1.0, 0, wall)
no_wall = build_attheme(palette[0], "light", 0, "tertiary", 1.35, 1, None)
assert b"chat_inBubble=" in with_wall and b"chat_wallpaper=-1" in with_wall
assert b"chat_outBubble=" in no_wall

# 5. Flask app boots under 3.1.3
from server import app
client = app.test_client()
assert client.get("/").status_code == 200
assert client.get("/health").get_json()["status"] == "ok"

# 6. bot.py imports cleanly against PTB 22.8 (module import only)
import bot  # noqa: F401

print("✅ Smoke test passed — palette:", palette)
