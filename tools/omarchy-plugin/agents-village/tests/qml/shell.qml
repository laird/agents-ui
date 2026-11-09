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

      console.log("server down")
      one.apply("", "")
      check("unreachable", one.reachable, false)
      check("the badge says so", one.badge.reachable, false)

      console.log(failures === 0 ? "ALL OK" : failures + " FAILED")
    }
  }
}
