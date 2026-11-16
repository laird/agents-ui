// What the notifier tells you, and when, worked out from the agents-tui
// server's /api/village. No QML here, so it runs under node for tests and
// under Quickshell for the service.
//
// It notifies on transitions only. Each poll is boiled down to a snapshot
// (per village: the keys of what needs you, the build's state, the deploys
// that landed or failed), and only a key the previous snapshot lacked, or a
// build that changed colour, is news. A village seen for the first time sets
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
    server: String(n.server || s.server || "http://nuc-1:7890").replace(/\/+$/, ""),
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

// The village page, opened on a project and, for a house or the manager, on
// that house's dialog.
function deepLink(base, project, house) {
  return base + "/village#" + encodeURIComponent(project) + (house ? "&house=" + encodeURIComponent(house) : "")
}

function shortSha(sha) {
  return String(sha || "").slice(0, 7)
}

// Deploys that landed or failed: each environment stop's running commit, and
// each voyage that docked or turned back. A docked voyage and the stop it
// updated share a key, so one deploy is one piece of news.
function deploysOf(v) {
  var p = (v && v.pipeline) || {}, out = {}
  ;(p.stops || []).forEach(function(s) {
    if (s.kind === "gate" || !s.sha || s.source === "branch") return
    out["deploy:" + s.name + ":" + s.sha + ":docked"] = { env: s.name, sha: s.sha, tag: s.tag || "", state: "docked", url: s.url || "" }
  })
  ;(p.voyages || []).forEach(function(y) {
    if ((y.state !== "docked" && y.state !== "failed") || !y.sha) return
    out["deploy:" + y.to + ":" + y.sha + ":" + y.state] = { env: y.to, from: y.from, sha: y.sha, tag: y.tag || "", state: y.state, url: y.url || "" }
  })
  return out
}

// One village, boiled down: what is compared from one poll to the next.
// `broken` holds from a red build until a green one, so red, running, green
// is one break and one fix.
function villageSnapshot(v, was, opts) {
  var Model = modelOf(opts), build = Model.buildState(v)
  return {
    needs: Model.attention("", v).map(function(a) { return a.key }),
    build: build,
    broken: build === "red" || (!!(was && was.broken) && build !== "green"),
    deploys: Object.keys(deploysOf(v))
  }
}

function snapshotOf(villagesDoc, opts) {
  var out = {}
  ;((villagesDoc && villagesDoc.villages) || []).forEach(function(v) { out[v.project] = villageSnapshot(v, null, opts) })
  return out
}

// What changed in one village since `was`: a list of events, each
// { kind, project, key, title, body, url }.
function villageEvents(base, v, was, opts) {
  var Model = modelOf(opts), events = [], project = v.project
  var village = deepLink(base, project, "")
  var oldNeeds = was.needs || [], oldDeploys = was.deploys || []
  Model.attention(base, v).forEach(function(a) {
    if (oldNeeds.indexOf(a.key) >= 0) return
    var e = { kind: a.kind, project: project, key: a.key, url: a.house ? deepLink(base, project, a.house) : village }
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
  var build = Model.buildState(v), wasBuild = was.build || "unknown"
  if (build === "red" && !was.broken && wasBuild !== "unknown") {
    events.push({ kind: "build", project: project, key: "build:red", title: project + ": build broken", body: Model.buildWords(v), url: village })
  } else if (build === "green" && was.broken) {
    events.push({ kind: "build", project: project, key: "build:green", title: project + ": build fixed", body: "The integration branch is green again.", url: village })
  }
  var deploys = deploysOf(v)
  Object.keys(deploys).forEach(function(key) {
    if (oldDeploys.indexOf(key) >= 0) return
    var d = deploys[key], what = d.tag || shortSha(d.sha)
    events.push(d.state === "failed"
      ? { kind: "deploy", project: project, key: key, title: project + ": deploy to " + d.env + " failed", body: what + " did not land; " + d.env + " keeps what it ran.", url: village }
      : { kind: "deploy", project: project, key: key, title: project + ": " + what + " deployed to " + d.env, body: d.from && d.from !== "dev" ? "Promoted from " + d.from + "." : "", url: village })
  })
  return events
}

// One poll: { snapshot, events }. `prev` is the last snapshot (null on the
// first poll ever); a village it does not know is only recorded. A village
// missing from this poll keeps its old snapshot, so a server that briefly
// drops it does not make everything news again when it comes back.
function step(prev, villagesDoc, base, opts) {
  var next = {}, events = []
  var old = prev && typeof prev === "object" ? prev : {}
  Object.keys(old).forEach(function(p) { next[p] = old[p] })
  ;((villagesDoc && villagesDoc.villages) || []).forEach(function(v) {
    next[v.project] = villageSnapshot(v, old[v.project], opts)
    if (old[v.project]) events = events.concat(villageEvents(base, v, old[v.project], opts))
  })
  return { snapshot: next, events: events }
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
    KINDS: KINDS, settingsOf: settingsOf, quietHours: quietHours, isQuiet: isQuiet, deepLink: deepLink,
    deploysOf: deploysOf, snapshotOf: snapshotOf, villageEvents: villageEvents, step: step, notifications: notifications
  }
}
