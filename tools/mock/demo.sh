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
#   VOYAGES=1 tools/mock/demo.sh       # nuc-1 and release Deployments on a 120 s loop: a deploy, a promotion, a failure
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
# VOYAGES=1: shop deploys to nuc-1 and promotes to release through scripted Deployments (see voyages.py).
[ -n "${VOYAGES:-}" ] && feature+=(--voyages)
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

echo "Village:    http://127.0.0.1:$PORT/village"
echo "Platform:   http://127.0.0.1:$PORT/village#$platform"
echo "World:      http://127.0.0.1:$PORT/village#world:shop"
echo "Demo files: $DEMO"
echo "Release:    touch $DEMO/state/release-now   (the ship sails)"
HOME="$DEMO/home" PATH="$HERE/bin:$PATH" "$BIN" --headless --web-port "$PORT" "${runtime[@]}" \
  --framework "$ROOT/tools/adapters/example-report.json" \
  --framework "exec:$ROOT/tools/adapters/adapter-template.py" "$@" & pids+=($!)
wait
