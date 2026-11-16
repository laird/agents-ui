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
  assert.deepStrictEqual(rows, [{ project: "agents-ui", workflow: "autocoder", houses: 3, working: 3, waiting: 2, limited: 0, stopped: false }])
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
    status: "working", context: null, limit: null, url: BASE + "/#/swarm/agents-ui/agent/worker-2"
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
    { key: "gate:370:conflict", kind: "gate", what: "Merge conflict", title: "#370", url: "https://github.com/laird/agents-ui/issues/370" },
    { key: "board:365:needs-approval", kind: "needs", what: "needs-approval", title: "#365 Record release Deployments from a release-published workflow (needs the workflow token scope)", url: "https://github.com/laird/agents-ui/issues/365" }
  ])
  const blocked = { project: "shop", manager: { status: "blocked", title: "Allow npm publish?", question: { prompt: "Allow  npm\n publish?" }, links: [{ url: "/#/swarm/shop/agent/manager" }] },
    houses: [{ id: "/src/shop-wt-1", role: "worker-1", status: "blocked", issue: 4, issue_title: "Checkout", links: [{ url: "/#/swarm/shop/agent/worker-1" }] }],
    board: [], pipeline: { gates: [{ issue: 9, state: "red-base", landed: true }] } }
  assert.deepStrictEqual(M.attention(BASE, blocked), [
    { key: "manager", kind: "needs", house: "manager", what: "Manager", title: "Allow npm publish?", question: "Allow npm publish?", url: BASE + "/#/swarm/shop/agent/manager" },
    { key: "house:/src/shop-wt-1:4", kind: "needs", house: "wt-1", what: "wt-1", title: "#4 Checkout", question: "", url: BASE + "/#/swarm/shop/agent/worker-1" }
  ], "a red base that landed anyway needs nobody")
  assert.strictEqual(M.needsYou(blocked), 2, "the red dot counts the same list")
  assert.strictEqual(M.needsYou(v), M.attention(BASE, v).length)
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

test("houses held by a usage limit are counted apart from those that need you (#484)", () => {
  const v = { project: "shop", houses: [
    { id: "/src/shop-wt-1", role: "worker-1", status: "idle", limit: { resets: "3pm (Europe/London)" }, links: [] },
    { id: "/src/shop-wt-2", role: "worker-2", status: "idle", limit: {}, links: [] },
    { id: "/src/shop-wt-3", role: "worker-3", status: "working", limit: { resets: "3pm" }, links: [] },
    { id: "/src/shop-wt-4", role: "worker-4", status: "blocked", links: [] }
  ], board: [], backlog: [], pipeline: { quay: [], stops: [] } }
  assert.strictEqual(M.limited(v), 2, "working again is not limited")
  assert.strictEqual(M.needsYou(v), 1, "only the blocked one needs you")
  assert.deepStrictEqual(M.swarmRows({ swarms: [] }, { villages: [v] }).map(r => [r.waiting, r.limited]), [[1, 2]])
  assert.deepStrictEqual(M.houseRows(BASE, v, null).map(r => r.limit), ["limited · til 3pm (Europe/London)", "rate limited", null, null])
})

