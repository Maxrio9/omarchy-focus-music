import QtQuick
import qs.Commons

Item {
    id: root
    property var bands: [0, 0, 0, 0, 0]
    property real volume: 100
    property bool playing: false
    property bool centered: false
    property color foreground: Color.foreground
    implicitWidth: Style.space(48)
    implicitHeight: Style.space(24)
    Repeater {
        model: root.bands.length
        Rectangle {
            required property int index
            width: root.width / Math.max(1, root.bands.length) * 0.7
            x: index * root.width / Math.max(1, root.bands.length)
            anchors.bottom: root.centered ? undefined : parent.bottom
            anchors.verticalCenter: root.centered ? parent.verticalCenter : undefined
            height: Math.max(Style.space(1.5), root.height * (root.playing ? Number(root.bands[index] || 0) : 0)
                             * Math.sqrt(Math.max(0, root.volume) / 100))
            color: root.foreground
            opacity: root.playing ? 0.85 : 0.4
            Behavior on height { NumberAnimation { duration: 85; easing.type: Easing.OutCubic } }
        }
    }
}
