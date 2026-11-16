// What the village widget shows, worked out from the agents-tui server's
// /api/swarms and /api/village. No QML here, so it runs under node for tests
// and under Quickshell for the panel.
//
// The widget never sends anything to agents: it only reads these two endpoints,
// so it works against a --read-only server.

function parse(raw) {
  try {
    var parsed = JSON.parse(String(raw || "").trim())
    return parsed && typeof parsed === "object" ? parsed : null
  } catch (e) {
    return null
  }
}

// "http://nuc-1:7890/" → "http://nuc-1:7890". A bare host gets http://.
function baseUrl(server) {
  var s = String(server || "").trim().replace(/\/+$/, "")
  if (s === "") return "http://nuc-1:7890"
  return /^[a-z]+:\/\//i.test(s) ? s : "http://" + s
}

// The server's links are relative to it (/#/swarm/...); issue links are absolute.
function absolute(base, url) {
  var u = String(url || "")
  if (u === "") return ""
  return u.charAt(0) === "/" ? base + u : u
}

function villageUrl(base, project) {
  return base + "/village#" + encodeURIComponent(project)
}

function villagesOf(doc) {
  return doc && Array.isArray(doc.villages) ? doc.villages : []
}

function swarmsOf(doc) {
  return doc && Array.isArray(doc.swarms) ? doc.swarms : []
}

function villageFor(villagesDoc, project) {
  var vs = villagesOf(villagesDoc)
  for (var i = 0; i < vs.length; i++) if (vs[i].project === project) return vs[i]
  return null
}

function swarmFor(swarmsDoc, project) {
  var ss = swarmsOf(swarmsDoc)
  for (var i = 0; i < ss.length; i++) if (ss[i].project_name === project) return ss[i]
  return null
}

// How many things in a village wait on a person: attention()'s list, counted.
function needsYou(v) {
  return attention("", v).length
}

// Houses held by a usage or rate limit (#484): counted apart from those
// waiting on you, since they wait on the limit, not on a person. As the
// village page's limits.js reads it (QML's JavaScript can't import that
// module, so this mirrors it): idle or done with a limit.
function isLimited(h) {
  return !!(h && h.limit && (h.status === "idle" || h.status === "done"))
}

function limited(v) {
  return v ? (v.houses || []).filter(isLimited).length : 0
}

// What a limited house says instead of its status: "limited · til 3pm".
function limitWords(h) {
  if (!isLimited(h)) return null
  var r = h.limit.resets
  return r ? "limited · " + (/^in /.test(r) ? r : "til " + r) : "rate limited"
}

function working(v) {
  return v ? (v.houses || []).filter(function(h) { return h.status === "working" }).length : 0
}

// The dashboard's list of swarms (the same list as / on the server), with
// each one's village counts. A server that only draws outside reports has no
// swarms, so its villages stand in.
function swarmRows(swarmsDoc, villagesDoc) {
  var rows = swarmsOf(swarmsDoc).map(function(s) {
    var v = villageFor(villagesDoc, s.project_name)
    return {
      project: s.project_name,
      workflow: s.workflow || (v && v.framework) || s.agent_type || "",
      houses: v ? (v.houses || []).length : (s.workers || []).length,
      working: v ? working(v) : Number(s.busy_count || 0),
      waiting: v ? needsYou(v) : Number(s.attention_count || 0),
      limited: limited(v),
      stopped: !!s.stopped
    }
  })
  if (rows.length > 0) return rows
  return villagesOf(villagesDoc).map(function(v) {
    return { project: v.project, workflow: v.framework || "", houses: (v.houses || []).length, working: working(v), waiting: needsYou(v), limited: limited(v), stopped: false }
  })
}

// Which village to show: the one the settings name, else the one already
// open, else the only one. "" is the list.
function chooseProject(rows, configured, current) {
  var has = function(p) { return p !== "" && rows.some(function(r) { return r.project === p }) }
  if (has(configured)) return configured
  if (has(current)) return current
  return rows.length === 1 ? rows[0].project : ""
}

// The list needs a way back to it only when there is more than one swarm
// and the settings don't pin one.
function canGoBack(rows, configured) {
  return rows.length > 1 && !rows.some(function(r) { return r.project === configured })
}

// A house's name: its worktree, less the project prefix ("agents-ui-wt-2" → "wt-2").
function houseName(project, id, role) {
  var base = String(id || "").split("/").filter(function(p) { return p !== "" }).pop() || role || ""
  return project && base.indexOf(project + "-") === 0 ? base.slice(project.length + 1) : base
}

