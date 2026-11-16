import QtQuick
import Quickshell
import Quickshell.Io
import "Model.js" as Model
import "Notify.js" as Notify

// Desktop notifications from the village: reads the agents-tui server's
// /api/village every `interval` seconds and sends a notification when
// something changes: a house or the manager starts waiting on you, a merge
// bounces at the gate, the build breaks or is fixed, a deploy lands or fails.
// What is news, and what is sent, is Notify.js; this file only polls, sends
// and opens.
//
// It keeps what it has seen in memory: the first poll after the shell starts
// records the village without a word, so a restart never repeats anything.
//
// Settings: ~/.config/agents-ui/notify.json (server, interval, kinds, quiet),
// re-read when it changes; quiet hours default to the screensaver's `dim`.
Item {
  id: root

  // Injected by omarchy-shell.
  property var shell: null
  property var manifest: null

  readonly property string home: Quickshell.env("HOME")
  property string notifyText: ""
  property string saverText: ""
  readonly property var settings: Notify.settingsOf(notifyText, saverText)
  // What the last poll saw, per village; null until the first answer.
  property var snapshot: null
  property int sent: 0
  // Nothing is polled until notify.json has been read (or found missing), so
  // the first poll goes to the configured server, not the default.
  property bool ready: false

  // A new server, or turning it off and on, starts from a fresh look.
  onSettingsChanged: if (!settings.enabled || (snapshot && snapshot.__server !== settings.server)) snapshot = null

  function poll() {
    if (!root.settings.enabled) return
    var server = root.settings.server
    var xhr = new XMLHttpRequest()
    xhr.onreadystatechange = function() {
      if (xhr.readyState !== XMLHttpRequest.DONE) return
      if (xhr.status !== 200 || server !== root.settings.server) return
      var doc = null
      try { doc = JSON.parse(xhr.responseText) } catch (e) { return }
      var was = root.snapshot
      var r = Notify.step(was, doc, server, { model: Model })
      r.snapshot.__server = server
      root.snapshot = r.snapshot
      if (!was) return
      Notify.notifications(r.events, root.settings, new Date()).forEach(root.send)
    }
    xhr.open("GET", server + "/api/village")
    xhr.timeout = 10000
    xhr.send()
  }

  function send(n) {
    root.sent++
    sender.createObject(root, { note: n })
  }

  // The village page, as Omarchy's web app when it has one, else the browser.
  function open(url) {
    if (!url) return
    Quickshell.execDetached(["sh", "-c", 'command -v omarchy-launch-webapp >/dev/null && exec omarchy-launch-webapp "$1" || exec omarchy-launch-browser "$1"', "sh", url])
  }

  // One notify-send per notification. It waits for the notification to
  // close; clicking it (the default action) prints "default", which opens
  // the village on what it is about.
  Component {
    id: sender
    Process {
      id: proc
      property var note: ({})
      command: ["notify-send", "--app-name=Agents village", "--icon=dialog-information",
                "--action=default=Open village", String(note.title || ""), Notify.markup(note.body)]
      running: true
      stdout: StdioCollector {
        onStreamFinished: if (this.text.trim() === "default") root.open(proc.note.url)
      }
      onExited: proc.destroy()
    }
  }

  Timer {
    interval: root.settings.interval * 1000
    running: root.ready && root.settings.enabled
    repeat: true
    triggeredOnStart: true
    onTriggered: root.poll()
  }

  FileView {
    id: notifyFile
    path: root.home + "/.config/agents-ui/notify.json"
    watchChanges: true
    printErrors: false
    onFileChanged: reload()
    onLoaded: { root.notifyText = notifyFile.text(); root.ready = true }
    onLoadFailed: { root.notifyText = ""; root.ready = true }
  }

  FileView {
    id: saverFile
    path: root.home + "/.config/agents-ui/screensaver.json"
    watchChanges: true
    printErrors: false
    onFileChanged: reload()
    onLoaded: root.saverText = saverFile.text()
    onLoadFailed: root.saverText = ""
  }
}
