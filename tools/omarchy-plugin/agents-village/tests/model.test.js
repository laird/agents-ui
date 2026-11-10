// node --test tools/omarchy-plugin/agents-village/tests/
// The widget's model against /api/swarms and /api/village as a live server
// served them (tests/fixtures), plus hand-made edge cases.
const test = require("node:test")
const assert = require("node:assert")
const fs = require("node:fs")
const path = require("node:path")
const M = require("../Model.js")

const fixture = name => JSON.parse(fs.readFileSync(path.join(__dirname, "fixtures", name), "utf8"))
const swarms = fixture("swarms.json")
const villages = fixture("village.json")
const BASE = "http://nuc-1:7890"

// Two swarms, for the list.
function twoSwarms() {
  const s = JSON.parse(JSON.stringify(swarms))
  s.swarms.push({ project_name: "shop", workflow: "Modernize", agent_type: "claude", stopped: false, busy_count: 0, attention_count: 1, workers: [{ role: "worker-1" }] })
  const v = JSON.parse(JSON.stringify(villages))
  v.villages.push({ project: "shop", framework: "modernize", houses: [{ id: "/src/shop-wt-1", role: "worker-1", status: "blocked", links: [] }], board: [], backlog: [], pipeline: { quay: [], stops: [] } })
  return [s, v]
}

test("parse and server urls", () => {
  assert.strictEqual(M.parse("not json"), null)
  assert.strictEqual(M.parse(""), null)
  assert.deepStrictEqual(M.parse('{"swarms": []}'), { swarms: [] })
  assert.strictEqual(M.baseUrl(""), "http://nuc-1:7890")
  assert.strictEqual(M.baseUrl("omarchy-1:7890/"), "http://omarchy-1:7890")
  assert.strictEqual(M.baseUrl("https://village.example/"), "https://village.example")
  assert.strictEqual(M.absolute(BASE, "/#/swarm/a/agent/b"), BASE + "/#/swarm/a/agent/b")
  assert.strictEqual(M.absolute(BASE, "https://github.com/o/r/issues/1"), "https://github.com/o/r/issues/1")
  assert.strictEqual(M.villageUrl(BASE, "agents ui"), BASE + "/village#agents%20ui")
})

test("the list is the dashboard's swarms, with village counts", () => {
  const rows = M.swarmRows(swarms, villages)
  assert.deepStrictEqual(rows, [{ project: "agents-ui", workflow: "autocoder", houses: 3, working: 3, waiting: 2, stopped: false }])
  const [s2, v2] = twoSwarms()
  assert.deepStrictEqual(M.swarmRows(s2, v2).map(r => [r.project, r.waiting]), [["agents-ui", 2], ["shop", 1]])
  // A server with only outside reports lists its villages.
  assert.deepStrictEqual(M.swarmRows({ swarms: [] }, v2).map(r => r.project), ["agents-ui", "shop"])
  assert.deepStrictEqual(M.swarmRows(null, null), [])
})

test("one swarm goes straight to its village; several start at the list", () => {
  const one = M.swarmRows(swarms, villages)
  assert.strictEqual(M.chooseProject(one, "", ""), "agents-ui")
  assert.strictEqual(M.canGoBack(one, ""), false, "nothing to go back to")
  const [s2, v2] = twoSwarms()
  const two = M.swarmRows(s2, v2)
  assert.strictEqual(M.chooseProject(two, "", ""), "", "the list")
  assert.strictEqual(M.chooseProject(two, "", "shop"), "shop", "the one picked stays open")
  assert.strictEqual(M.chooseProject(two, "shop", ""), "shop", "the settings pin one")
  assert.strictEqual(M.chooseProject(two, "gone", "agents-ui"), "agents-ui", "a pinned village that went away falls back")
  assert.strictEqual(M.chooseProject(two, "", "gone"), "", "a picked village that went away returns to the list")
  assert.strictEqual(M.canGoBack(two, ""), true)
  assert.strictEqual(M.canGoBack(two, "shop"), false, "pinned: no list to go back to")
})

test("houses: worktree, branch, issue, status, session link", () => {
  const v = M.villageFor(villages, "agents-ui")
  const rows = M.houseRows(BASE, v, M.swarmFor(swarms, "agents-ui"))
  assert.deepStrictEqual(rows[0], {
    name: "wt-2", role: "worker-2", branch: "feature/issue-378", issue: 378,
    title: "Omarchy bar widget for the village: live swarm status in the bar, a panel summary, and Open village",
    status: "working", context: null, url: BASE + "/#/swarm/agents-ui/agent/worker-2"
  })
  assert.deepStrictEqual(rows.map(r => r.name), ["wt-2", "wt-3", "wt-4"])
  assert.strictEqual(M.houseName("shop", "/src/other-repo", "worker-9"), "other-repo")
  assert.strictEqual(M.houseName("shop", "", "worker-9"), "worker-9")
  assert.deepStrictEqual(M.houseRows(BASE, null, null), [])
})

