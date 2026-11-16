#!/usr/bin/env bash
# --dim H-H (#382): refused when not two hours.
#   bash tools/omarchy-plugin/agents-village-screensaver/tests/install/option-dim.sh (tests/test_install.sh runs them all)
. "$(dirname "${BASH_SOURCE[0]}")/lib.sh"

bash "$INSTALL" --screensaver --dim 25-3 --dry-run >/dev/null 2>&1 && fail "bad --dim accepted" || ok "bad --dim refused"

finish
