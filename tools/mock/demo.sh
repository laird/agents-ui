#!/usr/bin/env bash
# Run agents-tui against simulated swarms, to see or test the village without
# real agents:
#
#   * "shop": an autocoder swarm under herdr (manager + 5 workers), with real
#     git worktrees, GitHub issues, tip refs, master-green and staging/prod;
#   * "billing": a modernize swarm under herdr whose manager runs specialist
#     sub-agents;
#   * "temporal:agents": a Temporal namespace of agentic workstreams;
#   * "larkspur", "agentex:larkspur", "crew:my-crew": villages from outside
#     adapters (tools/adapters), drawn from agents-village reports.
#   * "world:shop": the two together. Workstreams that fail on the platform
#     file incident issues that flow down to "shop", and every prod release
#     of "shop" rolls out to the platform's workers.
#
#   tools/mock/demo.sh                 # then open http://127.0.0.1:7878/village
#   RUNTIME=k8s tools/mock/demo.sh     # the platform as a simulated Kubernetes namespace instead of Temporal
#   STORY=1 RUNTIME=k8s tools/mock/demo.sh   # one incident from the cluster, fixed and released
#   FEATURE=1 tools/mock/demo.sh       # two features followed from spec to prod (banners, strip, stuck, unknown, live)
#   FEATURE=1 PROMOTION=feature tools/mock/demo.sh   # ... promoted as whole features (one barge)
#   LIFECYCLE=1 tools/mock/demo.sh     # every issue transition on a loop: arrive, claim, work, block, land, close, ship
#   VOYAGES=1 tools/mock/demo.sh       # deploy and release Deployments on a 120 s loop: a deploy, a promotion, a failure
#   GATE=1 tools/mock/demo.sh          # finished work goes through a merge gate: queue, gate, pass, fail, conflict, a train
#   CONFLICTS=1 tools/mock/demo.sh     # the merge gate's bounces, scripted: a conflict fixed, a failed gate, push retries, a hotspot
#   VALLEY=1 tools/mock/demo.sh        # the valley's ropes, caravan and barge (#530): fixture GitHub issues, shop's CI red
#   CHANNELS=1 tools/mock/demo.sh      # world:relay: issues the runtime raises by channel go round, raised to verified (channels.py)
#   PORT=8080 tools/mock/demo.sh
#   touch "$AGENTS_UI_DEMO_DIR/state/release-now"   # cut a release of "shop" now: its ship sails
#   tools/mock/demo.sh --source issues=none   # extra flags go to agents-tui
#
# Everything lives in a temporary directory (AGENTS_UI_DEMO_DIR to reuse one),
# with its own HOME, so your real ~/.claude and ~/.agents-ui are untouched.
# Real tmux swarms still show alongside the simulated ones.
set -euo pipefail

HERE=$(cd "$(dirname "$0")" && pwd)
ROOT=$(cd "$HERE/../.." && pwd)
PORT=${PORT:-7878}
TEMPORAL_PORT=${TEMPORAL_PORT:-7243}
DEMO=${AGENTS_UI_DEMO_DIR:-$(mktemp -d -t agents-ui-demo.XXXXXX)}
export AGENTS_UI_DEMO_DIR=$DEMO
# Simulated repos are local and cheap to read: refresh issues and the pipeline every 3 s, not 45.
export AGENTS_UI_VILLAGE_REFRESH_SECS=${AGENTS_UI_VILLAGE_REFRESH_SECS:-3}
BIN=${AGENTS_TUI:-$ROOT/target/debug/agents-tui}
if [ ! -x "$BIN" ]; then (cd "$ROOT" && cargo build); fi

pids=()
cleanup() { kill "${pids[@]}" 2>/dev/null || true; wait 2>/dev/null || true; }
trap cleanup EXIT INT TERM

