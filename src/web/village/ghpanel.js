/* ---------- The GitHub panel (#492) ----------
 * A sign between the docks and the hall, lettered GITHUB: what waits on
 * GitHub for this repo. Its label says what waits on you (review requests,
 * proposals) and the open pull requests; a click (or the keyboard, #478)
 * opens the whole panel, a line per count, each linking its GitHub search
 * (ghpanellines.js). It is the environment row's signpost (envart.js), so it
 * is a carved board in the village, an LED panel downtown and a holo-sign in
 * the future. Drawn only when the server's GitHub summary is there
 * (`github`, github.rs): with `gh` missing or signed out the village draws
 * as before. */
import { envSign } from './envart.js';
import { GH_SIGN, ghSignAt } from './layout.js';
import { repoUrl } from './dialogs.js';
import { registerHit, registerKey, registerLabels, registerLayer, tap } from './registry.js';
import { panelDialog, panelSub } from './ghpanellines.js';

registerLayer({ z: 52, draw(c, { v, lay }) {
  if (!v.github) return;
  const { x, y } = ghSignAt(lay.hallX);
  envSign(c, x, y, null, 'GITHUB');
  tap(x, y, GH_SIGN.w, GH_SIGN.h, { kind: 'github' });
} });

registerHit({ kind: 'github', dialog(o, sc) {
  const v = sc.v;
  return panelDialog(v.github, repoUrl(v), v.pipeline.integration, Date.now());
} });

/* "GitHub · 2 to review · 3 PRs" under the sign, kept clear of it. */
registerLabels(sc => {
  const g = sc.v.github;
  if (!g) return [];
  const { x, y } = ghSignAt(sc.L.hallX);
  return [{ avoid: { x, y, w: GH_SIGN.w, h: GH_SIGN.h } },
    { x: x + (GH_SIGN.w >> 1), y: y + GH_SIGN.h + 1, anchor: 'top', text: 'GitHub', sub: panelSub(g) }];
}, 12);

registerKey({ group: 'other', rows: () => [
  [c => envSign(c, 2, 0, null, 'GITHUB'), 'Sign lettered GITHUB by the hall: open pull requests, review requests, issues by label, what landed (click it)', 21]] });
