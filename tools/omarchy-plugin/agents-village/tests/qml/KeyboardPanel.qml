import QtQuick

// Stands in for qs.Ui's KeyboardPanel in the harness. The real one is a layer
// shell window, which needs a Wayland compositor; the widget's decisions do
// not, so its content gets a plain Item to live in instead. Same properties and
// sizing helpers the widget uses, nothing else.
Item {
  id: root
  required property Item anchorItem
  required property QtObject bar
  property var owner: null
  property bool open: false
  property bool centerOnBar: false
  property Item focusTarget: null
  property int contentWidth: 380
  property int contentHeight: 400
  width: contentWidth
  height: contentHeight
  default property alias contentItem: holder.children

  function fittedContentWidth(width, cap) {
    var w = Math.max(1, Number(width) || 1)
    return Math.round(cap !== undefined && Number(cap) > 0 ? Math.min(w, Number(cap)) : w)
  }
  function fittedContentHeight(height, cap) {
    var h = Math.max(1, Number(height) || 1)
    return Math.round(cap !== undefined && Number(cap) > 0 ? Math.min(h, Number(cap)) : h)
  }

  Item { id: holder; anchors.fill: parent }
}
