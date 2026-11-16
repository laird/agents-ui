#!/usr/bin/env bash
# install.sh --notify (#480): the service linked and enabled, its settings written.
#   bash tools/omarchy-plugin/agents-village-screensaver/tests/install/notify.sh (tests/test_install.sh runs them all)
. "$(dirname "${BASH_SOURCE[0]}")/lib.sh"

# --- notifications (--notify, #480): the service linked and enabled, its settings written
sandbox
mkdir -p "$HOME/.config/agents-ui"
echo '{ "kinds": ["build"], "interval": 30 }' > "$HOME/.config/agents-ui/notify.json"
out=$(bash "$INSTALL" --notify --server http://box:7890/ --quiet 22-6 2>&1) || fail "notify install exited $?: $out"
n="$HOME/.config/agents-ui/notify.json"
[ "$(jq -c . "$n")" = '{"kinds":["build"],"interval":30,"server":"http://box:7890","quiet":"22-6"}' ] \
  && ok "notify: server and quiet written, other settings kept" || fail "notify.json: $(cat "$n")"
[ -L "$HOME/.config/omarchy/plugins/agents-village-notify" ] && ok "notify: plugin linked" || fail "notify: not linked"
grep -q "omarchy plugin enable agents-ui.village-notify" "$T/calls" && ok "notify: service enabled" || fail "notify: not enabled: $(cat "$T/calls")"
[ ! -e "$HOME/.config/agents-ui/screensaver.json" ] && ok "notify: the screensaver is left alone" || fail "notify wrote screensaver.json"
bash "$INSTALL" --notify >/dev/null 2>&1 || fail "notify reinstall"
[ "$(jq -r .quiet "$n")" = 22-6 ] && [ "$(jq -r .server "$n")" = http://box:7890 ] \
  && ok "notify: reinstall without --quiet or --server keeps both" || fail "notify reinstall: $(cat "$n")"
out=$(bash "$INSTALL" --notify --uninstall 2>&1) || fail "notify uninstall exited: $out"
[ ! -e "$HOME/.config/omarchy/plugins/agents-village-notify" ] && [ ! -e "$n" ] && ok "notify: uninstall removes it and its settings" || fail "notify uninstall left things"
grep -q "omarchy plugin disable agents-ui.village-notify" "$T/calls" && ok "notify: uninstall disables it" || fail "notify: not disabled"
rm -rf "$T"
remote_sandbox
out=$(bash "$INSTALL" --notify --host box 2>&1) || fail "remote notify install exited $?: $out"
rn="$REMOTE/home/.config/omarchy/plugins/agents-village-notify"
[ -f "$rn/Model.js" ] && [ ! -L "$rn/Model.js" ] && [ -f "$rn/Notify.js" ] && [ -f "$rn/Service.qml" ] \
  && ok "notify: remote copy has Notify.js and Model.js" || fail "notify remote copy: $(ls -la "$rn" 2>&1)"
grep -q "omarchy plugin enable agents-ui.village-notify (OMARCHY_PATH=/fake/omarchy)" "$T/remote-calls" \
  && ok "notify: remote install enables it with OMARCHY_PATH set" || fail "notify remote enable: $(cat "$T/remote-calls")"
rm -rf "$T"
export HOME=$ORIG_HOME PATH=$ORIG_PATH

# --- bad arguments
bash "$INSTALL" --notify --quiet 7 --dry-run >/dev/null 2>&1 && fail "bad --quiet accepted" || ok "bad --quiet refused"
bash "$INSTALL" --notify --quiet 7-7 --dry-run >/dev/null 2>&1 && fail "--quiet 7-7 accepted" || ok "--quiet with one hour twice refused"
bash "$INSTALL" --notify --quiet 08-09 --dry-run >/dev/null 2>&1 && ok "--quiet with leading zeros taken" || fail "--quiet 08-09 refused"
bash "$INSTALL" --quiet 23-7 --dry-run >/dev/null 2>&1 && fail "--quiet without --notify accepted" || ok "--quiet needs --notify"

finish