test("flow strip: backlog, board, houses, gate, quay, each stop", () => {
  const v = M.villageFor(villages, "agents-ui")
  assert.deepStrictEqual(M.flow(v), [
    { label: "Backlog", count: 4 },
    { label: "Board", count: 1 },
    { label: "Houses", count: 3, detail: "3 working" },
    { label: "Gate", count: 0, detail: "0 gating" },
    { label: "Quay", count: 5 },
    { label: "nuc-1", count: 17 },
    { label: "release", count: 0 }
  ])
  const noGate = { houses: [], backlog: [], board: [], pipeline: { quay: [], stops: [] } }
  assert.deepStrictEqual(M.flow(noGate).map(i => i.label), ["Backlog", "Board", "Houses", "Quay"], "no gate on a remote village")
})

test("attention: the board, failed gates, blocked houses", () => {
  const v = M.villageFor(villages, "agents-ui")
  assert.deepStrictEqual(M.attention(BASE, v), [
    { what: "Merge conflict", title: "#370", url: "https://github.com/laird/agents-ui/issues/370" },
    { what: "needs-approval", title: "#365 Record release Deployments from a release-published workflow (needs the workflow token scope)", url: "https://github.com/laird/agents-ui/issues/365" }
  ])
  const blocked = { project: "shop", manager: { status: "blocked", title: "Allow npm publish?", links: [{ url: "/#/swarm/shop/agent/manager" }] },
    houses: [{ id: "/src/shop-wt-1", role: "worker-1", status: "blocked", issue: 4, issue_title: "Checkout", links: [{ url: "/#/swarm/shop/agent/worker-1" }] }],
    board: [], pipeline: { gates: [{ issue: 9, state: "red-base", landed: true }] } }
  assert.deepStrictEqual(M.attention(BASE, blocked), [
    { what: "Manager", title: "Allow npm publish?", url: BASE + "/#/swarm/shop/agent/manager" },
    { what: "wt-1", title: "#4 Checkout", url: BASE + "/#/swarm/shop/agent/worker-1" }
  ], "a red base that landed anyway needs nobody")
  assert.strictEqual(M.needsYou(blocked), 2)
})

test("features carry their build progress", () => {
  assert.deepStrictEqual(M.features(M.villageFor(villages, "agents-ui")), [
    { number: 367, title: "Village clarity pass: map too small, unlabeled repo, label collisions, unexplained harbour/quay/symbols", build: "Build 1/4", live: false, url: "https://github.com/laird/agents-ui/issues/367" }
  ])
})

test("the bar badge: working, needs you, sailing, unreachable", () => {
  assert.deepStrictEqual(M.badge(villages, "", true), { reachable: true, working: 3, alert: true, sailing: false, build: "unknown", buildWords: "" })
  const [, v2] = twoSwarms()
  assert.deepStrictEqual(M.badge(v2, "shop", true), { reachable: true, working: 0, alert: true, sailing: false, build: "unknown", buildWords: "" }, "one village when one is open")
  assert.strictEqual(M.badge(v2, "", true).working, 3, "all of them otherwise")
  assert.deepStrictEqual(M.badge(null, "", false), { reachable: false, working: 0, alert: false, sailing: false, build: "unknown", buildWords: "" })
  const sailing = { villages: [{ project: "x", houses: [], board: [], pipeline: { sailing: true } }] }
  assert.strictEqual(M.badge(sailing, "", true).sailing, true)
})

test("a ship sails when the last stop's release changes, never on first sight", () => {
  const at = (sha, source = "deployment") => ({ villages: [{ project: "x", houses: [], board: [], pipeline: { stops: [{ name: "nuc-1" }, { name: "release", source, sha }] } }] })
  assert.strictEqual(M.releaseKey(at("abc").villages[0]), "deployment:abc")
  assert.strictEqual(M.releaseKey(M.villageFor(villages, "agents-ui")), "", "no release yet")
  let r = M.releases(at("abc"), null)
  assert.deepStrictEqual(r.sailed, [], "first sight")
  r = M.releases(at("abc"), r.seen)
  assert.deepStrictEqual(r.sailed, [], "unchanged")
  r = M.releases(at("def"), r.seen)
  assert.deepStrictEqual(r.sailed, ["x"], "a new release")
  r = M.releases(at("def", "branch"), r.seen)
  assert.deepStrictEqual(r.sailed, ["x"], "the source is part of the key, as on the page")
  assert.strictEqual(M.badge(at("def"), "", true, ["x"]).sailing, true)
  assert.strictEqual(M.badge(at("def"), "", true, ["y"]).sailing, false)
})

