import threading
from flask import Flask, jsonify
from config import PORT

app = Flask(__name__)

@app.route("/")
def home():
    return "✅ Material You Theme Bot is running!"

@app.route("/health")
def health():
    return jsonify(status="ok")

def _run():
    app.run(host="0.0.0.0", port=PORT)

def start_server():
    threading.Thread(target=_run, daemon=True).start()
