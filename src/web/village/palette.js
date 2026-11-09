/* The village's colours and the drawing primitives every sprite uses. */

/* ---------- Palette (15-bit, outline #202030) ---------- */
export const P = {
  o: '#202030', skin: '#f8c898', skinS: '#d09068',
  grass: '#58a048', grassD: '#386830', grassL: '#88c858',
  path: '#d8b878', pathS: '#b08858', pathL: '#e8d098',
  wood: '#a06030', woodD: '#683818', woodL: '#c88848', plank: '#e0b070',
  stone: '#9898a8', stoneD: '#606078', stoneL: '#c8c8d8',
  roofB: '#4868b8', roofBD: '#283878',
  leaf: '#308838', leafD: '#205028', leafL: '#58b050',
  white: '#f8f8f8', gold: '#f8d040', glass: '#a8d0f0', lamp: '#f8e080', dark: '#383850',
  red: '#c04838', redD: '#802828', thatch: '#c8a048', thatchD: '#907028',
  teal: '#2878a0', tealD: '#185070', parch: '#e8dcc0',
  purple: '#7848a8', purpleD: '#503078', copper: '#d08840',
  slate: '#6878a0', slateD: '#485878', barn: '#a83028', barnD: '#701818',
  bug: '#e03838', fizzA: '#e070d0', fizzB: '#60e0c8', shield: '#3058b0', mill: '#d8c8a8', millD: '#a89878', sail: '#f0e8d8',
  dome: '#e0e4f0', domeD: '#a8b0c8', water: '#3870c0', waterD: '#2850a0', waterL: '#80b8f0',
  p0: '#e02828', seal: '#38c848', web: '#d8d8e8'
};
export const STATE = { blocked: '#f07020', done: '#3878e0', unknown: '#585870', stalled: '#8088a8' };
export const KINDCOL = { bug: '#e03838', feature: '#40b8e8', tests: '#e070d0', security: '#3058b0', research: '#c0a8f0', perf: '#d8c8a8', refactor: '#6878a0', docs: '#2878a0', chores: '#a83028', workshop: '#c88848', cottage: '#c8a048' };
export const NOTECOL = { 'needs-design': '#f8c800', 'needs-clarification': '#d0c0f8', 'needs-approval': '#e89890', 'too-complex': '#b02010', 'future': '#8088a8', 'proposal': '#c0a8f0', 'failed': '#e02828', 'conflict': '#f07020', 'unreachable': '#585870' };
export const STOPCOL = ['#38c848', '#f8d040', '#f8f8f8', '#4868e8', '#e070d0'];
/* One cloth colour per feature banner, in the order the server lists them. */
export const FEATURE_COLS = ['#40b8e8', '#e070d0', '#60e0c8', '#f07020', '#c0a8f0', '#a8c8f8'];
/* A sub-agent's accent (tie, apron, belt) matches the building its specialty would build. */
export const SPECIALTIES = [['architect', 'research'], ['security', 'security'], ['test', 'tests'], ['doc', 'docs'], ['coder', 'feature']];
export const subHat = s => {
  const role = s.agent_type.toLowerCase().split(':').pop();
  const hit = SPECIALTIES.find(([name]) => role.includes(name));
  return hit ? KINDCOL[hit[1]] : P.white;
};

export const R = (c, x, y, w, h, col) => { c.fillStyle = col; c.fillRect(Math.round(x), Math.round(y), w, h); };
export function hash(x, y) {
  let n = (Math.imul(x, 374761393) + Math.imul(y, 668265263)) | 0;
  n = Math.imul(n ^ (n >>> 13), 1274126177);
  return ((n ^ (n >>> 16)) >>> 0) / 4294967296;
}
export const blink = (t, ms) => Math.floor(t / ms) % 2 === 1;
/* Someone who needs you waves: arm up, hand swinging. Reduced motion keeps the arm up, still. */
export const waveFrame = t => blink(t, 220) ? 'wave2' : 'wave';
/* Light comes from the upper left: shadows fall right and down. */
export const SHADE = 'rgba(12,20,36,.28)', SHINE = 'rgba(255,255,240,.22)';
export const shadow = (c, x, y, w, h) => R(c, x, y, w, h, SHADE);
export const lit = st => st === 'working';
export const glassOf = st => st === 'working' ? P.lamp : (st === 'unknown' || st === 'stopped') ? P.dark : P.glass;

/* The Future theme's colours: deck plates, hull, cyan and magenta light. */
export const F = {
  deck: '#30365c', deckD: '#262b4c', deckL: '#40487a', seam: '#1c2140', rivet: '#525c94',
  navy: '#1c2140', space: '#120e2a', purple: '#4a3a78', purpleD: '#33285a',
  cyan: '#40e0f0', cyanD: '#2098b0', cyanL: '#b8f8ff', magenta: '#f048c0', magentaD: '#a83090',
  hull: '#c8d0e8', hullD: '#8a94b8', hullL: '#eef2ff', glass: '#6cc8e8', glassD: '#2c5878',
  plasma: '#7a4ae0', plasmaL: '#b890ff', panel: '#2a50a8', panelL: '#5a8ae0'
};
