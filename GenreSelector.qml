import QtQuick
import QtQuick.Controls as Controls
import qs.Commons
import qs.Ui

// Expand within the music window so an option popup cannot extend past its
// Wayland surface. The list owns scrolling while the selector is open.
Column {
    id: root
    property string label: "Genre"
    property string value: ""
    property var options: []
    property bool popupOpen: false
    property real maximumListHeight: Style.space(200)
    signal changed(string value)
    spacing: Style.spacing.labelGap
    function open() {
        popupOpen = true
        for (var i = 0; i < options.length; i++) {
            if (options[i].value === value) { optionList.currentIndex = i; break }
        }
        optionList.positionViewAtIndex(Math.max(0, optionList.currentIndex), ListView.Contain)
        optionList.forceActiveFocus()
    }
    function close() { popupOpen = false }
    function toggle() { if (popupOpen) close(); else open() }
    function selectCurrent() {
        if (optionList.currentIndex < 0 || optionList.currentIndex >= options.length) return
        value = options[optionList.currentIndex].value
        changed(value)
        close()
    }
    Text {
        text: root.label
        color: Qt.darker(Color.popups.text, 1.4)
        font.family: Style.font.family
        font.pixelSize: Style.font.caption
        font.bold: true
    }
    BorderSurface {
        id: trigger
        objectName: "genreTrigger"
        width: parent.width
        height: Style.spacing.controlHeight
        radius: Style.cornerRadius
        borderSpec: Border.controlSpec(trigger.activeFocus ? "focus" : (triggerHover.hovered ? "hover-cursor" : "normal"), Color.popups.text, Color.accent)
        color: Style.controlFill(trigger.activeFocus, triggerHover.hovered, Color.popups.text, Color.accent)
        activeFocusOnTab: true
        HoverHandler { id: triggerHover }
        Text {
            anchors.left: parent.left
            anchors.right: chevron.left
            anchors.verticalCenter: parent.verticalCenter
            anchors.leftMargin: trigger.borderLeft + Style.spacing.controlPaddingX
            anchors.rightMargin: Style.spacing.md
            text: {
                for (var i = 0; i < root.options.length; i++)
                    if (root.options[i].value === root.value) return root.options[i].label
                return root.value
            }
            textFormat: Text.PlainText
            color: Color.popups.text
            font.family: Style.font.family
            font.pixelSize: Style.font.body
            elide: Text.ElideRight
        }
        Text {
            id: chevron
            anchors.right: parent.right
            anchors.verticalCenter: parent.verticalCenter
            anchors.rightMargin: trigger.borderRight + Style.spacing.controlGap
            text: root.popupOpen ? "󰅃" : "󰅀"
            color: Qt.darker(Color.popups.text, 1.2)
            font.family: Style.font.family
            font.pixelSize: Style.font.body
        }
        MouseArea {
            anchors.fill: parent
            cursorShape: Qt.PointingHandCursor
            onClicked: { trigger.forceActiveFocus(); root.toggle() }
        }
        Keys.onReturnPressed: root.toggle()
        Keys.onEnterPressed: root.toggle()
        Keys.onSpacePressed: root.toggle()
        Keys.onDownPressed: root.open()
        Keys.onEscapePressed: root.close()
    }
    BorderSurface {
        visible: root.popupOpen
        width: parent.width
        height: Math.min(optionList.contentHeight + contentTopInset + contentBottomInset,
                         root.maximumListHeight)
        color: Color.popups.background
        borderSpec: Border.surfaceSpec("popups", "border", Color.popups.border, Style.normalBorderWidth)
        radius: Style.cornerRadius
        ListView {
            id: optionList
            objectName: "genreList"
            anchors.fill: parent
            anchors.leftMargin: parent.contentLeftInset
            anchors.rightMargin: parent.contentRightInset
            anchors.topMargin: parent.contentTopInset
            anchors.bottomMargin: parent.contentBottomInset
            clip: true
            model: root.options
            spacing: Style.spacing.labelGap
            boundsBehavior: Flickable.StopAtBounds
            currentIndex: 0
            Controls.ScrollBar.vertical: Controls.ScrollBar { policy: Controls.ScrollBar.AsNeeded }
            Keys.onEscapePressed: root.close()
            Keys.onReturnPressed: root.selectCurrent()
            Keys.onEnterPressed: root.selectCurrent()
            Keys.onPressed: function(event) {
                if (event.text === "j" || event.text === "k") {
                    currentIndex = Math.max(0, Math.min(count - 1, currentIndex + (event.text === "j" ? 1 : -1)))
                    event.accepted = true
                }
            }
            delegate: Button {
                required property var modelData
                required property int index
                width: optionList.width
                height: Style.spacing.popupRowHeight
                text: modelData.label
                leftAlign: true
                active: index === optionList.currentIndex
                onClicked: { optionList.currentIndex = index; root.selectCurrent() }
            }
        }
    }
}
