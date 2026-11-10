/* ---------- Data ----------
 * The feed: poll /api/village, fold each village into its scene (starting
 * the animations its changes call for), and show the village or world the
 * URL names, in the theme picked. */
import { state } from './state.js';
import { THEMES } from './themes/registry.js';
import { placeOverlays } from './registry.js';
import { cv, layout, shownScenes } from './layout.js';
import { esc, fit, toast } from './panels.js';
import { kbox, kbuttons, placeKeys, repoUrl } from './dialogs.js';
import { diff } from './trips.js';
import { releaseOf, sail } from './ship.js';
import { noteVoyages } from './voyages.js';
import { scrollToVillage } from './envrow.js';
import { fadeLive } from './features.js';
import { draw, updateWorld, worldWords } from './worlds.js';
import { legend, renderLegend } from './key.js';
import { hashName } from './saver.js';

const pick = document.getElementById('pick');
let themeName = 'medieval';

/* Fold a new model into its village's scene, starting the animations its
 * changes call for. */
function updateScene(v, t) {
  const scenes = state.scenes;
  const sc = scenes[v.project] || (scenes[v.project] = { v: null, porters: [], floats: [], gates: {}, voyage: null, supplies: [], nextAt: 0, fireworks: -1e9,
    lastDelivered: null, lastRelease: null, liveSince: {}, fade: {} });
  fadeLive(sc, v);
  const lay = layout(v);
  state.porters = sc.porters; state.floats = sc.floats;
  const said = diff(sc.v, v, lay, t, sc);
  sc.porters = state.porters; sc.floats = state.floats;
  // Reduced motion: nobody walks, so say what changed instead.
  if (state.reduce && said.length && shownScenes().some(([x]) => x === sc)) toast(`${v.project}: ${said.slice(0, 4).join(' · ')}${said.length > 4 ? ` · +${said.length - 4} more` : ''}`);
  // A new deployment at the last stop sails the ship. A village with no
  // release to compare (a runtime's stages) still lights up when work arrives.
  const rel = releaseOf(v), delivered = v.pipeline.stops.length ? v.pipeline.stops[v.pipeline.stops.length - 1].issues.join(',') : '';
  const was = sc.lastRelease, sameSource = rel && was && rel.split(':')[0] === was.split(':')[0];
  if (rel) { if (sc.v && sameSource && rel !== was) sail(sc, v, t); sc.lastRelease = rel; }
  else if (!was && sc.lastDelivered !== null && sc.v && delivered !== sc.lastDelivered && delivered) sc.fireworks = t;
  sc.lastDelivered = delivered;
  noteVoyages(sc, sc.v, v, t);
  sc.v = v; sc.L = lay;
  return sc;
}

/* Which village this is, always in the bar (the picker shows it only when
 * there are two or more), plus its GitHub page when a real issue URL shows
 * which repo it is (`repoUrl`), never a guess. */
