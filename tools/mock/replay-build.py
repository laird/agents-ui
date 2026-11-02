#!/usr/bin/env python3
"""Build a self-contained replay page from a recording of the demo.

    python3 tools/mock/record.py tools/mock/recordings/demo.json.gz 360   # while demos run on 7930 (k8s) and 7931 (temporal)
    python3 tools/mock/replay-build.py tools/mock/recordings/demo.json.gz replay.html

The page is the real village renderer (src/web/village.html) fed with the
recorded /api/village snapshots instead of a server, with a playback bar:
runtime (Temporal or Kubernetes), play/pause, speed and a scrubber.
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

src = (ROOT / "src/web/village.html").read_text()
head = src[src.index("<head>") + 6:src.index("</head>")]
body = src[src.index("<body>") + 6:src.index("</body>")]
head = head.replace('<meta charset="utf-8">', "").replace('<meta name="viewport" content="width=device-width, initial-scale=1">', "")
head = head.replace("<title>Worktree Hollow</title>", '<title>Worktree Hollow Simulation</title>\n<meta name="claude:viewport-fit-sides" content="cover">')

def must(old, new, where):
    assert old in where, old[:60]
    return where.replace(old, new, 1)

css = """
  :root { color-scheme: dark; }
  body { padding-block: 12px 24px; padding-left: max(16px, env(safe-area-inset-left, 0px)); padding-right: max(16px, env(safe-area-inset-right, 0px)); }
  .player { max-width: 1400px; margin: 0 auto 10px; display: flex; flex-wrap: wrap; gap: 10px 14px; align-items: center; padding: 10px 12px; }
  .player .seg { display: inline-flex; border: 2px solid var(--win-inner); border-radius: 4px; overflow: hidden; }
  .player .seg button { border: 0; border-radius: 0; }
  .player button { background: var(--win-bot); color: var(--ink); border: 2px solid var(--win-inner); border-radius: 4px; font: 700 13px var(--mono); padding: 5px 11px; cursor: pointer; }
  .player button[aria-pressed="true"] { background: var(--gold); color: #241400; }
  .player input[type=range] { flex: 1 1 180px; min-width: 0; accent-color: var(--gold); }
  .player .clock { font: 13px var(--mono); color: var(--ink-dim); font-variant-numeric: tabular-nums; }
  .player .about { flex-basis: 100%; font: 13px/1.5 var(--sans); color: var(--ink-dim); margin: 0; }
"""
head = must("</style>", css + "</style>", head)

player = """
  <div class="win player" role="group" aria-label="Replay">
    <span class="seg" role="group" aria-label="Runtime">
      <button id="rt-temporal" data-run="temporal" aria-pressed="true">Temporal</button>
      <button id="rt-k8s" data-run="k8s" aria-pressed="false">Kubernetes</button>
    </span>
    <button id="play" aria-pressed="true">Pause</button>
    <span class="seg" role="group" aria-label="Speed">
      <button id="sp1" data-speed="1" aria-pressed="true">1×</button>
      <button id="sp2" data-speed="2" aria-pressed="false">2×</button>
      <button id="sp4" data-speed="4" aria-pressed="false">4×</button>
    </span>
    <input id="scrub" type="range" min="0" value="0" aria-label="Position in the recording">
    <span class="clock" id="clock">0:00 / 0:00</span>
    <p class="about">Six minutes of agents-tui driven by its simulators: a coding swarm (the dev village) builds the app, and the app runs on Temporal or on a Kubernetes namespace. Failures raise incidents that float down to the swarm; releases ride up to the platform. Pick <b>shop ⇄</b> to see both, or one village alone.</p>
  </div>
"""
body = must('  <div class="win stage">', player + '  <div class="win stage">', body)
body = must('<a href="/">Dashboard</a>', "", body)

replay_js = """<script type="application/json" id="recording">""" + json.dumps(frames, separators=(",", ":")).replace("</", "<\\/") + """</script>
<script>
/* Replay: the village asks /api/village every few seconds; answer with the next recorded snapshot. */
const REC = JSON.parse(document.getElementById('recording').textContent);
const replay = { run: 'temporal', i: 0, playing: true, speed: 1 };
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
  if (!location.hash) location.hash = 'world:shop';
"""
body = must("  poll();\n", controls + "  poll();\n", body)
body = must("    setTimeout(poll, 3000);", "    setTimeout(poll, window.__pollMs || 3000);", body)
body = must("'Live · updated ' + new Date().toLocaleTimeString()", "'Replay of the simulation'", body)

Path(out).write_text(head.strip() + "\n" + body)
print("built", out, len(head) + len(body), "bytes")
