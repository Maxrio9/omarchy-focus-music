import QtQuick
import Quickshell
import QtQuick.Layouts
import Quickshell.Io
import qs.Commons
import qs.Ui

BarWidget {
    id: root
    moduleName: "maxrio9.focus-music"
    readonly property string controllerPath: decodeURIComponent(String(Qt.resolvedUrl("bin/focus-music")).replace(/^file:\/\//, ""))
    property var genres: []
    property bool popupOpen: false
    property bool popupPinned: false
    readonly property bool opened: popupOpen
    function open() { popupPinned = true; popupOpen = true }
    function close() { popupOpen = false; popupPinned = false }
    function runAction(action, argument) {
        var command = ["python3", controllerPath, action]
        if (argument !== undefined) command.push(String(argument))
        Quickshell.execDetached(command)
    }
    property var player: ({text: "󰎈", phase: "Stopped", banner: "Focus music", tooltip: "Focus music — click to choose a genre"})
    property string playbackOverride: ""
    property bool playbackPending: false
    readonly property string playbackPhase: playbackOverride || player.phase
    function togglePlayback() {
        playbackOverride = playbackPhase === "Playing" ? "Paused" : "Playing"
        playbackPending = true
        flushPlayback()
    }
    function flushPlayback() {
        if (playbackChange.running || !playbackPending) return
        playbackPending = false
        playbackChange.command = ["python3", controllerPath, "pause-set", playbackOverride === "Paused" ? "1" : "0"]
        playbackChange.running = true
    }
    Process {
        id: playbackChange
        stdout: StdioCollector { id: playbackResult }
        onExited: function(exitCode) {
            if (!root.playbackPending) {
                if (exitCode === 0) {
                    try { root.player = JSON.parse(playbackResult.text) } catch (e) {}
                }
                root.playbackOverride = ""
                if (!poll.running) poll.running = true
            }
            Qt.callLater(root.flushPlayback)
        }
    }
    property string catalogRevision: ""
    property bool genresPending: false
    property real displayedVolume: 20
    property bool volumePending: false
    function refreshGenres() {
        if (!genrePoll.running && genresPending) {
            genresPending = false
            genrePoll.running = true
        }
    }
    onPlayerChanged: {
        if (player.catalog_revision && player.catalog_revision !== catalogRevision) {
            catalogRevision = player.catalog_revision
            genresPending = true
            refreshGenres()
        }
        if (!volumeChange.running && !volumePending && player.volume !== undefined)
            displayedVolume = player.volume
    }
    function adjustVolume(delta) {
        displayedVolume = Math.max(0, Math.min(100, displayedVolume + delta))
        volumePending = true
        flushVolume()
    }
    function flushVolume() {
        if (volumeChange.running || !volumePending) return
        volumePending = false
        volumeChange.command = ["python3", controllerPath, "volume-set", String(displayedVolume)]
        volumeChange.running = true
    }
    Process {
        id: volumeChange
        stdout: StdioCollector { id: volumeResult }
        onExited: function(exitCode) {
            if (!root.volumePending && exitCode === 0) {
                try { root.displayedVolume = JSON.parse(volumeResult.text).volume } catch (e) {}
            }
            Qt.callLater(root.flushVolume)
        }
    }
    implicitWidth: button.implicitWidth
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
        onExited: Qt.callLater(root.refreshGenres)
        command: ["python3", root.controllerPath, "genres"]
        running: true
        stdout: StdioCollector {
            onStreamFinished: {
                try { root.genres = JSON.parse(text) } catch (e) {}
            }
        }
    }
    Timer {
        interval: root.popupOpen ? 400 : 2000
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
        tooltipText: ""
        onTooltipHoveredChanged: {
            if (tooltipHovered) { closeHover.stop(); root.popupOpen = true }
            else closeHover.restart()
        }
        onPressed: function(b) {
            if (b === Qt.RightButton) root.togglePlayback()
            else if (b === Qt.MiddleButton) root.runAction("skip")
            else {
                root.popupPinned = !root.popupPinned
                root.popupOpen = root.popupPinned
            }
        }
        onWheelMoved: function(delta) {
            root.adjustVolume(delta > 0 ? 5 : -5)
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
    Timer {
        id: closeHover
        interval: 450
        onTriggered: {
            if (!root.popupPinned && !button.tooltipHovered && !popup.containsMouse && !genreSelect.popupOpen) root.close()
        }
    }
    PopupCard {
        id: popup
        anchorItem: button
        bar: root.bar
        owner: root
        open: root.popupOpen
        triggerMode: root.popupPinned ? "click" : "hover"
        onContainsMouseChanged: {
            if (containsMouse) closeHover.stop()
            else closeHover.restart()
        }
        contentWidth: popup.fittedContentWidth(Style.space(380))
        contentHeight: popup.fittedContentHeight(controls.implicitHeight)
        MouseArea {
            objectName: "popupInput"
            // Include card padding, while leaving left-click controls below it.
            anchors.fill: parent
            anchors.margins: -popup.padding
            z: 10
            acceptedButtons: Qt.RightButton | Qt.MiddleButton
            onClicked: function(mouse) {
                if (mouse.button === Qt.RightButton) root.togglePlayback()
                else root.runAction("skip")
            }
            onWheel: function(event) {
                var delta = event.angleDelta.y || event.pixelDelta.y
                if (delta !== 0) root.adjustVolume(delta > 0 ? 5 : -5)
                event.accepted = true
            }
        }
        Column {
            id: controls
            anchors.fill: parent
            spacing: Style.space(10)
            RowLayout {
                width: parent.width
                spacing: Style.space(8)
                Text {
                    Layout.fillWidth: true
                    text: "Focus Music"
                    color: Color.foreground
                    font.family: Style.font.family
                    font.pixelSize: Style.font.subtitle
                    font.bold: true
                }
                MouseHint { button: "scroll" }
                Rectangle {
                    implicitWidth: Style.space(44)
                    implicitHeight: Style.space(2)
                    color: Qt.rgba(Color.foreground.r, Color.foreground.g, Color.foreground.b, 0.15)
                    Rectangle {
                        width: parent.width * root.displayedVolume / 100
                        height: parent.height
                        color: Qt.rgba(Color.foreground.r, Color.foreground.g, Color.foreground.b, 0.7)
                    }
                }
                Text {
                    text: Math.round(root.displayedVolume) + "%"
                    color: Qt.rgba(Color.foreground.r, Color.foreground.g, Color.foreground.b, 0.7)
                    font.family: Style.font.family
                    font.pixelSize: Style.font.caption
                }
            }
            Dropdown {
                id: genreSelect
                onPopupOpenChanged: if (!genreSelect.popupOpen) closeHover.restart()
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
            RowLayout {
                width: parent.width
                spacing: Style.space(8)
                Text {
                    Layout.fillWidth: true
                    text: {
                        for (var i = 0; i < root.genres.length; i++) {
                            var g = root.genres[i]
                            if (g.value === root.player.genre) return g.track_count.toLocaleString() + " tracks · " + g.hours + " hours"
                        }
                        return ""
                    }
                    elide: Text.ElideRight
                    color: Qt.rgba(Color.foreground.r, Color.foreground.g, Color.foreground.b, 0.7)
                    font.family: Style.font.family
                    font.pixelSize: Style.font.caption
                }
            }
            RowLayout {
                id: playbackControls
                width: parent.width
                spacing: Style.space(8)
                readonly property real buttonWidth: (width - spacing * 2) / 3
                Button {
                    Layout.preferredWidth: playbackControls.buttonWidth
                    Layout.minimumWidth: playbackControls.buttonWidth
                    Layout.maximumWidth: playbackControls.buttonWidth
                    Layout.preferredHeight: Style.space(36)
                    objectName: "previousTrack"
                    text: "Previous"
                    bordered: true
                    focusable: true
                    onClicked: root.runAction("previous")
                }
                Button {
                    Layout.preferredWidth: playbackControls.buttonWidth
                    Layout.minimumWidth: playbackControls.buttonWidth
                    Layout.maximumWidth: playbackControls.buttonWidth
                    Layout.preferredHeight: Style.space(36)
                    objectName: "playbackToggle"
                    text: root.playbackPhase === "Playing" ? "Pause" : "Play"
                    active: root.playbackPhase === "Playing"
                    bordered: true
                    focusable: true
                    tooltipText: "Right-click to play or pause"
                    onClicked: root.togglePlayback()
                    MouseHint {
                        button: "right"
                        anchors.right: parent.right
                        anchors.rightMargin: Style.space(8)
                        anchors.verticalCenter: parent.verticalCenter
                    }
                }
                Button {
                    Layout.preferredWidth: playbackControls.buttonWidth
                    Layout.minimumWidth: playbackControls.buttonWidth
                    Layout.maximumWidth: playbackControls.buttonWidth
                    Layout.preferredHeight: Style.space(36)
                    objectName: "nextTrack"
                    text: "Next"
                    bordered: true
                    focusable: true
                    tooltipText: "Middle-click for the next track"
                    onClicked: root.runAction("skip")
                    MouseHint {
                        button: "middle"
                        anchors.right: parent.right
                        anchors.rightMargin: Style.space(8)
                        anchors.verticalCenter: parent.verticalCenter
                    }
                }
            }
        }
    }

}
