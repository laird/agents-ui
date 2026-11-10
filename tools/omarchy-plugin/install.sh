#!/usr/bin/env bash
# install.sh — put the Agents village into Omarchy, here or on another Omarchy
# machine on the tailnet: the bar widget, or the village as the screensaver.
#
# USAGE:
#   tools/omarchy-plugin/install.sh [--host <ssh host>] [--enable] [--uninstall] [--dry-run]
#   tools/omarchy-plugin/install.sh --screensaver [--never-sleep] [--server URL]
#                                   [--cycle S] [--dim H-H] [--host <ssh host>] [--uninstall] [--dry-run]
#
#   (no --host)  link this checkout's plugin into ~/.config/omarchy/plugins/, so
#                edits here show up as you save them
#   --host H     copy it to H over ssh (omarchy-1, the uConsole, ...), replacing
#                any earlier copy; commands there run in a login shell, so
#                `omarchy` has its environment (OMARCHY_PATH)
#   --enable     also add the widget to the bar (`omarchy plugin enable`)
#   --uninstall  disable the plugin and remove it; with --screensaver, also put
#                Omarchy's own screensaver and idle settings back
#   --dry-run    print what would be done, change nothing
#
# The screensaver (--screensaver): when the session goes idle, the village
# opens full screen with a status strip, and any input closes it. It replaces
# Omarchy's text screensaver (idle.screensaver in ~/.config/omarchy/shell.json
# is set to 0, after a backup) and starts after the same idle time.
#   --never-sleep  this machine never locks itself or turns the display off
#                  when idle (idle.lock: 0), so the village stays up for good.
#                  For a status display like nuc-1; leave it off elsewhere.
#   --server URL   the agents-tui server (default http://nuc-1:7890)
#   --cycle S      seconds on each swarm when there are several (default 30, 0 stays)
#   --dim H-H      dim between these hours, e.g. 23-7 (default: never dims)
#
# The widget reads http://nuc-1:7890 by default; set Server in its settings to
# point it elsewhere.
set -euo pipefail

HOST=""
ENABLE=false
UNINSTALL=false
DRY_RUN=false
SCREENSAVER=false
NEVER_SLEEP=false
SERVER="http://nuc-1:7890"
CYCLE=30
DIM=""
need() { [ "$2" -ge 2 ] || { echo "install.sh: $1 needs a value" >&2; exit 1; }; }
while [ $# -gt 0 ]; do
  case "$1" in
    --host) need "$1" $#; HOST="$2"; shift 2 ;;
    --enable) ENABLE=true; shift ;;
    --uninstall) UNINSTALL=true; shift ;;
    --dry-run) DRY_RUN=true; shift ;;
    --screensaver) SCREENSAVER=true; shift ;;
    --never-sleep|--no-lock) NEVER_SLEEP=true; shift ;;
    --server) need "$1" $#; SERVER="${2%/}"; shift 2 ;;
    --cycle) need "$1" $#; CYCLE="$2"; shift 2 ;;
    --dim) need "$1" $#; DIM="$2"; shift 2 ;;
    -h|--help) sed -n '2,/^set -euo/p' "$0" | sed '$d; s/^# \{0,1\}//'; exit 0 ;;
    *) echo "install.sh: unknown argument: $1" >&2; exit 1 ;;
  esac
done
if [ "$SCREENSAVER" != true ] && { [ "$NEVER_SLEEP" = true ] || [ "$DIM" != "" ]; }; then
  echo "install.sh: --never-sleep and --dim go with --screensaver" >&2; exit 1
fi
[[ $CYCLE =~ ^[0-9]+$ ]] || { echo "install.sh: --cycle takes whole seconds" >&2; exit 1; }
[ -z "$DIM" ] || [[ $DIM =~ ^([01]?[0-9]|2[0-3])-([01]?[0-9]|2[0-3])$ ]] || { echo "install.sh: --dim takes hours like 23-7" >&2; exit 1; }

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
if [ "$SCREENSAVER" = true ]; then
  ID="agents-ui.village-screensaver"
  NAME="agents-village-screensaver"
  # The files the plugin needs; tests stay in the repo.
  FILES=(manifest.json Service.qml village-screensaver offline.html README.md)
  WHAT="the Agents village screensaver"
