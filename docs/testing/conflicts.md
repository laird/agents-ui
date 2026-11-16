# Merge-gate bounces in the mock demo (`CONFLICTS=1`, #430)

`CONFLICTS=1 tools/mock/demo.sh` scripts the merge gate's bounces: each issue
at the gate takes the next story (a conflict fixed on the second attempt, a
failed gate, a push rejected and re-synced twice, a hotspot file that
conflicts three times), as autocoder's merge-job files
(`tools/mock/conflicts.py`).

Tests:

```bash
python3 -m unittest discover -s tools/mock -p 'test_conflicts.py'
cargo test conflicts_demo   # reads its files as gate.rs does
```
