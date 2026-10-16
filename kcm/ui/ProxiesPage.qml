import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

Item {
    id: root

    property string selectedGroup: ""
    property string autoTested: ""
    property string loadedProfile: ""
    property string sourceKey: ""
    property bool sawAutoTest: false
    property string groupSignature: ""
    property string nodeSignature: ""
    property string groupNow: ""
    property string groupType: ""

    Tx {
        id: tx
    }

    ListModel {
        id: groupModel
    }

    ListModel {
        id: nodeModel
    }

    function delayText(ms) {
        if (ms < 0) {
            return tx.i18n("Not tested");
        }
        if (ms === 0) {
            return tx.i18n("Timeout");
        }
        return tx.i18n("%1 ms", ms);
    }

    function delayColor(ms) {
        if (ms < 0) {
            return Kirigami.Theme.disabledTextColor;
        }
        if (ms === 0) {
            return Kirigami.Theme.negativeTextColor;
        }
        const t = Math.max(0, Math.min(1, (ms - 80) / 520));
        return Qt.hsla(0.33 * (1 - t), 0.72, Kirigami.Theme.textColor.hslLightness, 1);
    }

    function sourceGroups() {
        if (kcm.client.coreRunning && kcm.client.draftActiveProfile === kcm.client.activeProfile) {
            return kcm.client.groups;
        }
        return kcm.client.profileGroups(kcm.client.draftActiveProfile);
    }

    function viewPref() {
        if (kcm.client.draftSortGlobal) {
            return {
                mode: kcm.client.sortMode || "profile",
                show: kcm.client.showUnreachable
            };
        }
        const stored = kcm.client.sortPrefs[kcm.client.draftActiveProfile + "\n" + root.selectedGroup];
        if (stored && stored.mode) {
            return {
                mode: stored.mode,
                show: stored.show_unreachable !== false
            };
        }
        return {
            mode: "profile",
            show: true
        };
    }

    function rememberSort(mode, show) {
        kcm.client.setViewPref(kcm.client.draftActiveProfile, root.selectedGroup, mode, show, kcm.client.draftSortGlobal);
        root.resort();
    }

    function chosenName(group) {
        const draft = kcm.client.draftSelections[group.name];
        const nodes = group.nodes || [];
        if (typeof draft === "string" && draft.length > 0) {
            for (let i = 0; i < nodes.length; ++i) {
                if (nodes[i].name === draft) {
                    return draft;
                }
            }
        }
        return group.now || "";
    }

    function currentGroup() {
        const groups = root.sourceGroups();
        for (let i = 0; i < groups.length; ++i) {
            if (groups[i].name === root.selectedGroup) {
                return groups[i];
            }
        }
        return groups.length > 0 ? groups[0] : null;
    }

    function compareNodes(left, right) {
        if (root.viewPref().mode === "name") {
            return left.name.localeCompare(right.name);
        }
        if (root.viewPref().mode === "delay") {
            function rank(ms) {
                if (ms < 0) {
                    return 1e15;
                }
                if (ms === 0) {
                    return 1e14;
                }
                return ms;
            }
            const difference = rank(left.delay_ms) - rank(right.delay_ms);
            if (difference !== 0) {
                return difference;
            }
            return left.name.localeCompare(right.name);
        }
        return left.order - right.order;
    }

    function resort() {
        for (let i = 0; i < nodeModel.count; ++i) {
            let best = i;
            for (let j = i + 1; j < nodeModel.count; ++j) {
                if (compareNodes(nodeModel.get(j), nodeModel.get(best)) < 0) {
                    best = j;
                }
            }
            if (best !== i) {
                nodeModel.move(best, i, 1);
            }
        }
    }

    function setNodeDelay(name, delay) {
        for (let i = 0; i < nodeModel.count; ++i) {
            if (nodeModel.get(i).name === name) {
                if (nodeModel.get(i).delay_ms !== delay) {
                    nodeModel.setProperty(i, "delay_ms", delay);
                }
                return;
            }
        }
    }

    function considerAutoTest() {
        if (!kcm.client.autoTestDelay || !kcm.client.coreRunning) {
            return;
        }
        if (kcm.client.draftActiveProfile !== kcm.client.activeProfile || root.selectedGroup.length === 0) {
            return;
        }
        const key = kcm.client.activeProfile + "\n" + root.selectedGroup;
        if (root.autoTested === key) {
            return;
        }
        root.autoTested = key;
        kcm.client.testGroupDelay(root.selectedGroup);
    }

    function syncFromClient() {
        const groups = root.sourceGroups();
        if (groups.length === 0) {
            groupModel.clear();
            nodeModel.clear();
            root.groupSignature = "";
            root.nodeSignature = "";
            root.selectedGroup = "";
            return;
        }
        const signature = groups.map(group => group.name).join("\n");
        if (signature !== root.groupSignature) {
            root.groupSignature = signature;
            const keep = root.selectedGroup;
            groupModel.clear();
            for (let i = 0; i < groups.length; ++i) {
                groupModel.append({
                    name: groups[i].name
                });
            }
            let index = 0;
            for (let i = 0; i < groups.length; ++i) {
                if (groups[i].name === keep) {
                    index = i;
                    break;
                }
            }
            root.selectedGroup = groups[index].name;
            groupBox.currentIndex = index;
        }
        const group = root.currentGroup();
        if (!group) {
            return;
        }
        root.groupNow = root.chosenName(group);
        root.groupType = group["type"];
        const names = (group.nodes || []).map(node => node.name).join("\n");
        if (names !== root.nodeSignature || root.selectedGroup !== group.name) {
            root.nodeSignature = names;
            nodeModel.clear();
            const nodes = group.nodes || [];
            for (let i = 0; i < nodes.length; ++i) {
                nodeModel.append({
                    name: nodes[i].name,
                    delay_ms: nodes[i].delay_ms,
                    order: i
                });
            }
        } else {
            const nodes = group.nodes || [];
            for (let i = 0; i < nodes.length; ++i) {
                root.setNodeDelay(nodes[i].name, nodes[i].delay_ms);
            }
        }
        root.resort();
        root.considerAutoTest();
    }

    function chooseNode(name) {
        if (root.groupType !== "Selector" || name === root.groupNow) {
            return;
        }
        const next = Object.assign({}, kcm.client.draftSelections);
        next[root.selectedGroup] = name;
        kcm.client.draftSelections = next;
        root.groupNow = name;
    }

    Connections {
        target: kcm.client
        function onProxiesChanged() {
            root.syncFromClient();
        }
        function onDelayUpdated(group, name, delayMs) {
            if (group !== root.selectedGroup) {
                return;
            }
            root.setNodeDelay(name, delayMs);
            if (root.viewPref().mode === "delay") {
                root.resort();
            }
        }
        function onDraftChanged() {
            if (root.loadedProfile !== kcm.client.draftActiveProfile) {
                root.loadedProfile = kcm.client.draftActiveProfile;
                root.groupSignature = "";
                root.nodeSignature = "";
            }
            root.syncFromClient();
        }
        function onStatusChanged() {
            const key = (kcm.client.coreRunning ? "1" : "0") + "\n" + kcm.client.activeProfile;
            const turnedOn = kcm.client.autoTestDelay && !root.sawAutoTest;
            root.sawAutoTest = kcm.client.autoTestDelay;
            if (root.sourceKey !== key) {
                root.sourceKey = key;
                root.groupSignature = "";
                root.nodeSignature = "";
                root.autoTested = "";
                root.syncFromClient();
                return;
            }
            if (turnedOn) {
                root.autoTested = "";
                root.considerAutoTest();
            }
        }
    }

    Component.onCompleted: syncFromClient()

    ColumnLayout {
        anchors.fill: parent
        spacing: Kirigami.Units.largeSpacing

        Kirigami.InlineMessage {
            Layout.fillWidth: true
            visible: !kcm.client.coreRunning
            type: Kirigami.MessageType.Warning
            text: tx.i18n("Core is not running")
            showCloseButton: false
        }

        QQC2.Label {
            visible: root.selectedGroup.length === 0
            text: tx.i18n("No proxy groups")
            opacity: 0.7
            Layout.fillWidth: true
        }

        RowLayout {
            Layout.fillWidth: true
            visible: root.selectedGroup.length > 0
            spacing: Kirigami.Units.largeSpacing

            QQC2.Label {
                text: tx.i18n("Group")
            }

            QQC2.ComboBox {
                id: groupBox
                model: groupModel
                textRole: "name"
                Layout.fillWidth: true
                Layout.maximumWidth: Kirigami.Units.gridUnit * 20
                onActivated: {
                    root.selectedGroup = currentText;
                    root.nodeSignature = "";
                    root.syncFromClient();
                }
            }

            Item {
                Layout.fillWidth: true
            }

            QQC2.Button {
                text: tx.i18n("Test delays")
                enabled: kcm.client.coreRunning && kcm.client.draftActiveProfile === kcm.client.activeProfile && !kcm.client.delayTesting && root.selectedGroup.length > 0
                onClicked: kcm.client.testGroupDelay(root.selectedGroup)
            }

            QQC2.ToolButton {
                id: sortButton
                icon.name: "view-sort"
                display: QQC2.AbstractButton.IconOnly
                onClicked: sortMenu.popup(sortButton, 0, height)

                QQC2.ToolTip.visible: hovered
                QQC2.ToolTip.delay: 700
                QQC2.ToolTip.text: tx.i18n("Sort and filter")

                QQC2.Menu {
                    id: sortMenu
                    QQC2.MenuItem {
                        text: tx.i18n("Profile order")
                        checkable: true
                        checked: root.viewPref().mode === "profile"
                        onTriggered: {
                            root.rememberSort("profile", root.viewPref().show);
                            checked = Qt.binding(() => root.viewPref().mode === "profile");
                        }
                    }
                    QQC2.MenuItem {
                        text: tx.i18n("Name")
                        checkable: true
                        checked: root.viewPref().mode === "name"
                        onTriggered: {
                            root.rememberSort("name", root.viewPref().show);
                            checked = Qt.binding(() => root.viewPref().mode === "name");
                        }
                    }
                    QQC2.MenuItem {
                        text: tx.i18n("Delay")
                        checkable: true
                        checked: root.viewPref().mode === "delay"
                        onTriggered: {
                            root.rememberSort("delay", root.viewPref().show);
                            checked = Qt.binding(() => root.viewPref().mode === "delay");
                        }
                    }
                    QQC2.MenuSeparator {}
                    QQC2.MenuItem {
                        text: tx.i18n("Show unreachable")
                        checkable: true
                        checked: root.viewPref().show
                        onTriggered: {
                            root.rememberSort(root.viewPref().mode, !root.viewPref().show);
                            checked = Qt.binding(() => root.viewPref().show);
                        }
                    }
                }
            }
        }

        Kirigami.Separator {
            Layout.fillWidth: true
            visible: root.selectedGroup.length > 0
        }

        ListView {
            Layout.fillWidth: true
            Layout.fillHeight: true
            clip: true
            visible: root.selectedGroup.length > 0
            model: nodeModel

            delegate: QQC2.RadioDelegate {
                id: nodeDelegate
                required property string name
                required property var delay_ms
                width: ListView.view ? ListView.view.width : 0
                visible: root.viewPref().show || delay_ms !== 0
                height: visible ? implicitHeight : 0
                text: name
                checked: root.groupNow === name
                enabled: root.groupType === "Selector"
                onClicked: {
                    root.chooseNode(name);
                    checked = Qt.binding(() => root.groupNow === name);
                }

                contentItem: RowLayout {
                    spacing: Kirigami.Units.largeSpacing

                    QQC2.Label {
                        text: nodeDelegate.text
                        elide: Text.ElideRight
                        Layout.fillWidth: true
                    }
                    QQC2.Label {
                        text: root.delayText(nodeDelegate.delay_ms)
                        color: root.delayColor(nodeDelegate.delay_ms)
                    }
                }
            }
        }
    }
}
