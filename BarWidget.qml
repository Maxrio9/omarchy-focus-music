import QtQuick
import Quickshell
import Quickshell.Io
import qs.Commons
import qs.Ui

BarWidget {
    id: root
    moduleName: "maxrio9.focus-music"
    readonly property string controllerPath: decodeURIComponent(String(Qt.resolvedUrl("bin/focus-music")).replace(/^file:\/\//, ""))
    property var genres: []
    property bool popupOpen: false
    readonly property bool opened: popupOpen
    function open() { popupOpen = true }
    function close() { popupOpen = false }
    function runAction(action, argument) {
        var command = ["python3", controllerPath, action]
        if (argument !== undefined) command.push(String(argument))
        Quickshell.execDetached(command)
    }
    property var player: ({text: "󰎈", phase: "Stopped", banner: "Focus music", tooltip: "Focus music — click to choose a genre"})
    implicitWidth: button.implicitWidth + nextButton.implicitWidth
    implicitHeight: button.implicitHeight

    Process {
        id: poll
        command: ["python3", root.controllerPath, "status"]
        stdout: StdioCollector {
            onStreamFinished: {
                try { root.player = JSON.parse(text) } catch (e) {}
            }
        }
    }
    Process {
        id: genrePoll
        command: ["python3", root.controllerPath, "genres"]
        running: true
        stdout: StdioCollector {
            onStreamFinished: {
                try { root.genres = JSON.parse(text) } catch (e) {}
            }
        }
    }
    Timer {
        interval: 2000
        running: true
        repeat: true
        triggeredOnStart: true
        onTriggered: if (!poll.running) poll.running = true
    }
    WidgetButton {
        id: button
        anchors.left: parent.left
        height: parent.height
        width: implicitWidth
        bar: root.bar
        labelVisible: false
        hasVisualContent: true
        fixedWidth: root.vertical ? Style.bar.statusSlot : Style.space(Math.max(80, Math.min(500, Number(root.setting("bannerWidth", 180)) || 180)))
        tooltipText: root.player.tooltip
        onPressed: function(b) {
            if (b === Qt.RightButton) root.runAction("toggle")
            else if (b === Qt.MiddleButton) root.runAction("skip")
            else root.popupOpen = !root.popupOpen
        }
        onWheelMoved: function(delta) {
            root.runAction("volume", delta > 0 ? 5 : -5)
        }
        OpticalGlyph {
            id: glyph
            anchors.left: parent.left
            anchors.leftMargin: Style.space(8)
            anchors.verticalCenter: parent.verticalCenter
            width: Style.bar.iconCanvas
            height: Style.bar.iconCanvas
            text: root.player.text
            fontFamily: button.fontFamily
            fontSize: Style.bar.iconFont
            color: button.foreground
        }
        Item {
            id: bannerViewport
            visible: !root.vertical
            anchors.left: glyph.right
            anchors.leftMargin: Style.space(7)
            anchors.right: parent.right
            anchors.rightMargin: Style.space(8)
            anchors.verticalCenter: parent.verticalCenter
            height: bannerText.implicitHeight
            clip: true
            readonly property real overflow: Math.max(0, bannerText.implicitWidth - width)
            readonly property real cycleWidth: bannerText.implicitWidth + Style.space(40)

            function resetScroll() {
                bannerText.x = 0
                if (overflow > 1 && visible && root.visible) marquee.restart()
                else marquee.stop()
            }
            onOverflowChanged: Qt.callLater(resetScroll)

            Text {
                id: bannerText
                anchors.verticalCenter: parent.verticalCenter
                text: root.player.phase === "Stopped" ? "Focus music" : (root.player.banner || "Loading…")
                textFormat: Text.PlainText
                color: button.foreground
                font.family: button.fontFamily
                font.pixelSize: Style.font.body
                renderType: Text.NativeRendering
                onTextChanged: Qt.callLater(bannerViewport.resetScroll)
            }
            Text {
                visible: bannerViewport.overflow > 1
                anchors.verticalCenter: parent.verticalCenter
                x: bannerText.x + bannerViewport.cycleWidth
                text: bannerText.text
                textFormat: Text.PlainText
                color: bannerText.color
                font: bannerText.font
                renderType: Text.NativeRendering
            }
            SequentialAnimation {
                id: marquee
                running: bannerViewport.visible && root.visible && bannerViewport.overflow > 1
                loops: Animation.Infinite
                onRunningChanged: if (!running) bannerText.x = 0
                PauseAnimation { duration: 1800 }
                NumberAnimation {
                    target: bannerText
                    property: "x"
                    from: 0
                    to: -bannerViewport.cycleWidth
                    duration: Math.max(1000, bannerViewport.cycleWidth / Style.space(24) * 1000)
                    easing.type: Easing.Linear
                }
                PropertyAction {
                    target: bannerText
                    property: "x"
                    value: 0
                }
            }
        }
    }
    BarIconButton {
        id: nextButton
        anchors.left: button.right
        height: parent.height
        width: implicitWidth
        bar: root.bar
        text: "󰒭"
        tooltipText: "Next track · also middle-click the music banner"
        slotSize: Style.bar.statusSlot
        onPressed: function(b) {
            root.runAction("skip")
        }
    }
    PopupCard {
        id: popup
        anchorItem: button
        bar: root.bar
        owner: root
        open: root.popupOpen
        contentWidth: popup.fittedContentWidth(Style.space(340))
        contentHeight: popup.fittedContentHeight(controls.implicitHeight)
        Column {
            id: controls
            anchors.fill: parent
            spacing: Style.space(10)
            Text {
                width: parent.width
                text: "Focus Music"
                color: Color.foreground
                font.family: Style.font.family
                font.pixelSize: Style.font.subtitle
                font.bold: true
            }
            Dropdown {
                width: parent.width
                label: "Genre"
                options: root.genres
                value: root.player.genre || "lofi-hiphop"
                onChanged: function(value) { root.runAction("play", value) }
            }
            Text {
                width: parent.width
                text: root.player.phase === "Stopped" ? "Choose a genre to start" : (root.player.banner || "Loading…")
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                color: Color.foreground
                font.family: Style.font.family
                font.pixelSize: Style.font.body
            }
            Text {
                width: parent.width
                text: {
                    for (var i = 0; i < root.genres.length; i++) {
                        var g = root.genres[i]
                        if (g.value === root.player.genre) return g.track_count.toLocaleString() + " tracks · " + g.hours + " hours"
                    }
                    return ""
                }
                color: Qt.darker(Color.foreground, 1.3)
                font.family: Style.font.family
                font.pixelSize: Style.font.caption
            }
            Row {
                spacing: Style.space(8)
                Button {
                    iconText: root.player.phase === "Playing" ? "󰏤" : "󰐊"
                    text: root.player.phase === "Playing" ? "Pause" : "Play"
                    onClicked: root.runAction("toggle")
                }
                Button { iconText: "󰒭"; text: "Next"; onClicked: root.runAction("skip") }
                Button { iconText: "󰓛"; text: "Stop"; onClicked: root.runAction("stop") }
            }
            Row {
                spacing: Style.space(8)
                Button { iconText: "󰕿"; text: "Quieter"; onClicked: root.runAction("volume", -5) }
                Button { iconText: "󰕾"; text: "Louder"; onClicked: root.runAction("volume", 5) }
            }
        }
    }

}