test("repo link and timestamps", () => {
  assert.strictEqual(M.repoUrl(M.villageFor(villages, "agents-ui")), "https://github.com/laird/agents-ui")
  assert.strictEqual(M.repoUrl({ houses: [], board: [], backlog: [] }), "")
  assert.strictEqual(M.ago(0, 1000), "never")
  assert.strictEqual(M.ago(1000, 3000), "just now")
  assert.strictEqual(M.ago(1000, 46000), "45s ago")
  assert.strictEqual(M.ago(1000, 1000 + 3 * 60000), "3m ago")
})

test("a swarm of only its manager (zero workers) is still listed", () => {
  const s = { swarms: [{ project_name: "solo", workflow: "autocoder", stopped: false, busy_count: 0, attention_count: 0, workers: [] }] }
  const v = { villages: [{ project: "solo", framework: "autocoder", manager: { role: "manager", status: "idle" }, houses: [], board: [], backlog: [], pipeline: { quay: [], stops: [] } }] }
  assert.deepStrictEqual(M.swarmRows(s, v), [{ project: "solo", workflow: "autocoder", houses: 0, working: 0, waiting: 0, stopped: false }])
  assert.deepStrictEqual(M.swarmRows(s, { villages: [] }).map(r => r.houses), [0], "before its village arrives")
  assert.deepStrictEqual(M.houseRows(BASE, v.villages[0], s.swarms[0]), [])
  assert.strictEqual(M.chooseProject(M.swarmRows(s, v), "", ""), "solo")
  assert.strictEqual(M.badge(v, "", true).working, 0)
})

// The build (#413's `build`, #415): the icon turns red for a red build, and
// the tooltip and panel say what is failing and since when.
const buildRunning = fixture("village-build-running.json")
const withBuild = (build) => ({ villages: [Object.assign({}, buildRunning.villages[0], { build: build })] })
const RED = { state: "red", since: "2026-11-09T20:40:00Z", tests: ["a", "b", "c"], url: "https://ci/run/1",
  jobs: [{ name: "test (macos-latest)", os: "macOS", state: "red" }, { name: "test (ubuntu-latest)", os: "Linux", state: "red" }] }

test("a running build, as recorded while CI ran on master", () => {
  const v = M.villageFor(buildRunning, "agents-ui")
  assert.strictEqual(M.buildState(v), "running")
  assert.strictEqual(M.buildWords(v), "build running")
  const b = M.badge(buildRunning, "", true)
  assert.strictEqual(b.build, "running")
  assert.strictEqual(b.buildWords, "build running")
})

test("a red build: how many tests fail, on which OS, since when", () => {
  const at = M.clock(RED.since)
  assert.match(at, /^\d\d:\d\d$/)
  const v = withBuild(RED).villages[0]
  assert.strictEqual(M.buildState(v), "red")
  assert.strictEqual(M.buildWords(v), "build red · 3 tests failing since " + at)
  assert.strictEqual(M.buildWords({ build: Object.assign({}, RED, { partial: true, tests: ["a"], jobs: [RED.jobs[0]] }) }),
    "build red on macOS · 1 test failing since " + at)
  assert.strictEqual(M.buildWords({ build: { state: "red" } }), "build red")
  assert.strictEqual(M.buildWords({ build: { state: "red", since: RED.since } }), "build red since " + at)
  const b = M.badge(withBuild(RED), "", true)
  assert.strictEqual(b.build, "red")
  assert.strictEqual(b.buildWords, "build red · 3 tests failing since " + at)
})

test("green, and villages that don't say", () => {
  assert.strictEqual(M.buildWords(withBuild({ state: "green", sha: "abc" }).villages[0]), "build green")
  assert.strictEqual(M.badge(withBuild({ state: "green" }), "", true).build, "green")
  // A village recorded before #413 has no build at all.
  assert.strictEqual(M.buildState(M.villageFor(villages, "agents-ui")), "unknown")
  assert.strictEqual(M.buildWords(M.villageFor(villages, "agents-ui")), "")
  assert.strictEqual(M.badge(villages, "", true).build, "unknown")
  assert.strictEqual(M.buildState({ build: { state: "weird" } }), "unknown")
})

test("the worst build of all the villages decides the icon", () => {
  const two = { villages: [Object.assign({}, buildRunning.villages[0], { project: "a", build: { state: "green" } }),
                           Object.assign({}, buildRunning.villages[0], { project: "b", build: RED })] }
  assert.strictEqual(M.badge(two, "", true).build, "red")
  assert.strictEqual(M.badge(two, "a", true).build, "green", "one picked: only that one")
})
