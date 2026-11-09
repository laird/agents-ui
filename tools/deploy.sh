#!/usr/bin/env bash
# deploy.sh — install this checkout on this machine, restart the served
# agents-tui, and record it as a GitHub Deployment so the village moves the
# deployed issues from the quay to the environment's stop.
#
# The village places an environment at the commit of its newest *successful*
# Deployment (src/web/deployments.rs). This script is what makes one: it opens
# the Deployment before installing, then marks it `success` once the server
# answers, or `failure` if the install or restart did not work.
#
# USAGE:
#   tools/deploy.sh [--env nuc-1] [--dry-run] [--no-restart]
#
#   --env         environment to record (default: nuc-1; must be listed in .autocoder.json)
#   --dry-run     run every check and print what would happen; install, restart and
#                 record nothing
#   --no-restart  install and record, but leave running servers alone
#
# What it refuses:
#   - a dirty tree (what is installed must be exactly the recorded commit;
#     harness state under .claude/ is ignored)
#   - a HEAD that is not on origin/<integration> (only landed work deploys)
#
# The served instance:
#   If a systemd user unit `agents-tui.service` exists it is restarted. Otherwise
#   this script runs the server itself, detached, and remembers its pid in
#   ~/.agents-ui/serve.pid so the next deploy can replace it:
#     agents-tui --headless --web-port $AGENTS_TUI_PORT --web-bind $AGENTS_TUI_BIND --read-only
#   AGENTS_TUI_PORT defaults to 7877; AGENTS_TUI_BIND to 127.0.0.1 plus this
#   machine's tailnet address (`tailscale ip -4`) when there is one. AGENTS_TUI_SERVE_ARGS replaces
#   the arguments entirely.
#
# EXIT: 0 deployed (or dry run passed) · 1 refused or setup error · 2 deploy failed (recorded as failure)
set -euo pipefail

ENVIRONMENT="nuc-1"
DRY_RUN=false
RESTART=true
while [ $# -gt 0 ]; do
  case "$1" in
    --env) [ $# -ge 2 ] || { echo "deploy.sh: --env needs a value" >&2; exit 1; }; ENVIRONMENT="$2"; shift 2 ;;
    --dry-run) DRY_RUN=true; shift ;;
    --no-restart) RESTART=false; shift ;;
    -h|--help) sed -n '2,/^set -euo/p' "$0" | sed '$d; s/^# \{0,1\}//'; exit 0 ;;
    *) echo "deploy.sh: unknown argument: $1" >&2; exit 1 ;;
  esac
done

[ -f "$HOME/.cargo/env" ] && source "$HOME/.cargo/env"
cd "$(git rev-parse --show-toplevel)"

say() { echo "deploy.sh: $*"; }
refuse() { echo "deploy.sh: refusing: $*" >&2; exit 1; }

# The environment must be one the village draws, or the deployment is invisible.
python3 - "$ENVIRONMENT" <<'PY' || refuse "environment '$ENVIRONMENT' is not in .autocoder.json"
import json, sys
envs = json.load(open(".autocoder.json")).get("environments", [])
sys.exit(0 if any(e.get("name") == sys.argv[1] for e in envs) else 1)
PY

INTEGRATION=$(git symbolic-ref --short refs/remotes/origin/HEAD 2>/dev/null | sed 's@^origin/@@' || true)
INTEGRATION="${INTEGRATION:-master}"

if [ -n "$(git status --porcelain --untracked-files=no -- . ':(exclude).claude')" ]; then
  git status --short --untracked-files=no -- . ':(exclude).claude' >&2
  refuse "the working tree has uncommitted changes; what is installed must be exactly the recorded commit"
fi

git fetch --quiet origin "$INTEGRATION" || refuse "cannot fetch origin/$INTEGRATION"
SHA=$(git rev-parse HEAD)
git merge-base --is-ancestor "$SHA" "origin/$INTEGRATION" \
  || refuse "HEAD ${SHA:0:12} is not on origin/$INTEGRATION; only landed work deploys"

say "environment: $ENVIRONMENT"
say "commit:      $SHA ($(git log -1 --format=%s "$SHA"))"

PORT="${AGENTS_TUI_PORT:-7877}"
# Localhost and the tailnet together, so the host's own browser keeps working.
TAILNET_IP="$(tailscale ip -4 2>/dev/null | head -1 || true)"
BIND="${AGENTS_TUI_BIND:-127.0.0.1${TAILNET_IP:+,$TAILNET_IP}}"
if [ -n "${AGENTS_TUI_SERVE_ARGS:-}" ]; then
  read -r -a SERVE_ARGS <<< "$AGENTS_TUI_SERVE_ARGS"
