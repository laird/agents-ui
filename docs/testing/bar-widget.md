# The Omarchy bar widget

`tools/omarchy-plugin/agents-village/`.

Tests:

```bash
node --test tools/omarchy-plugin/agents-village/tests/*.test.js   # its model, against recorded /api/swarms and /api/village; also run by: cargo test notify
python3 tools/omarchy-plugin/agents-village/tests/qml_check.py    # the real Panel.qml in Quickshell, offscreen
```

Install with `tools/omarchy-plugin/install.sh [--host <ssh host>] [--enable]`.