python3 "$HERE/swarm_sim.py" --dir "$DEMO" setup
# STORY=1: one incident, start to finish (see swarm_sim.py and k8s_sim.py).
story=(); [ -n "${STORY:-}" ] && story=(--story)
# FEATURE=1: the feature story (see swarm_sim.py); PROMOTION=feature moves features as a whole.
feature=(); [ -n "${FEATURE:-}" ] && feature=(--feature --promotion "${PROMOTION:-subtask}")
# LIFECYCLE=1: every transition of an issue's life on a 90 s loop (see swarm_sim.py).
[ -n "${LIFECYCLE:-}" ] && feature+=(--lifecycle)
# VOYAGES=1: shop deploys to deploy and promotes to release through scripted Deployments (see voyages.py).
[ -n "${VOYAGES:-}" ] && feature+=(--voyages)
# GATE=1: a simulated merge gate (see swarm_sim.py), in the demo's own tmp so
# agents-tui reads its jobs and slots and nothing else's.
# CONFLICTS=1: the same gate, with each issue's attempts scripted (conflicts.py).
if [ -n "${GATE:-}" ] || [ -n "${CONFLICTS:-}" ]; then
  if [ -n "${CONFLICTS:-}" ]; then feature+=(--conflicts); else feature+=(--gate); fi
  export AUTOCODER_TMP="$DEMO/tmp" AUTOCODER_MAX_GATES=2
  mkdir -p "$AUTOCODER_TMP"
fi
python3 "$HERE/swarm_sim.py" --dir "$DEMO" "${story[@]}" "${feature[@]}" run & pids+=($!)
# One runtime source at a time: Temporal, or a Kubernetes namespace.
RUNTIME=${RUNTIME:-temporal}
if [ "$RUNTIME" = k8s ]; then
  python3 "$HERE/k8s_sim.py" --dir "$DEMO" "${story[@]}" \
    --release-file "$DEMO/state/release" --incident-file "$DEMO/state/incidents.jsonl" & pids+=($!)
  runtime=(--runtime k8s:sim/shop-prod --world shop=k8s:shop-prod)
  platform=k8s:shop-prod
else
  python3 "$HERE/temporal_sim.py" --port "$TEMPORAL_PORT" --namespace agents \
    --release-file "$DEMO/state/release" --incident-file "$DEMO/state/incidents.jsonl" & pids+=($!)
  runtime=(--temporal agents --temporal-url "http://127.0.0.1:$TEMPORAL_PORT" --world shop=temporal:agents)
  platform=temporal:agents
fi
sleep 1

# A group of related repos (#510): lib-core, the two consumers built on it
# (shop and larkspur, both live here) and billing, a service shop calls. lib-core
# and billing have no swarm, so they are dormant. Each is a small repo under
# the demo's HOME, so the valley can read versions (#511): lib-core has
# released v1.4.0; shop and billing have taken it, larkspur still pins 1.3.0,
# so a ship waits at its dock. billing deploys v2.2.0 to staging and v2.4.0
# to prod (its environments' tags), and shop expects >=2.3 of it, so its
# staging cell in the environment grid is marked (#512).
mkdir -p "$DEMO/home/.agents-ui"
demo_repo() {  # demo_repo <name> <file> <contents>: a repo under ~/repos
  mkdir -p "$DEMO/home/repos/$1" && printf '%s\n' "$3" > "$DEMO/home/repos/$1/$2"
}
demo_repo lib-core Cargo.toml $'[package]\nname = "lib-core"\nversion = "1.4.0"'
(cd "$DEMO/home/repos/lib-core" && git init -q && git add -A \
  && { git diff --cached --quiet || git -c user.name=demo -c user.email=demo@localhost -c commit.gpgsign=false commit -qm "lib-core"; } \
  && git -c tag.gpgsign=false tag -f v1.3.0 >/dev/null && git -c tag.gpgsign=false tag -f v1.4.0 >/dev/null) \
  || echo "demo: could not tag lib-core; the valley shows no ships" >&2
demo_repo shop Cargo.lock $'[[package]]\nname = "lib-core"\nversion = "1.4.0"'
demo_repo larkspur package-lock.json '{"packages":{"node_modules/lib-core":{"version":"1.3.0"}}}'
demo_repo billing Cargo.toml $'[dependencies]\nlib-core = "1.4"'
demo_repo billing .autocoder.json '{"environments":[{"name":"staging","ref":"env/staging"},{"name":"prod","ref":"env/prod"}]}'
# billing's environments are branches on its own origin (a bare repo here):
# staging runs v2.2.0, prod v2.4.0.
rm -rf "$DEMO/home/repos/billing-origin.git"
billing_git() { git -c user.name=demo -c user.email=demo@localhost -c commit.gpgsign=false -c tag.gpgsign=false -C "$DEMO/home/repos/billing" "$@"; }
{ billing_git init -q && billing_git add -A \
  && { billing_git diff --cached --quiet || billing_git commit -qm "billing 2.2.0"; } \
  && billing_git tag -f v2.2.0 >/dev/null && billing_git commit -q --allow-empty -m "billing 2.4.0" \
  && billing_git tag -f v2.4.0 >/dev/null \
  && git init -q --bare "$DEMO/home/repos/billing-origin.git" \
  && { billing_git remote remove origin 2>/dev/null; billing_git remote add origin "$DEMO/home/repos/billing-origin.git"; } \
  && billing_git push -q -f origin 'v2.2.0^{commit}:refs/heads/env/staging' 'v2.4.0^{commit}:refs/heads/env/prod' HEAD:refs/heads/master \
  && billing_git fetch -q origin; } \
  || echo "demo: could not set up billing's environments; the valley's grid shows none for it" >&2
