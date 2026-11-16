#!/usr/bin/env bash
# install.sh --enable: the bar widget, and its enabling (#416).
#   bash tools/omarchy-plugin/agents-village-screensaver/tests/install/widget.sh (tests/test_install.sh runs them all)
. "$(dirname "${BASH_SOURCE[0]}")/lib.sh"

# --- the bar widget's --enable waits the same way
sandbox
echo 2 > "$T/enable-fails"
out=$(AGENTS_UI_ENABLE_PAUSE=0 bash "$INSTALL" --enable 2>&1) || fail "widget --enable gave up: $out"
[ "$(grep -c "omarchy plugin enable agents-ui.village$" "$T/calls")" = 3 ] && grep -q "Added the Agents village widget to the bar" <<<"$out" \
  && ok "widget --enable retries until Omarchy knows it" || fail "widget: $(cat "$T/calls") / $out"
echo 99 > "$T/enable-fails"
out=$(AGENTS_UI_ENABLE_TRIES=2 AGENTS_UI_ENABLE_PAUSE=0 bash "$INSTALL" --enable 2>&1) && fail "widget --enable claimed success: $out" || ok "widget --enable fails when it never takes"
grep -qF "omarchy plugin enable agents-ui.village" <<<"$out" && ok "widget says how to enable it by hand" || fail "message: $out"
rm -rf "$T"

finish
