# Agents village screensaver

The agents-tui village as Omarchy's screensaver. When the session has been
idle, the village opens full screen on every monitor, so the swarm's status
can be read from across the room. Any key, click or real mouse movement
closes it.

```bash
tools/omarchy-plugin/install.sh --screensaver                 # here
tools/omarchy-plugin/install.sh --screensaver --never-sleep   # a status display (nuc-1): never lock, never blank
tools/omarchy-plugin/install.sh --screensaver --host omarchy-1 --server http://nuc-1:7890
tools/omarchy-plugin/install.sh --screensaver --uninstall     # Omarchy's own screensaver and idle settings back
```

Add `--dry-run` to any of them to see what would change. `--cycle S` sets the
seconds spent on each swarm when there are several (default 30, 0 stays on
one). `--dim 23-7` dims the display between those hours. It is never blanked.

## What is on screen

`/village?screensaver=1` on the agents-tui server, or `/village#<project>&screensaver`
for one swarm. Any browser can show it this way. The launcher adds `dismiss=1`
so the page closes itself on input. A dashboard tab never does.

- The map fills the display with its area labels on. There is no bar,
  picker or button, and no cursor.
- The status strip under the map shows the swarm, how many houses are
  working, idle and blocked, and when the page last heard from the server.
  Below that it shows the gate, the quay and each environment, and how much
  is waiting on you (in red when anything is).
- With several swarms it cycles through each world, then each village that is
  not part of a world.
- For burn-in, the whole scene drifts a few pixels, and it dims in the night
  hours if `dim` is set.
- When the server stops answering, the map gives way to a large
  "Can't reach agents-tui on <host>" with the time. If the server is down
  when the screensaver starts, `offline.html` says the same and opens the
  village once the server answers.

## How it fits into Omarchy

Omarchy's idle service (`omarchy.idle`) runs its text screensaver after
`idle.screensaver` seconds, and locks after `idle.lock`. There is no hook for
a different screensaver. A wrapper earlier on `PATH` would never be found,
because the shell's `PATH` starts with `/usr/share/omarchy/bin`. So:

- `install.sh --screensaver` keeps a copy of `~/.config/omarchy/shell.json`
  (`~/.config/agents-ui/shell.json.before-screensaver`) and sets
  `idle.screensaver` to 0, which turns the text screensaver off.
- This plugin is a shell **service** (`Service.qml`). It watches for idle
  with the same `IdleMonitor`, after the time the text screensaver used
  (`timeout` in `~/.config/agents-ui/screensaver.json`), and runs
  `village-screensaver`. It does nothing while the session is locked or
  Omarchy's stay-awake indicator is on, or while the screensaver is toggled off.
- `village-screensaver` opens a Chromium kiosk window of class
  `org.omarchy.screensaver` on each monitor. That class gets Omarchy's
  screensaver window rules (fullscreen, on a special workspace), and
  `omarchy-launch-screensaver` will not start over it. Each window has a
  profile of its own with extensions off, so nothing but the village opens in
  it. The cursor is hidden while the screensaver is up and comes back when it
  closes. The screensaver closes when the page reports input, when one of its
  windows closes, or when focus moves to another window.
- `--never-sleep` also sets `idle.lock` to 0, so nothing locks the session or
  turns the display off. The installer says so when it does this.
- `--uninstall` puts back the `idle` section from the copy. Anything else
  changed in `shell.json` since then stays as it is.

Settings (`~/.config/agents-ui/screensaver.json`, written by the installer):

```json
{ "server": "http://nuc-1:7890", "cycle": 30, "dim": "23-7", "timeout": 150 }
```

## Tests

```bash
bash tools/omarchy-plugin/agents-village-screensaver/tests/test_install.sh   # installer and launcher, in a throwaway HOME
node --test tools/omarchy-plugin/agents-village-screensaver/tests/            # the page's decisions (src/web/village/saver.js)
python3 tools/omarchy-plugin/agents-village-screensaver/tests/qml_check.py   # the real Service.qml in Quickshell, offscreen
```

`cargo test screensaver` runs the first two.