else
  ID="agents-ui.village"
  NAME="agents-village"
  FILES=(manifest.json Panel.qml Model.js README.md)
  WHAT="the Agents village widget"
fi
SRC="$HERE/$NAME"
for f in "${FILES[@]}"; do [ -f "$SRC/$f" ] || { echo "install.sh: missing $SRC/$f" >&2; exit 1; }; done

# On $HOST, a command runs in a login shell. ssh alone gives a non-login,
# non-interactive shell, which lacks the session environment `omarchy` needs
# (OMARCHY_PATH, and Omarchy's PATH additions): `omarchy plugin enable` then
# fails with "OMARCHY_PATH is not set". A login shell reads
# /etc/profile.d/omarchy.sh, Omarchy's own way of setting both, so nothing
# here hard-codes where Omarchy lives.
sq() {  # sq <word>: the word as the shell reads it back, single-quoted only when it must be
  # Each ' becomes '\'' by sed, not ${1//...}: bash 3.2 (macOS's) reads the
  # quotes inside that expansion differently. The x keeps trailing newlines.
  if [[ $1 =~ ^[A-Za-z0-9_./:=@%+-]+$ ]]; then printf '%s' "$1"; return; fi
  local q
  q=$(printf '%sx' "$1" | sed "s/'/'\\\\''/g"); printf "'%s'" "${q%x}"
}
remote_cmd() {  # remote_cmd <command...>: the string ssh runs on $HOST
  local words=() a
  for a in "$@"; do words+=("$(sq "$a")"); done
  printf 'bash -lc %s' "$(sq "${words[*]}")"
}
run() {  # run <command...>: here, or on $HOST
  if [ "$DRY_RUN" = true ]; then
    if [ -n "$HOST" ]; then echo "would run on $HOST: $(remote_cmd "$@")"; else echo "would run: $*"; fi
  elif [ -n "$HOST" ]; then
    ssh "$HOST" "$(remote_cmd "$@")"
  else
    "$@"
  fi
}
# A script run with sh -c: in a dry run, only what it is for is printed.
run_script() {  # run_script <what it does> <script> [args...]
  local what=$1
  shift
  if [ "$DRY_RUN" = true ]; then echo "would${HOST:+ on $HOST}: $what"; else run sh -c "$@"; fi
}
# Best effort: the shell may not be running (a remote machine at its login screen).
shell_ok() { run sh -c 'command -v omarchy-shell >/dev/null && omarchy-shell shell rescanPlugins >/dev/null 2>&1 || true'; }

# Enabling races the shell (#416): a plugin just linked or copied is "not
# known" until the shell has rescanned and registered it, which takes a moment
# after rescanPlugins returns. So rescan, try to enable, and try again until it
# takes or the time is up ($AGENTS_UI_ENABLE_TRIES tries,
# $AGENTS_UI_ENABLE_PAUSE seconds apart: 20 x 0.5 s by default). One script,
# so over --host it is one ssh, not one per try.
ENABLE_TRIES=${AGENTS_UI_ENABLE_TRIES:-20}
ENABLE_PAUSE=${AGENTS_UI_ENABLE_PAUSE:-0.5}
[[ $ENABLE_TRIES =~ ^[1-9][0-9]*$ ]] || { echo "install.sh: AGENTS_UI_ENABLE_TRIES takes a whole number" >&2; exit 1; }
[[ $ENABLE_PAUSE =~ ^[0-9]+(\.[0-9]+)?$ ]] || { echo "install.sh: AGENTS_UI_ENABLE_PAUSE takes seconds" >&2; exit 1; }
ENABLE_SCRIPT='id=$1; tries=$2; pause=$3; i=0
while :; do
  command -v omarchy-shell >/dev/null && omarchy-shell shell rescanPlugins >/dev/null 2>&1
  if out=$(omarchy plugin enable "$id" 2>&1); then exit 0; fi
  i=$((i + 1))
  [ "$i" -lt "$tries" ] || { printf "%s\n" "$out" >&2; exit 1; }
  sleep "$pause"
