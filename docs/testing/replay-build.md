# Replay pages (`tools/mock/replay-build.py`)

`tools/mock/replay-build.py` bundles the village's modules into one
self-contained replay page (each module after its imports; two modules
declaring the same top-level name stop the build).

Tests:

```bash
python3 -m unittest discover -s tools/mock -p 'test_replay_build.py'   # also run by: cargo test replay_build
```
