/* ---------- Sound cues: the decisions ----------
 * Which quiet cue the screensaver plays, if any (#489), with no DOM, so node
 * can test it (tools/village-tests/soundcues.test.mjs). sounds.js plays them.
 *
 * Cues mark transitions only, seen between two polls, never a state that
 * lasts: a bell when more starts waiting on you, thunder when the build
 * breaks, birdsong when it is green again, a ship's horn when a deploy docks
 * at the production (right-most) environment. The first poll of a village
 * only sets the baseline, so opening the page is silent. */
import { saverSummary } from './saver.js';

/* Most important first: one poll plays at most one cue, the first of these. */
export const CUES = ['thunder', 'bell', 'horn', 'birdsong'];

/* What a village's cues are decided from. The build keeps its last settled
 * state (red or green) through a run, so red, running, green is still
 * birdsong; `docked` holds the production voyages that have docked. */
export function cueSnapshot(villages, before = new Map()) {
  const out = new Map();
  for (const v of villages || []) {
    if (!v || !v.project) continue;
    const prev = before.get(v.project), p = v.pipeline || {}, stops = p.stops || [];
    const prod = stops.length ? stops[stops.length - 1].name : null;
    const st = v.build && v.build.state;
    out.set(v.project, {
      waiting: saverSummary(v).waiting,
      build: st === 'red' || st === 'green' ? st : prev ? prev.build : null,
      docked: new Set((p.voyages || []).filter(vo => vo && vo.state === 'docked' && prod && vo.to === prod)
        .map(vo => `${vo.to}|${vo.sha}`)),
    });
  }
  return out;
}

/* The cues between two snapshots, most important first, each once. */
export function cuesBetween(before, after) {
  const found = new Set();
  for (const [project, now] of after) {
    const was = before.get(project);
    if (!was) continue;
    if (now.waiting > was.waiting) found.add('bell');
    if (was.build === 'green' && now.build === 'red') found.add('thunder');
    if (was.build === 'red' && now.build === 'green') found.add('birdsong');
    if ([...now.docked].some(k => !was.docked.has(k))) found.add('horn');
  }
  return CUES.filter(c => found.has(c));
}

/* Rate limit: the first cue not heard in the last `gap` ms, and nothing
 * within `quiet` ms of the last cue of any kind. `played` maps cue → ms. */
export function pickCue(cues, played, now, { gap = 120000, quiet = 15000 } = {}) {
  const last = Math.max(-Infinity, ...played.values());
  if (now - last < quiet) return null;
  return cues.find(c => !played.has(c) || now - played.get(c) >= gap) || null;
}

/* Each theme's set, as notes: start `at` and `dur` in seconds, `freq` in Hz
 * (gliding to `to`), a `wave` (or `noise`, low-passed at `freq`) and a peak
 * `gain`. All soft: nothing above 0.2. */
const tone = (at, dur, freq, wave = 'sine', gain = 0.12, to) => ({ at, dur, freq, wave, gain, ...(to ? { to } : {}) });
const hiss = (at, dur, freq, gain) => ({ at, dur, freq, wave: 'noise', gain });
const chirps = (base, wave) => [0, 0.18, 0.5, 0.66, 1.1].map((at, i) => tone(at, 0.12, base + i * 120, wave, 0.06, base + 700 + i * 90));

export const CUE_SOUNDS = {
  medieval: {
    bell: [tone(0, 2.4, 523, 'sine', 0.14), tone(0, 1.6, 1046, 'sine', 0.05), tone(0, 1.1, 1318, 'sine', 0.03)],
    thunder: [hiss(0, 2.8, 180, 0.18), hiss(0.4, 2.2, 90, 0.16)],
    birdsong: chirps(2200, 'sine'),
    horn: [tone(0, 1.8, 98, 'sawtooth', 0.07), tone(0, 1.8, 147, 'sawtooth', 0.04)],
  },
  modern: {
    bell: [tone(0, 0.6, 880, 'sine', 0.12), tone(0.22, 0.9, 660, 'sine', 0.12)],
    thunder: [hiss(0, 2.2, 220, 0.16), tone(0, 1.2, 70, 'sine', 0.1)],
    birdsong: chirps(2600, 'triangle'),
    horn: [tone(0, 1.4, 110, 'square', 0.04), tone(0, 1.4, 165, 'triangle', 0.08)],
  },
  future: {
    bell: [tone(0, 0.35, 1200, 'triangle', 0.08, 1500), tone(0.3, 0.6, 1800, 'sine', 0.08)],
    thunder: [tone(0, 1.8, 120, 'sawtooth', 0.08, 40), hiss(0, 1.6, 300, 0.1)],
    birdsong: chirps(1800, 'square').map(x => ({ ...x, gain: 0.03 })),
    horn: [tone(0, 1.5, 220, 'sawtooth', 0.05, 330), tone(0.1, 1.4, 110, 'sine', 0.08)],
  },
};

/* The theme's set; another theme falls back to the medieval one. */
export const cueSounds = theme => CUE_SOUNDS[theme] || CUE_SOUNDS.medieval;