function nameBar(name, v) {
  const vn = document.getElementById('vname'), a = document.getElementById('repo'), repo = v ? repoUrl(v) : null;
  vn.textContent = name;
  vn.hidden = !pick.hidden;
  a.hidden = !repo;
  if (repo) { a.href = repo; a.textContent = repo.replace(/^https:\/\/github\.com\//, '') + ' on GitHub'; }
}

/* `· autocoder · development`, `· agentex · runtime`: which framework drew it, and which half of the loop it is. */
function describe(v) {
  const fw = v.workflow || v.framework;
  return [fw, v.plane].filter(Boolean).map(x => '· ' + x).join(' ');
}
/* Show a village (`shop`) or a world (`world:shop`); `shop@release` scrolls
 * to that environment's village. */
function select(want) {
  const { villages, worlds } = state, [name, env] = String(want).split('@');
  const w = worlds.find(x => x.name === name);
  const v = villages.find(x => x.project === name);
  if (w) state.view = { world: w.name };
  else if (v || villages[0]) state.view = { village: (v || villages[0]).project };
  else return;
  const view = state.view;
  if (view.world) {
    document.getElementById('workflow').textContent = worldWords(w);
    nameBar(`${w.dev} \u21c4 ${w.platform}`, villages.find(x => x.project === w.dev));
  } else {
    const sc = state.scenes[view.village];
    document.getElementById('workflow').textContent = describe(sc.v);
    nameBar(sc.v.project, sc.v);
    if (cv.width !== sc.L.W || cv.height !== sc.L.H) { cv.width = sc.L.W; cv.height = sc.L.H; }
  }
  fit();
  if (state.reduce) draw(0);
  const sc = !view.world && state.scenes[view.village], e = env && sc && sc.L.envs.find(x => x.s.name === env);
  if (e && state.scrolledTo !== want) { state.scrolledTo = want; scrollToVillage(e.x); }
  placeOverlays();
}

/* Whether the server answered, for what shows it (the screensaver's strip). */
const polled = ok => window.dispatchEvent(new CustomEvent('village:polled', { detail: { ok, at: Date.now() } }));

async function poll() {
  try {
    const res = await fetch('/api/village');
    if (!res.ok) throw new Error(res.status);
    const data = await res.json();
    const villages = state.villages = data.villages || [];
    const worlds = state.worlds = data.worlds || [];
    const t = performance.now();
    villages.forEach(v => updateScene(v, t));
    worlds.forEach(w => updateWorld(w, t));
    const names = [...worlds.map(w => w.name), ...villages.map(v => v.project)];
    const label = n => { const w = worlds.find(x => x.name === n); return w ? `${w.dev} \u21c4 ${w.platform}` : n; };
    pick.hidden = names.length < 2;
    if (pick.options.length !== names.length || [...pick.options].some((o, i) => o.value !== names[i])) {
      pick.innerHTML = names.map(n => `<option value="${esc(n)}">${esc(label(n))}</option>`).join('');
    }
    const view = state.view;
    const want = String(hashName(location.hash) || (view && (view.world || view.village)) || names[0] || '');
    if (names.includes(want.split('@')[0])) pick.value = want.split('@')[0];
    document.getElementById('empty').hidden = villages.length > 0;
    cv.hidden = villages.length === 0;
    if (!villages.length) document.getElementById('empty').textContent = 'No swarms running. Start one from the dashboard or the agents-tui terminal.';
    else select(want.includes('@') && want.split('@')[0] === pick.value ? want : pick.value || names[0]);
    document.getElementById('status').textContent = 'Live · updated ' + new Date().toLocaleTimeString();
    polled(true);
  } catch (e) {
    document.getElementById('status').textContent = "Can't reach agents-tui. Retrying…";
    polled(false);
  }
  setTimeout(poll, 3000);
}

/* Start the page: the theme from the URL (else the one remembered), the
 * Style picker, the feed and the animation loop. */
export function start() {
  const themeFromUrl = new URLSearchParams(location.search).get('theme');
  themeName = THEMES[themeFromUrl] ? themeFromUrl : (() => {
    try { return THEMES[localStorage.getItem('village-theme')] ? localStorage.getItem('village-theme') : 'medieval'; } catch { return 'medieval'; }
  })();
  state.T = THEMES[themeName];

  const themePick = document.getElementById('theme');
  // One option per registered theme, in the order they registered.
  for (const [key, th] of Object.entries(THEMES)) themePick.add(new Option(th.name, key));
  themePick.value = themeName;
  themePick.addEventListener('change', () => {
    themeName = themePick.value; state.T = THEMES[themeName];
    try { localStorage.setItem('village-theme', themeName); } catch {}
    const view = state.view;
    if (!legend.hidden) renderLegend();
    if (view) select(view.world || view.village);
  });
  pick.addEventListener('change', () => { location.hash = pick.value; select(pick.value); });
  window.addEventListener('hashchange', () => select(hashName(location.hash)));

  poll();

  if (!state.reduce) {
    // The first frame records what is clickable; place the keyboard targets then, not a poll later.
    const placed = () => String(state.view && (state.view.world || state.view.village));
    const loop = t => { draw(t); if (!kbuttons.size && kbox.dataset.placed !== placed()) { kbox.dataset.placed = placed(); placeKeys(); } requestAnimationFrame(loop); };
    requestAnimationFrame(loop);
  }
}
