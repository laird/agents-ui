#!/usr/bin/env bash
# --cycle S (#382): refused when not whole seconds.
#   bash tools/omarchy-plugin/agents-village-screensaver/tests/install/option-cycle.sh (tests/test_install.sh runs them all)
. "$(dirname "${BASH_SOURCE[0]}")/lib.sh"

bash "$INSTALL" --screensaver --cycle x --dry-run >/dev/null 2>&1 && fail "bad --cycle accepted" || ok "bad --cycle refused"

finish
