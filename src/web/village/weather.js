/* ---------- Weather: the build's health over the village (#414) ----------
 * The integration branch's build (`build`, src/web/build.rs) as weather a
 * glance can't miss (buildweather.js decides which):
 *   storm    the build is red: dark sky over the whole village, rain, and
 *            now and then lightning. A red BUILD BROKEN sign on the quay
 *            names the failing jobs, and the commit that turned it red is
 *            a crate with a red flag beside it;
 *   squall   red on one OS or one job: a smaller cloud over the gatehouse;
 *   drizzle  the build is running;
 *   clear    green or unknown, as before.
 * Each theme has its own (`weather` in themes/*.js): a storm in the
 * village, smog and alarm lights downtown, a red alert in the future. With
 * motion reduced the rain stands still and nothing flashes. The art is
 * drawn at 2× (#458), in weatherart.js. */
import { state } from './state.js';
import { P, R, blink, shadow } from './palette.js';
import { alarmLamp, alarms, blameCrate, bolt, brokenSign, clouds, puff, rain, SIGN_W, SIGN_H } from './weatherart.js';
import { registerHit, registerKey, registerLabels, registerLayer, tap } from './registry.js';
import { link, repoUrl } from './dialogs.js';
import { buildSign, buildWords, failingJobs, shortSha, weatherOf } from './buildweather.js';

/* A theme without its own weather gets the village's. */
const STORM = { style: 'storm', cloud: '#50506a', cloudD: '#30303f', rain: P.glass, tint: 'rgba(16,20,44,0.32)', flash: 'rgba(255,255,236,0.38)', alarm: '#f03030' };
const weatherStyle = () => (state.T && state.T.weather) || STORM;

/* Weather over the dev village, under the labels and over everything else. */
registerLayer({ z: 150, draw(c, { v, lay }, t) {
  const kind = weatherOf(v.build);
  if (kind === 'clear' || !lay) return;
  const wx = weatherStyle(), W = lay.devW || lay.W, H = lay.devH || lay.H;
  if (kind === 'drizzle') { rain(c, 0, 12, W, H - 12, Math.floor(W / 8), wx.rain, t, 2); return; }
  if (kind === 'squall') {
    // Over the gatehouse (or where the road meets the quay when there is none).
    const g = lay.gate, x0 = g ? g.x0 - 4 : lay.roadX - 20, y0 = g ? g.y - 12 : lay.quayY - 26, w = g ? g.w + 8 : 46;
    clouds(c, x0, y0, w, wx, t);
    if (wx.style === 'storm') rain(c, x0, y0 + 5, w, 20, Math.floor(w / 2), wx.rain, t);
    else alarmLamp(c, x0 + (w >> 1), y0 + 6, state.reduce || blink(t, 420), wx.alarm);
    tap(x0, y0 - 2, w, 12, { kind: 'build' });
    return;
  }
  // A storm over the whole village. The tint pulses in a red alert.
  const pulse = wx.style === 'alert' && !state.reduce ? 0.6 + 0.4 * Math.abs(Math.sin(t / 700)) : 1;
  c.save(); c.globalAlpha = pulse; R(c, 0, 0, W, H, wx.tint); c.restore();
  if (wx.style === 'alert') {
    // Red light bars along the top, and every roof's alarm.
    for (let x = 2; x < W; x += 16) R(c, x, 1, 8, 2, blink(t + x * 20, 500) || state.reduce ? wx.alarm : P.redD);
    alarms(c, lay, wx, t);
  } else {
    clouds(c, 0, 4, W, wx, t);
    if (wx.style === 'smog') alarms(c, lay, wx, t);
    else rain(c, 0, 12, W, H - 12, Math.floor(W / 3), wx.rain, t);
  }
  // Now and then, lightning: a flash and a bolt, for 150 ms every 5 s.
  if (wx.style === 'storm' && !state.reduce && t % 5000 < 150) {
    R(c, 0, 0, W, H, wx.flash);
    bolt(c, (Math.floor(t / 5000) * 97) % Math.max(20, W - 20) + 10, 8);
  }
  tap(0, 0, W, 10, { kind: 'build' });
} });

/* Where the sign and the blamed commit's crate stand: on the quay, west of
 * the road down to it. */
