# Desktop notifications from the village (#480)

The notifier is `tools/omarchy-plugin/agents-village-notify/`; the page's
`#<project>&house=<name>` deep link is `src/web/village/deeplink.js` +
`houselink.js`. Its `Model.js` is a copy of the bar widget's, kept equal by
its node test.

Tests:

```bash
node --test tools/omarchy-plugin/agents-village-notify/tests/*.test.js   # also run by: cargo test notify
python3 tools/omarchy-plugin/agents-village-notify/tests/qml_check.py     # the real Service.qml in Quickshell, offscreen
```

Install with `tools/omarchy-plugin/install.sh --notify [--quiet 23-7] [--host <ssh host>]`.
