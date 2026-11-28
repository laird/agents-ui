// What the notifier tells you, and when, worked out from the agents-tui
// server's /api/village. No QML here, so it runs under node for tests and
// under Quickshell for the service.
//
// It notifies on transitions only. Each poll is boiled down to a snapshot
// (per village: the keys of what needs you, the build's state, what each
// environment runs), and only a key the previous snapshot lacked, a build
// that changed colour, or an environment running a new commit is news. A village seen for the first time sets
// its snapshot without a word, as the village page never sails on load.
//
// "Needs you" is the bar widget's rule: Model.attention(), keyed the same way,
// so the red dot and the notifier always agree on what is new. Model.js
// beside this file is a copy of agents-village/Model.js (Omarchy refuses a
// symlink inside a plugin), and tests/notify.test.js fails if they differ.

var M = typeof require === "function" ? require("./Model.js") : null

// What can be notified, in the order the settings list them.
var KINDS = ["needs", "gate", "build", "deploy"]

function modelOf(opts) {
  return (opts && opts.model) || M
}

// The settings, from ~/.config/agents-ui/notify.json, with the screensaver's
// `dim` hours standing in for `quiet` when notify.json does not say.
function settingsOf(notifyRaw, saverRaw) {
  var n = parseObject(notifyRaw), s = parseObject(saverRaw)
  var interval = Number(n.interval)
  var kinds = Array.isArray(n.kinds) ? n.kinds.filter(function(k) { return KINDS.indexOf(k) >= 0 }) : KINDS.slice()
  return {
    enabled: n.enabled !== false,
    // notify.json `server`, else the screensaver's, else agents-tui's default bind.
    server: String(n.server || s.server || "http://127.0.0.1:7890").replace(/\/+$/, ""),
    interval: isFinite(interval) && interval >= 5 ? Math.floor(interval) : 15,
    kinds: kinds,
    quiet: n.quiet !== undefined ? String(n.quiet || "") : String(s.dim || "")
  }
}

function parseObject(raw) {
  if (raw && typeof raw === "object") return raw
  try {
    var p = JSON.parse(String(raw || "").trim())
    return p && typeof p === "object" && !Array.isArray(p) ? p : {}
  } catch (e) {
    return {}
  }
}

// "23-7" → [23, 7]; null for anything else (no quiet hours).
function quietHours(spec) {
  var m = /^(\d{1,2})-(\d{1,2})$/.exec(String(spec || "").trim())
  if (!m) return null
  var a = Number(m[1]), b = Number(m[2])
  return a < 24 && b < 24 && a !== b ? [a, b] : null
}

// Inside the quiet hours at `date` (local time)? "23-7" wraps past midnight.
function isQuiet(spec, date) {
  var q = quietHours(spec)
  if (!q) return false
  var h = (date || new Date()).getHours()
  return q[0] < q[1] ? h >= q[0] && h < q[1] : h >= q[0] || h < q[1]
}

function shortSha(sha) {
  return String(sha || "").slice(0, 7)
}

// Board notes and failed deploys come from GitHub, which the server can lose
// for a while (it falls back to the branch, the board comes back empty). So
// they are remembered for this long after they were last seen: coming back
// after an outage is not news. Houses, the manager and the gate are read from
// this machine and are compared poll to poll.
var GRACE_MS = 10 * 60 * 1000

// What each environment runs now: its stop's commit, or a voyage that docked
// there. A stop that names no deployment (a branch, nothing) says nothing, so
// the environment keeps what it was last seen to run.
function envsOf(v, old) {
  var p = (v && v.pipeline) || {}, out = {}
  Object.keys(old || {}).forEach(function(k) { out[k] = old[k] })
  ;(p.stops || []).forEach(function(s) {
    if (s.kind === "gate" || !s.sha || s.source === "branch") return
    out[s.name] = { sha: s.sha, tag: s.tag || "" }
  })
  ;(p.voyages || []).forEach(function(y) {
    if (y.state === "docked" && y.sha) out[y.to] = { sha: y.sha, tag: y.tag || "", from: y.from || "" }
  })
  return out
}

function failedVoyages(v) {
  return (((v && v.pipeline) || {}).voyages || []).filter(function(y) { return y.state === "failed" && y.sha })
}

// One village, boiled down: what is compared from one poll to the next.
// `broken` holds from a red build until a green one, so red, running, green
// is one break and one fix; `known` is whether any build colour was seen.
// `seen` holds the GitHub-read keys (board notes, failed deploys) with when
// each was last seen.
function villageSnapshot(v, was, now, opts) {
  var Model = modelOf(opts), build = Model.buildState(v), w = was || {}
  var seen = {}
  Object.keys(w.seen || {}).forEach(function(k) { if (now - w.seen[k] < GRACE_MS) seen[k] = w.seen[k] })
  var needs = Model.attention("", v).map(function(a) { return a.key })
  needs.forEach(function(k) { if (k.indexOf("issue:") === 0) seen[k] = now })
  failedVoyages(v).forEach(function(y) { seen["failed:" + y.to + ":" + y.sha] = now })
  var envs = envsOf(v, w.envs)
  Object.keys(envs).forEach(function(k) { envs[k] = { sha: envs[k].sha, tag: envs[k].tag } })
  return {
    needs: needs,
    seen: seen,
    build: build,
    known: !!w.known || build !== "unknown",
    broken: build === "red" || (!!w.broken && build !== "green"),
    envs: envs
  }
}

