/* ---------- Sound cues ----------
 * Optional, quiet cues for a wall display (#489), off unless the screensaver
 * page is asked with `sound=1` (the launcher adds it for `"sound": true` in
 * screensaver.json). After each poll it compares every village with the last
 * poll (soundcues.js decides), and plays at most one cue in the theme's
 * voice, synthesized with Web Audio: no sound files. Silent in the `dim`
 * hours. A browser that blocks audio until a gesture just stays silent; the
 * launcher's kiosk windows allow it (--autoplay-policy). */
import { state } from './state.js';
import { THEMES } from './themes/registry.js';
import { dimmedAt, saverOptions } from './saver.js';
import { cueSnapshot, cueSounds, cuesBetween, pickCue } from './soundcues.js';

const soundOpts = saverOptions(location.search, location.hash);
let cueAudio = null;

function soundTheme() {
  const hit = Object.entries(THEMES).find(([, t]) => t === state.T);
  return hit ? hit[0] : 'medieval';
}

/* One note: an oscillator, or low-passed noise, under a soft envelope. */
function cueNote(ctx, out, x, t0) {
  const start = t0 + x.at, end = start + x.dur, env = ctx.createGain();
  env.gain.setValueAtTime(0.0001, start);
  env.gain.exponentialRampToValueAtTime(x.gain, start + Math.min(0.03, x.dur / 4));
  env.gain.exponentialRampToValueAtTime(0.0001, end);
  env.connect(out);
  let src;
  if (x.wave === 'noise') {
    const buf = ctx.createBuffer(1, Math.ceil(ctx.sampleRate * x.dur), ctx.sampleRate), d = buf.getChannelData(0);
    for (let i = 0; i < d.length; i++) d[i] = Math.random() * 2 - 1;
    const lp = ctx.createBiquadFilter();
    lp.type = 'lowpass'; lp.frequency.value = x.freq;
    src = ctx.createBufferSource(); src.buffer = buf;
    src.connect(lp); lp.connect(env);
  } else {
    src = ctx.createOscillator(); src.type = x.wave;
    src.frequency.setValueAtTime(x.freq, start);
    if (x.to) src.frequency.exponentialRampToValueAtTime(x.to, end);
    src.connect(env);
  }
  src.start(start); src.stop(end + 0.05);
}

function playCue(cue) {
  const Ctx = window.AudioContext || window.webkitAudioContext;
  if (!Ctx) return;
  try {
    cueAudio = cueAudio || new Ctx();
    if (cueAudio.state === 'suspended') cueAudio.resume().catch(() => {});
    const t0 = cueAudio.currentTime + 0.05;
    for (const x of cueSounds(soundTheme())[cue] || []) cueNote(cueAudio, cueAudio.destination, x, t0);
  } catch { /* no audio here: stay silent */ }
}

if (soundOpts.on && soundOpts.sound) {
  let snap = null;
  const played = new Map();
  window.addEventListener('village:polled', e => {
    if (!e.detail.ok) return;
    const next = cueSnapshot(state.villages, snap || undefined);
    const cues = snap ? cuesBetween(snap, next) : [];
    snap = next;
    if (!cues.length || dimmedAt(new Date().getHours(), soundOpts.dim)) return;
    const now = Date.now(), cue = pickCue(cues, played, now);
    if (!cue) return;
    played.set(cue, now);
    playCue(cue);
  });
}
