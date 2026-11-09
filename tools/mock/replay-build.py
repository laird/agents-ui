#!/usr/bin/env python3
"""Build a self-contained replay page from a recording of the demo.

    python3 tools/mock/record.py tools/mock/recordings/demo.json.gz 360   # while demos run on 7930 (k8s) and 7931 (temporal)
    python3 tools/mock/replay-build.py tools/mock/recordings/demo.json.gz replay.html
    python3 tools/mock/replay-build.py tools/mock/recordings/feature.json.gz replay.html   # FEATURE=1, both promotion modes

The page is the real village renderer (src/web/village/) fed with the
recorded /api/village snapshots instead of a server, with a playback bar:
runtime (Temporal or Kubernetes) or, for a feature recording, promotion mode
(per subtask or whole feature), play/pause, speed and a scrubber.
"""
import gzip
import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
rec, out = sys.argv[1], sys.argv[2]
frames = json.loads(gzip.decompress(Path(rec).read_bytes()))

# Links point at the demo's own dashboard and fake issues: nothing to open in a replay.
for run in frames.values():
    for f in run:
        for v in f["villages"]:
            for h in [v["manager"], *v["houses"]]:
                h["links"] = []
                h.pop("answer_url", None)
                h.pop("question", None)
            for item in v["backlog"] + v["board"]:
                item.pop("url", None)
            for f in v.get("features", []):
                f["url"] = ""
                f.pop("spec_url", None)

# A story recording (tools/mock/demo.sh with STORY=1) gets captions: each
# frame's step, worked out from the frames themselves and never going back.
STEPS = [
    ("Quiet", "The shop swarm works its backlog. The app runs on a Kubernetes namespace, at the top."),
    ("Crash", "In prod, agent-worker pods start crash-looping. Its house waves for attention."),
    ("Incident", "The crash files incident #{n}. A crate floats down the river to the dev village."),
    ("Fix", "{who} claims #{n} as a P0 and starts the fix."),
    ("Ask", "{who} stops to ask for permission, and waves at you."),
    ("Resume", "{who} has its answer and carries on with the fix."),
    ("Landed", "The fix lands on master and waits at the quay."),
    ("Green", "The sentinel tests master: its lock turns green."),
    ("Staging", "Promoted to staging."),
    ("Prod", "Released to prod. A cart carries the release up the road to the cluster."),
    ("Fixed", "The cluster runs the fix. Agent-worker is healthy again and incident #{n} is closed."),
]

def story_captions(run):
    out, stage, n, who = [], 0, None, "A worker"
    for f in run:
        dev = next((v for v in f["villages"] if v["plane"] == "development"), None)
        plat = next((v for v in f["villages"] if v["plane"] == "runtime"), None)
        world = (f["worlds"] or [{}])[0]
        incidents = world.get("incidents", [])
        if n is None and incidents:
            n = incidents[0]["number"]
        crashing = plat and any(h["role"] == "agent-worker" and h["status"] == "blocked" for h in plat["houses"])
        house = next((h for h in dev["houses"] if n and h["issue"] == n), None) if dev else None
        if house:
            who = house["role"]
        stops = {s["name"]: s["issues"] for s in dev["pipeline"]["stops"]} if dev else {}
        now = 0
        if crashing:
            now = 1
        if n:
            now = 2
        if house:
            now = 4 if house["status"] == "blocked" else (5 if stage >= 4 else 3)
        if n and dev and n in dev["pipeline"]["quay"]:
            now = 6
        if n and any(n in i for name, i in stops.items() if name.endswith("green")):
            now = 7
        if n and n in stops.get("staging", []):
            now = 8
        if n and n in stops.get("prod", []):
            now = 10 if not crashing and world.get("landed") else 9
        stage = max(stage, now)
        title, text = STEPS[stage]
        out.append({"step": stage, "title": title, "text": text.format(n=n or "?", who=who)})
    return out

captions = {k: story_captions(v) for k, v in frames.items() if k == "story"}

# A feature recording (FEATURE=1, one run per promotion mode) picks a mode
# instead of a runtime and opens on the dev village, where the banners fly.
features = set(frames) <= {"subtask", "feature"}
RUN_LABELS = {"temporal": "Temporal", "k8s": "Kubernetes", "subtask": "Per subtask", "feature": "Whole feature"}
runs = sorted(frames, key=lambda k: (k != "temporal", k != "subtask", k))
run_buttons = "\n".join(
    f'      <button id="rt-{k}" data-run="{k}" aria-pressed="{str(i == 0).lower()}">{RUN_LABELS.get(k, k)}</button>'
    for i, k in enumerate(runs))
