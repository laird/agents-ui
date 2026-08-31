import QtQuick
import Quickshell
import Quickshell.Io
import qs.Ui

// A bar readout for agents-tui swarms.
//
// The dashboard already polls tmux every 3s and holds the result in memory,
// so this widget asks it over loopback rather than shelling out to tmux a
// second time. That keeps one poller in the system: if the counts here are
// wrong, the dashboard is wrong, and there is only one place to look.
//
// The widget collapses out of the bar when nothing is running -- a machine
// with no swarm draws nothing rather than a dimmed zero.
BarWidget {
  id: root
  moduleName: "agents.swarm"

  // `settings` and `setting()` come from the qs.Ui BarWidget base, which the
  // bar host populates from this widget's shell.json entry. Redeclaring either
  // is a duplicate-property error, not an override.

  readonly property int port: Number(setting("port", 7878))
  readonly property int refreshIntervalSec: Math.max(1, Number(setting("refreshIntervalSec", 5)))
  readonly property bool notifyOnAttention: setting("notifyOnAttention", true) !== false
  readonly property string openCommand: String(setting("openCommand", "")).trim()

  property int busyCount: 0
  property int idleCount: 0
  property int attentionCount: 0
  property int swarmCount: 0
  property bool reachable: false

  // Nothing to say until a swarm exists. `visible` alone would still reserve
  // width in the bar's layout, so the implicit size has to collapse too.
  readonly property bool hasSwarm: reachable && swarmCount > 0
  visible: hasSwarm
  implicitWidth: hasSwarm ? button.implicitWidth : 0
  implicitHeight: hasSwarm ? button.implicitHeight : 0

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
    lastAttention = 0
  }

  // Notify on the rising edge only. Polling every few seconds against a
  // worker that sits waiting for ten minutes would otherwise fire a
  // notification every tick.
  property int lastAttention: 0

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

    for (var i = 0; i < swarms.length; i++) {
      var swarm = swarms[i] || {}
      if (swarm.stopped === true) continue
      live++
      busy += Number(swarm.busy_count || 0)
      idle += Number(swarm.idle_count || 0)
      attention += Number(swarm.attention_count || 0)
    }

    reachable = true
    swarmCount = live
    busyCount = busy
    idleCount = idle

    // omarchy-notification-send rather than notify-send: `--exec` makes the
    // notification itself open the dashboard, and `-r` replaces the previous
    // one instead of stacking a column of them down the screen.
    if (notifyOnAttention && attention > lastAttention) {
      notify.command = ["omarchy-notification-send",
                        "--app-name", "Agent Swarm",
                        "-u", "critical",
                        "-r", "9377",
                        attention === 1 ? "1 agent needs input"
                                        : attention + " agents need input",
                        "Click to open the swarm dashboard.",
                        "--exec", "omarchy-agents-dashboard"]
      notify.running = true
    }
    lastAttention = attention
    attentionCount = attention
  }

  Process { id: notify; running: false }
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
    // state that actually wants the user's eyes.
    text: root.attentionCount > 0
            ? "⚠ " + root.attentionCount
            : "⚙ " + root.busyCount + "/" + (root.busyCount + root.idleCount)

    onPressed: function(mouseButton) {
      if (mouseButton === Qt.RightButton) root.refresh()
      else root.openDashboard()
    }
  }

  function refresh() {
    if (!poll.running) poll.running = true
  }
}
