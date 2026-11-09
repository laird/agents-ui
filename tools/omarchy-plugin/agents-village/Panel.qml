import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Quickshell
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
// anything to an agent, so it works against a --read-only server.
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
  readonly property string repo: Model.repoUrl(village)
  readonly property string host: base.replace(/^[a-z]+:\/\//i, "").replace(/:\d+$/, "")

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
      root.villagesDoc = doc
      root.reachable = true
      root.lastOk = Date.now()
    })
    get("/api/swarms", function(doc) { if (doc !== null) root.swarmsDoc = doc })
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

  // The harness feeds recorded answers through the same path.
  function apply(swarmsText, villagesText) {
    swarmsDoc = Model.parse(swarmsText)
    var doc = Model.parse(villagesText)
    if (doc !== null) noteReleases(doc)
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

  onOpenedChanged: if (opened) { nowMs = Date.now(); refresh() }
  onBaseChanged: { swarmsDoc = null; villagesDoc = null; reachable = false; refresh() }
  Component.onCompleted: refresh()

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
    tooltipText: !root.reachable ? "Can't reach " + root.host
      : root.badge.working + " working" + (root.badge.alert ? " · something needs you" : "")
    onPressed: function(buttonCode) {
      if (buttonCode === Qt.RightButton) root.openVillage()
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
      onTextKey: function(t) {
        var key = t.toLowerCase()
        if (key === "r") root.refresh()
        else if (key === "o") root.openVillage()
        else if (key === "b" && root.canGoBack) root.back()
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
                width: parent.width
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

            Column {
              visible: root.attentionItems.length > 0
              width: parent.width
              spacing: Style.space(2)
              PanelSectionHeader { text: "Needs you"; foreground: root.urgent }
              Repeater {
                model: root.attentionItems
                Row2 {
                  required property var modelData
                  width: parent.width
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
                  width: parent.width
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
    signal clicked()
    foreground: root.foreground
    hasCursor: mouse.containsMouse
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
