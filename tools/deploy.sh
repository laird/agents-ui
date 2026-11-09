#!/usr/bin/env bash
# deploy.sh — install this checkout on this machine, restart the served
# agents-tui, and record it as a GitHub Deployment so the village moves the
# deployed issues from the quay to the environment's stop.
#
# The village places an environment at the commit of its newest *successful*
# Deployment (src/web/deployments.rs). This script is what makes one: it opens
# the Deployment before installing, then marks it `success` once the served
# village answers, or `failure` if the install, the restart or the check fails.
#
# USAGE:
#   tools/deploy.sh [--env nuc-1] [--dry-run] [--no-restart] [--no-release]
#
#   --env         environment to record (default: nuc-1; must be listed in .autocoder.json)
#   --dry-run     run every check and print the unit it would restart and the URL it
#                 would check; install, restart and record nothing
#   --no-restart  install and record, but leave the running server alone
#   --no-release  deploy without cutting a release afterwards
#
# Every deploy also releases: once the Deployment is recorded as `success`,
# tools/release.sh cuts a GitHub Release of the same commit under the next
# vYYYY.MM.DD.N tag, so the village's ship sails. A commit that already is the
# newest release is not released again. A failed release does not undo the
# deploy: it stays `success`, and the script exits 3 so the next deploy retries.
#
# What it refuses, before opening a Deployment:
#   - a dirty tree (what is installed must be exactly the recorded commit;
#     harness state under .claude/ is ignored)
#   - a HEAD that is not on origin/<integration> (only landed work deploys)
#   - no systemd user unit `agents-tui.service`. The served instance is that
#     unit and nothing else: this script never starts a server of its own.
#     The reference unit is docs/agents-tui.service (install steps in it).
#
# The served instance:
#   `systemctl --user restart agents-tui.service`, active or not, then wait until
#   the village answers at the unit's own address: --web-port and --web-bind
#   from its ExecStart (127.0.0.1 when it binds there; port 7878 when unset).
#   AGENTS_TUI_URL (e.g. http://127.0.0.1:7890) overrides the address, and
#   AGENTS_TUI_WAIT the seconds to wait (default 30).
#
# EXIT: 0 deployed and released (or dry run passed) · 1 refused or setup error ·
#       2 deploy failed (recorded as failure) · 3 deployed, but the release failed
set -euo pipefail

ENVIRONMENT="nuc-1"
DRY_RUN=false
RESTART=true
RELEASE=true
while [ $# -gt 0 ]; do
  case "$1" in
    --env) [ $# -ge 2 ] || { echo "deploy.sh: --env needs a value" >&2; exit 1; }; ENVIRONMENT="$2"; shift 2 ;;
    --dry-run) DRY_RUN=true; shift ;;
    --no-restart) RESTART=false; shift ;;
    --no-release) RELEASE=false; shift ;;
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

# The served instance is the systemd user unit. Without it there is nothing to
# deploy to, and starting a server here would serve somewhere nobody looks.
UNIT="agents-tui.service"
UNIT_TEXT=$(systemctl --user cat "$UNIT" 2>/dev/null) || {
  cat >&2 <<EOF
deploy.sh: refusing: the systemd user unit $UNIT is not installed, so there is no served
agents-tui to deploy to. Nothing was installed or recorded. Install the unit, then deploy again:

  mkdir -p ~/.config/systemd/user
  cp docs/agents-tui.service ~/.config/systemd/user/   # edit ExecStart's port and bind if needed
  systemctl --user daemon-reload
  systemctl --user enable --now $UNIT
EOF
  exit 1
}

# Where the unit serves: its ExecStart's --web-port and --web-bind.
unit_url() {
  python3 - "$UNIT_TEXT" <<'PY'
import shlex, sys
lines = [l.split("=", 1)[1] for l in sys.argv[1].splitlines() if l.strip().startswith("ExecStart=")]
args = shlex.split(lines[-1].lstrip("-@:+!")) if lines and lines[-1].strip() else []
def opt(name):
    for i, a in enumerate(args):
        if a == name and i + 1 < len(args): return args[i + 1]
        if a.startswith(name + "="): return a.split("=", 1)[1]
    return None
port = opt("--web-port") or "7878"
binds = [b.strip() for b in (opt("--web-bind") or "127.0.0.1").split(",") if b.strip()]
if "127.0.0.1" in binds or "localhost" in binds: host = "127.0.0.1"
elif "::1" in binds: host = "[::1]"
else:
    lits = [b for b in binds if b.lower() != "tailscale"]
    host = lits[0] if lits else "tailscale"
    if host in ("0.0.0.0", "::", "[::]"): host = "127.0.0.1"
    elif ":" in host and not host.startswith("["): host = f"[{host}]"
print(f"http://{host}:{port}")
PY
}
URL="${AGENTS_TUI_URL:-$(unit_url)}"
if [[ "$URL" == http://tailscale:* ]]; then
  TAILNET_IP="$(tailscale ip -4 2>/dev/null | head -1 || true)"
  [ -n "$TAILNET_IP" ] || refuse "$UNIT binds only the tailnet, and \`tailscale ip -4\` has no address"
  URL="http://$TAILNET_IP:${URL##*:}"
fi
URL="${URL%/}"
CHECK="$URL/api/village"
WAIT="${AGENTS_TUI_WAIT:-30}"

say "environment: $ENVIRONMENT"
say "commit:      $SHA ($(git log -1 --format=%s "$SHA"))"
say "unit:        $UNIT"
say "check:       $CHECK"

if [ "$DRY_RUN" = true ]; then
  say "dry run: would record a Deployment of ${SHA:0:12} to '$ENVIRONMENT'"
  say "dry run: would run: cargo install --path . --locked"
  if [ "$RESTART" = false ]; then
    say "dry run: would not restart $UNIT (--no-restart)"
  else
    say "dry run: would run: systemctl --user restart $UNIT"
    say "dry run: would wait up to ${WAIT}s for $CHECK to answer"
  fi
  if [ "$RELEASE" = false ]; then
    say "dry run: would not release (--no-release)"
  else
    bash tools/release.sh --commit "$SHA" --dry-run 2>&1 | sed 's/^release\.sh: /deploy.sh: release: /' \
      || say "dry run: the release check failed; the deploy would still go ahead"
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

if [ "$RESTART" = true ]; then
  # Restart whether or not it was running: an inactive unit is started.
  systemctl --user restart "$UNIT" || fail "systemctl --user restart $UNIT failed"
  # It is deployed when the village answers, not when the unit was started.
  deadline=$((SECONDS + WAIT))
  until curl -fsS -o /dev/null --max-time 5 "$CHECK" 2>/dev/null; do
    [ "$SECONDS" -lt "$deadline" ] || fail "$UNIT restarted, but $CHECK did not answer within ${WAIT}s (journalctl --user -u $UNIT)"
    sleep 1
  done
  say "restarted $UNIT; serving $URL/village"
fi

status success "agents-tui ${SHA:0:12} installed on $(hostname)"
say "deployed ${SHA:0:12} to $ENVIRONMENT"

if [ "$RELEASE" = false ]; then
  say "not releasing (--no-release)"
  exit 0
fi
# The deploy stands whatever happens here: it is recorded, and only the release is retried.
if ! bash tools/release.sh --commit "$SHA"; then
  echo "deploy.sh: deployed ${SHA:0:12} to $ENVIRONMENT, but the release failed; the next deploy releases it" >&2
  exit 3
fi
