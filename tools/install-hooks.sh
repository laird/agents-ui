#!/usr/bin/env bash
# install-hooks.sh — install this repo's git hooks: a pre-push hook that cuts a
# GitHub Release for each push of master made from the main checkout (see
# tools/hooks/pre-push). Pushes from the swarm's worktrees and merge jobs never
# release.
#
# USAGE:
#   tools/install-hooks.sh [--uninstall]
#
# The hook goes in the hooks directory every checkout shares (core.hooksPath,
# else <git common dir>/hooks), as a two-line shim that runs the main
# checkout's tools/hooks/pre-push, so it follows the repo as that file changes.
# An existing pre-push hook that is not this one is left alone: the script stops
# and says so.
set -euo pipefail

UNINSTALL=false
case "${1:-}" in
  --uninstall) UNINSTALL=true ;;
  "") ;;
  -h|--help) sed -n '2,/^set -euo/p' "$0" | sed '$d; s/^# \{0,1\}//'; exit 0 ;;
  *) echo "install-hooks.sh: unknown argument: $1" >&2; exit 1 ;;
esac

common=$(cd "$(git rev-parse --git-common-dir)" && pwd -P)
main=$(cd "$common/.." && pwd -P)
hooks=$(git config core.hooksPath || true)
case "$hooks" in
  "") hooks="$common/hooks" ;;
  /*) ;;
  *) hooks="$main/$hooks" ;;
esac
hook="$hooks/pre-push"
MARK="# agents-ui release hook"

if [ -e "$hook" ] && ! grep -qF "$MARK" "$hook"; then
  echo "install-hooks.sh: $hook exists and is not this repo's; leaving it alone." >&2
  echo "  To release on push as well, call tools/hooks/pre-push from it: exec bash \"$main/tools/hooks/pre-push\" \"\$@\"" >&2
  exit 1
fi
if [ "$UNINSTALL" = true ]; then
  rm -f "$hook"
  echo "install-hooks.sh: removed $hook"
  exit 0
fi
mkdir -p "$hooks"
printf '#!/usr/bin/env bash\n%s (tools/install-hooks.sh)\nexec bash "%s/tools/hooks/pre-push" "$@"\n' "$MARK" "$main" > "$hook"
chmod +x "$hook"
echo "install-hooks.sh: installed $hook"
echo "  A push of master from $main now releases it (log: ~/.agents-ui/release.log)."
echo "  AGENTS_UI_NO_RELEASE=1 git push ... pushes without releasing."