function sessionLink(links) {
  var l = (links || []).filter(function(l) { return /^\/#\/swarm\/[^/]+\/agent\//.test(l.url) })[0]
  return l ? l.url : ""
}

// One row per worker: worktree, branch, issue, status, context, and where a click goes.
function houseRows(base, v, swarm) {
  if (!v) return []
  var workers = swarm ? swarm.workers || [] : []
  return (v.houses || []).map(function(h) {
    var w = workers.filter(function(w) { return w.worktree_path === h.id })[0] || workers.filter(function(w) { return w.role === h.role })[0]
    return {
      name: houseName(v.project, h.id, h.role),
      role: h.role || "",
      branch: w && w.branch ? w.branch : "",
      issue: h.issue || null,
      title: h.issue_title || h.title || "",
      status: h.status || "unknown",
      context: w && w.context_pct !== null && w.context_pct !== undefined ? Number(w.context_pct) : null,
      limit: limitWords(h),
      url: absolute(base, sessionLink(h.links))
    }
  })
}

// The village's counter row: backlog, board, houses, the merge gate, the
// quay, then each environment stop.
function flow(v) {
  if (!v) return []
  var p = v.pipeline || {}
  var gates = p.gates || []
  var inGate = gates.filter(function(g) { return g.state === "queued" || g.state === "merging" || g.state === "gating" })
  var houses = v.houses || []
  var items = [
    { label: "Backlog", count: (v.backlog || []).length },
    { label: "Board", count: (v.board || []).length },
    { label: "Houses", count: houses.length, detail: working(v) + " working" }
  ]
  if (inGate.length > 0 || p.merge) {
    var gating = inGate.filter(function(g) { return g.state === "gating" }).length
    items.push({ label: "Gate", count: inGate.length, detail: gating + " gating" })
  }
  items.push({ label: "Quay", count: (p.quay || []).length })
  ;(p.stops || []).forEach(function(s) { items.push({ label: s.name, count: (s.issues || []).length }) })
  return items
}

var GATE_WORDS = { failed: "Gate failed", conflict: "Merge conflict", "red-base": "Red base" }

// Everything waiting on a person, each with where to go about it. This is the
// one rule for "needs you": the red dot is lit while it lists anything, and
// the notifier (agents-village-notify) notifies when an item first appears.
// `key` names the item the same way on every poll, so the two never disagree
// about what is new; `kind` is "needs" (a person must answer or decide) or
// "gate" (a merge bounced); `house` is the name a village deep link takes
// (`/village#<project>&house=<house>`), for the manager and houses.
function attention(base, v) {
  if (!v) return []
  var out = []
  if (v.manager && v.manager.status === "blocked") {
    out.push({ key: "manager", kind: "needs", house: "manager", what: "Manager", title: v.manager.title || "is waiting on you",
      question: questionOf(v.manager), url: absolute(base, sessionLink(v.manager.links)) })
  }
  ;(v.houses || []).forEach(function(h) {
    if (h.status !== "blocked") return
    var name = houseName(v.project, h.id, h.role)
    // Two houses with one short name (worktrees in two folders): the link names the full id.
    var twin = (v.houses || []).some(function(o) { return o !== h && houseName(v.project, o.id, o.role) === name })
    out.push({ key: "house:" + (h.id || name) + ":" + (h.issue || ""), kind: "needs", house: twin && h.id ? h.id : name, what: name,
      title: h.issue ? "#" + h.issue + " " + (h.issue_title || "") : "is waiting on you",
      question: questionOf(h), url: absolute(base, sessionLink(h.links)) })
  })
  ;((v.pipeline && v.pipeline.gates) || []).forEach(function(g) {
    if (!GATE_WORDS[g.state] || (g.state === "red-base" && g.landed)) return
    out.push({ key: "gate:" + g.issue + ":" + g.state, kind: "gate", what: GATE_WORDS[g.state], title: "#" + g.issue, url: g.failure_url || "" })
  })
  ;(v.board || []).forEach(function(n) {
    out.push({ key: "board:" + n.number + ":" + (n.label || ""), kind: "needs", what: n.label || "blocked", title: "#" + n.number + " " + (n.title || ""), url: n.url || "" })
  })
  return out
}

// The question an agent waits on, in one line: "Proceed? (y/n)". "" for none.
function questionOf(h) {
  var q = h && h.question
  return q && q.prompt ? String(q.prompt).replace(/\s+/g, " ").trim() : ""
}

function features(v) {
  if (!v) return []
  return (v.features || []).map(function(f) {
    var b = f.build || {}
    return { number: f.number, title: f.title || "", build: "Build " + Number(b.landed || 0) + "/" + Number(b.total || 0), live: !!f.live, url: f.url || "" }
  })
}

// What the last stop runs, as the village page tells a release: its sha,
// keyed by where that came from, so a gh outage falling back to the branch
// never counts as a release. "" when nothing says.
function releaseKey(v) {
  var stops = (v && v.pipeline && v.pipeline.stops) || []
  var last = stops[stops.length - 1]
  return last && last.sha ? String(last.source || "") + ":" + last.sha : ""
}

// Villages whose release changed since `seen` (project → key), and the new
// `seen`. A village seen for the first time sets its key without sailing,
// as the page never sails on load.
function releases(villagesDoc, seen) {
  var next = {}, sailed = []
  villagesOf(villagesDoc).forEach(function(v) {
    var key = releaseKey(v)
    next[v.project] = key
    var was = seen ? seen[v.project] : undefined
    if (key !== "" && was !== undefined && was !== "" && was !== key) sailed.push(v.project)
  })
  return { seen: next, sailed: sailed }
}

// The integration branch's build (#413's `build`): "red", "running", "green",
// or "unknown" when the village doesn't say.
var BUILD_RANK = { unknown: 0, green: 1, running: 2, red: 3 }
function buildState(v) {
  var s = v && v.build && v.build.state
  return BUILD_RANK[s] !== undefined ? s : "unknown"
}

// "20:40": when, on this machine's clock. "" for no time.
function clock(iso) {
  var t = Date.parse(iso || "")
  if (isNaN(t)) return ""
  var d = new Date(t), pad = function(n) { return (n < 10 ? "0" : "") + n }
  return pad(d.getHours()) + ":" + pad(d.getMinutes())
}

// The build in a few words: "build red · 3 tests failing since 20:40",
// "build red on macOS since 20:40", "build running", "build green"; "" when
// the village doesn't say.
function buildWords(v) {
  var b = (v && v.build) || {}, state = buildState(v)
  if (state === "unknown") return ""
  if (state !== "red") return "build " + state
  var oses = []
  ;(b.jobs || []).forEach(function(j) { if (j.state === "red" && j.os && oses.indexOf(j.os) < 0) oses.push(j.os) })
  var head = "build red" + (b.partial && oses.length ? " on " + oses.join(", ") : "")
  var n = (b.tests || []).length, since = clock(b.since)
  var tail = (n ? n + (n === 1 ? " test" : " tests") + " failing" : "") + (since ? (n ? " " : "") + "since " + since : "")
  return tail ? head + (n ? " · " : " ") + tail : head
}

// The bar icon: houses working, a red dot when anything waits on you, a ship
// while a deployment sails (`sailing`: the projects whose ship is out), dimmed
// when the server can't be reached. One village when one is picked, all of
// them otherwise.
function badge(villagesDoc, project, reachable, sailingProjects) {
  var vs = villagesOf(villagesDoc).filter(function(v) { return project === "" || v.project === project })
  var out = sailingProjects || []
  var sailing = vs.some(function(v) { return out.indexOf(v.project) >= 0 || !!(v.pipeline && v.pipeline.sailing) })
  var worst = vs.reduce(function(w, v) { return !w || BUILD_RANK[buildState(v)] > BUILD_RANK[buildState(w)] ? v : w }, null)
  return {
    reachable: !!reachable,
    working: vs.reduce(function(n, v) { return n + working(v) }, 0),
    alert: vs.some(function(v) { return needsYou(v) > 0 }),
    sailing: sailing,
    // The worst build among them, and its words: the icon turns red for a red one.
    build: worst ? buildState(worst) : "unknown",
    buildWords: worst ? buildWords(worst) : ""
  }
}

function ago(thenMs, nowMs) {
  if (!thenMs) return "never"
  var s = Math.max(0, Math.floor((nowMs - thenMs) / 1000))
  if (s < 5) return "just now"
  if (s < 60) return s + "s ago"
  if (s < 3600) return Math.floor(s / 60) + "m ago"
  return Math.floor(s / 3600) + "h ago"
}

// The repo's web page, from an issue link in the village.
function repoUrl(v) {
  if (!v) return ""
  var urls = []
  ;(v.houses || []).forEach(function(h) { (h.links || []).forEach(function(l) { urls.push(l.url) }) })
  ;(v.board || []).concat(v.backlog || []).forEach(function(n) { if (n.url) urls.push(n.url) })
  for (var i = 0; i < urls.length; i++) {
    var m = /^(https:\/\/github\.com\/[^/]+\/[^/]+)\/issues\/\d+$/.exec(String(urls[i]))
    if (m) return m[1]
  }
  return ""
}

if (typeof module !== "undefined") {
  module.exports = {
    parse: parse, baseUrl: baseUrl, absolute: absolute, villageUrl: villageUrl,
    villageFor: villageFor, swarmFor: swarmFor, needsYou: needsYou, limited: limited, limitWords: limitWords, swarmRows: swarmRows,
    chooseProject: chooseProject, canGoBack: canGoBack, houseName: houseName, houseRows: houseRows,
    flow: flow, attention: attention, features: features, badge: badge, ago: ago, repoUrl: repoUrl,
    releaseKey: releaseKey, questionOf: questionOf, releases: releases, buildState: buildState, buildWords: buildWords, clock: clock
  }
}
