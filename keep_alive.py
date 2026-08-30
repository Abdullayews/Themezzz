import logging
import os
import threading
import time
import urllib.request

logger = logging.getLogger("keep_alive")

PING_INTERVAL = 540  # 9 min — Render spins down after 15 min without traffic


def _ping_loop():
    time.sleep(60)  # let the Flask server boot first
    url = os.getenv("RENDER_EXTERNAL_URL", "").rstrip("/")
    if not url:
        logger.info("RENDER_EXTERNAL_URL not set — self-ping disabled")
        return
    while True:
        try:
            with urllib.request.urlopen(url + "/", timeout=30) as r:
                logger.info(f"🔔 self-ping → {r.status}")
        except Exception as e:
            logger.warning(f"self-ping failed: {e}")
        time.sleep(PING_INTERVAL)


def start_keep_alive():
    """Keeps the Render free instance awake 24/7 (no extra dependencies)."""
    threading.Thread(target=_ping_loop, daemon=True).start()
