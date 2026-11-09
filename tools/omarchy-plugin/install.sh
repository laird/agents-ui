#!/usr/bin/env bash
# install.sh — put the Agents village bar widget into Omarchy, here or on
# another Omarchy machine on the tailnet.
#
# USAGE:
#   tools/omarchy-plugin/install.sh [--host <ssh host>] [--enable] [--uninstall] [--dry-run]
#
#   (no --host)  link this checkout's plugin into ~/.config/omarchy/plugins/, so
#                edits here show up in the bar as you save them
#   --host H     copy it to H over ssh (omarchy-1, the uConsole, ...), replacing
#                any earlier copy
#   --enable     also add the widget to the bar (`omarchy plugin enable`)
#   --uninstall  disable the widget and remove the plugin
#   --dry-run    print what would be done, change nothing
#
# The widget reads http://nuc-1:7890 by default; set Server in its settings to
# point it elsewhere.
set -euo pipefail

ID="agents-ui.village"
NAME="agents-village"
SRC="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/$NAME"
HOST=""
ENABLE=false
UNINSTALL=false
DRY_RUN=false
while [ $# -gt 0 ]; do
  case "$1" in
    --host) [ $# -ge 2 ] || { echo "install.sh: --host needs a value" >&2; exit 1; }; HOST="$2"; shift 2 ;;
    --enable) ENABLE=true; shift ;;
    --uninstall) UNINSTALL=true; shift ;;
    --dry-run) DRY_RUN=true; shift ;;
    -h|--help) sed -n '2,/^set -euo/p' "$0" | sed '$d; s/^# \{0,1\}//'; exit 0 ;;
    *) echo "install.sh: unknown argument: $1" >&2; exit 1 ;;
  esac
done

# The files the plugin needs; tests and fixtures stay in the repo.
FILES=(manifest.json Panel.qml Model.js README.md)
for f in "${FILES[@]}"; do [ -f "$SRC/$f" ] || { echo "install.sh: missing $SRC/$f" >&2; exit 1; }; done

run() {  # run <command...>: here, or on $HOST
  if [ "$DRY_RUN" = true ]; then
    echo "would run${HOST:+ on $HOST}: $*"
  elif [ -n "$HOST" ]; then
    ssh "$HOST" "$(printf '%q ' "$@")"
  else
    "$@"
  fi
}
# Best effort: the shell may not be running (a remote machine at its login screen).
shell_ok() { run sh -c 'command -v omarchy-shell >/dev/null && omarchy-shell shell rescanPlugins >/dev/null 2>&1 || true'; }

DEST_REL=".config/omarchy/plugins/$NAME"

if [ "$UNINSTALL" = true ]; then
  run sh -c "command -v omarchy >/dev/null && omarchy plugin disable $ID >/dev/null 2>&1 || true"
  run sh -c "rm -rf \"\$HOME/$DEST_REL\""
  shell_ok
  echo "Removed the Agents village widget${HOST:+ from $HOST}."
  exit 0
fi

if [ -z "$HOST" ]; then
  DEST="$HOME/$DEST_REL"
  run mkdir -p "$(dirname "$DEST")"
  # A link, so this checkout is what the bar runs; replace a copy if one is there.
  [ -L "$DEST" ] || [ ! -e "$DEST" ] || run rm -rf "$DEST"
  run ln -sfn "$SRC" "$DEST"
else
  run sh -c "mkdir -p \"\$HOME/$DEST_REL\" && rm -f \"\$HOME/$DEST_REL\"/*"
  if [ "$DRY_RUN" = true ]; then
    echo "would copy ${FILES[*]} to $HOST:~/$DEST_REL/"
  else
    (cd "$SRC" && scp -q "${FILES[@]}" "$HOST:$DEST_REL/")
  fi
fi
shell_ok

if [ "$ENABLE" = true ]; then
  run sh -c "omarchy plugin enable $ID"
  echo "Added the Agents village widget to the bar${HOST:+ on $HOST}."
else
  echo "Installed the Agents village widget${HOST:+ on $HOST}. Add it to the bar with:"
  echo "  ${HOST:+ssh $HOST }omarchy plugin enable $ID"
  echo "or from the bar's widget settings (Agents village, under AI)."
fi
