// The notifier's decisions (Notify.js): what is news from one poll to the
// next, and what is sent. Run: node --test tools/omarchy-plugin/agents-village-notify/tests/
const test = require("node:test")
const assert = require("node:assert")
const path = require("node:path")
const fs = require("node:fs")
const N = require("../Notify.js")
const M = require("../Model.js")

const BASE = "http://127.0.0.1:7890"
const FIXTURE = path.join(__dirname, "../../agents-village/tests/fixtures/village.json")
const recorded = JSON.parse(fs.readFileSync(FIXTURE, "utf8"))

function village(over) {
  return Object.assign({ project: "shop", manager: { status: "idle", links: [] }, houses: [], board: [], backlog: [],
    pipeline: { gates: [], stops: [], quay: [] } }, over || {})
}
const doc = (...vs) => ({ villages: vs })
const blockedHouse = { id: "/src/shop-wt-2", role: "worker-2", status: "blocked", issue: 7, issue_title: "Checkout",
  question: { prompt: "Proceed with the migration?", kind: "yesno", options: [] }, links: [{ url: "/#/swarm/shop/agent/worker-2" }] }

test("Model.js here is the bar widget's own, byte for byte: one rule for what needs you", () => {
  // A copy, not a link: Omarchy refuses symlinks inside a plugin folder.
  // Change agents-village/Model.js, then: cp agents-village/Model.js agents-village-notify/
  assert.strictEqual(fs.readFileSync(path.join(__dirname, "../Model.js"), "utf8"),
    fs.readFileSync(path.join(__dirname, "../../agents-village/Model.js"), "utf8"))
})

test("the first poll records and says nothing, as the page never sails on load", () => {
  const r = N.step(null, recorded, BASE)
  assert.deepStrictEqual(r.events, [])
  assert.ok(r.snapshot["agents-ui"].needs.length > 0, "what already needs you is recorded, not news")
  assert.deepStrictEqual(N.step(r.snapshot, recorded, BASE).events, [], "the same state again is never news")
})

test("a house starts waiting on you: its question, and a link to its own issue (#570, #581)", () => {
  const before = N.step(null, doc(village()), BASE).snapshot
  const { events } = N.step(before, doc(village({ houses: [blockedHouse] })), BASE)
  assert.deepStrictEqual(events, [{ kind: "needs", project: "shop", key: "house:/src/shop-wt-2:7",
    url: BASE + "/village#shop&issue=7", title: "shop: wt-2 is waiting on you", body: "#7 Checkout\nProceed with the migration?" }])
})

test("the manager waiting, a board note and a bounce at the gate each link to the right deep link (#570, #581)", () => {
  const before = N.step(null, doc(village()), BASE).snapshot
  const after = village({ manager: { status: "blocked", title: "Allow npm publish?", links: [] },
    board: [{ number: 12, label: "needs-design", title: "Pick a queue", url: "https://github.com/o/shop/issues/12" }],
    pipeline: { gates: [{ issue: 9, state: "conflict", failure_url: "https://x/9" }], stops: [] } })
  const events = N.step(before, doc(after), BASE).events
  assert.deepStrictEqual(events.map(e => [e.kind, e.title, e.url]), [
    ["needs", "shop: Manager is waiting on you", BASE + "/village#shop&house=manager"],
    ["gate", "shop: merge conflict on #9", BASE + "/village#shop&gate=9"],
    ["needs", "shop: #12 is needs-design", BASE + "/village#shop&issue=12"]
  ])
  assert.strictEqual(events[2].body, "Pick a queue")
})

test("the red dot and the notifier agree: news is a new key in attention()", () => {
  const v = village({ houses: [blockedHouse] })
  assert.strictEqual(M.needsYou(v), 1)
  const snap = N.snapshotOf(doc(v)).shop
  assert.deepStrictEqual(snap.needs, M.attention("", v).map(a => a.key))
})

test("waiting, answered, waiting again is news again; still waiting is not", () => {
  let s = N.step(null, doc(village()), BASE).snapshot
  const waiting = doc(village({ houses: [blockedHouse] }))
  const answered = doc(village({ houses: [Object.assign({}, blockedHouse, { status: "working" })] }))
  let r = N.step(s, waiting, BASE); assert.strictEqual(r.events.length, 1); s = r.snapshot
  r = N.step(s, waiting, BASE); assert.strictEqual(r.events.length, 0, "no repeat"); s = r.snapshot
  r = N.step(s, answered, BASE); assert.strictEqual(r.events.length, 0); s = r.snapshot
  assert.strictEqual(N.step(s, waiting, BASE).events.length, 1)
})

