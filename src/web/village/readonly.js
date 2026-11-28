/* ---------- Whether this server takes control requests (#506, #565) ----------
 * Asked once from /api/config and cached; a failed ask is asked again next
 * time, not remembered as read-only. Every feature that posts anything
 * (worker controls, a decision, filing an issue) hides its buttons when this
 * resolves false, instead of only finding out from a 403 after a click. */
let writable = null;
export function canWrite() {
  if (!writable) {
    writable = fetch('api/config').then(r => (r.ok ? r.json() : Promise.reject(r.status))).then(c => !c.read_only)
      .catch(() => { writable = null; return false; });
  }
  return writable;
}