test("a swarm of only its manager (zero workers) is still listed", () => {
  const s = { swarms: [{ project_name: "solo", workflow: "autocoder", stopped: false, busy_count: 0, attention_count: 0, workers: [] }] }
  const v = { villages: [{ project: "solo", framework: "autocoder", manager: { role: "manager", status: "idle" }, houses: [], board: [], backlog: [], pipeline: { quay: [], stops: [] } }] }
  assert.deepStrictEqual(M.swarmRows(s, v), [{ project: "solo", workflow: "autocoder", houses: 0, working: 0, waiting: 0, limited: 0, stopped: false }])
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

// ---- #493: Join swarm, notifying from the bar, keyboard navigation ----------

test("join swarm: a plain machine name only, since omarchy-launch-or-focus evals the command", () => {
  for (const ok of ["nuc", "nuc-1", "box.local", "A9_b-c.d"]) assert.ok(M.validMachine(ok), ok)
  for (const bad of ["nuc; rm -rf ~", "$(id)", "-rf", "", " nuc", "nuc box", "`id`", "a/b", "x".repeat(65), null, undefined])
    assert.ok(!M.validMachine(bad), String(bad))
  assert.deepStrictEqual(M.joinCommand("nuc"), ["omarchy-launch-or-focus-tui", "--app-id=org.omarchy.swarm-nuc-join", "swarm", "nuc"])
  assert.strictEqual(M.joinCommand("nuc; rm -rf ~"), null)
  assert.strictEqual(M.joinCommand("$(id)"), null)
  assert.strictEqual(M.joinCommand("-rf"), null)
})

test("join swarm: one machine's terminal never matches another's (omarchy-launch-or-focus matches \\b<class>\\b)", () => {
  const finds = (m, cls) => new RegExp("\\b" + M.joinAppId(m) + "\\b", "i").test(cls)
  assert.ok(finds("nuc", M.joinAppId("nuc")))
  assert.ok(!finds("nuc", M.joinAppId("nuc-1")), "nuc is not nuc-1")
  assert.ok(!finds("nuc", M.joinAppId("nuc.lan")))
  assert.ok(!finds("a.b", M.joinAppId("axb")), "a dot is not a wildcard")
  assert.ok(finds("box.lan", M.joinAppId("box.lan")))
})

test("join swarm: hidden without a machine or without swarm here, disabled for a bad name", () => {
  assert.strictEqual(M.joinState("", true), "hidden")
  assert.strictEqual(M.joinState("nuc", false), "hidden")
  assert.strictEqual(M.joinState("nuc", true), "ready")
  assert.strictEqual(M.joinState("nuc; rm -rf ~", true), "disabled")
})

const waitingDoc = blocked => ({ villages: [{ project: "shop", manager: { status: "idle", links: [] }, board: [], backlog: [], pipeline: { gates: [] },
  houses: [{ id: "/src/shop-wt-1", role: "worker-1", status: blocked ? "blocked" : "working", issue: 4, issue_title: "Checkout", links: [] }] }] })

test("notify: on the rising edge only, never on the first poll, never twice", () => {
  let r = M.freshNeeds(null, M.needsNow(BASE, waitingDoc(true)))
  assert.deepStrictEqual(r.fresh, [], "the first poll records what already needs you")
  r = M.freshNeeds(M.freshNeeds(null, M.needsNow(BASE, waitingDoc(false))).keys, M.needsNow(BASE, waitingDoc(true)))
  assert.deepStrictEqual(r.fresh.map(i => i.id), ["shop|house:/src/shop-wt-1:4"])
  assert.deepStrictEqual(M.freshNeeds(r.keys, M.needsNow(BASE, waitingDoc(true))).fresh, [], "still waiting is not news")
})

test("notify: the same keys as the desktop notifier, per village", () => {
  const items = M.needsNow(BASE, villages)
  const v = M.villageFor(villages, "agents-ui")
  assert.deepStrictEqual(items.map(i => i.id), M.attention(BASE, v).map(a => "agents-ui|" + a.key))
})

test("notify: one replaceable notification saying how many, what is new and where to go", () => {
  const all = M.needsNow(BASE, waitingDoc(true))
  const n = M.notice(BASE, all, all)
  assert.deepStrictEqual(n, { title: "1 needs you", body: "wt-1: #4 Checkout", url: BASE + "/village#shop&house=wt-1" })
  const cmd = M.noticeCommand(n, 0)
  assert.deepStrictEqual(cmd.slice(0, 9), ["omarchy-notification-send", "--app-name", "Agents village", "-u", "critical", "-p", "1 needs you", "wt-1: #4 Checkout", "--exec"],
    "the first: no id to replace; -p prints the one it gets; --exec comes last, the rest is the click's argv")
  assert.deepStrictEqual(M.noticeCommand(n, 812).slice(5, 8), ["-p", "-r", "812"], "the next replaces the widget's own last notice")
  assert.strictEqual(M.noticeCommand({ title: "t", body: "#4 Vec<u8> & co", url: "u" }, 0)[7], "#4 Vec&lt;u8&gt; &amp; co", "the body is markup-escaped")
  assert.strictEqual(cmd[cmd.length - 1], n.url)
  assert.strictEqual(M.notice(BASE, all, []), null)
  const many = Array.from({ length: 6 }, (_, i) => ({ project: "p", what: "w" + i, title: "t", key: String(i) }))
  assert.deepStrictEqual(M.notice(BASE, many, many).body.split("\n").slice(-1), ["and 2 more"])
  assert.strictEqual(M.notice(BASE, many, many).title, "6 need you")
})

test("notify: the widget stays quiet only while the desktop notifier covers it", () => {
  const nuc = "http://nuc-1:7890"
  assert.strictEqual(M.widgetNotifies(true, null, nuc), true, "no notifier")
  assert.strictEqual(M.widgetNotifies(false, null, nuc), false, "notify off")
  assert.strictEqual(M.widgetNotifies(true, '{"server":"http://nuc-1:7890/"}', nuc), false, "the notifier watches this server")
  assert.strictEqual(M.widgetNotifies(true, '{}', nuc), false, "the notifier's default server is nuc-1")
  assert.strictEqual(M.widgetNotifies(true, '{"server":"http://other:7890"}', nuc), true, "it watches another server")
  assert.strictEqual(M.widgetNotifies(true, '{"kinds":["build"]}', nuc), true, "it does not notify what needs you")
  assert.strictEqual(M.widgetNotifies(true, '{"enabled":false}', nuc), true, "a notifier turned off hands it back")
  assert.strictEqual(M.widgetNotifies(true, "{not json", nuc), false, "a notifier whose settings don't parse still runs")
})

test("keyboard: the rows in panel order, and a cursor held at the ends", () => {
  const rows = [{ project: "a" }, { project: "b" }]
  assert.deepStrictEqual(M.navRows(rows, [], [], ""), [{ section: "swarms", id: "swarm:a", pick: "a" }, { section: "swarms", id: "swarm:b", pick: "b" }])
  const nav = M.navRows(rows, [{ key: "k", url: "n" }], [{ name: "wt-1", url: "h1" }, { name: "wt-2", url: "h2" }], "a")
  assert.deepStrictEqual(nav.map(r => r.section + ":" + r.url), ["needs:n", "houses:h1", "houses:h2"])
  assert.strictEqual(M.cursorFor(nav, "house:wt-2"), 2)
  assert.strictEqual(M.cursorFor(nav.slice(1), "house:wt-2"), 1, "the same row when one above it goes")
  assert.strictEqual(M.cursorFor(nav, "house:wt-9"), -1)
  assert.strictEqual(M.cursorFor(nav, ""), -1)
  assert.strictEqual(M.moveCursor(-1, 1, 3), 0)
  assert.strictEqual(M.moveCursor(-1, -1, 3), 2)
  assert.strictEqual(M.moveCursor(0, -1, 3), 0)
  assert.strictEqual(M.moveCursor(2, 1, 3), 2)
  assert.strictEqual(M.moveCursor(1, 1, 3), 2)
  assert.strictEqual(M.moveCursor(5, 1, 0), -1)
})