const signAt = lay => ({ x: lay.roadX - 46, y: lay.quayY - 13 });

/* The BUILD BROKEN sign and the blame crate, while the build is red. */
registerLayer({ z: 86, draw(c, { v, lay }) {
  if (!buildSign(v.build) || !lay) return;
  const { x, y } = signAt(lay);
  shadow(c, x + 1, y + SIGN_H, SIGN_W, 1);
  brokenSign(c, x, y);
  tap(x, y, SIGN_W, SIGN_H, { kind: 'build' });
  const b = v.build;
  if (b.blame) {
    // The commit that turned it red: a crate under a red flag.
    const cx = x - 10, cy = y + 6;
    shadow(c, cx, cy + 7, 7, 1);
    blameCrate(c, cx, cy);
    tap(cx, cy - 7, 7, 14, { kind: 'blame' });
  }
} });

/* The sign's words, readable from across the room. */
registerLabels(sc => {
  const sign = buildSign(sc.v.build);
  if (!sign || !sc.L) return [];
  const { x, y } = signAt(sc.L);
  return [{ x: x + 13, y: y - 1, anchor: 'bottom', text: sign.title, sub: sign.detail, cls: 'buildsign' }];
}, 5);

/* ---------- Dialogs ---------- */
const buildLinks = (v, b) => [link('Open the run', b.url),
  ...(b.jobs || []).filter(j => j.url).map(j => link(`${j.name} · ${j.state}`, j.url)),
  repoUrl(v) && b.sha ? link(`Commit ${shortSha(b.sha)}`, `${repoUrl(v)}/commit/${b.sha}`) : null];

registerHit({ kind: 'build', dialog(o, sc) {
  const v = sc.v, b = v.build || {}, branch = v.pipeline.integration || 'The integration branch';
  const title = { red: b.partial ? 'Build partly broken' : 'Build broken', running: 'Build running', green: 'Build green' }[b.state] || 'Build';
  return { title, line: `${branch}: ${buildWords(b) || b.state || 'unknown'}`,
    meta: [failingJobs(b) || null, b.sha ? `at ${shortSha(b.sha)}` : null, b.blame ? `turned red at ${shortSha(b.blame)}` : null,
      ...(b.tests || []).slice(0, 12), (b.tests || []).length > 12 ? `+${b.tests.length - 12} more failing` : null, b.source ? `per ${b.source}` : null],
    links: buildLinks(v, b) };
} });

registerHit({ kind: 'blame', dialog(o, sc) {
  const v = sc.v, b = v.build || {}, base = repoUrl(v);
  return { title: `Commit ${shortSha(b.blame)}`, line: `turned ${v.pipeline.integration || 'the build'} red${b.since ? ` · ${buildWords({ state: 'red', since: b.since }).replace('build red · ', '')}` : ''}`,
    meta: [failingJobs(b) || null], links: [link('Open the run', b.url), base ? link('Open the commit', `${base}/commit/${b.blame}`) : null] };
} });

/* ---------- The Key's rows, in the theme's own weather ---------- */
const STORM_WORDS = { storm: 'Storm over the village', smog: 'Smog and alarm lights over the town', alert: 'Red alert over the base' };
registerKey({ group: 'Weather: the build', rows: theme => {
  const wx = (theme && theme.weather) || STORM;
  return [
    [c => { R(c, 0, 0, 14, 14, wx.tint); puff(c, 0, 2, wx); rain(c, 2, 8, 10, 4, 3, wx.rain, 0, 2); },
      `${STORM_WORDS[wx.style] || STORM_WORDS.storm}: the build is red on every OS (lightning now and then)`],
    [c => { puff(c, 0, 3, wx); rain(c, 4, 9, 7, 3, 2, wx.rain, 0, 2); }, 'A cloud over the gatehouse only: red on one OS or one job'],
    [c => rain(c, 2, 2, 10, 10, 4, wx.rain, 0, 2), 'Drizzle: the build is running'],
    [c => brokenSign(c, 0, 0), 'BUILD BROKEN sign on the quay: the failing jobs; click for the tests and the run', SIGN_W],
    [c => blameCrate(c, 3, 7), 'A crate under a red flag on the quay: the commit that turned the build red'],
  ];
} });

