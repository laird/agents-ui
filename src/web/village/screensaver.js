/* ---------- Screensaver mode ----------
 * `/village?screensaver=1` (or `#<project>&screensaver`): the village full
 * screen as an idle display, readable from across the room. No bar, picker or
 * buttons; the map fitted to the whole display with its area labels on; a
 * large status strip under it; a big "can't reach" in place of a frozen map.
 * With several swarms it cycles through them. Burn-in care: the scene drifts
 * a few pixels, and dims in the night hours if asked. It is never blanked.
 *
 * What it shows and when is decided in saver.js; this only puts it on screen.
 * data.js says after each poll whether the server answered ('village:polled'). */
import { state } from './state.js';
import { staleWords, cycleOrder, dimmedAt, dismisses, driftAt, gateWords, nextName, saverOptions, saverSummary } from './saver.js';
import { esc, fit } from './panels.js';
import { placeOverlays } from './registry.js';

const saverOpts = saverOptions(location.search, location.hash);

/* The village the strip describes: the one shown, or a world's dev village. */
function shownVillage() {
  const view = state.view;
  if (!view) return null;
  const w = view.world && state.worlds.find(x => x.name === view.world);
  const name = w ? w.dev : view.village;
  return state.villages.find(v => v.project === name) || null;
}

function renderSaverStrip(box, lastOk) {
  const v = shownVillage();
  if (!v) { box.innerHTML = '<div class="ss-row"><span class="ss-name">Looking for swarms…</span></div>'; return; }
  const s = saverSummary(v), view = state.view;
  const title = view.world ? view.world : s.project;
  const stops = s.stops.map(st => `<span class="ss-item"><b>${st.count}</b> ${esc(st.name)}</span>`).join('');
  box.innerHTML = `<div class="ss-row">
      <span class="ss-name">${esc(title)}</span>
      <span class="ss-item ss-working"><b>${s.working}</b> working</span>
      <span class="ss-item"><b>${s.idle}</b> idle</span>
      <span class="ss-item${s.blocked ? ' ss-red' : ''}"><b>${s.blocked}</b> blocked</span>
      <span class="ss-time">${lastOk ? 'Updated ' + new Date(lastOk).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' }) : ''}</span>
    </div>
    <div class="ss-row">
      <span class="ss-item${s.gate.failed ? ' ss-red' : ''}">Gate <b>${esc(gateWords(s.gate))}</b></span>
      <span class="ss-item"><b>${s.quay}</b> at the quay</span>
      ${stops}
      <span class="ss-wait${s.waiting ? ' ss-red' : ''}">${s.waiting ? `<b>${s.waiting}</b> waiting on you` : 'Nothing waiting on you'}</span>
    </div>`;
}

function renderSaverOffline(box, lastOk) {
  box.innerHTML = `<div class="ss-off-big">Can't reach agents-tui on ${esc(location.hostname || 'this server')}</div>
    <div class="ss-off-small">${new Date().toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })} · ${lastOk ? 'last answer ' + staleWords(Date.now() - lastOk) : 'no answer yet'} · retrying</div>`;
}

/* Show another swarm: replace the hash (never push it, so the launcher's
 * window keeps one history entry and may close itself) and let data.js's
 * hashchange handler select it. */
function cycleSaver() {
  const names = cycleOrder(state.villages, state.worlds);
  const view = state.view, current = view && (view.world || view.village);
  const next = nextName(names, current);
  if (!next || next === current) return;
  history.replaceState(null, '', location.pathname + location.search + '#' + encodeURIComponent(next) + '&screensaver');
  window.dispatchEvent(new HashChangeEvent('hashchange'));
}

/* Close on input, when the launcher asked. The title is what the launcher
 * watches for (a page may not be allowed to close its own window); closing
 * is tried as well. A short grace lets the window settle first. */
function armDismissal() {
  const t0 = performance.now();
  let start = null;
  const onInput = e => {
    if (performance.now() - t0 < 1500) return;
    if ((e.type === 'pointermove' || e.type === 'mousemove') && !start) { start = { x: e.x, y: e.y }; return; }
    if (!dismisses(e, start)) return;
    document.title = 'agents-village-screensaver-exit';
    window.close();
  };
  for (const type of ['keydown', 'pointerdown', 'wheel', 'touchstart', 'pointermove']) window.addEventListener(type, onInput, { passive: true, capture: true });
}

/* The strip's height settles once it has words in it: fit the map to what is left. */
function refit() {
  if (!document.getElementById('map').hidden) { fit(); placeOverlays(); }
}

if (saverOpts.on) {
  state.screensaver = true;
  document.documentElement.classList.add('screensaver');
  document.title = 'Village screensaver';
  // The area labels stay on, whatever was remembered.
  document.getElementById('labels-on').checked = true;
  const strip = document.createElement('div'), off = document.createElement('div');
  strip.className = 'ss-strip'; strip.id = 'ss-strip'; strip.setAttribute('aria-live', 'polite');
  off.className = 'ss-off'; off.id = 'ss-off'; off.hidden = true; off.setAttribute('role', 'alert');
  document.body.append(strip, off);

  let lastOk = 0, reachable = true;
  window.addEventListener('village:polled', e => {
    reachable = e.detail.ok;
    if (reachable) lastOk = e.detail.at;
    document.documentElement.classList.toggle('ss-offline', !reachable);
    off.hidden = reachable;
    if (reachable) renderSaverStrip(strip, lastOk); else renderSaverOffline(off, lastOk);
    refit();
  });
  window.addEventListener('hashchange', () => { if (reachable) { renderSaverStrip(strip, lastOk); refit(); } });

  if (saverOpts.cycle) setInterval(cycleSaver, saverOpts.cycle * 1000);

  // Burn-in care: drift every 20 s (eased by a matching CSS transition) and dim at night.
  const care = () => {
    const d = driftAt(Date.now());
    document.body.style.translate = `${d.x}px ${d.y}px`;
    document.documentElement.classList.toggle('ss-dim', dimmedAt(new Date().getHours(), saverOpts.dim));
  };
  care();
  setInterval(care, 20000);

  if (saverOpts.dismiss) armDismissal();
}
