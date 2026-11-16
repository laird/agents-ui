// The village page's modules, in the order main.js loads them (#520), for
// the node tests that load the whole page: main.js's own imports, with
// './modules.js' (which build.rs writes, and which is not on disk) standing
// for every script under src/web/village/ but main.js, sorted by path, the
// rule build.rs and tools/mock/replay-build.py follow too.
// Not a test file: `node --test` only runs *.test.mjs.
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

export const VILLAGE = new URL('../../src/web/village/', import.meta.url);

/* Every script under the village, as paths with `/`, sorted; dotfiles skipped, as build.rs does. */
export function villageScripts(root = fileURLToPath(VILLAGE)) {
  const out = [];
  const walk = dir => fs.readdirSync(dir, { withFileTypes: true }).forEach(e => {
    if (e.name.startsWith('.')) return;
    const p = path.join(dir, e.name);
    if (e.isDirectory()) walk(p);
    else if (e.name.endsWith('.js')) out.push(path.relative(root, p).split(path.sep).join('/'));
  });
  walk(root);
  return out.sort();
}

/* What modules.js imports. */
export const generatedModules = () => villageScripts().filter(f => f !== 'main.js');

/* The modules main.js loads, in order (each may of course import more). */
export function pageModules() {
  const main = fs.readFileSync(new URL('main.js', VILLAGE), 'utf8');
  return [...main.matchAll(/^import '\.\/(.+)';$/gm)].flatMap(([, m]) => (m === 'modules.js' ? generatedModules() : [m]));
}

/* Load the page's modules as main.js does (without starting it). */
export async function loadPage() {
  for (const m of pageModules()) await import(new URL(m, VILLAGE));
}
