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
        visible: root.button !== "scroll"
        opacity: button === "middle" ? 1 : 0.35
    }
    // A downward press mark belongs to the mouse; scrolling uses a separate
    // outlined wheel flanked by arrows, without the mouse-body silhouette.
    Text {
        visible: root.button === "middle"
        anchors.horizontalCenter: parent.horizontalCenter
        y: Style.space(9)
        text: "⌄"
        color: Color.foreground
        font.family: Style.font.family
        font.pixelSize: Style.font.caption
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
