# Agents village notifications

A desktop notification when something in the agents-tui village changes, so
you hear about it without watching the bar or the village:

- **A house or the manager starts waiting on you** (a question or a
  permission prompt): which house, its issue, and the question.
- **A note lands on the board** (an issue blocked on `needs-*`).
- **A merge bounces at the gate**: it failed, conflicted, or met a red base.
- **The build breaks**, and when it is **fixed** again.
- **A deploy or promotion lands** in an environment, with its version, and
  when one **fails**.

Clicking a notification opens the village (as an Omarchy web app when
`omarchy-launch-webapp` is there, else in the browser). For a house or the
manager, it opens on that house's dialog: `/village#<project>&house=<name>`,
which any browser can open too.

## When it notifies

On transitions only. What is already true when the shell starts is recorded,
not announced, as the village page never sails its ship on load. The same
state is never announced twice: a house still waiting stays quiet, and it is
news again only if it is answered and then waits again. Several changes seen
in one poll come as one notification that lists them.

Nothing is announced during quiet hours, and what changed then is not saved
up for the morning.

## One rule with the bar widget

"Needs you" is the bar widget's own rule (`Model.attention()` in
`agents-village/Model.js`): the red dot is lit while it lists anything, and
this service notifies when an item first appears in it. Each item carries a
`key`, the same on every poll, which is what "first appears" compares.
`Model.js` here is a copy (Omarchy refuses a symlink inside a plugin folder),
and `tests/notify.test.js` fails if the two differ: after changing the bar
widget's, `cp agents-village/Model.js agents-village-notify/`.

This service is the only thing that sends notifications. The bar widget, the
village page and the screensaver send none, so a machine running all of them
is notified once.

## Install

```bash
tools/omarchy-plugin/install.sh --notify                            # here, reading http://nuc-1:7890
tools/omarchy-plugin/install.sh --notify --server http://nuc-1:7890 --quiet 23-7
tools/omarchy-plugin/install.sh --notify --host omarchy-1           # another Omarchy machine
tools/omarchy-plugin/install.sh --notify --uninstall
```

`--dry-run` shows what would be done.

## Settings

`~/.config/agents-ui/notify.json`, re-read when it changes:

```json
{ "server": "http://nuc-1:7890", "interval": 15, "kinds": ["needs", "gate", "build", "deploy"], "quiet": "23-7" }
```

| Setting | Default | |
|---|---|---|
| `server` | the screensaver's, else `http://nuc-1:7890` | the agents-tui web server |
| `interval` | 15 | seconds between polls (at least 5) |
| `kinds` | all four | `needs` (houses, the manager, the board), `gate`, `build`, `deploy` |
| `quiet` | the screensaver's `dim` | hours with no notifications, e.g. `23-7`; `""` for none |
| `enabled` | true | `false` stops it without uninstalling |

It only reads `/api/village`, so a `--read-only` server is enough.

## Tests

```bash
node --test tools/omarchy-plugin/agents-village-notify/tests/          # what is news and what is sent (Notify.js)
python3 tools/omarchy-plugin/agents-village-notify/tests/qml_check.py # the real Service.qml in Quickshell, offscreen
omarchy plugin validate tools/omarchy-plugin/agents-village-notify     # the manifest
```
