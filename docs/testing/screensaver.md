# The village screensaver

`/village?screensaver=1` (`src/web/village/saver.js` + `screensaver.js`); the
Omarchy service, launcher and installer are in
`tools/omarchy-plugin/agents-village-screensaver/`.

Tests:

```bash
node --test tools/omarchy-plugin/agents-village-screensaver/tests/*.test.mjs
bash tools/omarchy-plugin/agents-village-screensaver/tests/test_install.sh
# both of the above are also run by: cargo test screensaver
python3 tools/omarchy-plugin/agents-village-screensaver/tests/qml_check.py   # the real Service.qml in Quickshell, offscreen
```

Install with `tools/omarchy-plugin/install.sh --screensaver [--never-sleep] [--host <ssh host>]`.
