import QtQuick
import qs.Commons

Item {
    property string button: "right"
    width: Style.space(15)
    height: Style.space(21)
    Rectangle {
        anchors.fill: parent
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
        anchors.horizontalCenter: parent.horizontalCenter
        y: Style.space(3)
        width: Style.space(3)
        height: Style.space(6)
        radius: Style.space(1)
        color: Color.foreground
        opacity: button === "middle" ? 1 : 0.35
    }
}
