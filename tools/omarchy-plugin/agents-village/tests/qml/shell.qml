import Quickshell
import QtQuick
import "Fixtures.js" as Fixtures

// Drives the real Panel.qml (as BarPanel) with recorded server answers: which
// view it opens on, picking a swarm and going back, the badge, and how it
// shows an unreachable server. qml_check.py builds the harness and reads this.
ShellRoot {
  Item {
    id: harness
    property int failures: 0
    function check(name, got, want) {
      if (JSON.stringify(got) === JSON.stringify(want)) console.log("  ok   " + name)
      else { failures++; console.log("  FAIL " + name + ": got " + JSON.stringify(got) + ", wanted " + JSON.stringify(want)) }
    }

    BarPanel {
      id: one
      // Nothing listens on this port: the widget's own fetches fail quietly and
      // only the recorded answers count.
      settings: ({ serverUrl: "http://127.0.0.1:9" })
    }
    BarPanel {
      id: two
      settings: ({ serverUrl: "http://127.0.0.1:9" })
    }
    BarPanel {
      id: joiner
      settings: ({ serverUrl: "http://127.0.0.1:9", machine: "nuc" })
    }
    BarPanel {
      id: evil
      settings: ({ serverUrl: "http://127.0.0.1:9", machine: "nuc; rm -rf ~" })
    }
    BarPanel {
      id: quiet
      settings: ({ serverUrl: "http://127.0.0.1:9", notify: false })
    }
    BarPanel {
      id: pinned
      settings: ({ serverUrl: "http://127.0.0.1:9", village: "shop" })
    }

    Component.onCompleted: {
      console.log("one swarm")
      one.apply(Fixtures.swarms, Fixtures.villages)
      check("goes straight to its village", one.project, "agents-ui")
      check("no way back to a list of one", one.canGoBack, false)
      check("houses", one.houses.map(function(h) { return h.name }), ["wt-2", "wt-3", "wt-4"])
      check("badge counts the working", one.badge.working, 3)
      check("something needs you", one.badge.alert, true)
      check("flow starts at the backlog", one.flowItems[0].label, "Backlog")

      console.log("two swarms")
      two.apply(Fixtures.twoSwarms, Fixtures.twoVillages)
      check("starts at the list", two.project, "")
      check("lists both", two.rows.map(function(r) { return r.project }), ["agents-ui", "shop"])
      two.pick("shop")
      check("picking opens that village", two.project, "shop")
      check("with a way back", two.canGoBack, true)
      check("the badge follows the open village", two.badge.working, 0)
      two.back()
      check("back returns to the list", two.project, "")

      console.log("settings")
      pinned.apply(Fixtures.twoSwarms, Fixtures.twoVillages)
      check("a pinned village opens directly", pinned.project, "shop")
      check("and has no list to go back to", pinned.canGoBack, false)
      check("the server setting is read", pinned.base, "http://127.0.0.1:9")
      check("the interval defaults to 5s", pinned.intervalMs, 5000)

      console.log("a release")
      var shipped = function(sha) {
        var doc = JSON.parse(Fixtures.villages)
        doc.villages[0].pipeline.stops[1].sha = sha
        doc.villages[0].pipeline.stops[1].source = "deployment"
        return JSON.stringify(doc)
      }
      one.apply(Fixtures.swarms, shipped("aaa1111"))
      check("no ship on first sight", one.badge.sailing, false)
      one.apply(Fixtures.swarms, shipped("bbb2222"))
      check("the ship sails on a new release", one.badge.sailing, true)

      console.log("the build")
      one.apply(Fixtures.swarms, Fixtures.villages)
      check("no build: the icon is not red", one.buildRed, false)
      one.apply(Fixtures.swarms, Fixtures.buildRunning)
      check("a running build", one.badge.build, "running")
      check("running is not red", one.buildRed, false)
      check("the panel says it runs", one.buildWords, "build running")
      one.apply(Fixtures.swarms, Fixtures.buildRed)
      check("a red build turns the icon red", one.buildRed, true)
      check("the panel says what fails", one.buildWords.indexOf("build red · 3 tests failing since ") === 0, true)

      console.log("keyboard")
      two.apply(Fixtures.twoSwarms, Fixtures.twoVillages)
      check("no row highlighted at first", two.cursor, -1)
      two.moveCursor(1)
      check("j/down highlights the first swarm", two.cursor, 0)
      two.moveCursor(1); two.moveCursor(1)
      check("held at the last", two.cursor, 1)
      two.activateCursor()
      check("enter opens the highlighted swarm", two.project, "shop")
      check("a new view starts with no highlight", two.cursor, -1)
      two.moveCursor(1)
      check("in a village, Needs you comes first", two.navItems[two.cursor].section, "needs")
      two.moveCursor(1)
      check("then the houses", two.navItems[two.cursor].id, "house:wt-1")
      // A note lands on the board above the highlighted house: it stays on the house.
      var more = JSON.parse(Fixtures.twoVillages)
      more.villages[1].board = [{ number: 5, label: "needs-design", title: "x", url: "https://x/5" }]
      two.apply(Fixtures.twoSwarms, JSON.stringify(more))
      check("the highlight stays on its row when rows above it change", two.navItems[two.cursor].id, "house:wt-1")
      two.back()

      console.log("join swarm")
      // The Process check for `swarm` runs on its own; settle it for the checks.
      joiner.swarmOnPath = true; evil.swarmOnPath = true
      check("ready with a machine and swarm here", joiner.joinState, "ready")
      check("disabled for a name that is not a host", evil.joinState, "disabled")
      check("hidden without a machine", one.joinState, "hidden")
      joiner.join()
      evil.join()

      console.log("notify")
      var waiting = function(blocked) {
        return JSON.stringify({ villages: [{ project: "shop", manager: { status: "idle", links: [] }, board: [], backlog: [],
          pipeline: { gates: [], stops: [], quay: [] },
          houses: [{ id: "/src/shop-wt-1", role: "worker-1", status: blocked ? "blocked" : "working", issue: 4, issue_title: "Checkout", links: [] }] }] })
      }
      check("notify off in its settings: the widget does not notify", quiet.notifies, false)
      check("no notifier installed, notify on: the widget notifies", joiner.notifies, true)
      joiner.apply(Fixtures.swarms, waiting(false))
      check("first poll: nothing", joiner.lastNotice, null)
      joiner.apply(Fixtures.swarms, waiting(true))
      check("a house starts waiting", joiner.lastNotice.title, "1 needs you")
      check("opening its dialog", joiner.lastNotice.url, "http://127.0.0.1:9/village#shop&house=wt-1")
      joiner.apply(Fixtures.swarms, waiting(true))
      joiner.apply("", "")
      joiner.apply(Fixtures.swarms, waiting(true))
      check("back after the server was down: still waiting is not news", joiner.lastNotice.title, "1 needs you")
      check("the keys outlast the blip", joiner.needKeys.length, 1)
      joiner.apply("", "")
      joiner.apply(Fixtures.swarms, waiting(false))
      joiner.apply("", "")
      joiner.apply(Fixtures.swarms, waiting(true))
      check("what starts waiting during a blip is news when it answers", joiner.needKeys.length, 1)
      quiet.apply(Fixtures.swarms, waiting(false))
      quiet.apply(Fixtures.swarms, waiting(true))
      check("notify off: worked out, not sent", quiet.lastNotice.title, "1 needs you")

      console.log("server down")
      one.apply("", "")
      check("unreachable", one.reachable, false)
      check("the badge says so", one.badge.reachable, false)

      // The notifications and the terminal are processes started from the
      // event loop: give them a moment before qml_check.py reads what ran.
      finish.start()
    }
    Timer {
      id: finish
      interval: 1500
      onTriggered: console.log(harness.failures === 0 ? "ALL OK" : harness.failures + " FAILED")
    }
  }
}