cat > "$DEMO/home/.agents-ui/group.json" <<'GROUP'
[{ "name": "demo", "repos": [
  { "repo": "~/repos/lib-core" },
  { "repo": "~/repos/shop", "depends_on": [{ "repo": "~/repos/lib-core", "via": "package" }, { "repo": "~/repos/billing", "via": "api", "expects": ">=2.3" }] },
  { "repo": "~/repos/larkspur", "depends_on": [{ "repo": "~/repos/lib-core", "via": "package" }] },
  { "repo": "~/repos/billing", "depends_on": [{ "repo": "~/repos/lib-core", "via": "package" }] } ] }]
GROUP
# VALLEY=1 (#530): what ties the valley's repos together on GitHub, served by
# the gh shim from state/valley/ (the demo has no GitHub). shop#7 waits on
# lib-core#12 (a rope); lib-core#1 has subtasks in lib-core, shop and
# larkspur, two of three landed (a caravan); and shop's CI is red with a log
# naming lib-core (a barge back up the river, and a storm over shop).
if [ -n "${VALLEY:-}" ]; then
  mkdir -p "$DEMO/state/valley"
  cat > "$DEMO/state/valley/lib-core.json" <<'ISSUES'
[{"number": 1, "title": "Money everywhere", "state": "OPEN", "body": "Prices in cents, across the repos.\n\n- [x] demo/lib-core#2\n- [x] demo/shop#9\n- [ ] demo/larkspur#4"},
 {"number": 2, "title": "The money type", "state": "CLOSED", "stateReason": "COMPLETED", "body": ""},
 {"number": 12, "title": "Cents arithmetic", "state": "OPEN", "body": ""}]
ISSUES
  cat > "$DEMO/state/valley/shop.json" <<'ISSUES'
[{"number": 7, "title": "Charge in cents", "state": "OPEN", "body": "Depends on: demo/lib-core#12"},
 {"number": 9, "title": "Cents in the cart", "state": "CLOSED", "stateReason": "COMPLETED", "body": "Sub-task of demo/lib-core#1"}]
ISSUES
  cat > "$DEMO/state/valley/larkspur.json" <<'ISSUES'
[{"number": 4, "title": "Cents in reports", "state": "OPEN", "body": "Sub-task of demo/lib-core#1"}]
ISSUES
  # A GitHub origin for each, so the valley knows which repo an issue link names.
  for r in lib-core shop larkspur; do
    git -C "$DEMO/home/repos/$r" rev-parse --git-dir >/dev/null 2>&1 || git init -q "$DEMO/home/repos/$r"
    git -C "$DEMO/home/repos/$r" remote remove origin 2>/dev/null || true
    git -C "$DEMO/home/repos/$r" remote add origin "https://github.com/demo/$r"
  done
fi
echo "Village:    http://127.0.0.1:$PORT/village"
echo "Valley:     http://127.0.0.1:$PORT/village?valley=demo"
echo "Platform:   http://127.0.0.1:$PORT/village#$platform"
echo "World:      http://127.0.0.1:$PORT/village#world:shop"
# CHANNELS=1 (#560): a second world whose runtime raises its own issues, each
# badged with its channel, built, deployed and verified (channels.py).
channels=()
if [ -n "${CHANNELS:-}" ]; then
  channels=(--framework "exec:python3 $HERE/channels.py" --world relay=agentex:relay --incident-label incident --incident-label channel:)
  echo "Channels:   http://127.0.0.1:$PORT/village#world:relay"
fi
echo "Demo files: $DEMO"
echo "Release:    touch $DEMO/state/release-now   (the ship sails)"
HOME="$DEMO/home" PATH="$HERE/bin:$PATH" "$BIN" --headless --web-port "$PORT" "${runtime[@]}" \
  --framework "$ROOT/tools/adapters/example-report.json" \
  --framework "exec:$ROOT/tools/adapters/adapter-template.py" "${channels[@]}" "$@" & pids+=($!)
wait