title = "Bug to Fix in Prod" if captions else "Feature to Prod" if features else "Worktree Hollow Simulation"
about = (
    "Two features followed from spec to prod by agents-tui and its simulators: banners for the features, a trail to each "
    "subtask and a strip that counts them through build, dev, UAT and prod. One subtask waits on a design, one is dropped, "
    "and for a while the village can't tell what UAT runs. Pick a promotion mode: per subtask, or the whole feature as one barge."
    if features else
    "Six minutes of agents-tui driven by its simulators: a coding swarm (the dev village) builds the app, and the app runs on "
    "Temporal or on a Kubernetes namespace. Failures raise incidents that float down to the swarm; releases ride up to the "
    "platform. Pick <b>shop ⇄</b> to see both, or one village alone.")

def village_page():
    """The village page as one self-contained file: its stylesheet and script inlined."""
    d = ROOT / "src/web/village"
    page = (d / "index.html").read_text()
    for tag, inline in (
        ('<link rel="stylesheet" href="/village/village.css">', "<style>\n" + (d / "village.css").read_text() + "</style>"),
        ('<script type="module" src="/village/main.js"></script>', "<script>\n" + (d / "main.js").read_text() + "</script>"),
    ):
        assert tag in page, tag
        page = page.replace(tag, inline, 1)
    return page

src = village_page()
head = src[src.index("<head>") + 6:src.index("</head>")]
body = src[src.index("<body>") + 6:src.index("</body>")]
head = head.replace('<meta charset="utf-8">', "").replace('<meta name="viewport" content="width=device-width, initial-scale=1">', "")
head = head.replace("<title>Worktree Hollow</title>", '<title>' + title + '</title>\n<meta name="claude:viewport-fit-sides" content="cover">')

def must(old, new, where):
    assert old in where, old[:60]
    return where.replace(old, new, 1)

css = """
  :root { color-scheme: dark; }
  [hidden] { display: none !important; }
  body { padding-block: 12px 24px; padding-left: max(16px, env(safe-area-inset-left, 0px)); padding-right: max(16px, env(safe-area-inset-right, 0px)); }
  .player { max-width: 1400px; margin: 0 auto 10px; display: flex; flex-wrap: wrap; gap: 10px 14px; align-items: center; padding: 10px 12px; }
  .player .seg { display: inline-flex; border: 2px solid var(--win-inner); border-radius: 4px; overflow: hidden; }
  .player .seg button { border: 0; border-radius: 0; }
  .player button { background: var(--win-bot); color: var(--ink); border: 2px solid var(--win-inner); border-radius: 4px; font: 700 13px var(--mono); padding: 5px 11px; cursor: pointer; }
  .player button[aria-pressed="true"] { background: var(--gold); color: #241400; }
  .player input[type=range] { flex: 1 1 180px; min-width: 0; accent-color: var(--gold); }
  .player .clock { font: 13px var(--mono); color: var(--ink-dim); font-variant-numeric: tabular-nums; }
  .player .about { flex-basis: 100%; font: 13px/1.5 var(--sans); color: var(--ink-dim); margin: 0; }
  .story { flex-basis: 100%; display: grid; gap: 8px; }
  .steps { display: flex; flex-wrap: wrap; gap: 4px; list-style: none; margin: 0; padding: 0; }
  .steps li { font: 700 11px var(--mono); color: var(--ink-dim); border: 2px solid var(--rule); border-radius: 4px; padding: 2px 7px; }
  .steps li.done { color: var(--ink); border-color: var(--win-inner); }
  .steps li.now { color: #241400; background: var(--gold); border-color: var(--gold); }
  .caption { font: 700 16px/1.4 var(--mono); color: var(--gold); margin: 0; min-height: 2.8em; }
"""
head = must("</style>", css + "</style>", head)

player = """
  <div class="win player" role="group" aria-label="Replay">
    <span class="seg" id="runs" role="group" aria-label="RUN_GROUP">
""" + run_buttons + """
    </span>
    <button id="play" aria-pressed="true">Pause</button>
    <span class="seg" role="group" aria-label="Speed">
      <button id="sp1" data-speed="1" aria-pressed="true">1×</button>
      <button id="sp2" data-speed="2" aria-pressed="false">2×</button>
      <button id="sp4" data-speed="4" aria-pressed="false">4×</button>
    </span>
    <input id="scrub" type="range" min="0" value="0" aria-label="Position in the recording">
    <span class="clock" id="clock">0:00 / 0:00</span>
    <div class="story" id="story" hidden><ol class="steps" id="steps"></ol><p class="caption" id="caption" aria-live="polite"></p></div>
    <p class="about" id="about">""" + about + """</p>
  </div>
"""
player = player.replace("RUN_GROUP", "Promotion" if features else "Runtime")
body = must('  <div class="win stage">', player + '  <div class="win stage">', body)
body = must('<a href="/">Dashboard</a>', "", body)

