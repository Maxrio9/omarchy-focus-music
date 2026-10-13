import QtQuick
import qs.Commons

Item {
    property string button: "right"
    implicitWidth: Style.space(button === "scroll" ? 28 : 15)
    implicitHeight: Style.space(21)
    width: implicitWidth
    height: implicitHeight
    Rectangle {
        width: Style.space(15)
        height: parent.height
        anchors.left: parent.left
        radius: width / 2
        color: "transparent"
        border.width: Style.space(1)
        border.color: Color.foreground
        opacity: 0.65
    }
    Rectangle {
        visible: button === "right"
        x: parent.width / 2 + Style.space(1)
        y: Style.space(3)
        width: parent.width / 2 - Style.space(3)
        height: Style.space(7)
        radius: Style.space(2)
        color: Color.foreground
    }
    Rectangle {
        x: Style.space(6)
        y: Style.space(3)
        width: Style.space(3)
        height: Style.space(6)
        radius: Style.space(1)
        color: Color.foreground
        opacity: button === "middle" || button === "scroll" ? 1 : 0.35
    }
    Text {
        visible: button === "scroll"
        anchors.right: parent.right
        anchors.verticalCenter: parent.verticalCenter
        text: "↕"
        color: Color.foreground
        font.family: Style.font.family
        font.pixelSize: Style.font.caption
    }
}
