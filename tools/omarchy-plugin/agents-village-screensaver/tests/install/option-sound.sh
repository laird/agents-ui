#!/usr/bin/env bash
# --sound (#489): off unless asked; with it, the page gets sound=1 and the
# kiosk windows may play audio without a click.
#   bash tools/omarchy-plugin/agents-village-screensaver/tests/install/option-sound.sh (tests/test_install.sh runs them all)
. "$(dirname "${BASH_SOURCE[0]}")/lib.sh"

sandbox
echo '{ "idle": { "screensaver": 120 } }' > "$HOME/.config/omarchy/shell.json"
cfg="$HOME/.config/agents-ui/screensaver.json"
bash "$INSTALL" --screensaver >/dev/null 2>&1 || fail "install exited"
[ "$(jq -r .sound "$cfg")" = false ] && ok "sound off unless asked" || fail "sound: $(cat "$cfg")"
bash "$INSTALL" --screensaver --sound >/dev/null 2>&1 || fail "install --sound exited"
[ "$(jq -c .sound "$cfg")" = true ] && ok "--sound writes true (a boolean)" || fail "sound: $(cat "$cfg")"
rm -rf "$T"

bash "$INSTALL" --sound --dry-run >/dev/null 2>&1 && fail "--sound without --screensaver accepted" || ok "--sound needs --screensaver"
out=$(bash "$INSTALL" --screensaver --sound --dry-run 2>&1) && grep -q "sound on" <<<"$out" && ok "dry run says sound on" || fail "sound dry run: $out"
out=$(bash "$INSTALL" --screensaver --dry-run 2>&1) && ! grep -q "sound" <<<"$out" && ok "dry run says nothing of sound when off" || fail "sound dry run: $out"

sandbox
mkdir -p "$HOME/.config/agents-ui"
echo '{ "server": "http://127.0.0.1:9" }' > "$HOME/.config/agents-ui/screensaver.json"
out=$(bash "$LAUNCHER" --dry-run 2>&1) || fail "launcher dry run exited: $out"
grep -q "sound=false" <<<"$out" && ! grep -q "sound%3D1" <<<"$out" && ! grep -q "autoplay" <<<"$out" && ok "launcher: no sound unless set" || fail "sound: $out"
echo '{ "server": "http://127.0.0.1:9", "sound": true }' > "$HOME/.config/agents-ui/screensaver.json"
out=$(bash "$LAUNCHER" --dry-run 2>&1) || fail "launcher dry run exited: $out"
grep -q "cycle%3D30%26sound%3D1" <<<"$out" && grep -q "sound=true" <<<"$out" && ok "launcher asks the page for sound when set" || fail "sound: $out"
grep -q "^args:    --autoplay-policy=no-user-gesture-required" <<<"$out" && ok "launcher lets the kiosk windows play audio" || fail "autoplay: $out"
rm -rf "$T"

finish
