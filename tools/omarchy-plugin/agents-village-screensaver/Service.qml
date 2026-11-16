import QtQuick
import Quickshell
import Quickshell.Io
import Quickshell.Wayland

// Starts the village screensaver (village-screensaver, beside this file) when
// the session has been idle for `timeout` seconds. install.sh --screensaver
// turns Omarchy's own text screensaver off (idle.screensaver: 0 in
// shell.json), so this is what runs in its place; Omarchy's idle service
// still locks on its own timer unless --never-sleep turned that off too.
//
// Settings: ~/.config/agents-ui/screensaver.json, re-read when it changes.
//
// "Since you last looked" (#479) counts from the last unlock of the machine.
// A plugin cannot see Omarchy's lock, so while the session is idle or locked
// this asks `omarchy-shell lock isLocked` every few seconds, and when it goes
// from locked to unlocked writes the time (unix seconds) to
// ${XDG_STATE_HOME:-~/.local/state}/agents-village-screensaver/last-unlock,
// which village-screensaver reads.
Item {
  id: root

  // Injected by omarchy-shell.
  property var shell: null
  property var manifest: null

  readonly property string home: Quickshell.env("HOME")
  readonly property string configPath: home + "/.config/agents-ui/screensaver.json"
  readonly property string launcher: decodeURIComponent(String(Qt.resolvedUrl("village-screensaver")).replace(/^file:\/\//, ""))
  property var settings: ({})
  readonly property int timeout: {
    var n = Number(settings.timeout)
    return isFinite(n) && n > 0 ? Math.floor(n) : 150
  }

  readonly property string stateHome: Quickshell.env("XDG_STATE_HOME") || (home + "/.local/state")
  readonly property string unlockPath: stateHome + "/agents-village-screensaver/last-unlock"
  // What answers "true" or "false"; the check replaces it with a stand-in.
  property var lockCommand: ["sh", "-c", 'PATH="$PATH:/usr/share/omarchy/bin" exec omarchy-shell lock isLocked']
  // The lock as last seen. Only a change from locked to unlocked is an unlock.
  property bool locked: false

  function noteLock(answer) {
    if (answer !== "true" && answer !== "false") return  // no answer is no news
    var now = answer === "true"
    if (root.locked && !now) recordUnlock()
    root.locked = now
  }

  function recordUnlock() {
    console.log("agents-village-screensaver: unlocked, writing " + root.unlockPath)
    Quickshell.execDetached(["sh", "-c", 'mkdir -p "${0%/*}" && date +%s > "$0.tmp" && mv -f "$0.tmp" "$0"', root.unlockPath])
  }

  function checkLock() {
    if (!lockQuery.running) lockQuery.running = true
  }

  function launch() {
    console.log("agents-village-screensaver: idle for " + root.timeout + "s, starting " + root.launcher)
    Quickshell.execDetached(["bash", "-lc", '"$0"', root.launcher])
  }

  FileView {
    id: config
    path: root.configPath
    watchChanges: true
    printErrors: false
    onFileChanged: reload()
    onLoaded: {
      try { root.settings = JSON.parse(config.text()) || ({}) } catch (e) { root.settings = ({}) }
    }
    onLoadFailed: root.settings = ({})
  }

  IdleMonitor {
    id: idle
    enabled: root.timeout > 0
    timeout: root.timeout
    respectInhibitors: true
    onIsIdleChanged: {
      if (idle.isIdle) root.launch()
      else root.checkLock()  // someone is back: a lock, if any, is about to go
    }
  }

  Process {
    id: lockQuery
    command: root.lockCommand
    stdout: StdioCollector {
      id: lockAnswer
      onStreamFinished: root.noteLock(lockAnswer.text.trim())
    }
  }

  // Polls only while it matters: idle (a lock may come) or locked (until it goes).
  Timer {
    interval: 5000
    repeat: true
    triggeredOnStart: true
    running: idle.isIdle || root.locked
    onTriggered: root.checkLock()
  }
}
