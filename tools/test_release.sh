#!/usr/bin/env bash
# Tests for tools/release.sh and the pre-push release hook. Each case runs the
# real scripts in a throwaway git repo with its own origin and a stub `gh` on
# PATH, so nothing is released or recorded for real.
#
#   bash tools/test_release.sh       # also run by `cargo test release_script`
set -uo pipefail

HERE=$(cd "$(dirname "$0")" && pwd)
# Resolved (pwd -P): on macOS mktemp's /var/folders is a link to /private/var,
# and install-hooks.sh writes the resolved path into the hook.
TMP=$(cd "$(mktemp -d -t release-test.XXXXXX)" && pwd -P)
trap 'rm -rf "$TMP"' EXIT
FAILS=0

STUBS="$TMP/bin"; mkdir -p "$STUBS"
cat > "$STUBS/gh" <<'EOF'
#!/usr/bin/env bash
echo "gh $*" >> "$STUB_LOG"
case "$*" in
  "release create"*) exit "${STUB_CREATE_RC:-0}" ;;
  "release view"*"--json url"*) echo "https://github.com/o/r/releases/tag/x" ;;
  "release view"*) exit 1 ;;
  *"/statuses"*) ;;
  *deployments*) echo 7 ;;
esac
EOF
chmod +x "$STUBS/gh"

git init -q --bare -b master "$TMP/origin.git"
git init -q -b master "$TMP/main"
(
  cd "$TMP/main"
  git config user.email t@example.com; git config user.name test
  mkdir -p tools/hooks
  cp "$HERE/release.sh" "$HERE/install-hooks.sh" tools/; cp "$HERE/hooks/pre-push" tools/hooks/
  echo one > file; git add -A; git commit -qm one
  git remote add origin "$TMP/origin.git"; git push -q origin master
)
ONE=$(git -C "$TMP/main" rev-parse HEAD)

