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

    readonly property bool systemProxyUsable: session.coreRunning && !session.tunEnabled
    property string routeGroup: ""

    readonly property var selectorGroups: {
        const groups = session.groups;
        const out = [];
        for (let i = 0; i < groups.length; ++i) {
            if (groups[i]["type"] === "Selector") {
                out.push(groups[i]);
            }
        }
        return out;
    }

    function currentRouteGroup() {
        const groups = selectorGroups;
        for (let i = 0; i < groups.length; ++i) {
            if (routeGroup.length > 0 && groups[i].name === routeGroup) {
                return groups[i];
            }
        }
        if (session.mode === "global") {
            for (let i = 0; i < groups.length; ++i) {
                if (groups[i].name === "GLOBAL") {
                    return groups[i];
                }
            }
        }
        return groups.length > 0 ? groups[0] : null;
    }

    function delayColor(ms) {
        if (ms === 0) {
            return Kirigami.Theme.negativeTextColor;
        }
        const t = Math.max(0, Math.min(1, (ms - 80) / 520));
        return Qt.hsla(0.33 * (1 - t), 0.72, Kirigami.Theme.textColor.hslLightness, 1);
    }

    readonly property string routeSort: configured("routeSort", "profile")

    function sortedNodes(nodes, mode) {
        const copy = nodes.slice();
        if (mode === "name") {
            copy.sort((left, right) => left.name.localeCompare(right.name));
        } else if (mode === "delay") {
            function rank(ms) {
                if (ms < 0) {
                    return 1e15;
                }
                if (ms === 0) {
                    return 1e14;
                }
                return ms;
            }
            copy.sort((left, right) => rank(left["delay_ms"]) - rank(right["delay_ms"]) || left.name.localeCompare(right.name));
        }
        return copy;
    }

    readonly property var routeNodes: {
        const group = currentRouteGroup();
        return group ? sortedNodes(group.nodes || [], routeSort) : [];
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
            if (systemProxyUsable) {
                session.setSystemProxyEnabled(!session.systemProxyEnabled);
            }
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
        Component.onCompleted: {
            refresh();
            refreshLists();
        }
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
        onTriggered: {
            session.refresh();
            if (root.expanded) {
                session.refreshLists();
            }
        }
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
        enabled: root.systemProxyUsable
        checkable: true
        checked: session.systemProxyEnabled
        onTriggered: {
            if (root.systemProxyUsable) {
                session.setSystemProxyEnabled(!session.systemProxyEnabled);
            }
        }
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
        id: popupRoot

        readonly property real contentHeight: popupColumn.implicitHeight + Kirigami.Units.largeSpacing * 2
        readonly property real popupLimit: {
            const available = root.availableScreenRect ? root.availableScreenRect.height : 0;
            return available > Kirigami.Units.gridUnit * 8 ? available * 0.72 : Kirigami.Units.gridUnit * 28;
        }
        readonly property real popupHeight: Math.min(Math.max(contentHeight, Kirigami.Units.gridUnit * 8), popupLimit)

        implicitWidth: Kirigami.Units.gridUnit * 20
        implicitHeight: popupHeight
        Layout.preferredWidth: implicitWidth
        Layout.minimumWidth: Kirigami.Units.gridUnit * 16
        Layout.maximumWidth: Kirigami.Units.gridUnit * 24
        Layout.preferredHeight: popupHeight
        Layout.minimumHeight: popupHeight
        Layout.maximumHeight: popupHeight

        Flickable {
            id: flick
            anchors.fill: parent
            anchors.leftMargin: Kirigami.Units.largeSpacing
            anchors.rightMargin: Kirigami.Units.largeSpacing
            anchors.topMargin: Kirigami.Units.largeSpacing
            anchors.bottomMargin: Kirigami.Units.largeSpacing
            clip: true
            contentWidth: width
            contentHeight: popupColumn.implicitHeight
            boundsBehavior: Flickable.StopAtBounds
            flickableDirection: Flickable.VerticalFlick

            PC3.ScrollBar.vertical: PC3.ScrollBar {
                policy: PC3.ScrollBar.AsNeeded
            }

            ColumnLayout {
                id: popupColumn
                width: flick.width
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

            PC3.Label {
                text: session.tunEnabled ? i18n("TUN is on.") : i18n("TUN is off.")
                font: Kirigami.Theme.smallFont
                opacity: 0.75
                Layout.fillWidth: true
            }

            RowLayout {
                Layout.fillWidth: true
                spacing: Kirigami.Units.gridUnit * 2

                PC3.Switch {
                    text: i18n("Core")
                    checked: session.coreDesired
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

                Item {
                    Layout.fillWidth: true
                    Layout.minimumWidth: Kirigami.Units.gridUnit
                }

                Item {
                    implicitWidth: proxySwitch.implicitWidth
                    implicitHeight: proxySwitch.implicitHeight

                    PC3.Switch {
                        id: proxySwitch
                        anchors.fill: parent
                        text: i18n("System proxy")
                        checked: session.systemProxyEnabled
                        enabled: root.systemProxyUsable
                        onToggled: {
                            if (checked !== session.systemProxyEnabled) {
                                session.setSystemProxyEnabled(checked);
                            }
                        }
                    }

                    HoverHandler {
                        id: proxyHover
                    }

                    PC3.ToolTip.visible: proxyHover.hovered
                    PC3.ToolTip.delay: Kirigami.Units.longDuration
                    PC3.ToolTip.text: i18n("This takes effect only while the core is running and TUN is off.")
                }
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

            PC3.Label {
                visible: session.profiles.length > 0
                text: i18n("Profile")
                font: Kirigami.Theme.smallFont
                opacity: 0.75
                Layout.fillWidth: true
                Layout.topMargin: Kirigami.Units.smallSpacing
            }

            PC3.ComboBox {
                visible: session.profiles.length > 0
                Layout.fillWidth: true
                model: session.profiles
                textRole: "name"
                currentIndex: {
                    for (let i = 0; i < session.profiles.length; ++i) {
                        if (session.profiles[i].id === session.activeProfile) {
                            return i;
                        }
                    }
                    return 0;
                }
                onActivated: index => {
                    const id = session.profiles[index].id;
                    if (id !== session.activeProfile) {
                        session.setActiveProfile(id);
                    }
                }
            }

            PC3.Label {
                visible: root.selectorGroups.length > 0
                text: i18n("Route")
                font: Kirigami.Theme.smallFont
                opacity: 0.75
                Layout.fillWidth: true
                Layout.topMargin: Kirigami.Units.smallSpacing
            }

            PC3.ComboBox {
                visible: root.selectorGroups.length > 1
                Layout.fillWidth: true
                model: root.selectorGroups
                textRole: "name"
                currentIndex: {
                    const group = root.currentRouteGroup();
                    if (!group) {
                        return 0;
                    }
                    for (let i = 0; i < root.selectorGroups.length; ++i) {
                        if (root.selectorGroups[i].name === group.name) {
                            return i;
                        }
                    }
                    return 0;
                }
                onActivated: index => {
                    root.routeGroup = root.selectorGroups[index].name;
                }
            }

            Repeater {
                model: root.routeNodes
                delegate: RowLayout {
                    required property var modelData
                    Layout.fillWidth: true
                    spacing: Kirigami.Units.largeSpacing

                    PC3.RadioButton {
                        text: modelData.name
                        clip: true
                        checked: {
                            const group = root.currentRouteGroup();
                            return !!group && group.now === modelData.name;
                        }
                        Layout.fillWidth: true
                        onToggled: {
                            const group = root.currentRouteGroup();
                            if (checked && group && group.now !== modelData.name) {
                                session.selectProxy(group.name, modelData.name);
                            }
                        }
                    }

                    PC3.Label {
                        readonly property int delayMs: modelData["delay_ms"]
                        visible: delayMs >= 0
                        text: delayMs === 0 ? i18n("Timeout") : i18n("%1 ms", delayMs)
                        color: root.delayColor(delayMs)
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
}
