#!/usr/bin/env bash
# Every page option (agents-village-screensaver/options/*.sh, #522) is wired
# through without editing anything else: install.sh lists it in --help and
# writes its default to screensaver.json, and the launcher reads it back.
#   bash tools/omarchy-plugin/agents-village-screensaver/tests/install/options.sh (tests/test_install.sh runs them all)
. "$(dirname "${BASH_SOURCE[0]}")/lib.sh"

. "$ROOT/agents-village-screensaver/options.sh"
saver_load_options
[ "${#SAVER_KEYS[@]}" -ge 3 ] && ok "the option files load (${SAVER_KEYS[*]})" || fail "options: ${SAVER_KEYS[*]-none}"
help=$(bash "$INSTALL" --help)
sandbox
echo '{ "idle": { "screensaver": 150 } }' > "$HOME/.config/omarchy/shell.json"
bash "$INSTALL" --screensaver >/dev/null 2>&1 || fail "install exited"
cfg="$HOME/.config/agents-ui/screensaver.json"
i=0
while [ "$i" -lt "${#SAVER_KEYS[@]}" ]; do
  k=${SAVER_KEYS[$i]} flag=${SAVER_FLAGS[$i]} def=${SAVER_DEFAULTS[$i]}
  grep -qe "^  $flag " <<<"$help" && ok "$flag in --help" || fail "$flag missing from --help"
  [ "$(jq -r --arg k "$k" '.[$k] | tostring' "$cfg")" = "$def" ] && ok "$k written with its default ($def)" || fail "$k: $(cat "$cfg")"
  [ "${SAVER_TYPES[$i]}" != number ] || [ "$(jq -r --arg k "$k" '.[$k] | type' "$cfg")" = number ] && ok "$k is a ${SAVER_TYPES[$i]}" || fail "$k type: $(cat "$cfg")"
  [[ $def =~ ${SAVER_CHECKS[$i]} ]] && ok "$k's default passes its own check" || fail "$k default $def fails ${SAVER_CHECKS[$i]}"
  i=$((i + 1))
done
# A value given on the command line is the one written.
bash "$INSTALL" --screensaver --cycle 7 --dim 1-5 --recap 0 >/dev/null 2>&1 || fail "install with options exited"
[ "$(jq -c '{cycle, dim, recap}' "$cfg")" = '{"cycle":7,"dim":"1-5","recap":0}' ] && ok "given values written" || fail "given: $(cat "$cfg")"
# Page options go with --screensaver only.
out=$(bash "$INSTALL" --cycle 5 --dry-run 2>&1) && fail "--cycle without --screensaver accepted" || ok "a page option needs --screensaver"
grep -q -- "--cycle go with --screensaver" <<<"$out" && ok "says which option" || fail "message: $out"
rm -rf "$T"

finish
