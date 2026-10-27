#!/usr/bin/env bash
# Run agents-tui against simulated swarms, to see or test the village without
# real agents:
#
#   * "shop": an autocoder swarm under herdr (manager + 5 workers), with real
#     git worktrees, GitHub issues, tip refs, master-green and staging/prod;
#   * "billing": a modernize swarm under herdr whose manager runs specialist
#     sub-agents;
#   * "temporal:agents": a Temporal namespace of agentic workstreams.
#
#   tools/mock/demo.sh                 # then open http://127.0.0.1:7878/village
#   PORT=8080 tools/mock/demo.sh
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
BIN=${AGENTS_TUI:-$ROOT/target/debug/agents-tui}
if [ ! -x "$BIN" ]; then (cd "$ROOT" && cargo build); fi

pids=()
cleanup() { kill "${pids[@]}" 2>/dev/null || true; wait 2>/dev/null || true; }
trap cleanup EXIT INT TERM

python3 "$HERE/swarm_sim.py" --dir "$DEMO" setup
python3 "$HERE/swarm_sim.py" --dir "$DEMO" run & pids+=($!)
python3 "$HERE/temporal_sim.py" --port "$TEMPORAL_PORT" --namespace agents & pids+=($!)
sleep 1

echo "Village:    http://127.0.0.1:$PORT/village"
echo "Demo files: $DEMO"
HOME="$DEMO/home" PATH="$HERE/bin:$PATH" "$BIN" --headless --web-port "$PORT" \
  --temporal agents --temporal-url "http://127.0.0.1:$TEMPORAL_PORT" & pids+=($!)
wait
