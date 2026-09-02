import QtQuick
import Quickshell
import Quickshell.Io
import qs.Commons
import qs.Ui

// A bar readout for agents-tui swarms.
//
// The dashboard already polls tmux every 3s and holds the result in memory,
// so this widget asks it over loopback rather than shelling out to tmux a
// second time. That keeps one poller in the system: if the counts here are
// wrong, the dashboard is wrong, and there is only one place to look.
//
// The widget holds its slot whether or not a swarm is running: it is the
// only always-available way into the dashboard, so it stays clickable and
// goes dim rather than disappearing.
BarWidget {
  id: root
  moduleName: "agents.swarm"

  // `settings` and `setting()` come from the qs.Ui BarWidget base, which the
  // bar host populates from this widget's shell.json entry. Redeclaring either
  // is a duplicate-property error, not an override.

  readonly property int port: Number(setting("port", 7878))
  readonly property int refreshIntervalSec: Math.max(1, Number(setting("refreshIntervalSec", 5)))
  readonly property bool notifyOnAttention: setting("notifyOnAttention", true) !== false
  readonly property string openWith: String(setting("openWith", "Web dashboard"))
  readonly property string openCommand: String(setting("openCommand", "")).trim()

  property int busyCount: 0
  property int idleCount: 0
  property int attentionCount: 0
  property int stuckCount: 0

  // Waiting is not an error, so red is reserved for broken and the theme's
  // accent carries attention. Accent is a real theme colour in 21 of the 22
  // stock themes, which keeps the bar looking like the rest of the desktop.
  //
  // It is not usable unconditionally: kanagawa sets accent equal to
  // foreground, and an alert the same colour as ordinary text does not alert.
  // Where accent is too close to the bar's own text to read as a signal, fall
  // back to an amber that always does -- the point is standing out, and a
  // theme colour that cannot be seen fails the requirement it was chosen for.
  readonly property color fallbackAttention: "#d29922"
  readonly property color attentionColor: {
    var a = Color.accent
    var f = root.bar ? root.bar.barForeground : Color.foreground
    var distance = Math.abs(a.r - f.r) + Math.abs(a.g - f.g) + Math.abs(a.b - f.b)
    return distance > 0.25 ? a : root.fallbackAttention
  }
  property int swarmCount: 0
  property bool reachable: false

  // The bar is the only always-available way into the dashboard, so the widget
  // holds its slot even with no swarm running: the icon alone, dimmed to say
  // there is nothing to count, but still a click target.
  readonly property bool hasSwarm: reachable && swarmCount > 0
  implicitWidth: button.implicitWidth
  implicitHeight: button.implicitHeight

  Process {
    id: poll
    running: false
    // --max-time bounds the poll below the refresh interval so a wedged
    // dashboard cannot stack up curl processes.
    command: ["curl", "-s", "--max-time", "2",
              "http://127.0.0.1:" + root.port + "/api/swarms"]

    stdout: StdioCollector {
      waitForEnd: true
      onStreamFinished: root.applySnapshot(text)
    }

    onExited: function(exitCode) {
      if (exitCode !== 0) root.markUnreachable()
    }
  }

  Timer {
    interval: root.refreshIntervalSec * 1000
    running: true
    repeat: true
    triggeredOnStart: true
    onTriggered: if (!poll.running) poll.running = true
  }

  function markUnreachable() {
    reachable = false
    swarmCount = 0
    busyCount = 0
    idleCount = 0
    attentionCount = 0
    stuckCount = 0
    lastAttention = 0
    blockedSwarms = []
  }

  // Notify on the rising edge only. Polling every few seconds against a
  // worker that sits waiting for ten minutes would otherwise fire a
  // notification every tick.
  property int lastAttention: 0

  // [{ name, count, roles, role? }] for swarms with at least one blocked
  // agent. `roles` names every blocked agent in that swarm, so the headline
  // can say who rather than only how many. `role` keeps its narrower
  // meaning — the sole blocked agent of a swarm that has exactly one —
  // because `blockedRoute` keys the deep link off precisely that.
  property var blockedSwarms: []

  // Roles of every agent in this swarm that is waiting for input, manager
  // first then workers in order. Manager and workers carry
  // `waiting_for_input` in the same snapshot this widget already polls, so
  // no extra request is needed.
  function blockedAgentRoles(swarm) {
    var roles = []
    var manager = swarm.manager
    if (manager && manager.waiting_for_input === true) {
      var managerRole = String(manager.role || "")
      if (managerRole !== "") roles.push(managerRole)
    }
    var workers = Array.isArray(swarm.workers) ? swarm.workers : []
    for (var i = 0; i < workers.length; i++) {
      if (workers[i] && workers[i].waiting_for_input === true) {
        var workerRole = String(workers[i].role || "")
        if (workerRole !== "") roles.push(workerRole)
      }
    }
    return roles
  }

  // Deepest screen that still shows everything that needs attention:
  //   1 blocked agent, 1 swarm  -> that agent's session view
  //   >1 blocked agent, 1 swarm -> that swarm's detail screen
  //   blocked agents in >1 swarm -> top-level repos list (no route)
  function blockedRoute(blocked) {
    if (!blocked || blocked.length !== 1) return ""
    var only = blocked[0]
    if (only.count === 1 && only.role) {
      return "/swarm/" + encodeURIComponent(only.name) + "/agent/" + encodeURIComponent(only.role)
    }
    return "/swarm/" + encodeURIComponent(only.name)
  }

  // Builds the click-through command for the alert, routed to the deepest
  // screen that still covers every blocked agent it describes. Returned as
  // separate argv words: omarchy-notification-send's --exec runs them
  // as-is and explicitly rejects a single pre-quoted string (it reads as an
  // attempt to smuggle a whole command through one argument).
  function dashboardExecArgs(blocked) {
    var route = blockedRoute(blocked)

    // An explicit openCommand wins; otherwise `openWith` picks the interface.
    // Wrapped in `bash -lc` because a user-set command can carry its own
    // quoting and arguments, which whitespace-splitting would mangle.
    if (openCommand !== "") return ["bash", "-lc", openCommand]
    if (openWith === "Terminal UI") {
      return ["bash", "-lc", "xdg-terminal-exec --app-id=org.omarchy.agents-tui -e agents-tui"]
    }
    return route === "" ? ["omarchy-agents-dashboard"] : ["omarchy-agents-dashboard", route]
  }

  // How a role reads in prose. The manager takes a definite article — there
  // is only ever one, and "manager needs input" reads like a headline with
  // a word missing. Worker roles are already names and stand on their own.
  function roleLabel(role) {
    return role === "manager" ? "the manager" : role
  }

  // "the manager", "the manager and worker-2",
  // "the manager, worker-1 and worker-2".
  function rolePhrase(roles) {
    var labels = []
    for (var i = 0; i < roles.length; i++) labels.push(roleLabel(roles[i]))
    if (labels.length === 1) return labels[0]
    return labels.slice(0, -1).join(", ") + " and " + labels[labels.length - 1]
  }

  // Say who needs input, not just how many. Which repo is blocked decides
  // where to go next, and the role decides what it means once you get
  // there: a blocked manager has stalled the whole swarm's coordination, a
  // blocked worker has parked one issue while the rest keep moving.
  //
  // Specificity drops one step at a time — named roles, then a count within
  // the named swarm, then a bare total — because a snapshot can carry an
  // `attention_count` without any agent flagged `waiting_for_input`, and
  // the generic wording has to stay reachable for that.
  function attentionHeadline(total, blocked) {
    var agents = total === 1 ? "1 agent" : total + " agents"

    if (!blocked || blocked.length === 0) return agents + " need input"
    if (blocked.length === 1) {
      var only = blocked[0]
      var roles = Array.isArray(only.roles) ? only.roles : []

      // Name them only when every blocked agent in the swarm resolved to a
      // role, and only while the list is short enough to stay one glance
      // long. Past that the count is the more readable summary.
      if (roles.length > 0 && roles.length === only.count && roles.length <= 3) {
        var phrase = rolePhrase(roles)
        if (phrase.indexOf("the ") === 0) phrase = "The" + phrase.slice(3)
        return phrase + " in " + only.name +
               (total === 1 ? " needs" : " need") + " input"
      }

      return agents + " in " + only.name + " need" +
             (total === 1 ? "s" : "") + " input"
    }

    // Several swarms blocked at once: lead with the total, then break it down
    // so the alert still says where without becoming a paragraph.
    var parts = []
    for (var i = 0; i < blocked.length; i++) {
      parts.push(blocked[i].name + " (" + blocked[i].count + ")")
    }
    return agents + " need input: " + parts.join(", ")
  }

  function applySnapshot(output) {
    var raw = String(output || "").trim()
    if (raw === "") { markUnreachable(); return }

    var parsed
    try {
      parsed = JSON.parse(raw)
    } catch (e) {
      console.warn("agents.swarm", "bad JSON from dashboard:", e)
      markUnreachable()
      return
    }

    var swarms = parsed && Array.isArray(parsed.swarms) ? parsed.swarms : []
    var busy = 0, idle = 0, attention = 0, live = 0
    var stuck = 0
    // Which projects are actually blocked. With more than one swarm running,
    // "1 agent needs input" does not say where to look.
    var blocked = []

    for (var i = 0; i < swarms.length; i++) {
      var swarm = swarms[i] || {}
      if (swarm.stopped === true) continue
      live++
      busy += Number(swarm.busy_count || 0)
      idle += Number(swarm.idle_count || 0)
      var swarmAttention = Number(swarm.attention_count || 0)
      attention += swarmAttention

      var agents = [swarm.manager].concat(swarm.workers || [])
      for (var a = 0; a < agents.length; a++) {
        var health = agents[a] ? String(agents[a].health || "Healthy") : "Healthy"
        if (health === "Dead" || health === "Stalled") stuck++
      }
      if (swarmAttention > 0) {
        var entry = { name: String(swarm.project_name || "unknown"),
                      count: swarmAttention,
                      // Every blocked role, so the headline can name them
                      // instead of falling back to a bare count.
                      roles: blockedAgentRoles(swarm) }
        // `role` stays narrower than `roles` on purpose: it is set only when
        // this swarm has exactly one blocked agent, which is the one case
        // dense enough to deep-link straight to that agent's session instead
        // of stopping at the swarm or repos list.
        if (swarmAttention === 1 && entry.roles.length > 0) {
          entry.role = entry.roles[0]
        }
        blocked.push(entry)
      }
    }

    reachable = true
    stuckCount = stuck
    swarmCount = live
    busyCount = busy
    idleCount = idle
    blockedSwarms = blocked

    if (notifyOnAttention && attention > lastAttention) {
      requestNotification(attentionHeadline(attention, blocked), blocked)
    }
    // The id is kept even when nothing is blocked: the next alert then
    // updates that same toast instead of adding a second one.

    lastAttention = attention
    attentionCount = attention
  }

  // Id of the toast currently on screen. Keeping exactly one swarm alert up
  // depends on this being an id the daemon actually handed out: ids are
  // assigned sequentially, so the made-up constant used previously matched
  // nothing and every send created another toast, which is how these stacked
  // a column deep.
  //
  // Replacing via `-r` is the mechanism that works here. Closing by id over
  // D-Bus does not -- this daemon accepts CloseNotification and returns
  // success without removing the popup -- so the alert is always superseded,
  // never cleared and re-sent.
  property int notifyId: 0

  function requestNotification(headline, blocked) {
    var cmd = ["omarchy-notification-send",
               "--app-name", "Agent Swarm",
               "-u", "critical",
               // -p prints the id the daemon assigned; it is the only handle
               // that can update this toast rather than adding another.
               "-p"]
    if (notifyId > 0) cmd = cmd.concat(["-r", String(notifyId)])
    notify.command = cmd.concat([headline,
                                 "Click to open the swarm dashboard.",
                                 "--exec"], dashboardExecArgs(blocked))
    notify.running = true
  }

  Process {
    id: notify
    running: false
    stdout: StdioCollector {
      waitForEnd: true
      onStreamFinished: {
        var id = parseInt(String(text).trim(), 10)
        if (!isNaN(id) && id > 0) root.notifyId = id
      }
    }
  }

  Process { id: opener; running: false }

  function openDashboard() {
    var cmd = root.openCommand !== "" ? root.openCommand : "omarchy-agents-dashboard"
    if (root.bar) root.bar.run(cmd)
    else { opener.command = ["bash", "-lc", cmd]; opener.running = true }
  }

  WidgetButton {
    id: button
    anchors.fill: parent
    bar: root.bar
    horizontalMargin: 7.5

    // Attention outranks work in progress: a blocked worker is the only
    // state that actually wants the user's eyes. Nerd Font glyphs rather than
    // ⚙/⚠ emoji, so the weight and baseline match the rest of the bar
    // instead of falling back to the symbol font.
    text: root.stuckCount > 0
            ? "󰀨 " + root.stuckCount
            : root.attentionCount > 0
              ? "󰀪 " + root.attentionCount
              : root.hasSwarm
              ? "󰛡 " + root.busyCount + "/" + (root.busyCount + root.idleCount)
              : "󰛡"

    dimmed: !root.hasSwarm
    active: root.stuckCount > 0 || root.attentionCount > 0
    // Red only for broken; amber for waiting.
    activeColor: root.stuckCount > 0
                   ? (root.bar ? root.bar.urgent : Color.urgent)
                   : root.attentionColor

    tooltipText: root.stuckCount > 0
                   ? root.stuckCount + (root.stuckCount === 1 ? " agent is stuck or dead" : " agents are stuck or dead")
                   : root.attentionCount > 0
                   ? root.attentionHeadline(root.attentionCount, root.blockedSwarms)
                   : root.hasSwarm
                     ? root.busyCount + " busy, " + root.idleCount + " idle"
                     : root.reachable ? "No swarm running" : "Swarm dashboard unreachable"

    onPressed: function(mouseButton) {
      if (mouseButton === Qt.RightButton) root.refresh()
      else root.openDashboard()
    }
  }

  function refresh() {
    if (!poll.running) poll.running = true
  }
}
