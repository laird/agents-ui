#!/usr/bin/env bash
# install.sh --screensaver: settings written, Omarchy's idle settings changed and put back, enabling (#416), dry run.
#   bash tools/omarchy-plugin/agents-village-screensaver/tests/install/screensaver.sh (tests/test_install.sh runs them all)
. "$(dirname "${BASH_SOURCE[0]}")/lib.sh"

# --- install, then uninstall, with an idle section of the user's own
sandbox
cat > "$HOME/.config/omarchy/shell.json" <<'JSON'
{ "bar": { "position": "top" }, "idle": { "screensaver": 120, "lock": 600 } }
JSON
out=$(bash "$INSTALL" --screensaver --never-sleep --server http://box:7890/ --cycle 20 --dim 23-7 2>&1) || fail "install exited $?: $out"
cfg="$HOME/.config/agents-ui/screensaver.json"
[ "$(jq -r .server "$cfg")" = "http://box:7890" ] && ok "server written" || fail "server: $(cat "$cfg")"
[ "$(jq -r .cycle "$cfg")" = 20 ] && [ "$(jq -r .dim "$cfg")" = 23-7 ] && ok "cycle and dim written" || fail "cycle/dim: $(cat "$cfg")"
[ "$(jq -r .timeout "$cfg")" = 120 ] && ok "takes Omarchy's screensaver timeout" || fail "timeout: $(cat "$cfg")"
s="$HOME/.config/omarchy/shell.json"
[ "$(jq -c .idle "$s")" = '{"screensaver":0,"lock":0}' ] && ok "never-sleep: screensaver and lock off" || fail "idle: $(jq -c .idle "$s")"
[ "$(jq -r .bar.position "$s")" = top ] && ok "rest of shell.json kept" || fail "bar lost"
[ -L "$HOME/.config/omarchy/plugins/agents-village-screensaver" ] && ok "plugin linked" || fail "plugin not linked"
grep -q "omarchy plugin enable agents-ui.village-screensaver" "$T/calls" && ok "service enabled" || fail "not enabled: $(cat "$T/calls")"
grep -q "will no longer lock itself" <<<"$out" && ok "says plainly it will not lock" || fail "no lock warning: $out"

# Installing again keeps the first backup: uninstall still restores the original.
bash "$INSTALL" --screensaver >/dev/null 2>&1 || fail "second install"
[ "$(jq -r .timeout "$cfg")" = 120 ] && ok "reinstall keeps the original timeout" || fail "reinstall timeout: $(cat "$cfg")"
jq '.bar.position = "bottom"' "$s" > "$s.tmp" && mv "$s.tmp" "$s"
bash "$INSTALL" --screensaver --uninstall >/dev/null 2>&1 || fail "uninstall exited"
[ "$(jq -c .idle "$s")" = '{"screensaver":120,"lock":600}' ] && ok "uninstall restores idle" || fail "restored idle: $(jq -c .idle "$s")"
[ "$(jq -r .bar.position "$s")" = bottom ] && ok "uninstall keeps later changes" || fail "later change lost"
[ ! -e "$cfg" ] && [ ! -e "$HOME/.config/agents-ui/shell.json.before-screensaver" ] && ok "settings and backup removed" || fail "left files behind"
[ ! -e "$HOME/.config/omarchy/plugins/agents-village-screensaver" ] && ok "plugin removed" || fail "plugin left"
grep -q "omarchy plugin disable agents-ui.village-screensaver" "$T/calls" && ok "service disabled" || fail "not disabled"
rm -rf "$T"

# --- without --never-sleep the lock stays; no idle section at all is restored as none
sandbox
echo '{ "bar": {} }' > "$HOME/.config/omarchy/shell.json"
out=$(bash "$INSTALL" --screensaver 2>&1) || fail "install exited: $out"
s="$HOME/.config/omarchy/shell.json"
[ "$(jq -c .idle "$s")" = '{"screensaver":0}' ] && ok "lock left to Omarchy" || fail "idle: $(jq -c .idle "$s")"
[ "$(jq -r .timeout "$HOME/.config/agents-ui/screensaver.json")" = 150 ] && ok "default timeout 150" || fail "timeout"
grep -q "still locks" <<<"$out" && ok "says the lock stays" || fail "no lock note: $out"
bash "$INSTALL" --screensaver --uninstall >/dev/null 2>&1
[ "$(jq -c .idle "$s")" = null ] && ok "no idle section restored as none" || fail "idle: $(jq -c .idle "$s")"
rm -rf "$T"

# --- enabling waits for Omarchy to know the plugin (#416)
sandbox
s="$HOME/.config/omarchy/shell.json"
echo '{ "idle": { "screensaver": 150, "lock": 300 } }' > "$s"
echo 3 > "$T/enable-fails"
out=$(AGENTS_UI_ENABLE_PAUSE=0 bash "$INSTALL" --screensaver --never-sleep 2>&1) || fail "install gave up while Omarchy was still registering the plugin: $out"
[ "$(grep -c "omarchy plugin enable agents-ui.village-screensaver" "$T/calls")" = 4 ] && ok "retries enable until Omarchy knows the plugin" || fail "enable calls: $(cat "$T/calls")"
[ "$(grep -c "omarchy-shell shell rescanPlugins" "$T/calls")" -ge 4 ] && ok "rescans before each try" || fail "rescans: $(cat "$T/calls")"
[ "$(jq -c .idle "$s")" = '{"screensaver":0,"lock":0}' ] && ok "then turns Omarchy's screensaver off" || fail "idle: $(jq -c .idle "$s")"
rm -rf "$T"

# --- if enabling never takes, shell.json is left alone: never no screensaver at all
sandbox
s="$HOME/.config/omarchy/shell.json"
echo '{ "idle": { "screensaver": 150, "lock": 300 } }' > "$s"
echo 99 > "$T/enable-fails"
out=$(AGENTS_UI_ENABLE_TRIES=3 AGENTS_UI_ENABLE_PAUSE=0 bash "$INSTALL" --screensaver --never-sleep 2>&1) && fail "install claimed success without enabling: $out" || ok "fails when the plugin never gets enabled"
[ "$(grep -c "omarchy plugin enable agents-ui.village-screensaver" "$T/calls")" = 3 ] && ok "gives up after its tries" || fail "enable calls: $(cat "$T/calls")"
[ "$(jq -c .idle "$s")" = '{"screensaver":150,"lock":300}' ] && ok "shell.json unchanged: Omarchy's screensaver still on" || fail "idle: $(jq -c .idle "$s")"
[ ! -e "$HOME/.config/agents-ui/screensaver.json" ] && [ ! -e "$HOME/.config/agents-ui/shell.json.before-screensaver" ] && ok "no settings or backup written" || fail "wrote settings"
grep -q "is not known" <<<"$out" && grep -qF "omarchy-shell shell rescanPlugins && omarchy plugin enable agents-ui.village-screensaver" <<<"$out" \
  && ok "says why and how to enable it by hand" || fail "message: $out"
grep -q "left as they were" <<<"$out" && ok "says the idle settings were not touched" || fail "message: $out"
rm -rf "$T"

# --- a dry run changes nothing
sandbox
echo '{ "idle": { "screensaver": 150, "lock": 300 } }' > "$HOME/.config/omarchy/shell.json"
out=$(bash "$INSTALL" --screensaver --never-sleep --dry-run 2>&1) || fail "dry run exited: $out"
[ "$(jq -c .idle "$HOME/.config/omarchy/shell.json")" = '{"screensaver":150,"lock":300}' ] && [ ! -e "$HOME/.config/agents-ui" ] && [ ! -e "$T/calls" ] \
  && ok "dry run changes nothing" || fail "dry run changed things"
grep -q "would: write ~/.config/agents-ui/screensaver.json" <<<"$out" && grep -q "idle.lock to 0" <<<"$out" && ok "dry run says what it would do" || fail "dry run: $out"
rm -rf "$T"

# --- bad arguments
bash "$INSTALL" --never-sleep >/dev/null 2>&1 && fail "--never-sleep without --screensaver accepted" || ok "--never-sleep needs --screensaver"
bash "$INSTALL" --notify --screensaver --dry-run >/dev/null 2>&1 && fail "--notify with --screensaver accepted" || ok "--notify and --screensaver install separately"

finish
