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
    Rectangle {
        visible: root.button === "scroll"
        anchors.centerIn: parent
        width: Style.space(5)
        height: Style.space(11)
        radius: Style.space(1)
        color: "transparent"
        border.width: Style.space(1)
        border.color: Color.foreground
        Rectangle {
            anchors.centerIn: parent
            width: parent.width - Style.space(2)
            height: Style.space(1)
            color: Color.foreground
        }
    }
    Text {
        visible: root.button === "scroll"
        anchors.left: parent.left
        anchors.verticalCenter: parent.verticalCenter
        text: "↑"
        color: Color.foreground
        font.family: Style.font.family
        font.pixelSize: Style.font.body
    }
    Text {
        visible: root.button === "scroll"
        anchors.right: parent.right
        anchors.verticalCenter: parent.verticalCenter
        text: "↓"
        color: Color.foreground
        font.family: Style.font.family
        font.pixelSize: Style.font.body
    }
}
