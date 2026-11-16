#!/usr/bin/env python3
"""Load the real Service.qml in Quickshell, offscreen, against a stand-in server.

It must load with Quickshell's own modules and the two .js files beside it
(Model.js, the bar widget's), read its settings, record the first poll without
a word, then send one notification for a house that starts waiting on you, and
open the village on that house when the notification is clicked. notify-send
and omarchy-launch-webapp are stand-ins that record what they were asked.

    python3 tools/omarchy-plugin/agents-village-notify/tests/qml_check.py
"""
import json, os, shutil, subprocess, tempfile, threading, time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
# A check that cannot run fails: a skip that reads as a pass would let QML errors ship.
if shutil.which("quickshell") is None:
    raise SystemExit("quickshell is not installed; the service cannot be checked")

def village(houses):
    return {"villages": [{"project": "shop", "manager": {"status": "idle", "links": []}, "houses": houses,
                          "board": [], "backlog": [], "pipeline": {"gates": [], "stops": [], "quay": []}}]}

QUIET = village([{"id": "/src/shop-wt-2", "role": "worker-2", "status": "working", "issue": 7, "links": []}])
WAITING = village([{"id": "/src/shop-wt-2", "role": "worker-2", "status": "blocked", "issue": 7, "issue_title": "Checkout",
                    "question": {"prompt": "Proceed?", "kind": "yesno", "options": []}, "links": []}])

class Server(BaseHTTPRequestHandler):
    hits = 0
    def do_GET(self):
        if self.path != "/api/village":
            self.send_error(404); return
        Server.hits += 1
        body = json.dumps(QUIET if Server.hits == 1 else WAITING).encode()
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)
    def log_message(self, *a):
        pass

HARNESS = """import QtQuick
import Quickshell
ShellRoot {
  NotifyService { id: s }
  Timer { interval: 2000; running: true; onTriggered: s.poll() }
  Timer { interval: 4000; running: true
    onTriggered: { console.log("CHECK " + JSON.stringify({ server: s.settings.server, interval: s.settings.interval,
      quiet: s.settings.quiet, sent: s.sent })); Qt.quit() } }
}
"""

def main():
    httpd = ThreadingHTTPServer(("127.0.0.1", 0), Server)
    threading.Thread(target=httpd.serve_forever, daemon=True).start()
    server = f"http://127.0.0.1:{httpd.server_address[1]}"
    # Short paths: a Wayland socket path must fit in 108 bytes.
    with tempfile.TemporaryDirectory(prefix="avn-", dir="/tmp") as d:
        d = Path(d)
        cfg = d / "home/.config/agents-ui"
        cfg.mkdir(parents=True)
        (cfg / "notify.json").write_text(json.dumps({"server": server + "/", "interval": 60}))
        # Quiet hours that never cover the run, whatever the time.
        h = time.localtime().tm_hour
        dim = f"{(h + 2) % 24}-{(h + 3) % 24}"
        (cfg / "screensaver.json").write_text(json.dumps({"server": "http://elsewhere:1", "dim": dim}))
        shell = d / "shell"
        shell.mkdir()
        shutil.copy(ROOT / "Service.qml", shell / "NotifyService.qml")
        shutil.copy(ROOT / "Notify.js", shell / "Notify.js")
        shutil.copy(ROOT / "Model.js", shell / "Model.js")
        (shell / "shell.qml").write_text(HARNESS)
        bin_ = d / "bin"
        bin_.mkdir()
        log = d / "calls"
        for name, says in (("notify-send", "default"), ("omarchy-launch-webapp", "")):
            p = bin_ / name
            p.write_text(f'#!/bin/sh\nprintf "%s\\n" "{name} $*" >> "{log}"\n' + (f'echo {says}\n' if says else ""))
            p.chmod(0o755)
        runtime = d / "rt"
        runtime.mkdir(mode=0o700)
        env = dict(os.environ, QT_QPA_PLATFORM="offscreen", HOME=str(d / "home"), XDG_RUNTIME_DIR=str(runtime),
                   PATH=f"{bin_}:{os.environ['PATH']}")
        for name in ("WAYLAND_DISPLAY", "DISPLAY", "QT_QPA_PLATFORMTHEME"):
            env.pop(name, None)
        out = subprocess.run(["quickshell", "-p", str(shell)], capture_output=True, text=True, env=env, timeout=30)
        text = out.stdout + out.stderr
        lines = [l for l in text.splitlines() if "CHECK " in l]
        assert lines, f"Service.qml did not load:\n{text}"
        got = json.loads(lines[-1].split("CHECK ", 1)[1])
        assert got["server"] == server, got
        assert got["interval"] == 60 and got["quiet"] == dim, got
        assert Server.hits >= 2, f"polled {Server.hits} times\n{text}"
        assert got["sent"] == 1, f"one notification, for the house that started waiting: {got}\n{text}"
        # The detached opener may land a moment after quickshell exits.
        for _ in range(20):
            calls = log.read_text() if log.exists() else ""
            if "omarchy-launch-webapp" in calls:
                break
            time.sleep(0.1)
        assert "notify-send --app-name=Agents village" in calls and "shop: wt-2 is waiting on you #7 Checkout\nProceed?" in calls, calls
        assert f"omarchy-launch-webapp {server}/village#shop&house=wt-2" in calls, calls
    httpd.shutdown()
    print("Service.qml: loads, reads its settings, notifies on the change only, opens the house it is about")

main()
