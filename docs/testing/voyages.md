# Deploy voyages in the mock demo (`VOYAGES=1`)

`VOYAGES=1 tools/mock/demo.sh` scripts Deployments for this repo's own
environments (`deploy`, then `release`): a deploy, a promotion and a failed
deploy on a 120 s loop, so the environment row's voyages can be seen without
real ones (`tools/mock/voyages.py`).

Tests:

```bash
python3 -m unittest discover -s tools/mock -p 'test_voyages.py'
```
