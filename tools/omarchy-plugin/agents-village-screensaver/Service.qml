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
    onIsIdleChanged: if (idle.isIdle) root.launch()
  }
}
