# Channel incidents in the mock demo (`CHANNELS=1`)

`CHANNELS=1 tools/mock/demo.sh` adds `world:relay`, drawn from
`tools/mock/channels.py`. Every 80 s one of the runtime's two workstreams
raises an issue in its channel (`channel:log-analyzer`, then
`channel:user-feedback`). It is built, lands, is deployed to prod and is
verified, 20 s each. The step comes from the clock, so the script keeps no
state.

To check it without the rest of the demo, serve only the script and read the
world back:

```bash
cargo build --release
target/release/agents-tui --headless --read-only --web-port 7918 --web-bind 127.0.0.1 \
  --source swarms=none --framework "exec:python3 tools/mock/channels.py" \
  --world relay=agentex:relay --incident-label channel: &
curl -s http://127.0.0.1:7918/api/village \
  | python3 -c "import json,sys; w=json.load(sys.stdin)['worlds'][0]; print(w['counts'], w['incidents'], w['verified'])"
kill %1
```

In the first 60 s of each cycle, `incidents` holds the issue with its `channel`
and `raised_by`, and `landed` and then `deployed` count it. In the last 20 s
it moves to `verified`, and `verified` counts it. `python3 tools/mock/channels.py --at <epoch seconds>`
prints the reports for any moment.
