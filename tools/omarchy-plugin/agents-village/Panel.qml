import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Quickshell
import Quickshell.Io
import qs.Commons
import qs.Commons as Commons
import qs.Ui
import "Model.js" as Model

// The agents-tui village in the Omarchy bar: a house glyph with how many
// agents are working, a red dot when anything waits on you, and a panel
// that starts at the server's list of swarms (the dashboard's) and opens one
// swarm's village. With one swarm it goes straight to that village.
//
// Display only: it reads /api/swarms and /api/village and never sends
// anything to an agent, so it works against a --read-only server. It can
// open a terminal on the swarm (Join swarm: `swarm <machine>`), and notify
// when something starts waiting on you, unless the desktop notifier
// (agents-village-notify) is installed to do that.
Panel {
  id: root
  moduleName: "agents-village"
  ipcTarget: "agents-village"

  // ---- Settings (manifest schema) -----------------------------------------
  readonly property string base: Model.baseUrl(setting("serverUrl", "http://nuc-1:7890"))
  readonly property string pinned: String(setting("village", "") || "")
  readonly property int intervalMs: Math.max(2, Number(setting("refreshIntervalSec", 5))) * 1000
  readonly property bool showWorking: setting("showWorking", true) !== false
  readonly property bool showAlert: setting("showAlert", true) !== false
  readonly property bool showSailing: setting("showSailing", true) !== false
  readonly property string machine: String(setting("machine", "") || "").trim()
  readonly property bool notifySetting: setting("notify", true) !== false

  // ---- What the server said ----------------------------------------------
  property var swarmsDoc: null
  property var villagesDoc: null
  property bool reachable: false
  property double lastOk: 0
  property double nowMs: Date.now()
  // The swarm picked from the list; "" until one is.
  property string picked: ""
  // Releases seen so far (project → key), and when each project's ship got back.
  property var releasesSeen: null
  property var sailingUntil: ({})
  readonly property var sailing: Object.keys(sailingUntil).filter(function(p) { return sailingUntil[p] > nowMs })
  // How long the ship is drawn out after a release.
  readonly property int sailMs: 120000

  readonly property var rows: Model.swarmRows(swarmsDoc, villagesDoc)
  readonly property string project: Model.chooseProject(rows, pinned, picked)
  readonly property bool canGoBack: project !== "" && Model.canGoBack(rows, pinned)
  readonly property var village: project !== "" ? Model.villageFor(villagesDoc, project) : null
  readonly property var swarm: project !== "" ? Model.swarmFor(swarmsDoc, project) : null
  readonly property var houses: Model.houseRows(base, village, swarm)
  readonly property var flowItems: Model.flow(village)
  readonly property var attentionItems: Model.attention(base, village)
  readonly property var featureItems: Model.features(village)
  readonly property var badge: Model.badge(villagesDoc, project, reachable, sailing)
  readonly property string buildState: Model.buildState(village)
  // The icon turns red for a red build (#415).
  readonly property bool buildRed: reachable && badge.build === "red"
  readonly property string buildWords: Model.buildWords(village)
  readonly property string repo: Model.repoUrl(village)
  readonly property string host: base.replace(/^[a-z]+:\/\//i, "").replace(/:\d+$/, "")

  // ---- Join swarm, notifying, the keyboard (#493) ------------------------------
  property bool swarmOnPath: false
  readonly property string joinState: Model.joinState(machine, swarmOnPath)
  // notify.json of the desktop notifier, or null when it is not installed.
  property var notifierConfig: null
  readonly property bool notifies: Model.widgetNotifies(notifySetting, notifierConfig, base)
  // What needed you at the last answer (keys), null before the first one.
  property var needKeys: null
  property var lastNotice: null
  // The id the daemon gave the last notice, so the next one replaces it.
  property int noticeId: 0
  // The highlighted row (Model.navRows): its index, and its id, which keeps
  // it on the same row when the rows above it change.
  property int cursor: -1
  property string cursorId: ""
  readonly property var navItems: Model.navRows(rows, attentionItems, houses, project)
  onProjectChanged: { cursor = -1; cursorId = "" }
  onNavItemsChanged: cursor = Model.cursorFor(navItems, cursorId)
  onCursorChanged: if (cursor >= 0) Qt.callLater(revealCursor)

  readonly property color foreground: bar ? bar.foreground : Commons.Color.foreground
  readonly property color urgent: bar ? bar.urgent : Commons.Color.urgent
  readonly property color dim: Qt.darker(foreground, 1.55)
  readonly property string fontFamily: bar ? bar.fontFamily : Style.font.family

  implicitWidth: button.implicitWidth
  implicitHeight: button.implicitHeight

  // ---- Fetching ------------------------------------------------------------
  function get(path, done) {
    var xhr = new XMLHttpRequest()
    xhr.onreadystatechange = function() {
      if (xhr.readyState !== XMLHttpRequest.DONE) return
      done(xhr.status === 200 ? Model.parse(xhr.responseText) : null)
    }
    xhr.open("GET", root.base + path)
    xhr.timeout = 4000
    xhr.send()
  }

  function refresh() {
    get("/api/village", function(doc) {
      if (doc === null) { root.reachable = false; return }
      root.noteReleases(doc)
      root.noteNeeds(doc)
      root.villagesDoc = doc
      root.reachable = true
      root.lastOk = Date.now()
    })
    get("/api/swarms", function(doc) { if (doc !== null) root.swarmsDoc = doc })
    // Installing the desktop notifier creates this file; see it without a restart.
    notifierFile.reload()
  }

  // A release (the last stop's deployment changing) sets the ship sailing.
  function noteReleases(doc) {
    var r = Model.releases(doc, releasesSeen)
    releasesSeen = r.seen
    if (r.sailed.length === 0) return
    var until = Object.assign({}, sailingUntil)
    r.sailed.forEach(function(p) { until[p] = Date.now() + sailMs })
    sailingUntil = until
    nowMs = Date.now()
  }

  // Something started waiting on you: one notification, replacing the last.
  // The first poll, and the first after the server came back, only record.
  function noteNeeds(doc) {
    var all = Model.needsNow(base, doc)
    var r = Model.freshNeeds(needKeys, all)
    needKeys = r.keys
    var n = Model.notice(base, all, r.fresh)
    if (!n) return
    lastNotice = n
    if (!notifies) return
    // One at a time, so each knows the id to replace; a newer one waits.
    if (sending) pendingNotice = n
    else sendNotice(n)
  }
  property var pendingNotice: null
  // Our own flag: Process.running does not turn true until the process starts.
  property bool sending: false
  function sendNotice(n) {
    sending = true
    noticeSender.command = Model.noticeCommand(n, noticeId)
    noticeSender.running = true
  }

  function join() {
    var cmd = Model.joinCommand(machine)
    if (cmd && joinState === "ready") Quickshell.execDetached(cmd)
  }

  function moveCursor(dy) {
    cursor = Model.moveCursor(cursor, dy, navItems.length)
    cursorId = cursor >= 0 ? navItems[cursor].id : ""
  }
  // Scroll the panel so the highlighted row is in view.
  function revealCursor() {
    var row = null
    for (var i = 0; i < rowItems.length; i++) if (rowItems[i] && rowItems[i].current) row = rowItems[i]
    if (!row) return
    var y = row.mapToItem(column, 0, 0).y
    if (y < flick.contentY) flick.contentY = y
    else if (y + row.height > flick.contentY + flick.height) flick.contentY = y + row.height - flick.height
  }
  // The rows that can be highlighted, registered as they are made.
  property var rowItems: []
  function activateCursor() {
    var item = navItems[cursor]
    if (!item) return
    if (item.pick !== undefined) pick(item.pick)
    else openUrl(item.url || "")
  }

  // The harness feeds recorded answers through the same path.
  function apply(swarmsText, villagesText) {
    swarmsDoc = Model.parse(swarmsText)
    var doc = Model.parse(villagesText)
    if (doc !== null) { noteReleases(doc); noteNeeds(doc) }
    villagesDoc = doc
    reachable = villagesDoc !== null
    if (reachable) lastOk = Date.now()
  }

  function pick(p) { picked = p }
  function back() { picked = "" }

  function openUrl(url) {
    if (url === "") return
    Quickshell.execDetached(["omarchy-launch-browser", url])
  }
  // The full village as a web app when Omarchy can make one, else in the browser.
  function openVillage() {
    var url = project !== "" ? Model.villageUrl(base, project) : base + "/village"
    Quickshell.execDetached(["sh", "-c", 'command -v omarchy-launch-webapp >/dev/null && exec omarchy-launch-webapp "$1" || exec omarchy-launch-browser "$1"', "sh", url])
  }

  onOpenedChanged: if (opened) {
    nowMs = Date.now(); refresh()
    if (machine !== "" && !swarmCheck.running) swarmCheck.running = true
  }
  // Another server: what waited on the old one says nothing about the new one.
  onBaseChanged: { swarmsDoc = null; villagesDoc = null; reachable = false; needKeys = null; refresh() }
  Component.onCompleted: refresh()

  // Sends the notice; prints the id the daemon gave it (-p).
  Process {
    id: noticeSender
    stdout: StdioCollector {
      onStreamFinished: { var id = parseInt(this.text.trim(), 10); if (id > 0) root.noticeId = id }
    }
    onExited: {
      root.sending = false
      if (!root.pendingNotice) return
      var n = root.pendingNotice
      root.pendingNotice = null
      Qt.callLater(function() { root.sendNotice(n) })
    }
  }

  // Is `swarm` installed here? The Join swarm button hides when it isn't.
  // Checked again each time the panel opens, so installing it needs no restart.
  Process {
    id: swarmCheck
    command: ["sh", "-c", "command -v swarm >/dev/null"]
    running: root.machine !== ""
    onExited: function(code) { root.swarmOnPath = code === 0 }
  }

  FileView {
    id: notifierFile
    path: Quickshell.env("HOME") + "/.config/agents-ui/notify.json"
    watchChanges: true
    printErrors: false
    onFileChanged: reload()
    onLoaded: root.notifierConfig = notifierFile.text()
    onLoadFailed: root.notifierConfig = null
  }

  Timer {
    interval: root.intervalMs
    repeat: true
    running: true
    onTriggered: { root.nowMs = Date.now(); root.refresh() }
  }

  // ---- Bar icon --------------------------------------------------------------
  BarIconButton {
    id: button
    anchors.fill: parent
    bar: root.bar
    // nf-md-home_city: a house among houses.
    text: "󰴄"
    labelText: root.showWorking && root.badge.working > 0 ? String(root.badge.working) : ""
    // Dimmed, like Omarchy's Tailscale icon when it is off, while the server can't be reached.
    dimmed: !root.reachable
    // A red build turns the icon red (#415), Omarchy's own urgent colour.
    active: root.buildRed
    tooltipText: !root.reachable ? "Can't reach " + root.host
      : root.badge.working + " working" + (root.badge.alert ? " · something needs you" : "")
        + (root.badge.build === "red" || root.badge.build === "running" ? " · " + root.badge.buildWords : "")
    onPressed: function(buttonCode) {
      if (buttonCode === Qt.RightButton) root.openVillage()
      else if (buttonCode === Qt.MiddleButton) root.join()
      else root.toggle()
    }

    Rectangle {
      visible: root.reachable && root.showAlert && root.badge.alert
      width: Math.max(4, Math.round(button.height / 5))
      height: width
      radius: width / 2
      color: root.urgent
      anchors.top: parent.top
      anchors.right: parent.right
      anchors.topMargin: Math.round(button.height / 6)
      anchors.rightMargin: Math.round(button.width / 8)
    }
    Text {
      // A build running on the integration branch: a small clock at the foot.
      visible: root.reachable && root.badge.build === "running"
      // nf-md-progress_clock
      text: "󰦖"
      color: root.barForeground
      font.family: root.fontFamily
      font.pixelSize: Math.round(button.height / 3)
      anchors.bottom: parent.bottom
      anchors.left: parent.left
    }
    Text {
      visible: root.reachable && root.showSailing && root.badge.sailing
      // nf-md-sail_boat
      text: "󰌕"
      color: root.barForeground
      font.family: root.fontFamily
      font.pixelSize: Math.round(button.height / 3)
      anchors.bottom: parent.bottom
      anchors.right: parent.right
    }
  }

  // ---- Panel -----------------------------------------------------------------
  KeyboardPanel {
    id: panel
    anchorItem: button
    owner: root
    bar: root.bar
    open: root.opened
    focusTarget: keys
    contentWidth: panel.fittedContentWidth(Style.space(380))
    contentHeight: panel.fittedContentHeight(column.implicitHeight, Style.space(600))

    PanelKeyCatcher {
      id: keys
      anchors.fill: parent
      onCloseRequested: root.close()
      onTabRequested: function(direction) { root.switchPanel(direction) }
      onMoveRequested: function(dx, dy) { if (dy !== 0) root.moveCursor(dy) }
      onActivateRequested: root.activateCursor()
      onTextKey: function(t) {
        var key = t.toLowerCase()
        if (key === "r") root.refresh()
        else if (key === "o") root.openVillage()
        else if (key === "b" && root.canGoBack) root.back()
        else if (key === "s") root.join()
      }

      Flickable {
        id: flick
        anchors.fill: parent
        contentWidth: width
        contentHeight: column.implicitHeight
        clip: true
        boundsBehavior: Flickable.StopAtBounds
        interactive: contentHeight > height
        ScrollBar.vertical: ScrollBar { policy: ScrollBar.AsNeeded }

        Column {
          id: column
          x: Style.space(2)
          width: flick.width - Style.space(4)
          spacing: Style.space(10)

          // Header: where we are, how fresh, and the way back to the list.
          RowLayout {
            width: parent.width
            spacing: Style.space(8)

            Label {
              visible: root.canGoBack
              text: "‹"
              size: Style.font.title
              link: true
              onClicked: root.back()
            }
            Column {
              Layout.fillWidth: true
              spacing: Style.space(2)
              Label {
                width: parent.width
                text: root.project !== "" ? root.project : "Agents village"
                size: Style.font.title
                bold: true
                link: root.project !== "" && root.repo !== ""
                onClicked: root.openUrl(root.repo)
              }
              Label {
                width: parent.width
                text: !root.reachable
                  ? "Can't reach " + root.host
                  : root.host + " · updated " + Model.ago(root.lastOk, root.nowMs)
                color: root.reachable ? root.dim : root.urgent
                size: Style.font.caption
              }
            }
          }

          PanelSeparator { foreground: root.foreground }

          // ---- The list of swarms -------------------------------------------
          Column {
            visible: root.reachable && root.project === ""
            width: parent.width
            spacing: Style.space(2)

            PanelSectionHeader { text: "Swarms"; foreground: root.foreground }
            Label { visible: root.rows.length === 0; text: "No swarms on " + root.host; color: root.dim }
            Repeater {
              model: root.rows
              Row2 {
                required property var modelData
                required property int index
                width: parent.width
                current: root.cursor === index
                name: modelData.project
                sub: modelData.workflow
                trailing: modelData.stopped ? "stopped"
                  : modelData.working + " working" + (modelData.waiting > 0 ? " · " + modelData.waiting + " need you" : "")
                alert: modelData.waiting > 0
                onClicked: root.pick(modelData.project)
              }
            }
          }

          // ---- One village --------------------------------------------------
          Column {
            visible: root.reachable && root.village !== null
            width: parent.width
            spacing: Style.space(10)

            // The counter row, as on the village page.
            Flow {
              width: parent.width
              spacing: Style.space(6)
              Repeater {
                model: root.flowItems
                Rectangle {
                  required property var modelData
                  implicitWidth: cell.implicitWidth + Style.space(12)
                  implicitHeight: cell.implicitHeight + Style.space(8)
                  color: "transparent"
                  radius: Style.space(4)
                  border.width: 1
                  border.color: Qt.rgba(root.foreground.r, root.foreground.g, root.foreground.b, 0.25)
                  Column {
                    id: cell
                    anchors.centerIn: parent
                    Label { anchors.horizontalCenter: parent.horizontalCenter; text: String(modelData.count); bold: true }
                    Label { anchors.horizontalCenter: parent.horizontalCenter; text: modelData.label; color: root.dim; size: Style.font.caption }
                  }
                }
              }
            }

            // The integration branch's build: red, running or green, with where to look.
            Row2 {
              visible: root.buildWords !== ""
              width: parent.width
              name: "Build"
              sub: root.buildWords
              alert: root.buildState === "red"
              onClicked: root.openUrl(root.village && root.village.build ? root.village.build.url || "" : "")
            }

            Column {
              visible: root.attentionItems.length > 0
              width: parent.width
              spacing: Style.space(2)
              PanelSectionHeader { text: "Needs you"; foreground: root.urgent }
              Repeater {
                model: root.attentionItems
                Row2 {
                  required property var modelData
                  required property int index
                  width: parent.width
                  current: root.cursor === index
                  name: modelData.what
                  sub: modelData.title
                  alert: true
                  onClicked: root.openUrl(modelData.url)
                }
              }
            }

            Column {
              width: parent.width
              spacing: Style.space(2)
              PanelSectionHeader { text: "Houses"; foreground: root.foreground }
              Label { visible: root.houses.length === 0; text: "No workers"; color: root.dim }
              Repeater {
                model: root.houses
                Row2 {
                  required property var modelData
                  required property int index
                  width: parent.width
                  current: root.cursor === root.attentionItems.length + index
                  name: modelData.name + (modelData.branch !== "" ? "  " + modelData.branch : "")
                  sub: modelData.issue ? "#" + modelData.issue + " " + modelData.title : modelData.title
                  trailing: modelData.status + (modelData.context !== null ? " · " + modelData.context + "%" : "")
                  alert: modelData.status === "blocked"
                  onClicked: root.openUrl(modelData.url)
                }
              }
            }

            Column {
              visible: root.featureItems.length > 0
              width: parent.width
              spacing: Style.space(2)
              PanelSectionHeader { text: "Features"; foreground: root.foreground }
              Repeater {
                model: root.featureItems
                Row2 {
                  required property var modelData
                  width: parent.width
                  name: "#" + modelData.number + " " + modelData.title
                  trailing: modelData.live ? "live" : modelData.build
                  onClicked: root.openUrl(modelData.url)
                }
              }
            }
          }

          PanelSeparator { foreground: root.foreground }

          RowLayout {
            width: parent.width
            spacing: Style.space(8)
            Label { text: "Open village"; link: true; onClicked: root.openVillage() }
            Label {
              id: joinLabel
              visible: root.joinState !== "hidden"
              text: root.joinState === "ready" ? "Join swarm" : "Join swarm (bad machine name)"
              link: root.joinState === "ready"
              color: root.joinState === "ready" ? Commons.Color.accent : root.dim
              onClicked: root.join()
            }
            Label { text: "Dashboard"; link: true; onClicked: root.openUrl(root.base + "/") }
            Item { Layout.fillWidth: true }
            Label { text: "Refresh"; link: true; color: root.dim; onClicked: root.refresh() }
          }
        }
      }
    }
  }

  // ---- Pieces ----------------------------------------------------------------
  component Label: Text {
    id: label
    property real size: Style.font.body
    property bool bold: false
    property bool link: false
    signal clicked()
    textFormat: Text.PlainText
    color: link ? Commons.Color.accent : root.foreground
    font.family: root.fontFamily
    font.pixelSize: size
    font.bold: bold
    elide: Text.ElideRight
    MouseArea {
      anchors.fill: parent
      enabled: label.link
      cursorShape: Qt.PointingHandCursor
      onClicked: label.clicked()
    }
  }

  // A row: a name and a line under it on the left, a state on the right.
  // (Not `left`/`right`: those are every Item's anchor lines.)
  component Row2: CursorSurface {
    id: row
    property string name: ""
    property string sub: ""
    property string trailing: ""
    property bool alert: false
    // Highlighted from the keyboard (j/k, the arrows).
    property bool current: false
    signal clicked()
    Component.onCompleted: root.rowItems = root.rowItems.concat([row])
    Component.onDestruction: root.rowItems = root.rowItems.filter(function(r) { return r !== row })
    foreground: root.foreground
    hasCursor: mouse.containsMouse || current
    implicitHeight: content.implicitHeight + Style.spacing.rowPaddingX

    MouseArea {
      id: mouse
      anchors.fill: parent
      hoverEnabled: true
      cursorShape: Qt.PointingHandCursor
      onClicked: row.clicked()
    }
    RowLayout {
      id: content
      anchors.left: parent.left
      anchors.right: parent.right
      anchors.verticalCenter: parent.verticalCenter
      anchors.leftMargin: Style.space(10)
      anchors.rightMargin: Style.space(10)
      spacing: Style.space(8)
      Column {
        Layout.fillWidth: true
        spacing: Style.space(1)
        Label { width: parent.width; text: row.name; color: row.alert ? root.urgent : root.foreground }
        Label { width: parent.width; visible: row.sub !== ""; text: row.sub; color: root.dim; size: Style.font.caption }
      }
      Label { visible: row.trailing !== ""; text: row.trailing; color: row.alert ? root.urgent : root.dim; size: Style.font.caption }
    }
  }
}
