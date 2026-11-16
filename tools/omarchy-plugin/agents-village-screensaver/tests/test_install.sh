#!/usr/bin/env bash
# Tests for install.sh --screensaver, --notify and --enable, and the
# screensaver's launcher, in throwaway HOMEs with stand-ins for the omarchy
# commands. One file per feature in tests/install/ (#522): a new feature or
# page option adds its own file there, and this runs every one.
#   bash tools/omarchy-plugin/agents-village-screensaver/tests/test_install.sh
set -uo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/install" && pwd)"
failed=""
for t in "$DIR"/*.sh; do
  [ "$(basename "$t")" = lib.sh ] && continue
  bash "$t" || failed="$failed $(basename "$t")"
done
[ -z "$failed" ] && echo "all passed" || { echo "failed:$failed"; exit 1; }
