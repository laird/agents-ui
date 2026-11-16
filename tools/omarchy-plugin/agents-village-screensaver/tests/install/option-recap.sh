#!/usr/bin/env bash
# --recap (#479): since= is the last unlock, after a long enough spell, never on a forced run.
#   bash tools/omarchy-plugin/agents-village-screensaver/tests/install/option-recap.sh (tests/test_install.sh runs them all)
. "$(dirname "${BASH_SOURCE[0]}")/lib.sh"

sandbox
mkdir -p "$HOME/.config/agents-ui"
echo '{ "server": "http://127.0.0.1:9" }' > "$HOME/.config/agents-ui/screensaver.json"
# "Since you last looked" (#479): since= is the last unlock of the machine,
# only after a spell of at least `recap` minutes, and never on a forced run.
export XDG_STATE_HOME="$T/state"
mkdir -p "$XDG_STATE_HOME/agents-village-screensaver"
NOW=1794800000
echo $((NOW - 7200)) > "$XDG_STATE_HOME/agents-village-screensaver/last-unlock"
out=$(AGENTS_VILLAGE_SCREENSAVER_NOW=$NOW bash "$LAUNCHER" --dry-run 2>&1)
grep -q "%26since%3D$((NOW - 7200))" <<<"$out" && ok "since= is the last unlock, two hours ago" || fail "since: $out"
out=$(AGENTS_VILLAGE_SCREENSAVER_NOW=$NOW bash "$LAUNCHER" force --dry-run 2>&1)
grep -q "since" <<<"$out" && fail "forced run asked for a recap: $out" || ok "a forced run has no recap"
echo $((NOW - 300)) > "$XDG_STATE_HOME/agents-village-screensaver/last-unlock"
out=$(AGENTS_VILLAGE_SCREENSAVER_NOW=$NOW bash "$LAUNCHER" --dry-run 2>&1)
grep -q "since" <<<"$out" && fail "five minutes asked for a recap: $out" || ok "a short spell has no recap"
echo $((NOW - 7200)) > "$XDG_STATE_HOME/agents-village-screensaver/last-unlock"
echo '{ "server": "http://127.0.0.1:9", "recap": 0 }' > "$HOME/.config/agents-ui/screensaver.json"
out=$(AGENTS_VILLAGE_SCREENSAVER_NOW=$NOW bash "$LAUNCHER" --dry-run 2>&1)
grep -q "since" <<<"$out" && fail "recap 0 still asked: $out" || ok "recap 0 turns it off"
echo '{ "server": "http://127.0.0.1:9", "recap": 1 }' > "$HOME/.config/agents-ui/screensaver.json"
rm "$XDG_STATE_HOME/agents-village-screensaver/last-unlock"
out=$(AGENTS_VILLAGE_SCREENSAVER_NOW=$NOW bash "$LAUNCHER" --dry-run 2>&1)
grep -q "since" <<<"$out" && fail "no unlock recorded still asked: $out" || ok "with no unlock recorded, no recap"
for junk in 0089 99999999999999999999999 x 17948x00000; do
  echo "$junk" > "$XDG_STATE_HOME/agents-village-screensaver/last-unlock"
  out=$(AGENTS_VILLAGE_SCREENSAVER_NOW=$NOW bash "$LAUNCHER" --dry-run 2>&1)
  grep -q "since" <<<"$out" && fail "junk $junk: $out" || ok "a corrupt record ($junk) is no record"
done
unset XDG_STATE_HOME
rm -rf "$T"
export HOME=$ORIG_HOME

finish
