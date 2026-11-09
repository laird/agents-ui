"""Tests for the self-contained replay page (replay-build.py).

    python3 -m unittest discover -s tools/mock -p 'test_replay_build.py'   # also run by `cargo test replay_build`

The village page is ES modules (src/web/village/); the replay inlines them as
one module script. A classic <script> cannot hold an `import`, which is how the
replay broke when the page became modules (#392).
"""

import os
import re
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
VILLAGE = ROOT / "src/web/village"
IMPORT = re.compile(r"^import\s+(?:\{[^}]*\}\s+from\s+)?'([^']+)';", re.M)


class ReplayBuildTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.tmp = tempfile.TemporaryDirectory()
        out = Path(cls.tmp.name) / "replay.html"
        run = subprocess.run([sys.executable, str(HERE / "replay-build.py"), str(HERE / "recordings/story.json.gz"), str(out)],
                             capture_output=True, text=True)
        cls.rc, cls.err = run.returncode, run.stderr
        cls.page = out.read_text() if out.exists() else ""
        cls.scripts = re.findall(r"<script([^>]*)>(.*?)</script>", cls.page, re.S)

    @classmethod
    def tearDownClass(cls):
        cls.tmp.cleanup()

    def test_builds(self):
        self.assertEqual(self.rc, 0, self.err)

    def test_no_classic_script_imports(self):
        classic = [body for attrs, body in self.scripts if "type=" not in attrs]
        self.assertTrue(classic, "the replay's own script is a classic one")
        for body in classic:
            self.assertNotRegex(body, r"(?m)^\s*import\b")

    def test_the_village_is_one_module_without_imports(self):
        modules = [body for attrs, body in self.scripts if 'type="module"' in attrs]
        self.assertEqual(len(modules), 1)
        self.assertNotRegex(modules[0], r"(?m)^\s*(import|export)\b")

    def test_every_module_once_after_what_it_imports(self):
        bundle = next(body for attrs, body in self.scripts if 'type="module"' in attrs)
        order = re.findall(r"(?m)^// ---- (\S+)$", bundle)
        files = sorted(p.relative_to(VILLAGE).as_posix() for p in VILLAGE.rglob("*.js"))
        self.assertEqual(sorted(order), files, "every module, each once")
        self.assertEqual(order[-1], "main.js")
        at = {rel: i for i, rel in enumerate(order)}
        for rel in order:
            path = VILLAGE / rel
            for spec in IMPORT.findall(path.read_text()):
                dep = (path.parent / spec).resolve().relative_to(VILLAGE).as_posix()
                # A cycle runs the module that was reached first before its import, as the browser does.
                if at[dep] > at[rel]:
                    back = (VILLAGE / dep).read_text()
                    self.assertTrue(any((VILLAGE / dep).parent.joinpath(s).resolve() == path for s in IMPORT.findall(back)),
                                    f"{rel} runs before {dep}, which it imports")

    def test_the_replay_drives_the_feed(self):
        self.assertIn("setTimeout(poll, window.__pollMs || 3000)", self.page)
        self.assertIn("'Replay of the simulation'", self.page)
        self.assertIn("for (const k in state.scenes) delete state.scenes[k];", self.page)
        self.assertNotIn('src="/village/', self.page)
        self.assertNotIn('href="/village/', self.page)


if __name__ == "__main__":
    unittest.main()
