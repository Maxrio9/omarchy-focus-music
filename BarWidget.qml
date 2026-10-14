import QtQuick
import Quickshell
import QtQuick.Layouts
import QtQuick.Controls as Controls
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
    function close() { popupOpen = false; popupPinned = false; settingsOpen = false }
    function runAction(action, argument) {
        var command = ["python3", controllerPath, action]
        if (argument !== undefined) command.push(String(argument))
        Quickshell.execDetached(command)
    }
    property bool settingsOpen: false
    onSettingsOpenChanged: popupScroll.contentY = 0
    property var preferences: ({barDisplay: "banner", visualizerWidth: 100, otherMedia: "pause-resume", bannerWidth: null, placement: "left"})
    readonly property string displayMode: preferences.barDisplay || "banner"
    readonly property int bannerWidth: preferences.bannerWidth || Number(root.setting("bannerWidth", 180)) || 180
    readonly property int visualizerWidth: preferences.visualizerWidth || 100
    property var popupBands: [0, 0, 0, 0, 0]
    property var spectrumBands: [0, 0, 0, 0, 0]
    readonly property bool spectrumWanted: playbackPhase === "Playing" && (displayMode === "visualizer" || popupOpen)
    onSpectrumWantedChanged: {
        spectrumProcess.running = spectrumWanted
        if (!spectrumWanted) spectrumBands = [0, 0, 0, 0, 0]
    }
    function savePreference(key, value) {
        var next = Object.assign({}, preferences)
        next[key] = value
        if (key === "placement" && value === "right") next.barDisplay = "icon"
        preferences = next
        preferenceGrace.restart()
        var changes = {}; changes[key] = value
        runAction("settings-set", JSON.stringify(changes))
    }
    Process {
        id: mediaBridge
        command: ["/usr/bin/python3", decodeURIComponent(String(Qt.resolvedUrl("bin/focus-media")).replace(/^file:\/\//, ""))]
        running: true
    }
    Timer { interval: 5000; running: true; repeat: true; onTriggered: if (!mediaBridge.running) mediaBridge.running = true }
    Timer { id: preferenceGrace; interval: 1500 }
    Process {
        id: spectrumProcess
        command: ["python3", decodeURIComponent(String(Qt.resolvedUrl("bin/focus-visualizer")).replace(/^file:\/\//, ""))]
        stdout: SplitParser {
            onRead: function(line) {
                try { var frame = JSON.parse(line); root.spectrumBands = frame.bands || [0, 0, 0, 0, 0]; root.popupBands = frame.popupBands || root.spectrumBands.slice(0, 5) } catch (e) {}
            }
        }
        onExited: root.spectrumBands = [0, 0, 0, 0, 0]
    }
    Timer {
        interval: 3000
        running: root.spectrumWanted
        repeat: true
        onTriggered: if (!spectrumProcess.running) spectrumProcess.running = true
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
        if (player.preferences && !preferenceGrace.running) preferences = player.preferences
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
        fixedWidth: root.vertical || root.displayMode === "icon" ? Style.bar.statusSlot : Style.space(root.displayMode === "visualizer" ? root.visualizerWidth : Math.max(80, Math.min(500, root.bannerWidth)))
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
            visible: root.displayMode !== "visualizer" || root.vertical
            anchors.left: root.displayMode === "icon" ? undefined : parent.left
            anchors.horizontalCenter: root.displayMode === "icon" ? parent.horizontalCenter : undefined
            anchors.leftMargin: Style.space(8)
            anchors.verticalCenter: parent.verticalCenter
            width: Style.bar.iconCanvas
            height: Style.bar.iconCanvas
            text: root.player.text
            fontFamily: button.fontFamily
            fontSize: Style.bar.iconFont
            color: button.foreground
        }
        Spectrum {
            objectName: "barSpectrum"
            visible: !root.vertical && root.displayMode === "visualizer"
            anchors.left: parent.left
            anchors.leftMargin: Style.space(8)
            anchors.right: parent.right
            anchors.rightMargin: Style.space(8)
            anchors.verticalCenter: parent.verticalCenter
            height: Style.space(19)
            bands: root.spectrumBands
            volume: root.displayedVolume * Number(root.player.volume_factor === undefined ? 1 : root.player.volume_factor)
            playing: root.playbackPhase === "Playing"
            foreground: button.foreground
        }
        Item {
            id: bannerViewport
            visible: !root.vertical && root.displayMode === "banner"
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
            enabled: !genreSelect.popupOpen && !root.settingsOpen
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
        Flickable {
            id: popupScroll
            anchors.fill: parent
            contentWidth: width
            contentHeight: controls.implicitHeight
            clip: true
            interactive: contentHeight > height
            boundsBehavior: Flickable.StopAtBounds
            Controls.ScrollBar.vertical: Controls.ScrollBar { policy: Controls.ScrollBar.AsNeeded }
        Column {
            id: controls
            width: popupScroll.width
            spacing: Style.space(10)
            RowLayout {
                width: parent.width
                spacing: Style.space(8)
                Text {
                    text: "Focus Music"
                    color: Color.foreground
                    font.family: Style.font.family
                    font.pixelSize: Style.font.subtitle
                    font.bold: true
                }
                Item {
                    Layout.fillWidth: true
                    implicitHeight: Style.space(24)
                    Spectrum {
                        objectName: "popupSpectrum"
                        centered: true
                        anchors.centerIn: parent
                        width: Style.space(48)
                        height: Style.space(24)
                        bands: root.popupBands
                        volume: root.displayedVolume * Number(root.player.volume_factor === undefined ? 1 : root.player.volume_factor)
                        playing: root.playbackPhase === "Playing"
                    }
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
            Column {
                visible: !root.settingsOpen
                width: parent.width
                spacing: Style.space(10)
            GenreSelector {
                id: genreSelect
                objectName: "genreSelector"
                maximumListHeight: Math.max(Style.space(32), Math.min(Style.space(200), popup.availableCardHeight - popup.verticalContentInset - Style.space(250)))
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
            Column {
                visible: root.settingsOpen
                width: parent.width
                spacing: Style.space(10)
                GenreSelector {
                    id: displaySelect
                    objectName: "displaySetting"
                    width: parent.width
                    label: "Bar display"
                    value: root.displayMode
                    options: [{value:"banner", label:"Scrolling track banner"}, {value:"visualizer", label:"Visualizer"}, {value:"icon", label:"Compact music icon"}]
                    onChanged: function(value) { root.savePreference("barDisplay", value) }
                    onPopupOpenChanged: if (popupOpen) { placementSelect.close(); otherMediaSelect.close() }
                }
                GenreSelector {
                    id: placementSelect
                    objectName: "placementSetting"
                    width: parent.width
                    label: "Placement"
                    value: root.preferences.placement
                    options: [{value:"left", label:"Left of date and time"}, {value:"clock-right", label:"Right of date and time"}, {value:"right", label:"Right-hand icons"}]
                    onChanged: function(value) { root.savePreference("placement", value) }
                    onPopupOpenChanged: if (popupOpen) { displaySelect.close(); otherMediaSelect.close() }
                }
                GenreSelector {
                    id: otherMediaSelect
                    objectName: "otherMediaSetting"
                    width: parent.width
                    label: "When other media plays"
                    value: root.preferences.otherMedia || "pause-resume"
                    options: [{value:"pause-resume", label:"Pause and resume · fill silence"}, {value:"duck-strong", label:"Lower volume greatly"}, {value:"duck-gentle", label:"Lower volume slightly"}, {value:"none", label:"No action"}]
                    onChanged: function(value) { root.savePreference("otherMedia", value) }
                    onPopupOpenChanged: if (popupOpen) { displaySelect.close(); placementSelect.close() }
                }
                RowLayout {
                    visible: root.displayMode === "visualizer"
                    width: parent.width
                    Button { text: "−"; focusable: true; bordered: true; onClicked: root.savePreference("visualizerWidth", Math.max(80, root.visualizerWidth - 20)) }
                    Text {
                        Layout.fillWidth: true
                        text: "Visualizer width: " + root.visualizerWidth
                        color: Color.foreground
                        font.family: Style.font.family
                        font.pixelSize: Style.font.caption
                        horizontalAlignment: Text.AlignHCenter
                    }
                    Button { text: "+"; focusable: true; bordered: true; onClicked: root.savePreference("visualizerWidth", Math.min(500, root.visualizerWidth + 20)) }
                }
                RowLayout {
                    visible: root.displayMode === "banner"
                    width: parent.width
                    Button { text: "−"; focusable: true; bordered: true; onClicked: root.savePreference("bannerWidth", Math.max(80, root.bannerWidth - 20)) }
                    Text {
                        Layout.fillWidth: true
                        text: "Banner width: " + root.bannerWidth
                        color: Color.foreground
                        font.family: Style.font.family
                        font.pixelSize: Style.font.caption
                        horizontalAlignment: Text.AlignHCenter
                    }
                    Button { text: "+"; focusable: true; bordered: true; onClicked: root.savePreference("bannerWidth", Math.min(500, root.bannerWidth + 20)) }
                }
            }
            Button {
                objectName: "settingsButton"
                width: parent.width
                text: root.settingsOpen ? "Back to music" : "Settings"
                iconText: root.settingsOpen ? "󰌍" : "󰒓"
                focusable: true
                onClicked: {
                    genreSelect.close()
                    displaySelect.close()
                    placementSelect.close()
                    otherMediaSelect.close()
                    root.settingsOpen = !root.settingsOpen
                }
            }
        }
        }
    }

}
