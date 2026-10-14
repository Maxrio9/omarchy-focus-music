import QtQuick
import qs.Commons

Item {
    id: root
    property string button: "right"
    Accessible.name: button === "scroll" ? "Scroll to adjust volume" : (button === "middle" ? "Middle-click for next track" : "Right-click to play or pause")
    implicitWidth: Style.space(button === "scroll" ? 28 : 15)
    implicitHeight: Style.space(21)
    width: implicitWidth
    height: implicitHeight
    Rectangle {
        visible: root.button !== "scroll"
        width: Style.space(15)
        height: parent.height
        anchors.left: parent.left
        radius: width / 2
        color: "transparent"
        border.width: Style.space(1)
        border.color: Color.foreground
        opacity: 0.65
    }
    Canvas {
        visible: root.button === "right"
        width: Style.space(15)
        height: parent.height
        readonly property color ink: Color.foreground
        onInkChanged: requestPaint()
        onWidthChanged: requestPaint()
        onHeightChanged: requestPaint()
        onPaint: {
            var ctx = getContext("2d")
            ctx.reset()
            ctx.scale(width / 15, height / 21)
            // Follow the outer mouse curve, with a straight inner edge and
            // button seam instead of an oval resembling the wheel.
            ctx.fillStyle = ink
            ctx.beginPath()
            ctx.moveTo(8.5, 1.8)
            ctx.bezierCurveTo(11.3, 2, 13.6, 4.3, 13.6, 7.4)
            ctx.lineTo(13.6, 10)
            ctx.lineTo(8.5, 10)
            ctx.closePath()
            ctx.fill()
            ctx.globalAlpha = 0.45
            ctx.strokeStyle = ink
            ctx.lineWidth = 0.6
            ctx.beginPath()
            ctx.moveTo(7.5, 2)
            ctx.lineTo(7.5, 10)
            ctx.moveTo(2, 10.5)
            ctx.lineTo(13, 10.5)
            ctx.stroke()
        }
    }
    Rectangle {
        x: Style.space(6)
        y: Style.space(3)
        width: Style.space(3)
        height: Style.space(6)
        radius: Style.space(1)
        color: Color.foreground
        visible: root.button !== "scroll"
        opacity: button === "middle" ? 1 : 0.35
    }
    Canvas {
        visible: root.button === "scroll"
        anchors.fill: parent
        readonly property color ink: Color.foreground
        onInkChanged: requestPaint()
        onWidthChanged: requestPaint()
        onHeightChanged: requestPaint()
        onPaint: {
            var ctx = getContext("2d")
            ctx.reset()
            ctx.scale(width / 34, height / 26)
            ctx.strokeStyle = ink
            ctx.lineWidth = 1.2
            ctx.lineCap = "round"
            ctx.lineJoin = "round"
            // Zoom into the mouse buttons; leave the bottom edges open.
            ctx.beginPath()
            ctx.moveTo(6, 20)
            ctx.lineTo(6, 14.8)
            ctx.quadraticCurveTo(6, 7, 17, 7)
            ctx.quadraticCurveTo(28, 7, 28, 14.8)
            ctx.lineTo(28, 20)
            ctx.stroke()
            // Plain wheel outline, without internal horizontal marks.
            ctx.strokeRect(14.5, 11, 5, 7)
            ctx.beginPath()
            ctx.moveTo(17, 8)
            ctx.lineTo(17, 10)
            ctx.moveTo(7, 18)
            ctx.lineTo(13, 18)
            ctx.moveTo(21, 18)
            ctx.lineTo(27, 18)
            ctx.stroke()
            // Up above and down just below the cropped mouse.
            ctx.beginPath()
            ctx.moveTo(14.5, 3.5)
            ctx.lineTo(17, 1)
            ctx.lineTo(19.5, 3.5)
            ctx.moveTo(17, 1)
            ctx.lineTo(17, 5)
            ctx.moveTo(17, 21)
            ctx.lineTo(17, 24)
            ctx.moveTo(14.5, 21.5)
            ctx.lineTo(17, 24)
            ctx.lineTo(19.5, 21.5)
            ctx.stroke()
        }
    }
}
