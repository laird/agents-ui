#!/usr/bin/env bash
# Tests for tools/deploy.sh. Each case runs the real script in a throwaway git
# repo (with its own origin) and stub `systemctl`, `gh`, `cargo`, `curl` and
# `tailscale` on PATH, so nothing is installed, restarted or recorded for real.
#
#   bash tools/test_deploy.sh        # also run by `cargo test deploy_script`
set -uo pipefail

HERE=$(cd "$(dirname "$0")" && pwd)
SCRIPT="$HERE/deploy.sh"
# Resolved (pwd -P): on macOS mktemp's /var/folders is a link to /private/var,
# and git and the scripts report the resolved path.
TMP=$(cd "$(mktemp -d -t deploy-test.XXXXXX)" && pwd -P)
trap 'rm -rf "$TMP"' EXIT
FAILS=0

# ---- stubs ---------------------------------------------------------------
STUBS="$TMP/bin"; mkdir -p "$STUBS"
cat > "$STUBS/systemctl" <<'EOF'
#!/usr/bin/env bash
echo "systemctl $*" >> "$STUB_LOG"
case "$*" in
  "--user cat agents-tui.service")
    [ "${STUB_UNIT:-present}" = present ] || { echo "No files found for agents-tui.service." >&2; exit 1; }
    printf '# ~/.config/systemd/user/agents-tui.service\n[Service]\n%s\n' "${STUB_EXECSTART:-ExecStart=%h/.cargo/bin/agents-tui --headless --web-port 7890 --web-bind 127.0.0.1,100.98.1.2 --source swarms=herdr}" ;;
  "--user restart agents-tui.service") exit "${STUB_RESTART_RC:-0}" ;;
  *) exit 0 ;;
esac
EOF
cat > "$STUBS/gh" <<'EOF'
#!/usr/bin/env bash
echo "gh $*" >> "$STUB_LOG"
case "$*" in
  "release create"*) exit "${STUB_CREATE_RC:-0}" ;;
  "release view"*"--json url"*) echo "https://github.com/o/r/releases/tag/x" ;;
  "release view"*) exit 1 ;;
  *"/statuses"*) ;;
  *deployments*) echo 42 ;;
