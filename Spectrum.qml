import QtQuick
import qs.Commons

Item {
    id: root
    property var bands: [0, 0, 0, 0, 0]
    property real volume: 100
    property bool playing: false
    property int barCount: 5
    // Smooth independent FFT windows before animating between their levels.
    property var smoothedBands: []
    onBandsChanged: {
        var next = []
        for (var i = 0; i < barCount; ++i) {
            var target = Math.max(0, Math.min(1, Number(bands[i]) || 0))
            var previous = smoothedBands[i] || 0
            var blend = target > previous ? 0.45 : 0.25
            next.push(previous + (target - previous) * blend)
        }
        smoothedBands = next
    }
    onPlayingChanged: if (!playing) smoothedBands = []
    property color foreground: Color.foreground
    implicitWidth: Style.space(48)
    implicitHeight: Style.space(24)
    Repeater {
        model: root.barCount
        Rectangle {
            required property int index
            width: root.width / Math.max(1, root.barCount) * 0.7
            x: index * root.width / Math.max(1, root.barCount)
            anchors.bottom: parent.bottom
            height: Math.max(Style.space(1.5), root.height * (root.playing ? Number(root.smoothedBands[index] || 0) : 0)
                             * Math.sqrt(Math.max(0, root.volume) / 100))
            color: root.foreground
            opacity: root.playing ? 0.85 : 0.4
            Behavior on height { NumberAnimation { duration: Style.duration(55); easing.type: Easing.Linear } }
        }
    }
}
