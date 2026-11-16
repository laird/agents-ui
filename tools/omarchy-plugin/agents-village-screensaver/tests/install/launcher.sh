#!/usr/bin/env bash
# the launcher's --dry-run: the page it opens, from the settings file.
#   bash tools/omarchy-plugin/agents-village-screensaver/tests/install/launcher.sh (tests/test_install.sh runs them all)
. "$(dirname "${BASH_SOURCE[0]}")/lib.sh"

# --- the launcher's dry run: the page it opens, from the settings file
sandbox
mkdir -p "$HOME/.config/agents-ui"
echo '{ "server": "http://127.0.0.1:9", "cycle": 45, "dim": "22-6" }' > "$HOME/.config/agents-ui/screensaver.json"
out=$(bash "$LAUNCHER" --dry-run 2>&1) || fail "launcher dry run exited: $out"
# Port 9 does not answer, so it opens the local "can't reach" page with the village after #.
grep -q "opens:   file://.*/offline.html#http%3A%2F%2F127.0.0.1%3A9%2Fvillage%3Fscreensaver%3D1%26dismiss%3D1%26cycle%3D45%26dim%3D22-6" <<<"$out" \
  && ok "launcher opens the offline page for a server that does not answer" || fail "launcher: $out"
grep -q "class:   org.omarchy.screensaver" <<<"$out" && ok "launcher window is Omarchy's screensaver class" || fail "class: $out"

finish