esac
EOF
cat > "$STUBS/cargo" <<'EOF'
#!/usr/bin/env bash
echo "cargo $*" >> "$STUB_LOG"
EOF
cat > "$STUBS/curl" <<'EOF'
#!/usr/bin/env bash
echo "curl ${*: -1}" >> "$STUB_LOG"
[ "${STUB_ANSWER:-yes}" = yes ]
EOF
cat > "$STUBS/tailscale" <<'EOF'
#!/usr/bin/env bash
echo 100.64.0.9
EOF
chmod +x "$STUBS"/*

# ---- a landed checkout -----------------------------------------------------
git init -q --bare -b master "$TMP/origin.git"
git init -q -b master "$TMP/work"
(
  cd "$TMP/work"
  git config user.email t@example.com; git config user.name test
  mkdir tools; cp "$SCRIPT" tools/deploy.sh; cp "$HERE/release.sh" tools/release.sh
  echo '{"environments":[{"name":"nuc-1"}]}' > .autocoder.json
  git add -A; git commit -qm "landed"
  git remote add origin "$TMP/origin.git"; git push -q origin master
)

# run <name> [env assignments...] -- [deploy.sh args...]: sets RC, OUT, LOG
run() {
  local name=$1; shift
  local envs=()
  while [ $# -gt 0 ] && [ "$1" != -- ]; do envs+=("$1"); shift; done
  [ "${1:-}" = -- ] && shift
  export STUB_LOG="$TMP/$name.log"; : > "$STUB_LOG"
  OUT=$(cd "$TMP/work" && env -u AGENTS_TUI_URL -u AGENTS_TUI_WAIT HOME="$TMP/home" PATH="$STUBS:$PATH" \
    ${envs[@]+"${envs[@]}"} bash tools/deploy.sh "$@" 2>&1)
  RC=$?
  LOG=$(cat "$STUB_LOG")
  CASE=$name
}
check() {  # check <description> <command...>
  if ! "${@:2}"; then
    echo "FAIL [$CASE] $1"; echo "  rc=$RC"; echo "$OUT" | sed 's/^/  out: /'; echo "$LOG" | sed 's/^/  log: /'
    FAILS=$((FAILS + 1))
  fi
}
has() { grep -qF -- "$1" <<< "$2"; }
hasnt() { ! grep -qF -- "$1" <<< "$2"; }

# ---- cases -----------------------------------------------------------------
run missing-unit STUB_UNIT=missing --
check "exits 1" test "$RC" -eq 1
check "says the unit is missing" has "agents-tui.service is not installed" "$OUT"
check "says how to install it" has "cp docs/agents-tui.service ~/.config/systemd/user/" "$OUT"
check "opens no Deployment" hasnt "gh " "$LOG"
check "installs nothing" hasnt "cargo " "$LOG"
check "restarts nothing" hasnt "restart" "$LOG"

run missing-unit-dry STUB_UNIT=missing -- --dry-run
check "dry run exits 1 too" test "$RC" -eq 1
check "dry run says the unit is missing" has "agents-tui.service is not installed" "$OUT"

run dry-run -- --dry-run
check "exits 0" test "$RC" -eq 0
check "names the unit it would restart" has "would run: systemctl --user restart agents-tui.service" "$OUT"
check "names the URL it would check" has "http://127.0.0.1:7890/api/village" "$OUT"
check "records nothing" hasnt "gh " "$LOG"
check "installs nothing" hasnt "cargo " "$LOG"

run inactive-unit --
check "exits 0" test "$RC" -eq 0
check "installs" has "cargo install --path . --locked" "$LOG"
check "restarts the unit, inactive or not" has "systemctl --user restart agents-tui.service" "$LOG"
check "checks the unit's own address" has "curl http://127.0.0.1:7890/api/village" "$LOG"
check "records success" has "state=success" "$LOG"

run unanswering STUB_ANSWER=no AGENTS_TUI_WAIT=1 --
check "exits 2" test "$RC" -eq 2
check "says it did not answer" has "did not answer within 1s" "$OUT"
check "records failure" has "state=failure" "$LOG"
check "does not record success" hasnt "state=success" "$LOG"

run restart-fails STUB_RESTART_RC=1 --
check "exits 2" test "$RC" -eq 2
check "records failure" has "state=failure" "$LOG"

run url-override AGENTS_TUI_URL=http://127.0.0.1:9999/ --
check "checks AGENTS_TUI_URL" has "curl http://127.0.0.1:9999/api/village" "$LOG"

run default-port "STUB_EXECSTART=ExecStart=/usr/bin/agents-tui --headless" -- --dry-run
check "defaults to 127.0.0.1:7878" has "http://127.0.0.1:7878/api/village" "$OUT"

run tailnet-only "STUB_EXECSTART=ExecStart=/usr/bin/agents-tui --headless --web-port=7890 --web-bind=tailscale" -- --dry-run
check "resolves a tailnet-only bind" has "http://100.64.0.9:7890/api/village" "$OUT"

# ---- the release that follows every deploy --------------------------------------
run releases --
check "exits 0" test "$RC" -eq 0
check "records the nuc-1 Deployment first" has "environment=nuc-1" "$LOG"
check "then releases the same commit under a date tag" has "gh release create v$(date +%Y.%m.%d)." "$LOG"
check "and records the release Deployment" has "environment=release" "$LOG"
SHA=$(git -C "$TMP/work" rev-parse HEAD)
check "of the deployed commit" has "--target $SHA" "$LOG"

run release-fails STUB_CREATE_RC=1 --
check "exits 3" test "$RC" -eq 3
check "the deploy still stands" has "state=success" "$LOG"
check "and never records failure" hasnt "state=failure" "$LOG"
check "says the release failed and will be retried" has "but the release failed; the next deploy releases it" "$OUT"

run no-release -- --no-release
check "exits 0" test "$RC" -eq 0
check "deploys" has "state=success" "$LOG"
check "does not release" hasnt "release create" "$LOG"
check "says so" has "not releasing (--no-release)" "$OUT"

run dry-run-release -- --dry-run
check "names the tag and commit it would release" has "release: dry run: would run: gh release create v$(date +%Y.%m.%d)." "$OUT"
run dry-run-no-release -- --dry-run --no-release
check "a dry run with --no-release says it would not release" has "would not release (--no-release)" "$OUT"

# Released already: tag the deployed commit on origin, and a redeploy cuts no second release.
git -C "$TMP/work" tag "v$(date +%Y.%m.%d).1" && git -C "$TMP/work" push -q origin --tags
run redeploy --
check "exits 0" test "$RC" -eq 0
check "deploys again" has "state=success" "$LOG"
check "but does not release the same commit twice" hasnt "release create" "$LOG"

run no-restart -- --no-restart
check "exits 0" test "$RC" -eq 0
check "leaves the unit alone" hasnt "restart" "$LOG"
run no-restart-missing STUB_UNIT=missing -- --no-restart
check "--no-restart still needs the unit" test "$RC" -eq 1

if [ "$FAILS" -gt 0 ]; then echo "deploy.sh tests: $FAILS failed"; exit 1; fi
echo "deploy.sh tests: all passed"
