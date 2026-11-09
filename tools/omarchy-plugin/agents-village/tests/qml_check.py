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
        for name, doc in (("swarms", swarms), ("villages", villages), ("twoSwarms", two_swarms), ("twoVillages", two_villages))))
    (harness / "Commons").symlink_to(SHELL / "Commons")
    shutil.copytree(SHELL / "Ui", harness / "Ui", symlinks=True)
    shutil.copy(ROOT / "tests/qml/KeyboardPanel.qml", harness / "Ui/KeyboardPanel.qml")
    runtime = harness / "runtime"
    runtime.mkdir(mode=0o700)
    env["XDG_RUNTIME_DIR"] = str(runtime)

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
print(f'{{"qml_checks": {len(checks)}, "failed": 0}}')