done'
enable_plugin() {  # enable_plugin: enable $ID once the shell knows it; false, saying why, if it never does
  if [ "$DRY_RUN" = true ]; then
    if [ -n "$HOST" ]; then echo "would run on $HOST: $(remote_cmd sh -c "omarchy plugin enable $ID"), once Omarchy knows the plugin (rescanning, up to $ENABLE_TRIES tries)"
    else echo "would run: omarchy plugin enable $ID, once Omarchy knows the plugin (rescanning, up to $ENABLE_TRIES tries)"; fi
    return 0
  fi
  local err
  if err=$(run sh -c "$ENABLE_SCRIPT" sh "$ID" "$ENABLE_TRIES" "$ENABLE_PAUSE" 2>&1); then return 0; fi
  echo "install.sh: could not enable $ID${HOST:+ on $HOST}${err:+: $err}" >&2
  echo "install.sh: the plugin is in place; enable it by hand${HOST:+ on $HOST} once Omarchy has picked it up:" >&2
  echo "  omarchy-shell shell rescanPlugins && omarchy plugin enable $ID" >&2
  return 1
}

# The screensaver's settings, and Omarchy's idle settings around it. The
# original shell.json is kept once, before the first change, so uninstall
# puts back what was there however often install ran. Only `idle` is put
# back: anything else changed in shell.json since stays.
CONFIGURE='set -e
shell="$HOME/.config/omarchy/shell.json"; dir="$HOME/.config/agents-ui"; backup="$dir/shell.json.before-screensaver"
mkdir -p "$dir" "$(dirname "$shell")"
# No shell.json means Omarchy uses its defaults; start from them, so the bar keeps its layout.
[ -f "$shell" ] || cp "${OMARCHY_PATH:-/usr/share/omarchy}/config/omarchy/shell.json" "$shell" 2>/dev/null || echo "{}" > "$shell"
[ -f "$backup" ] || cp "$shell" "$backup"
timeout=$(jq -r ".idle.screensaver // 150" "$backup")
[ "$timeout" -gt 0 ] 2>/dev/null || timeout=150
jq -n --arg server "$1" --argjson cycle "$2" --arg dim "$3" --argjson timeout "$timeout" \
  "{server: \$server, cycle: \$cycle, dim: \$dim, timeout: \$timeout}" > "$dir/screensaver.json"
tmp=$(mktemp)
jq --argjson never "$4" ".idle = ((.idle // {}) + {screensaver: 0} + (if \$never then {lock: 0} else {} end))" "$shell" > "$tmp"
cat "$tmp" > "$shell"; rm -f "$tmp"'
RESTORE='shell="$HOME/.config/omarchy/shell.json"; dir="$HOME/.config/agents-ui"; backup="$dir/shell.json.before-screensaver"
if [ -f "$backup" ] && [ -f "$shell" ]; then
  tmp=$(mktemp)
  jq --slurpfile b "$backup" "if (\$b[0].idle) then .idle = \$b[0].idle else del(.idle) end" "$shell" > "$tmp" && cat "$tmp" > "$shell"
  rm -f "$tmp"
fi
rm -f "$backup" "$dir/screensaver.json"'

DEST_REL=".config/omarchy/plugins/$NAME"

