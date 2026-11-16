# Agents village for the Omarchy bar

A bar widget for the agents-tui village, so the swarm is a glance and a click
away on any Omarchy machine on the tailnet (nuc-1, omarchy-1, the uConsole),
without a browser tab open.

## The bar icon

A house glyph, with:

- **a count:** how many houses are working;
- **a red dot:** something waits on you: a blocked house or manager, a note on
  the board, a merge gate that failed or conflicted;
- **a ship:** a release just went out: the last stop's deployment changed, as
  when the village page sails its ship. It shows for two minutes, and never for the
  release already there when the widget starts;
- **dimmed:** the server can't be reached. The tooltip says so.

The red dot is lit while `Model.attention()` lists anything, the same list as
the panel's **Needs you**. That is the one rule for "needs you", shared with the
desktop notifier (`../agents-village-notify`, `install.sh --notify`).

## Notifications

When something starts waiting on you (an item first appears in that list),
the widget sends one notification: "2 need you", what is new, and a click that
opens the village on that house's dialog. It carries a fixed id, so the next
one replaces it instead of stacking. The first poll after the widget starts,
or after the server could not be reached, only records what it sees.

While the desktop notifier is installed (`~/.config/agents-ui/notify.json`
exists and does not say `"enabled": false`), the widget sends nothing: the
notifier says the same things and more (the build, deploys), and a machine
running both is notified once. Turn the widget's own off with **Notify** in its
settings.

With a village open in the panel, the icon counts that village; otherwise it
counts every village on the server. Left-click opens the panel. Right-click opens
the full village. Middle-click joins the swarm (below).

## The panel

It starts at the server's **list of swarms**, the same list as its dashboard
(`/`). Pick one to see its village; **‹** goes back to the list. With only one
swarm on the server, it goes straight to that swarm's village, with nothing to go
back to.

A village shows:

- its name (linked to the repo) and when it was last read;
- **the flow strip:** backlog · board · houses · gate · quay · each environment,
  as on the village page;
- **Needs you:** blocked houses, gate failures and conflicts, and board notes,
  each linking to the issue or the agent's session;
- **Houses:** one row per worker: worktree, branch, issue, status, and context %
  once the server reports it. A row opens the agent's session page;
- **Features:** each epic with its Build N/M progress;
- **Open village** (as an Omarchy web app when `omarchy-launch-webapp` is there,
  else in the browser), **Join swarm** and **Dashboard**.

**Join swarm** runs `swarm <machine>` (`~/.local/bin/swarm`, from
`remote-swarm-setup.sh`) in a terminal, or focuses that terminal if it is
already open (`omarchy-launch-or-focus-tui --app-id=org.omarchy.swarm-<machine>`).
It shows once **Machine** is set and `swarm` is on `PATH`. Omarchy runs that
command through `eval`, so the machine must be a plain host name (letters,
digits, `.`, `_`, `-`, starting with a letter or digit); for anything else the
button says "bad machine name" and does nothing.

Keys while it is open:

| Key | |
|---|---|
| `j` / `k`, `↓` / `↑` | move the highlight over the rows: the swarms, or Needs you then Houses |
| `Enter` / `Space` | open the highlighted row |
| `b` | back to the list of swarms |
| `o` | open the village |
| `s` | join the swarm |
| `r` | refresh |
| `Esc` | close |
| `Tab` / `Shift+Tab` | the next or previous panel on the bar |

To open it from the keyboard, bind its plugin id in
`~/.config/hypr/bindings.lua`:

```lua
o.bind("SUPER + ALT + A", "Agents village", { panel = "agents-ui.village" })
```

The widget is display only. It reads `/api/swarms` and `/api/village` and sends
nothing to agents, so a `--read-only` server is enough.

## Settings

| Setting | Default | |
|---|---|---|
| Server | `http://nuc-1:7890` | the agents-tui web server |
| Village | empty | a project to always show; empty starts at the list |
| Refresh interval | 5 s | how often it reads the server |
| Show how many are working / red dot / ship | on | which badges to draw |
| Machine | empty | the swarm's machine for **Join swarm** (e.g. `nuc`); empty hides it |
| Notify | on | a notification when something starts waiting on you (not while the desktop notifier is installed) |

`allowMultiple` is on, so two instances can watch two servers or two villages.

## Install

From a checkout of agents-ui:

```bash
tools/omarchy-plugin/install.sh --enable                  # this machine: links the checkout, adds the widget
tools/omarchy-plugin/install.sh --host omarchy-1 --enable # another Omarchy machine, over ssh
tools/omarchy-plugin/install.sh --uninstall               # take it out again
```

Locally the plugin is a link to the checkout, so edits show up in the bar as you
save them. A remote install is a copy, and running it again replaces it. Without
`--enable`, add it from the bar's widget settings (Agents village, under AI) or
with `omarchy plugin enable agents-ui.village`. `--dry-run` shows what would be
done.

## Tests

```bash
node --test tools/omarchy-plugin/agents-village/tests/        # the model, against recorded server answers
python3 tools/omarchy-plugin/agents-village/tests/qml_check.py # the real Panel.qml in Quickshell, offscreen
omarchy plugin validate tools/omarchy-plugin/agents-village   # the manifest
```

`tests/fixtures` holds `/api/swarms` and `/api/village` as a live server
answered them. The QML check needs `quickshell` and Omarchy's shell modules
(`OMARCHY_SHELL`, default `/usr/share/omarchy/shell`).
