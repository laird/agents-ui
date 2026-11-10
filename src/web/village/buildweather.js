/* ---------- Build weather ----------
 * What the village's weather says about its integration branch's build
 * (`build` in /api/village, src/web/build.rs). No DOM, so its tests run
 * under node (tools/village-tests/buildweather.test.mjs):
 *   red          a storm over the whole village;
 *   red, partial a squall over the gatehouse only (one OS or one job);
 *   running      a drizzle;
 *   green/none   clear weather. */

/* The weather for a build: 'storm', 'squall', 'drizzle' or 'clear'. */
export function weatherOf(build) {
  const s = build && build.state;
  if (s === 'red') return build.partial ? 'squall' : 'storm';
  if (s === 'running') return 'drizzle';
  return 'clear';
}

const plural = (n, word) => `${n} ${word}${n === 1 ? '' : 's'}`;

/* The failing jobs in a few words, one per OS (or job when its OS is
 * unknown), with their failing tests: "macOS: 2 tests · Linux". */
export function failingJobs(build) {
  const groups = new Map();
  ((build && build.jobs) || []).filter(j => j.state === 'red').forEach(j => {
    const key = j.os || j.name;
    const g = groups.get(key) || new Set();
    (j.tests || []).forEach(t => g.add(t));
    groups.set(key, g);
  });
  return [...groups].map(([name, tests]) => (tests.size ? `${name}: ${plural(tests.size, 'test')}` : name)).join(' · ');
}

/* The sign at the quay while the build is red: { title, detail }, or null. */
export function buildSign(build) {
  if (!build || build.state !== 'red') return null;
  return { title: 'BUILD BROKEN', detail: failingJobs(build) || (build.source ? `per ${build.source}` : '') };
}

/* "20:40" for a time today, "Nov 9 17:49" for an earlier day. */
export function redSince(iso, now = new Date()) {
  const d = new Date(iso);
  if (!iso || Number.isNaN(d.getTime())) return '';
  const hm = `${String(d.getHours()).padStart(2, '0')}:${String(d.getMinutes()).padStart(2, '0')}`;
  if (d.toDateString() === now.toDateString()) return hm;
  return `${d.toLocaleString('en', { month: 'short' })} ${d.getDate()} ${hm}`;
}

/* The live status line's words for the build: "build red · 3 tests failing
 * since 20:40", "build running", "build green", or '' when unknown. */
export function buildWords(build, now = new Date()) {
  const s = build && build.state;
  if (s === 'red') {
    const n = (build.tests || []).length;
    const since = redSince(build.since, now);
    return ['build red', n ? `${plural(n, 'test')} failing` : '', since ? `since ${since}` : ''].filter(Boolean).join(' · ');
  }
  if (s === 'running') return 'build running';
  if (s === 'green') return 'build green';
  return '';
}

/* A short commit id. */
export const shortSha = sha => String(sha || '').slice(0, 7);
