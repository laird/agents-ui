# A replay of a repo's history (`tools/mock/history.py`)

`tools/mock/history.py` rebuilds a replay of a repo's real development from
git and GitHub history (`--no-github` for git only).

Tests:

```bash
python3 -m unittest discover -s tools/mock -p 'test_history.py'
```