replay_js = """<script type="application/json" id="recording">""" + json.dumps(frames, separators=(",", ":")).replace("</", "<\\/") + """</script>
<script>
/* Replay: the village asks /api/village every few seconds; answer with the next recorded snapshot. */
const REC = JSON.parse(document.getElementById('recording').textContent);
const CAPTIONS = """ + json.dumps(captions) + """;
const replay = { run: """ + json.dumps(runs[0]) + """, i: 0, playing: true, speed: 1 };
window.__pollMs = 3000;
window.fetch = async () => {
  const frames = REC[replay.run];
  const f = frames[Math.min(replay.i, frames.length - 1)];
  if (replay.playing) replay.i = (replay.i + 1) % frames.length;
  window.dispatchEvent(new Event('replay-tick'));
  return { ok: true, json: async () => JSON.parse(JSON.stringify(f)) };
};
</script>
"""
body = must("<script>\n(() => {", replay_js + "<script>\n(() => {", body)

controls = """
  // Replay controls.
  const fmt = n => `${Math.floor(n * 3 / 60)}:${String(n * 3 % 60).padStart(2, '0')}`;
  const scrub = document.getElementById('scrub'), clock = document.getElementById('clock'), play = document.getElementById('play');
  const showClock = () => { const n = REC[replay.run].length; scrub.max = n - 1; scrub.value = replay.i; clock.textContent = `${fmt(replay.i)} / ${fmt(n)}`; };
  window.addEventListener('replay-tick', showClock);
  // A story recording: one run, captions instead of the runtime choice.
  if (CAPTIONS[replay.run]) {
    document.getElementById('runs').hidden = true;
    document.getElementById('story').hidden = false;
    document.getElementById('about').textContent = 'One bug from start to finish, recorded from agents-tui and its simulators: the app crashes in prod (the cluster, top), the dev village (below) fixes it, and the fix ships back up.';
    const steps = document.getElementById('steps'), caption = document.getElementById('caption');
    const titles = [...new Set(CAPTIONS[replay.run].map(c => c.step))].sort((a, b) => a - b);
    const names = {}; CAPTIONS[replay.run].forEach(c => { names[c.step] = c.title; });
    steps.innerHTML = titles.map(s => `<li data-step="${s}">${s}. ${names[s]}</li>`).join('');
    const showCaption = () => {
      const c = CAPTIONS[replay.run][Math.max(0, replay.i - 1)] || CAPTIONS[replay.run][0];
      caption.textContent = c.text;
      steps.querySelectorAll('li').forEach(li => { const s = Number(li.dataset.step); li.className = s === c.step ? 'now' : s < c.step ? 'done' : ''; });
    };
    window.addEventListener('replay-tick', showCaption);
    scrub.addEventListener('input', showCaption);
    showCaption();
  }
  const press = (sel, on) => document.querySelectorAll(sel).forEach(b => b.setAttribute('aria-pressed', String(on(b))));
  document.querySelectorAll('[data-run]').forEach(b => b.addEventListener('click', () => {
    replay.run = b.dataset.run; replay.i = 0; press('[data-run]', x => x === b);
    for (const k in scenes) delete scenes[k];
    poll();
  }));
  document.querySelectorAll('[data-speed]').forEach(b => b.addEventListener('click', () => {
    replay.speed = Number(b.dataset.speed); window.__pollMs = 3000 / replay.speed; press('[data-speed]', x => x === b);
  }));
  play.addEventListener('click', () => { replay.playing = !replay.playing; play.textContent = replay.playing ? 'Pause' : 'Play'; play.setAttribute('aria-pressed', String(replay.playing)); });
  scrub.addEventListener('input', () => { replay.i = Number(scrub.value); showClock(); });
  if (!location.hash) location.hash = """ + json.dumps("shop" if features else "world:shop") + """;
"""
body = must("  poll();\n", controls + "  poll();\n", body)
body = must("    setTimeout(poll, 3000);", "    setTimeout(poll, window.__pollMs || 3000);", body)
body = must("'Live · updated ' + new Date().toLocaleTimeString()", "'Replay of the simulation'", body)

Path(out).write_text(head.strip() + "\n" + body)
print("built", out, len(head) + len(body), "bytes")
