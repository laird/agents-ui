#!/usr/bin/env bash
# --cycle S (#382): refused when not whole seconds.
#   bash tools/omarchy-plugin/agents-village-screensaver/tests/install/option-cycle.sh (tests/test_install.sh runs them all)
. "$(dirname "${BASH_SOURCE[0]}")/lib.sh"

bash "$INSTALL" --screensaver --cycle x --dry-run >/dev/null 2>&1 && fail "bad --cycle accepted" || ok "bad --cycle refused"
bash "$INSTALL" --screensaver --cycle 010 --dry-run >/dev/null 2>&1 && fail "--cycle 010 accepted" || ok "a leading zero is refused (jq may not read it)"
bash "$INSTALL" --screensaver --cycle 0 --dry-run >/dev/null 2>&1 && ok "--cycle 0 taken (stays on one swarm)" || fail "--cycle 0 refused"

finish
