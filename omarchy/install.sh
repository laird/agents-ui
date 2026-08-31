#!/bin/bash
# Install the Omarchy integration for the agents-tui swarm dashboard.
#
# Idempotent: safe to re-run after a `git pull`. Config files are appended to
# only once, guarded by a marker line.

set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
MARKER="-- agents-tui swarm dashboard (installed by agents-ui/omarchy/install.sh)"

BIN_DIR="$HOME/.local/bin"
UNIT_DIR="$HOME/.config/systemd/user"
PLUGIN_DIR="$HOME/.config/omarchy/plugins"
HYPR_DIR="$HOME/.config/hypr"

step() { printf '\n\033[1m==> %s\033[0m\n' "$*"; }
warn() { printf '  \033[33m!\033[0m %s\n' "$*" >&2; }
ok()   { printf '  \033[32m✓\033[0m %s\n' "$*"; }

# --------------------------------------------------------------- preflight

command -v hyprctl >/dev/null || { echo "This installer expects Hyprland." >&2; exit 1; }
command -v curl    >/dev/null || { echo "curl is required (the bar widget polls with it)." >&2; exit 1; }

if ! command -v agents-tui >/dev/null; then
  warn "agents-tui is not on PATH. Run 'cargo install --path .' from the repo root."
fi

# Deliberately no "does the binary work" probe here. agents-tui ignores
# unrecognized flags rather than erroring, so `agents-tui --headless --help`
# does not print usage and exit -- it starts the daemon and blocks forever.
# Presence on PATH is the only safe check.

# ------------------------------------------------------------------- bin

step "Installing omarchy-agents-dashboard"
mkdir -p "$BIN_DIR"
install -m 0755 "$HERE/bin/omarchy-agents-dashboard" "$BIN_DIR/omarchy-agents-dashboard"
ok "$BIN_DIR/omarchy-agents-dashboard"

case ":$PATH:" in
  *":$BIN_DIR:"*) ;;
  *) warn "$BIN_DIR is not on your PATH; the keybinding will fail until it is." ;;
esac

# --------------------------------------------------------------- service

step "Installing systemd user service"
mkdir -p "$UNIT_DIR"

# The unit ships with no AGENTS_DIR: agents-tui reads the autocoder plugin from
# Claude Code's installed_plugins.json, an absolute path that works with no
# meaningful cwd. Pointing it at a checkout is a per-machine choice, so it stays
# a commented line in the unit rather than something this installer guesses at.
install -m 0644 "$HERE/agents-ui.service" "$UNIT_DIR/agents-ui.service"

if ! claude plugin list 2>/dev/null | grep -q autocoder; then
  warn "The autocoder plugin is not installed; the daemon will fall back to an"
  warn "../agents checkout, which a systemd service cannot resolve."
  warn "Install it with: claude plugin install autocoder"
fi
systemctl --user daemon-reload
ok "$UNIT_DIR/agents-ui.service"

# ---------------------------------------------------------------- plugin

step "Installing the Agent Swarm bar widget"
mkdir -p "$PLUGIN_DIR"
rm -rf "$PLUGIN_DIR/agents.swarm"
cp -r "$HERE/plugins/agents-swarm" "$PLUGIN_DIR/agents.swarm"
ok "$PLUGIN_DIR/agents.swarm"
omarchy plugin validate "$PLUGIN_DIR/agents.swarm" \
  && ok "manifest validates" \
  || warn "omarchy plugin validate rejected the manifest"
echo "     Enable it with: omarchy plugin enable agents.swarm"

# ------------------------------------------------------------- hyprland

step "Wiring Hyprland"

BINDINGS="$HYPR_DIR/bindings.lua"
if [ ! -f "$BINDINGS" ]; then
  warn "$BINDINGS not found; skipping the keybinding."
elif grep -qF -e "$MARKER" "$BINDINGS"; then
  ok "bindings.lua already wired"
elif grep -qE '"SUPER \+ A"' "$BINDINGS"; then
  warn "SUPER + A is already bound in your bindings.lua; not overwriting."
  warn "Bind another key to: omarchy-agents-dashboard"
else
  cp "$BINDINGS" "$BINDINGS.bak.$(date +%s)"
  {
    echo ""
    echo "$MARKER"
    echo 'o.bind("SUPER + A", "Agent swarm dashboard", "omarchy-agents-dashboard")'
    echo 'o.window("agents-dashboard", { opacity = "1.0 1.0", tag = "-default-opacity" })'
  } >> "$BINDINGS"
  ok "SUPER + A -> omarchy-agents-dashboard (backup written alongside)"
fi

step "Validating Hyprland config"
if hyprctl reload >/dev/null 2>&1; then
  ERRORS="$(hyprctl configerrors 2>/dev/null || true)"
  if [ -n "$ERRORS" ] && [ "$ERRORS" != "no errors" ]; then
    warn "hyprctl reported config errors:"
    echo "$ERRORS" >&2
  else
    ok "config reloaded cleanly"
  fi
else
  warn "hyprctl reload failed; is Hyprland running?"
fi

cat <<EOF

Done. Next:

  systemctl --user enable --now agents-ui.service
  omarchy plugin enable agents.swarm

Then press SUPER + A.

Logs: journalctl --user -u agents-ui -f
EOF