# Before changing anything: `omarchy` has to run where the plugin goes, or the
# install would stop half way, with Omarchy's screensaver already turned off
# in shell.json and the village's never turned on.
if [ "$UNINSTALL" != true ] && { [ "$SCREENSAVER" = true ] || [ "$ENABLE" = true ]; }; then
  if [ "$DRY_RUN" = true ]; then
    echo "would check that omarchy runs${HOST:+ on $HOST} (omarchy plugin list)"
  elif ! err=$(run sh -c 'command -v omarchy >/dev/null || { echo "omarchy: command not found"; exit 1; }; omarchy plugin list 2>&1 >/dev/null' 2>&1); then
    echo "install.sh: omarchy does not run${HOST:+ on $HOST}${err:+: $err}" >&2
    echo "install.sh: nothing was changed." >&2
    exit 1
  fi
fi

if [ "$UNINSTALL" = true ]; then
  run sh -c "command -v omarchy >/dev/null && omarchy plugin disable $ID >/dev/null 2>&1 || true"
  run sh -c "rm -rf \"\$HOME/$DEST_REL\""
  if [ "$SCREENSAVER" = true ]; then
    run_script "put Omarchy's own screensaver and idle settings back (from ~/.config/agents-ui/shell.json.before-screensaver)" "$RESTORE"
  fi
  shell_ok
  echo "Removed $WHAT${HOST:+ from $HOST}."
  [ "$SCREENSAVER" = true ] && echo "Omarchy's own screensaver and idle settings are back."
  exit 0
fi

if [ -z "$HOST" ]; then
  DEST="$HOME/$DEST_REL"
  run mkdir -p "$(dirname "$DEST")"
  # A link, so this checkout is what runs; replace a copy if one is there.
  [ -L "$DEST" ] || [ ! -e "$DEST" ] || run rm -rf "$DEST"
  run ln -sfn "$SRC" "$DEST"
else
  run sh -c "mkdir -p \"\$HOME/$DEST_REL\" && rm -f \"\$HOME/$DEST_REL\"/*"
  if [ "$DRY_RUN" = true ]; then
    echo "would copy ${FILES[*]} to $HOST:~/$DEST_REL/"
  else
    (cd "$SRC" && scp -q -p "${FILES[@]}" "$HOST:$DEST_REL/")
  fi
fi
shell_ok

if [ "$SCREENSAVER" = true ]; then
  # Enable first, and only then turn Omarchy's own screensaver off: a failed
  # enable must not leave the machine with no screensaver at all (#416).
  # A service has nowhere to be placed: enabling it is what starts it.
  if ! enable_plugin; then
    echo "install.sh: Omarchy's own screensaver and idle settings were left as they were." >&2
    exit 1
  fi
  run_script "write ~/.config/agents-ui/screensaver.json (server $SERVER, cycle ${CYCLE}s${DIM:+, dim $DIM}) and set idle.screensaver to 0$([ "$NEVER_SLEEP" = true ] && echo ' and idle.lock to 0') in ~/.config/omarchy/shell.json" \
    "$CONFIGURE" sh "$SERVER" "$CYCLE" "$DIM" "$NEVER_SLEEP"
  echo "Installed $WHAT${HOST:+ on $HOST}: after the idle time Omarchy's screensaver used, the village opens full screen; any input closes it."
  echo "It shows $SERVER. Try it now, from a terminal${HOST:+ on $HOST}: ~/$DEST_REL/village-screensaver force"
  if [ "$NEVER_SLEEP" = true ]; then
    echo "NOTE: this machine${HOST:+ ($HOST)} will no longer lock itself or turn its display off when idle."
    echo "      Undo with: tools/omarchy-plugin/install.sh --screensaver --uninstall${HOST:+ --host $HOST}"
  else
    echo "Omarchy still locks the session on its own idle timer."
  fi
  exit 0
fi

if [ "$ENABLE" = true ]; then
  enable_plugin || exit 1
  echo "Added $WHAT to the bar${HOST:+ on $HOST}."
else
  echo "Installed $WHAT${HOST:+ on $HOST}. Add it to the bar with:"
  echo "  ${HOST:+ssh $HOST }omarchy plugin enable $ID"
  echo "or from the bar's widget settings (Agents village, under AI)."
fi
