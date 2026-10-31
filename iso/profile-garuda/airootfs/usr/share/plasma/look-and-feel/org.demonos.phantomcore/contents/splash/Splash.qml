// DemonOS PhantomCore Plasma splash (images copied in at ISO build time from
// iso/theme/make-theme-assets.py output).
import QtQuick

Rectangle {
    id: root
    color: "#07000a"
    property int stage

    Image {
        anchors.fill: parent
        source: "images/background.png"
        fillMode: Image.PreserveAspectCrop
    }
    Image {
        id: logo
        anchors.centerIn: parent
        source: "images/logo.png"
        width: 192; height: 192
        fillMode: Image.PreserveAspectFit
        opacity: 0.4 + 0.1 * Math.min(root.stage, 6)
    }
    Text {
        anchors.top: logo.bottom
        anchors.topMargin: 28
        anchors.horizontalCenter: parent.horizontalCenter
        text: "DemonOS PhantomCore  //  Lillith is waking"
        color: "#ff2d55"
        font.pixelSize: 26
        font.family: "JetBrains Mono"
    }
}