test("the build breaks, and is fixed; the first colour seen is not news", () => {
  const b = (state, extra) => doc(village({ build: Object.assign({ state }, extra || {}) }))
  let s = N.step(null, b("green"), BASE).snapshot
  let r = N.step(s, b("red", { tests: ["a", "b"] }), BASE)
  assert.deepStrictEqual(r.events.map(e => [e.title, e.body]), [["shop: build broken", "build red · 2 tests failing"]])
  // #570, #581: why the build is red, not the bare map.
  assert.strictEqual(r.events[0].url, BASE + "/village#shop&build")
  r = N.step(r.snapshot, b("running"), BASE)
  assert.deepStrictEqual(r.events, [], "running after red is not fixed yet")
  r = N.step(r.snapshot, b("green"), BASE)
  assert.deepStrictEqual(r.events.map(e => e.title), ["shop: build fixed"])
  assert.deepStrictEqual(N.step(N.step(null, b("unknown"), BASE).snapshot, b("red"), BASE).events, [],
    "red after a build nobody reported is the first colour seen")
})

test("green, a poll that cannot say, then red: the break is news, so its fix has one", () => {
  const b = state => doc(village({ build: { state } }))
  let r = N.step(N.step(null, b("green"), BASE).snapshot, b("unknown"), BASE)
  r = N.step(r.snapshot, b("red"), BASE)
  assert.deepStrictEqual(r.events.map(e => e.title), ["shop: build broken"])
})

const env = (sha, tag, voyages, source) => doc(village({ pipeline: { gates: [], quay: [], voyages: voyages || [],
  stops: [{ name: "deploy", source: source || "deployment", kind: "environment", sha, tag, issues: [] }] } }))

test("a deploy lands with its version, a promotion says where from, a failure says so", () => {
  let s = N.step(null, env("aaaaaaa1", "v1"), BASE).snapshot
  let r = N.step(s, env("bbbbbbb2", "v2", [{ from: "dev", to: "deploy", state: "docked", sha: "bbbbbbb2", tag: "v2" }]), BASE)
  assert.deepStrictEqual(r.events.map(e => [e.title, e.body]), [["shop: v2 deployed to deploy", ""]], "the voyage and its stop are one deploy")
  // #570, #581: that environment's deploy, not the bare map.
  assert.strictEqual(r.events[0].url, BASE + "/village#shop&env=deploy")
  r = N.step(r.snapshot, env("bbbbbbb2", "v2", [{ from: "deploy", to: "release", state: "failed", sha: "bbbbbbb2" }]), BASE)
  assert.deepStrictEqual(r.events.map(e => e.title), ["shop: deploy to release failed"])
  assert.strictEqual(r.events[0].url, BASE + "/village#shop&env=release")
  assert.match(r.events[0].body, /^bbbbbbb did not land/)
  r = N.step(r.snapshot, env("bbbbbbb2", "v2", [{ from: "deploy", to: "release", state: "docked", sha: "bbbbbbb2" }]), BASE)
  assert.deepStrictEqual(r.events.map(e => [e.title, e.body]), [["shop: bbbbbbb deployed to release", "Promoted from deploy."]])
  r = N.step(r.snapshot, env("bbbbbbb2", "v2", [{ from: "deploy", to: "release", state: "docked", sha: "bbbbbbb2" }]), BASE)
  assert.deepStrictEqual(r.events, [], "a docked voyage still reported is not news twice")
})

test("a GitHub outage and its end are not news: stops fall back to the branch, the board empties, then all come back", () => {
  const note = { number: 12, label: "needs-design", title: "Pick a queue", url: "https://github.com/o/shop/issues/12" }
  const up = () => doc(village({ board: [note], pipeline: { gates: [], quay: [],
    voyages: [{ from: "deploy", to: "release", state: "failed", sha: "c3" }],
    stops: [{ name: "deploy", source: "deployment", kind: "environment", sha: "b2", tag: "v2", issues: [] }] } }))
  const down = env("deadbee", "", [], "branch")
  let t = 1e12
  let r = N.step(null, up(), BASE, { now: t })
  r = N.step(r.snapshot, down, BASE, { now: t += 15000 }); assert.deepStrictEqual(r.events, [])
  r = N.step(r.snapshot, down, BASE, { now: t += 5 * 60000 }); assert.deepStrictEqual(r.events, [])
  r = N.step(r.snapshot, up(), BASE, { now: t += 15000 })
  assert.deepStrictEqual(r.events, [], "the same deploy, note and failure as before the outage")
  // A note gone for longer than the grace is news when it comes back.
  r = N.step(r.snapshot, down, BASE, { now: t += 1000 })
  r = N.step(r.snapshot, down, BASE, { now: t += N.GRACE_MS + 1000 })
  r = N.step(r.snapshot, up(), BASE, { now: t += 1000 })
  assert.deepStrictEqual(r.events.map(e => e.kind), ["needs", "deploy"], "the note and the failure again; deploy still runs b2")
})

test("a stop that starts naming a deployment for an environment never seen is not a deploy", () => {
  const s = N.step(null, env("a1", "", [], "branch"), BASE).snapshot
  assert.deepStrictEqual(N.step(s, env("a1", "v1"), BASE).events, [])
})