function snapshotOf(villagesDoc, opts) {
  var out = {}, now = (opts && opts.now) || Date.now()
  ;((villagesDoc && villagesDoc.villages) || []).forEach(function(v) { out[v.project] = villageSnapshot(v, null, now, opts) })
  return out
}

// What changed in one village since `was`: a list of events, each
// { kind, project, key, title, body, url }.
function villageEvents(base, v, was, opts) {
  var Model = modelOf(opts), events = [], project = v.project
  var oldNeeds = was.needs || [], oldSeen = was.seen || {}, oldEnvs = was.envs || {}
  Model.attention(base, v).forEach(function(a) {
    if (oldNeeds.indexOf(a.key) >= 0 || oldSeen[a.key]) return
    // attention()'s url is already the right deep link (#570, #581): house,
    // issue or gate.
    var e = { kind: a.kind, project: project, key: a.key, url: a.url }
    if (a.kind === "gate") {
      e.title = project + ": " + a.what.toLowerCase() + " on " + a.title
      e.body = "The merge gate handed the work back."
    } else if (a.house) {
      e.title = project + ": " + a.what + " is waiting on you"
      e.body = [a.title, a.question].filter(Boolean).join("\n")
    } else {
      e.title = project + ": " + a.title.split(" ")[0] + " is " + a.what
      e.body = a.title.split(" ").slice(1).join(" ")
    }
    events.push(e)
  })
  var build = Model.buildState(v)
  if (build === "red" && !was.broken && was.known) {
    events.push({ kind: "build", project: project, key: "build:red", title: project + ": build broken", body: Model.buildWords(v), url: Model.itemLink(base, project, { kind: "build" }) })
  } else if (build === "green" && was.broken) {
    events.push({ kind: "build", project: project, key: "build:green", title: project + ": build fixed", body: "The integration branch is green again.", url: Model.itemLink(base, project, { kind: "build" }) })
  }
  // A deploy is an environment running a new commit. A stop is news only
  // once its environment was known; a docked voyage is news by itself.
  var docked = {}
  ;(((v.pipeline || {}).voyages) || []).forEach(function(y) { if (y.state === "docked" && y.sha) docked[y.to] = y })
  var envs = envsOf(v, null)
  Object.keys(envs).forEach(function(env) {
    var now = envs[env], before = oldEnvs[env]
    if (before ? before.sha === now.sha : !docked[env]) return
    var what = now.tag || shortSha(now.sha)
    events.push({ kind: "deploy", project: project, key: "deploy:" + env + ":" + now.sha, url: Model.itemLink(base, project, { kind: "env", env: env }),
      title: project + ": " + what + " deployed to " + env,
      body: now.from && now.from !== "dev" ? "Promoted from " + now.from + "." : "" })
  })
  failedVoyages(v).forEach(function(y) {
    var key = "failed:" + y.to + ":" + y.sha
    if (oldSeen[key]) return
    events.push({ kind: "deploy", project: project, key: key, url: Model.itemLink(base, project, { kind: "env", env: y.to }), title: project + ": deploy to " + y.to + " failed",
      body: (y.tag || shortSha(y.sha)) + " did not land; " + y.to + " keeps what it ran." })
  })
  return events
}

// One poll: { snapshot, events }. `prev` is the last snapshot (null on the
// first poll ever); a village it does not know is only recorded. A village
// missing from this poll keeps its old snapshot, so a server that briefly
// drops it does not make everything news again when it comes back.
// `opts.now` (ms) is the clock, for tests.
function step(prev, villagesDoc, base, opts) {
  var next = {}, events = [], now = (opts && opts.now) || Date.now()
  var old = prev && typeof prev === "object" ? prev : {}
  Object.keys(old).forEach(function(p) { next[p] = old[p] })
  ;((villagesDoc && villagesDoc.villages) || []).forEach(function(v) {
    next[v.project] = villageSnapshot(v, old[v.project], now, opts)
    if (old[v.project]) events = events.concat(villageEvents(base, v, old[v.project], opts))
  })
  return { snapshot: next, events: events }
}

// Notification daemons (mako, swaync) read the body as Pango markup: an
// issue title with < or & would break it.
function markup(text) {
  return String(text || "").replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;")
}

// What to send: the events the settings want, outside quiet hours, a burst
// grouped into one notification. Each is { title, body, url }.
function notifications(events, settings, date) {
  var s = settings || settingsOf("", "")
  if (!s.enabled || isQuiet(s.quiet, date)) return []
  var want = events.filter(function(e) { return s.kinds.indexOf(e.kind) >= 0 })
  if (want.length <= 1) return want.map(function(e) { return { title: e.title, body: e.body || "", url: e.url } })
  var projects = []
  want.forEach(function(e) { if (projects.indexOf(e.project) < 0) projects.push(e.project) })
  var lines = want.slice(0, 5).map(function(e) {
    return projects.length === 1 ? e.title.slice(e.project.length + 2) : e.title
  })
  if (want.length > 5) lines.push("and " + (want.length - 5) + " more")
  return [{
    title: (projects.length === 1 ? projects[0] + ": " : "") + want.length + " things in the village",
    body: lines.join("\n"),
    url: want[0].url
  }]
}

if (typeof module !== "undefined") {
  module.exports = {
    KINDS: KINDS, settingsOf: settingsOf, quietHours: quietHours, isQuiet: isQuiet,
    envsOf: envsOf, markup: markup, GRACE_MS: GRACE_MS, snapshotOf: snapshotOf, villageEvents: villageEvents, step: step, notifications: notifications
  }
}
