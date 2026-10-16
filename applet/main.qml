/*
    SPDX-FileCopyrightText: 2026 clash-kde contributors
    SPDX-License-Identifier: GPL-3.0-or-later
*/

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls as QQC2
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

    onExpandedChanged: {
        if (!expanded) {
            routeMenu.close();
        }
    }

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

    // GLOBAL only carries traffic while the mode is global. In rule mode it is a
    // synthetic list, and choosing a node there leaves the real group unchanged.
    readonly property var selectorGroups: {
        const groups = session.groups;
        const out = [];
        for (let i = 0; i < groups.length; ++i) {
            if (groups[i]["type"] !== "Selector") {
                continue;
            }
            const synthetic = groups[i].name === "GLOBAL";
            if (session.mode === "global" ? !synthetic : synthetic) {
                continue;
            }
            out.push(groups[i]);
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
    readonly property bool showUnreachable: Plasmoid.configuration.showUnreachable !== false

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

    function nodesFor(group) {
        const nodes = sortedNodes(group.nodes || [], routeSort);
        if (showUnreachable) {
            return nodes;
        }
        return nodes.filter(node => node["delay_ms"] !== 0);
    }

    // Size from the row count. A height bound to implicitHeight collapses this menu.
    function fitRouteMenu(menu, count) {
        const row = Math.ceil(Kirigami.Units.gridUnit * 1.8);
        const available = root.availableScreenRect && root.availableScreenRect.height > row * 4
                ? root.availableScreenRect.height : Kirigami.Units.gridUnit * 28;
        const limit = Math.max(row * 6, available - Kirigami.Units.gridUnit * 2);
        const natural = Math.max(row, count * row + Kirigami.Units.smallSpacing * 2);
        menu.height = Math.min(natural, limit);
        Qt.callLater(() => {
            const item = menu.contentItem;
            if (!item) {
                return;
            }
            item.clip = true;
            const measured = item.contentHeight + menu.topPadding + menu.bottomPadding;
            if (measured > menu.height && menu.height < limit) {
                menu.height = Math.min(measured, limit);
            }
            if (item.contentHeight > item.height + 1) {
                item.interactive = true;
            }
        });
    }

    readonly property var singleGroupNodes: selectorGroups.length === 1 ? nodesFor(selectorGroups[0]) : []

    function perform(action) {
        switch (action) {
        case "popup":
            expanded = !expanded;
            break;
        case "toggleCore":
            session.setCoreDesired(!session.coreDesired);
            break;
        case "toggleSystemProxy":
            if (session.systemProxyEnabled || systemProxyUsable) {
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

    readonly property color stateColor: {
        if (session.coreRunning) {
            return Kirigami.Theme.positiveTextColor;
        }
        if (session.coreDesired) {
            return Kirigami.Theme.neutralTextColor;
        }
        return Kirigami.Theme.negativeTextColor;
    }
    readonly property string stateText: {
        if (session.coreRunning) {
            return i18n("Core is running");
        }
        if (session.coreDesired) {
            return i18n("Core is starting");
        }
        return i18n("Core is stopped");
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
        enabled: session.systemProxyEnabled || root.systemProxyUsable
        checkable: true
        checked: session.systemProxyEnabled
        onTriggered: {
            if (session.systemProxyEnabled || root.systemProxyUsable) {
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
        Layout.minimumWidth: implicitWidth
        Layout.maximumWidth: implicitWidth
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

            RowLayout {
                Layout.fillWidth: true
                spacing: Kirigami.Units.smallSpacing

                Rectangle {
                    Layout.alignment: Qt.AlignVCenter
                    implicitWidth: Kirigami.Units.largeSpacing
                    implicitHeight: implicitWidth
                    radius: width / 2
                    color: root.stateColor
                }

                PC3.Label {
                    text: root.stateText
                    color: root.stateColor
                    font.bold: true
                    font.pointSize: Kirigami.Theme.defaultFont.pointSize + 1
                    Layout.fillWidth: true
                }
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
                        enabled: session.systemProxyEnabled || root.systemProxyUsable
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

            PC3.Button {
                id: routeButton
                visible: root.selectorGroups.length > 0
                Layout.fillWidth: true
                Layout.topMargin: Kirigami.Units.smallSpacing
                text: {
                    const group = root.currentRouteGroup();
                    return group && group.now.length > 0 ? i18n("Route: %1", group.now) : i18n("Route");
                }
                onClicked: {
                    if (routeMenu.opened) {
                        routeMenu.close();
                    } else {
                        // Opens to the right of the button, as its own window, so it is not clipped by this popup.
                        routeMenu.popup(routeButton, routeButton.width, 0);
                    }
                }
            }

            RowLayout {
                Layout.fillWidth: true
                spacing: Kirigami.Units.smallSpacing

                PC3.Button {
                    text: i18n("Settings")
                    icon.name: "preferences-system-network"
                    Layout.fillWidth: true
                    onClicked: root.perform("openSettings")
                }

                PC3.Button {
                    id: copyProxyButton
                    property bool copied: false
                    text: copied ? i18n("Copied") : i18n("Copy command")
                    icon.name: copied ? "dialog-ok" : "edit-copy"
                    Layout.fillWidth: true
                    onClicked: {
                        if (session.copyTerminalProxyCommand()) {
                            copied = true;
                            copyReset.restart();
                        }
                    }

                    Timer {
                        id: copyReset
                        interval: 2000
                        onTriggered: copyProxyButton.copied = false
                    }

                    HoverHandler {
                        id: copyHover
                    }

                    PC3.ToolTip.visible: copyHover.hovered && !copyProxyButton.copied
                    PC3.ToolTip.delay: Kirigami.Units.longDuration
                    PC3.ToolTip.text: i18n("Copy a command that sets this proxy in the terminal.")
                }
            }
        }
        }

        component RouteMenuItem: PC3.MenuItem {
            id: nodeItem
            required property var modelData
            required property string groupName
            required property string currentName

            readonly property int delayMs: modelData["delay_ms"]
            readonly property bool current: currentName === modelData.name

            // The stock check mark is drawn under the label. Draw our own, in its own column.
            checkable: false
            onTriggered: session.selectProxy(groupName, modelData.name)

            leftPadding: Kirigami.Units.largeSpacing
            rightPadding: Kirigami.Units.gridUnit + Kirigami.Units.smallSpacing
            topPadding: Kirigami.Units.mediumSpacing
            bottomPadding: Kirigami.Units.mediumSpacing

            contentItem: RowLayout {
                spacing: Kirigami.Units.largeSpacing

                Item {
                    implicitWidth: Kirigami.Units.iconSizes.small
                    implicitHeight: implicitWidth
                    Layout.preferredWidth: implicitWidth
                    Layout.preferredHeight: implicitHeight
                    Layout.alignment: Qt.AlignVCenter

                    Kirigami.Icon {
                        anchors.fill: parent
                        source: "checkmark"
                        visible: nodeItem.current
                    }
                }

                Item {
                    readonly property int nameWidth: Math.max(Kirigami.Units.gridUnit * 8, Math.min(nameMetrics.advanceWidth, Kirigami.Units.gridUnit * 12))
                    implicitWidth: nameWidth
                    implicitHeight: nameLabel.implicitHeight
                    Layout.fillWidth: true
                    Layout.preferredWidth: nameWidth
                    Layout.minimumWidth: Kirigami.Units.gridUnit * 8
                    Layout.alignment: Qt.AlignVCenter

                    TextMetrics {
                        id: nameMetrics
                        font: nameLabel.font
                        text: nameLabel.text
                    }

                    PC3.Label {
                        id: nameLabel
                        anchors.fill: parent
                        text: nodeItem.modelData.name
                        elide: Text.ElideRight
                        font.bold: nodeItem.current
                        verticalAlignment: Text.AlignVCenter
                    }
                }

                PC3.Label {
                    visible: nodeItem.delayMs >= 0
                    text: nodeItem.delayMs === 0 ? i18n("Timeout") : i18n("%1 ms", nodeItem.delayMs)
                    color: root.delayColor(nodeItem.delayMs)
                    horizontalAlignment: Text.AlignRight
                    verticalAlignment: Text.AlignVCenter
                    Layout.alignment: Qt.AlignVCenter
                }
            }
        }

        PC3.Menu {
            id: routeMenu
            // Popup.Item is clipped to the applet window. Popup.Window is the stock menu window.
            popupType: QQC2.Popup.Window
            cascade: true
            readonly property int entryCount: root.selectorGroups.length > 1 ? root.selectorGroups.length : root.singleGroupNodes.length
            onAboutToShow: root.fitRouteMenu(routeMenu, entryCount)

            Instantiator {
                model: root.singleGroupNodes
                delegate: RouteMenuItem {
                    groupName: root.selectorGroups.length === 1 ? root.selectorGroups[0].name : ""
                    currentName: root.selectorGroups.length === 1 ? root.selectorGroups[0].now : ""
                }
                onObjectAdded: (index, object) => routeMenu.insertItem(index, object)
                onObjectRemoved: (index, object) => routeMenu.removeItem(object)
            }

            Instantiator {
                model: root.selectorGroups.length > 1 ? root.selectorGroups : []
                delegate: PC3.Menu {
                    id: groupMenu
                    required property var modelData
                    title: modelData.name
                    popupType: QQC2.Popup.Window
                    cascade: true
                    onAboutToShow: root.fitRouteMenu(groupMenu, root.nodesFor(groupMenu.modelData).length)
                    Instantiator {
                        model: root.nodesFor(groupMenu.modelData)
                        delegate: RouteMenuItem {
                            groupName: groupMenu.modelData.name
                            currentName: groupMenu.modelData.now
                        }
                        onObjectAdded: (index, object) => groupMenu.insertItem(index, object)
                        onObjectRemoved: (index, object) => groupMenu.removeItem(object)
                    }
                }
                onObjectAdded: (index, object) => routeMenu.insertMenu(index, object)
                onObjectRemoved: (index, object) => routeMenu.removeMenu(object)
            }
        }
    }
}