else
  SERVE_ARGS=(--headless --web-port "$PORT" --web-bind "$BIND" --read-only)
fi
PIDFILE="$HOME/.agents-ui/serve.pid"
LOGFILE="$HOME/.agents-ui/serve.log"
UNIT="agents-tui.service"
has_unit() { systemctl --user cat "$UNIT" >/dev/null 2>&1; }

if [ "$DRY_RUN" = true ]; then
  say "dry run: would record a Deployment of ${SHA:0:12} to '$ENVIRONMENT'"
  say "dry run: would run: cargo install --path . --locked"
  if [ "$RESTART" = false ]; then
    say "dry run: would not restart anything (--no-restart)"
  elif has_unit; then
    say "dry run: would run: systemctl --user restart $UNIT"
  else
    say "dry run: would (re)start: agents-tui ${SERVE_ARGS[*]} (pid in $PIDFILE)"
  fi
  say "dry run: checks passed, nothing changed"
  exit 0
fi

command -v gh >/dev/null || refuse "gh is not installed; it records the deployment"

# Open the deployment first, so a failed install is on record too.
DEPLOYMENT_ID=$(gh api "repos/{owner}/{repo}/deployments" \
  -f ref="$SHA" -f environment="$ENVIRONMENT" \
  -f description="agents-tui installed on $(hostname)" \
  -F auto_merge=false -F 'required_contexts[]' --jq .id) \
  || refuse "GitHub would not create the deployment"
status() {  # status <state> <description>
  gh api "repos/{owner}/{repo}/deployments/$DEPLOYMENT_ID/statuses" \
    -f state="$1" -f description="$2" >/dev/null \
    || echo "deploy.sh: could not record '$1' on deployment $DEPLOYMENT_ID" >&2
}
fail() { echo "deploy.sh: $*" >&2; status failure "$*"; exit 2; }
status in_progress "installing ${SHA:0:12}"
say "deployment $DEPLOYMENT_ID opened"

cargo install --path . --locked || fail "cargo install failed"
BIN="$(command -v agents-tui || echo "$HOME/.cargo/bin/agents-tui")"

if [ "$RESTART" = true ]; then
  if has_unit; then
    systemctl --user restart "$UNIT" || fail "systemctl --user restart $UNIT failed"
    sleep 2
    systemctl --user is-active --quiet "$UNIT" || fail "$UNIT is not running after the restart"
    say "restarted $UNIT"
  else
    mkdir -p "$(dirname "$PIDFILE")"
    if [ -f "$PIDFILE" ]; then
      OLD=$(cat "$PIDFILE")
      # Only stop what this script started: the pid must still be an agents-tui.
      if [ -n "$OLD" ] && ps -p "$OLD" -o comm= 2>/dev/null | grep -q '^agents-tui'; then
        kill "$OLD" 2>/dev/null || true
        for _ in $(seq 1 20); do ps -p "$OLD" >/dev/null 2>&1 || break; sleep 0.25; done
        ps -p "$OLD" >/dev/null 2>&1 && kill -9 "$OLD" 2>/dev/null || true
        say "stopped the previous server (pid $OLD)"
      fi
    fi
    setsid nohup "$BIN" "${SERVE_ARGS[@]}" >"$LOGFILE" 2>&1 < /dev/null &
    echo $! > "$PIDFILE"
    say "started $BIN ${SERVE_ARGS[*]} (pid $(cat "$PIDFILE"), log $LOGFILE)"
    # It is deployed when it answers, not when it was launched.
    if [ -z "${AGENTS_TUI_SERVE_ARGS:-}" ]; then
      for _ in $(seq 1 40); do
        curl -fsS -o /dev/null "http://$BIND:$PORT/api/village" 2>/dev/null && break
        ps -p "$(cat "$PIDFILE")" >/dev/null 2>&1 || fail "the server exited at startup (see $LOGFILE)"
        sleep 0.5
      done
      curl -fsS -o /dev/null "http://$BIND:$PORT/api/village" || fail "the server is not answering on $BIND:$PORT (see $LOGFILE)"
      say "serving http://$BIND:$PORT/village"
    else
      sleep 2
      ps -p "$(cat "$PIDFILE")" >/dev/null 2>&1 || fail "the server exited at startup (see $LOGFILE)"
    fi
  fi
fi

status success "agents-tui ${SHA:0:12} installed on $(hostname)"
say "deployed ${SHA:0:12} to $ENVIRONMENT"
