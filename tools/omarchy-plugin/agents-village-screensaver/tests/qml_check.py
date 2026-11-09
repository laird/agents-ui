#!/usr/bin/env python3
"""Load the real Service.qml in Quickshell, offscreen, and check what it read.

It must load with Quickshell's own modules (a typo or a missing import fails
here, not in the shell), find village-screensaver beside itself, and take its
idle timeout from ~/.config/agents-ui/screensaver.json (150 without one).
There is no compositor offscreen, so the idle monitor itself never fires.

    python3 tools/omarchy-plugin/agents-village-screensaver/tests/qml_check.py
"""
import json, os, shutil, subprocess, tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
# A check that cannot run fails: a skip that reads as a pass would let QML errors ship.
if shutil.which("quickshell") is None:
    raise SystemExit("quickshell is not installed; the service cannot be checked")

HARNESS = """import QtQuick
import Quickshell
ShellRoot {
  SaverService { id: s }
  Timer { interval: 1500; running: true
    onTriggered: { console.log("CHECK " + JSON.stringify({ timeout: s.timeout, launcher: s.launcher })); Qt.quit() } }
}
"""

def run(settings):
    # Short paths: a Wayland socket path must fit in 108 bytes.
    with tempfile.TemporaryDirectory(prefix="avs-", dir="/tmp") as d:
        d = Path(d)
        (d / "home/.config/agents-ui").mkdir(parents=True)
        if settings is not None:
            (d / "home/.config/agents-ui/screensaver.json").write_text(json.dumps(settings))
        shell = d / "shell"
        shell.mkdir()
        shutil.copy(ROOT / "Service.qml", shell / "SaverService.qml")
        shutil.copy(ROOT / "village-screensaver", shell / "village-screensaver")
        (shell / "shell.qml").write_text(HARNESS)
        runtime = d / "rt"
        runtime.mkdir(mode=0o700)
        env = dict(os.environ, QT_QPA_PLATFORM="offscreen", HOME=str(d / "home"), XDG_RUNTIME_DIR=str(runtime))
        for name in ("WAYLAND_DISPLAY", "DISPLAY", "QT_QPA_PLATFORMTHEME"):
            env.pop(name, None)
        out = subprocess.run(["quickshell", "-p", str(shell)], capture_output=True, text=True, env=env, timeout=30)
        text = out.stdout + out.stderr
        lines = [l for l in text.splitlines() if "CHECK " in l]
        assert lines, f"Service.qml did not load:\n{text}"
        got = json.loads(lines[-1].split("CHECK ", 1)[1])
        assert got["launcher"] == str(shell / "village-screensaver"), got
        return got

assert run(None)["timeout"] == 150, "default timeout"
assert run({"timeout": 42})["timeout"] == 42, "timeout from the settings file"
assert run({"timeout": 0})["timeout"] == 150, "a zero timeout falls back to 150"
print("Service.qml: loads, finds its launcher, reads its timeout")
