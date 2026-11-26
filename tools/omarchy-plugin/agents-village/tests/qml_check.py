#!/usr/bin/env python3
"""Load the real Panel.qml in Quickshell, offscreen, and check its decisions.

tests/qml/shell.qml drives the widget with the recorded server answers in
tests/fixtures: which view it opens on, picking a swarm, going back, the
badge, an unreachable server. A QML error (a typo, a missing import, a
property Omarchy's Ui does not have) fails here instead of in the bar.

Needs quickshell and Omarchy's shell modules (qs.Commons, qs.Ui), which the
harness links in. OMARCHY_SHELL points elsewhere than /usr/share/omarchy/shell.
"""
import json, os, re, shutil, subprocess, tempfile, threading
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SHELL = Path(os.environ.get("OMARCHY_SHELL", "/usr/share/omarchy/shell"))

# A check that cannot run fails: a skip that reads as a pass would let QML errors ship.
if shutil.which("quickshell") is None:
    raise SystemExit("quickshell is not installed; the widget cannot be checked")
if not SHELL.is_dir():
    raise SystemExit(f"Omarchy's shell modules are not at {SHELL}; set OMARCHY_SHELL")

fixture = lambda name: json.loads((ROOT / "tests/fixtures" / name).read_text())
swarms, villages = fixture("swarms.json"), fixture("village.json")
# Recorded while CI ran on master (#413's `build`), and the same village red.
build_running = fixture("village-build-running.json")
build_red = json.loads(json.dumps(build_running))
build_red["villages"][0]["build"] = {"state": "red", "since": "2026-11-09T20:40:00Z", "tests": ["a", "b", "c"],
                                     "url": "https://ci/run/1", "jobs": [{"name": "test (macos-latest)", "os": "macOS", "state": "red"}]}
two_swarms = json.loads(json.dumps(swarms))
two_swarms["swarms"].append({"project_name": "shop", "workflow": "Modernize", "agent_type": "claude", "stopped": False,
                             "busy_count": 0, "attention_count": 1, "workers": [{"role": "worker-1"}]})
two_villages = json.loads(json.dumps(villages))
two_villages["villages"].append({"project": "shop", "framework": "modernize", "board": [], "backlog": [],
                                 "houses": [{"id": "/src/shop-wt-1", "role": "worker-1", "status": "blocked", "links": []}],
                                 "pipeline": {"quay": [], "stops": []}})

env = dict(os.environ, QT_QPA_PLATFORM="offscreen")
for name in ("WAYLAND_DISPLAY", "DISPLAY", "QT_QPA_PLATFORMTHEME"):
    env.pop(name, None)

with tempfile.TemporaryDirectory(prefix="agents-village-qml-") as directory:
    harness = Path(directory)
    shutil.copy(ROOT / "tests/qml/shell.qml", harness / "shell.qml")
    shutil.copy(ROOT / "Model.js", harness / "Model.js")
    # Under another name: Panel.qml extends qs.Ui's Panel, and a Panel.qml
    # beside it would resolve that name to itself.
    shutil.copy(ROOT / "Panel.qml", harness / "BarPanel.qml")
    (harness / "Fixtures.js").write_text("".join(
        f"var {name} = {json.dumps(json.dumps(doc))}\n"
        for name, doc in (("swarms", swarms), ("villages", villages), ("twoSwarms", two_swarms), ("twoVillages", two_villages),
                          ("buildRunning", build_running), ("buildRed", build_red))))
    (harness / "Commons").symlink_to(SHELL / "Commons")
    shutil.copytree(SHELL / "Ui", harness / "Ui", symlinks=True)
    shutil.copy(ROOT / "tests/qml/KeyboardPanel.qml", harness / "Ui/KeyboardPanel.qml")
    runtime = harness / "runtime"
    runtime.mkdir(mode=0o700)
    env["XDG_RUNTIME_DIR"] = str(runtime)
    # A HOME of its own (no desktop notifier installed), and stand-ins for
    # what the widget runs, recording their arguments: the checks must never
    # send a real notification or open a terminal (#493).
    (harness / "home").mkdir()
    env["HOME"] = str(harness / "home")
    stubs = harness / "bin"
    stubs.mkdir()
    calls = harness / "calls"
    for name in ("omarchy-notification-send", "omarchy-launch-or-focus-tui", "swarm", "omarchy-launch-browser", "omarchy-launch-webapp"):
        (stubs / name).write_text(f'#!/bin/sh\nprintf "%s\\n" "{name} $*" >> "{calls}"\n')
        (stubs / name).chmod(0o755)
    env["PATH"] = f"{stubs}:{os.environ['PATH']}"

    shell = subprocess.Popen(["quickshell", "-p", str(harness)], stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, env=env)
    timer = threading.Timer(30, shell.kill)
    timer.start()
    lines = []
    for line in shell.stdout:
        lines.append(line)
        if re.search(r"qml:\s+(ALL OK|\d+ FAILED)", re.sub(r"\x1b\[[0-9;]*m", "", line)):
            break
    timer.cancel()
    shell.kill()
    shell.wait()
    ran = calls.read_text() if calls.exists() else ""

plain = re.sub(r"\x1b\[[0-9;]*m", "", "".join(lines))
checks = re.findall(r"qml:\s+(ok|FAIL)\s+(.*?)\s*$", plain, re.MULTILINE)
failed = [name for state, name in checks if state == "FAIL"]
assert checks, "the harness produced no checks:\n" + plain[-3000:]
assert not failed, "failed: " + "; ".join(failed)
assert "ALL OK" in plain, "harness did not finish:\n" + plain[-3000:]
if os.environ.get("QML_CHECK_VERBOSE"):
    print(plain)
# QML warnings or errors about this widget's own files are bugs too.
warnings = [l for l in plain.splitlines() if re.search(r"(BarPanel|Model)\.(qml|js)", l)]
assert not warnings, "QML warnings:\n" + "\n".join(warnings)
# What the widget ran: one notification for the house that started waiting,
# the terminal for a good machine name, nothing for a bad one.
# (Earlier checks feed other recorded villages through `one`, and a conflict
# appearing there is news too; the house below is the one checked.)
notes = [l for l in ran.splitlines() if l.startswith("omarchy-notification-send ") and "wt-1: #4 Checkout" in l]
# Twice: when it starts waiting, and when it waits again after an answer
# seen between two blips. Never for still waiting, nor from the widget with
# notify off.
assert len(notes) == 2, "a notification per rising edge, no more:\n" + ran
assert notes[0].startswith("omarchy-notification-send --app-name Agents village -u critical -p "), notes[0]
assert " 1 needs you wt-1: #4 Checkout --exec " in notes[0], notes[0]
joins = [l for l in ran.splitlines() if l.startswith("omarchy-launch-or-focus-tui ")]
assert joins == ["omarchy-launch-or-focus-tui --app-id=org.omarchy.swarm-box-join swarm box"], "join runs for box only:\n" + ran
print(f'{{"qml_checks": {len(checks)}, "failed": 0}}')
