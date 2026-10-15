/*
    SPDX-FileCopyrightText: 2026 clash-kde contributors
    SPDX-License-Identifier: GPL-3.0-or-later
*/

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.kde.plasma.components as PC3
import org.kde.plasma.core as PlasmaCore
import org.kde.plasma.plasmoid
import org.kde.plasma.plasma5support as Plasma5Support

PlasmoidItem {
    id: root

    activationTogglesExpanded: false
    preferredRepresentation: compactRepresentation
    Plasmoid.icon: "network-vpn"

    readonly property var modes: ["rule", "global", "direct"]

    function configured(key, fallback) {
        const value = Plasmoid.configuration[key];
        return (typeof value === "string" && value.length > 0) ? value : fallback;
    }

    function formatRate(bps) {
        if (bps < 1024) {
            return i18n("%1 B/s", Math.round(bps));
        }
        if (bps < 1024 * 1024) {
            return i18n("%1 KB/s", (bps / 1024).toFixed(1));
        }
        return i18n("%1 MB/s", (bps / 1024 / 1024).toFixed(2));
    }

    function perform(action) {
        switch (action) {
        case "popup":
            expanded = !expanded;
            break;
        case "toggleCore":
            session.setCoreDesired(!session.coreDesired);
            break;
        case "toggleSystemProxy":
            session.setSystemProxyEnabled(!session.systemProxyEnabled);
            break;
        case "cycleMode": {
            const index = Math.max(0, modes.indexOf(session.mode));
            session.setMode(modes[(index + 1) % modes.length]);
            break;
        }
        case "openSettings":
            settingsLauncher.exec("systemsettings kcm_clashkde");
            break;
        default:
            break;
        }
    }

    ClashCalls {
        id: session
        Component.onCompleted: refresh()
    }

    readonly property string stateText: {
        if (session.coreRunning) {
            return i18n("Running");
        }
        if (session.coreDesired) {
            return i18n("Waiting to start");
        }
        return i18n("Stopped");
    }
    readonly property string summary: {
        if (session.error.length > 0 && !session.ready) {
            return session.error;
        }
        const proxy = session.systemProxyApplied ? i18n("System proxy on") : i18n("System proxy off");
        return stateText + "\n" + proxy;
    }

    Plasma5Support.DataSource {
        id: settingsLauncher
        engine: "executable"
        connectedSources: []
        onNewData: sourceName => disconnectSource(sourceName)
        function exec(command) {
            connectSource(command);
        }
    }

    Timer {
        interval: root.expanded ? 1000 : 4000
        running: true
        repeat: true
        onTriggered: session.refresh()
    }

    PlasmaCore.Action {
        id: coreAction
        text: session.coreDesired ? i18n("Stop") : i18n("Start")
        icon.name: session.coreDesired ? "media-playback-stop" : "media-playback-start"
        onTriggered: session.setCoreDesired(!session.coreDesired)
    }

    PlasmaCore.Action {
        id: proxyAction
        text: i18n("System proxy")
        icon.name: "network-vpn"
        checkable: true
        checked: session.systemProxyEnabled
        onTriggered: session.setSystemProxyEnabled(!session.systemProxyEnabled)
    }

    PlasmaCore.Action {
        id: settingsAction
        text: i18n("Settings")
        icon.name: "preferences-system-network"
        onTriggered: root.perform("openSettings")
    }

    Plasmoid.contextualActions: [coreAction, proxyAction, settingsAction]

    compactRepresentation: Item {
        id: compact
        activeFocusOnTab: true
        Accessible.name: i18n("Mihomo")
        Accessible.description: root.summary
        Accessible.role: Accessible.Button

        Keys.onPressed: event => {
            switch (event.key) {
            case Qt.Key_Space:
            case Qt.Key_Enter:
            case Qt.Key_Return:
            case Qt.Key_Select:
                root.perform(root.configured("clickAction", "popup"));
                event.accepted = true;
                break;
            }
        }

        TapHandler {
            acceptedButtons: Qt.LeftButton | Qt.MiddleButton
            onTapped: (eventPoint, button) => {
                const key = button === Qt.MiddleButton ? "middleClickAction" : "clickAction";
                const fallback = button === Qt.MiddleButton ? "toggleCore" : "popup";
                root.perform(root.configured(key, fallback));
            }
        }

        WheelHandler {
            onWheel: event => {
                const action = root.configured("wheelAction", "none");
                if (action !== "none") {
                    root.perform(action);
                    event.accepted = true;
                }
            }
        }

        Kirigami.Icon {
            anchors.centerIn: parent
            width: Math.min(parent.width, parent.height)
            height: width
            source: "network-vpn"
            active: compactHover.hovered || root.expanded
            opacity: session.coreRunning ? 1 : 0.45

            Behavior on opacity {
                OpacityAnimator {
                    duration: Kirigami.Units.shortDuration
                    easing.type: Easing.OutCubic
                }
            }
        }

        HoverHandler {
            id: compactHover
        }

        PlasmaCore.ToolTipArea {
            anchors.fill: parent
            mainText: i18n("Mihomo")
            subText: root.summary
            textFormat: Text.PlainText
        }
    }

    fullRepresentation: Item {
        implicitWidth: Kirigami.Units.gridUnit * 16
        implicitHeight: popupColumn.implicitHeight + Kirigami.Units.largeSpacing * 2

        ColumnLayout {
            id: popupColumn
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.top: parent.top
            anchors.margins: Kirigami.Units.largeSpacing
            spacing: Kirigami.Units.smallSpacing

            PC3.Label {
                text: root.stateText
                font.bold: true
                Layout.fillWidth: true
            }

            PC3.Label {
                visible: session.message.length > 0 && !session.coreRunning
                text: session.message
                wrapMode: Text.Wrap
                font: Kirigami.Theme.smallFont
                Layout.fillWidth: true
            }

            PC3.Label {
                visible: session.error.length > 0
                text: session.error
                wrapMode: Text.Wrap
                color: Kirigami.Theme.negativeTextColor
                font: Kirigami.Theme.smallFont
                Layout.fillWidth: true
            }

            PC3.Label {
                visible: session.coreRunning
                text: session.listenAddress
                opacity: 0.75
                font: Kirigami.Theme.smallFont

                HoverHandler {
                    id: listenHover
                }

                PC3.ToolTip.visible: listenHover.hovered
                PC3.ToolTip.delay: Kirigami.Units.longDuration
                PC3.ToolTip.text: i18n("Other programs can use this address as it is.")
            }

            PC3.Label {
                visible: session.coreRunning
                text: "↑ " + root.formatRate(session.uploadBps) + "   ↓ " + root.formatRate(session.downloadBps)
                font: Kirigami.Theme.smallFont
                Layout.fillWidth: true
            }

            PC3.Label {
                visible: session.region.length > 0
                text: session.region
                font: Kirigami.Theme.smallFont
                opacity: 0.75
                Layout.fillWidth: true
            }

            PC3.Switch {
                text: i18n("Core")
                checked: session.coreDesired
                Layout.fillWidth: true
                onToggled: {
                    if (checked !== session.coreDesired) {
                        session.setCoreDesired(checked);
                    }
                }

                HoverHandler {
                    id: coreHover
                }

                PC3.ToolTip.visible: coreHover.hovered
                PC3.ToolTip.delay: Kirigami.Units.longDuration
                PC3.ToolTip.text: i18n("This switch only starts the local proxy.")
            }

            PC3.Switch {
                text: i18n("System proxy")
                checked: session.systemProxyEnabled
                Layout.fillWidth: true
                onToggled: {
                    if (checked !== session.systemProxyEnabled) {
                        session.setSystemProxyEnabled(checked);
                    }
                }

                HoverHandler {
                    id: proxyHover
                }

                PC3.ToolTip.visible: proxyHover.hovered
                PC3.ToolTip.delay: Kirigami.Units.longDuration
                PC3.ToolTip.text: i18n("KDE and browsers follow this only while the core is running.")
            }

            PC3.ComboBox {
                Layout.fillWidth: true
                model: [i18n("Rule"), i18n("Global"), i18n("Direct")]
                currentIndex: Math.max(0, root.modes.indexOf(session.mode))
                onActivated: index => {
                    const next = root.modes[index];
                    if (next !== session.mode) {
                        session.setMode(next);
                    }
                }
            }

            PC3.Button {
                text: i18n("Settings")
                icon.name: "preferences-system-network"
                Layout.fillWidth: true
                onClicked: root.perform("openSettings")
            }
        }
    }
}