# rel <name> [env...] -- [release.sh args...]: sets RC, OUT, LOG
rel() {
  local name=$1; shift
  local envs=()
  while [ $# -gt 0 ] && [ "$1" != -- ]; do envs+=("$1"); shift; done
  [ "${1:-}" = -- ] && shift
  export STUB_LOG="$TMP/$name.log"; : > "$STUB_LOG"
  OUT=$(cd "$TMP/main" && env HOME="$TMP/home" PATH="$STUBS:$PATH" RELEASE_DATE=2026.11.09 ${envs[@]+"${envs[@]}"} bash tools/release.sh "$@" 2>&1)
  RC=$?
  LOG=$(cat "$STUB_LOG")
  CASE=$name
}
check() {
  if ! "${@:2}"; then
    echo "FAIL [$CASE] $1"; echo "  rc=$RC"; echo "$OUT" | sed 's/^/  out: /'; echo "$LOG" | sed 's/^/  log: /'
    FAILS=$((FAILS + 1))
  fi
}
has() { grep -qF -- "$1" <<< "$2"; }
hasnt() { ! grep -qF -- "$1" <<< "$2"; }
tag_origin() { git -C "$TMP/main" tag -f "$1" "$2" >/dev/null && git -C "$TMP/main" push -q -f origin "refs/tags/$1"; }

# ---- tags --------------------------------------------------------------------
rel first-of-day -- --next-tag
check "the day's first release is .1" test "$OUT" = v2026.11.09.1
rel first-auto --
check "exits 0" test "$RC" -eq 0
check "releases the tip under the day's first tag" has "gh release create v2026.11.09.1 --target $ONE --generate-notes" "$LOG"
check "records a release Deployment" has "environment=release" "$LOG"
check "does not look up a tag it just made up" test "$(grep -cx 'gh release view v2026.11.09.1' <<< "$LOG")" -eq 0

tag_origin v2026.11.09.1 "$ONE"
tag_origin v2026.11.08.4 "$ONE"
rel next-of-day -- --next-tag
check "counts up within the day, ignoring other days" test "$OUT" = v2026.11.09.2

# ---- no empty releases -------------------------------------------------------
rel unchanged --
check "exits 0" test "$RC" -eq 0
check "says it is already released" has "is already the newest release (v2026.11.09.1)" "$OUT"
check "creates no release" hasnt "release create" "$LOG"

(cd "$TMP/main" && echo two > file && git commit -qam two && git push -q origin master)
TWO=$(git -C "$TMP/main" rev-parse HEAD)
rel dry-run -- --dry-run
check "names the tag and commit" has "would run: gh release create v2026.11.09.2 --target $TWO" "$OUT"
check "changes nothing" hasnt "gh " "$LOG"
rel release-fails STUB_CREATE_RC=1 --
check "exits 1" test "$RC" -eq 1
check "says gh failed" has "gh release create failed" "$OUT"
check "records no Deployment for a release that does not exist" hasnt "environment=release" "$LOG"

# The newest release is found by number, not by text: .10 is after .9.
tag_origin v2026.11.09.9 "$ONE"
tag_origin v2026.11.09.10 "$TWO"
rel newest-by-number --
check "takes .10 as newer than .9" has "is already the newest release (v2026.11.09.10)" "$OUT"
check "so releases nothing" hasnt "release create" "$LOG"

# ---- the pre-push hook ---------------------------------------------------------
# A stand-in release.sh that only logs what it was asked to release.
cat > "$TMP/main/tools/release.sh" <<EOF
#!/usr/bin/env bash
echo "release \$*" >> "$TMP/released"
EOF
(cd "$TMP/main" && HOME="$TMP/home" bash tools/install-hooks.sh >/dev/null)
CASE=install
check "installs a pre-push hook" test -x "$TMP/main/.git/hooks/pre-push"
check "the hook runs the main checkout's script" grep -qF "$TMP/main/tools/hooks/pre-push" "$TMP/main/.git/hooks/pre-push"
git -C "$TMP/main" worktree add -q -b worker "$TMP/main-wt-1" master

# push_from <dir> <refspec> [env...]: commit and push, then wait for the hook's job.
push_from() {
  local dir=$1 spec=$2; shift 2
  : > "$TMP/released"
  (cd "$dir" && git config user.email t@example.com && git config user.name test \
    && echo "$RANDOM" > file && git commit -qam change \
    && env HOME="$TMP/home" AGENTS_UI_RELEASE_WAIT=10 "$@" git push -q origin "$spec" 2>/dev/null)
  PUSHED=$(git -C "$dir" rev-parse HEAD)
  for _ in $(seq 1 40); do [ -s "$TMP/released" ] && break; sleep 0.25; done
  RELEASED=$(cat "$TMP/released")
}
push_from "$TMP/main" master
CASE=hook-main
check "a push of master from the main checkout releases the pushed commit" test "$RELEASED" = "release --commit $PUSHED"

git -C "$TMP/main-wt-1" fetch -q origin && git -C "$TMP/main-wt-1" reset -q --hard origin/master
push_from "$TMP/main-wt-1" HEAD:master
CASE=hook-worktree
check "a push of master from a worktree (a worker, a merge job) does not release" test -z "$RELEASED"

push_from "$TMP/main" HEAD:refs/heads/feature/x
CASE=hook-branch
check "pushing another branch from the main checkout does not release" test -z "$RELEASED"

git -C "$TMP/main" fetch -q origin && git -C "$TMP/main" reset -q --hard origin/master
push_from "$TMP/main" master AGENTS_UI_NO_RELEASE=1
CASE=hook-opt-out
check "AGENTS_UI_NO_RELEASE=1 pushes without releasing" test -z "$RELEASED"

echo "not ours" > "$TMP/main/.git/hooks/pre-push"
OUT=$(cd "$TMP/main" && bash tools/install-hooks.sh 2>&1); RC=$?; CASE=foreign-hook
check "leaves someone else's pre-push hook alone" test "$RC" -eq 1
check "and keeps it" grep -qx "not ours" "$TMP/main/.git/hooks/pre-push"

if [ "$FAILS" -gt 0 ]; then echo "release.sh tests: $FAILS failed"; exit 1; fi
echo "release.sh tests: all passed"