test("a village that drops out of a poll keeps its snapshot", () => {
  const waiting = doc(village({ houses: [blockedHouse] }))
  let s = N.step(null, waiting, BASE).snapshot
  s = N.step(s, doc(), BASE).snapshot
  assert.deepStrictEqual(N.step(s, waiting, BASE).events, [])
})

test("settings: defaults, kinds, quiet hours from the screensaver's dim", () => {
  assert.deepStrictEqual(N.settingsOf("", ""), { enabled: true, server: "http://127.0.0.1:7890", interval: 15, kinds: ["needs", "gate", "build", "deploy"], quiet: "" })
  const s = N.settingsOf('{"server":"http://box:1/","interval":2,"kinds":["build","bogus"]}', '{"dim":"23-7"}')
  assert.deepStrictEqual(s, { enabled: true, server: "http://box:1", interval: 15, kinds: ["build"], quiet: "23-7" })
  assert.strictEqual(N.settingsOf('{"quiet":""}', '{"dim":"23-7"}').quiet, "", "notify.json can turn quiet hours off")
  assert.strictEqual(N.settingsOf("{not json", "").server, "http://127.0.0.1:7890")
  assert.strictEqual(N.settingsOf('{"enabled":false}', "").enabled, false)
})

test("quiet hours, wrapping past midnight", () => {
  const at = h => new Date(2026, 10, 15, h, 30)
  assert.ok(N.isQuiet("23-7", at(23)) && N.isQuiet("23-7", at(3)) && !N.isQuiet("23-7", at(7)) && !N.isQuiet("23-7", at(12)))
  assert.ok(N.isQuiet("1-5", at(2)) && !N.isQuiet("1-5", at(6)))
  assert.ok(!N.isQuiet("", at(3)) && !N.isQuiet("7-7", at(7)) && !N.isQuiet("25-3", at(1)))
})

test("what is sent: one at a time, a burst grouped, nothing in quiet hours or of an unwanted kind", () => {
  const e = (kind, project, title) => ({ kind, project, key: title, title: project + ": " + title, body: "b", url: "u-" + title })
  const noon = new Date(2026, 10, 15, 12)
  const settings = N.settingsOf("", "")
  assert.deepStrictEqual(N.notifications([e("build", "shop", "build broken")], settings, noon),
    [{ title: "shop: build broken", body: "b", url: "u-build broken" }])
  assert.deepStrictEqual(N.notifications([e("needs", "shop", "a"), e("gate", "shop", "b")], settings, noon),
    [{ title: "shop: 2 things in the village", body: "a\nb", url: "u-a" }])
  const many = ["a", "b", "c", "d", "e", "f", "g"].map((t, i) => e("needs", i % 2 ? "shop" : "mill", t))
  const [g] = N.notifications(many, settings, noon)
  assert.strictEqual(g.title, "7 things in the village")
  assert.deepStrictEqual(g.body.split("\n"), ["mill: a", "shop: b", "mill: c", "shop: d", "mill: e", "and 2 more"])
  assert.deepStrictEqual(N.notifications(many, N.settingsOf('{"quiet":"0-23"}', ""), noon), [], "quiet hours")
  assert.deepStrictEqual(N.notifications(many, N.settingsOf('{"kinds":["deploy"]}', ""), noon), [])
  assert.deepStrictEqual(N.notifications(many, N.settingsOf('{"enabled":false}', ""), noon), [])
})

// The deep link builder itself (one item vs many, every kind) is tested
// once, in agents-village/tests/model.test.js: Model.js here is its copy
// (byte-equality is the test above), so nothing here would test anything new.

test("two houses with one short name: the link names the house by its full id", () => {
  // No `issue` on either (#581 links a worker with one straight to it instead): what
  // disambiguates here is the full id, not whether the link is a house or an issue link.
  const noIssue = Object.assign({}, blockedHouse, { issue: undefined, issue_title: undefined })
  const twins = village({ houses: [Object.assign({}, noIssue, { id: "/a/shop-wt-2" }), Object.assign({}, noIssue, { id: "/b/shop-wt-2" })] })
  const before = N.step(null, doc(village()), BASE).snapshot
  assert.deepStrictEqual(N.step(before, doc(twins), BASE).events.map(e => e.url),
    [BASE + "/village#shop&house=%2Fa%2Fshop-wt-2", BASE + "/village#shop&house=%2Fb%2Fshop-wt-2"])
})

test("the body is escaped for the daemon's markup", () => {
  assert.strictEqual(N.markup("#12 Parse Vec<u8> & friends > 3"), "#12 Parse Vec&lt;u8&gt; &amp; friends &gt; 3")
  assert.strictEqual(N.markup(undefined), "")
})

test("a branch or the green lock moving is not a deploy", () => {
  const stops = sha => doc(village({ pipeline: { gates: [], quay: [], stops: [
    { name: "green", kind: "gate", source: "deployment", sha, issues: [] }, { name: "deploy", source: "branch", sha, issues: [] }] } }))
  const s = N.step(null, stops("a1"), BASE).snapshot
  assert.deepStrictEqual(N.step(s, stops("b2"), BASE).events, [])
})
