import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

Item {
    id: root

    property string sortMode: "profile"
    property bool showUnreachable: true
    property string selectedGroup: ""
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

    function currentGroup() {
        const groups = kcm.client.groups;
        for (let i = 0; i < groups.length; ++i) {
            if (groups[i].name === root.selectedGroup) {
                return groups[i];
            }
        }
        return groups.length > 0 ? groups[0] : null;
    }

    function compareNodes(left, right) {
        if (root.sortMode === "name") {
            return left.name.localeCompare(right.name);
        }
        if (root.sortMode === "delay") {
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

    function syncFromClient() {
        const groups = kcm.client.groups;
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
        root.groupNow = group.now;
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
    }

    function chooseNode(name) {
        if (root.groupType !== "Selector" || name === root.groupNow) {
            return;
        }
        root.groupNow = name;
        kcm.client.selectProxy(root.selectedGroup, name);
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
            if (root.sortMode === "delay") {
                root.resort();
            }
        }
    }

    Component.onCompleted: syncFromClient()

    onSortModeChanged: resort()

    ColumnLayout {
        anchors.fill: parent
        spacing: Kirigami.Units.largeSpacing

        QQC2.Label {
            visible: kcm.client.groups.length === 0
            text: kcm.client.coreRunning ? tx.i18n("No proxy groups") : tx.i18n("Core is not running")
            opacity: 0.7
            Layout.fillWidth: true
        }

        RowLayout {
            Layout.fillWidth: true
            visible: kcm.client.groups.length > 0
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
                enabled: kcm.client.coreRunning && !kcm.client.delayTesting && root.selectedGroup.length > 0
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
                        checked: root.sortMode === "profile"
                        onTriggered: {
                            root.sortMode = "profile";
                            checked = Qt.binding(() => root.sortMode === "profile");
                        }
                    }
                    QQC2.MenuItem {
                        text: tx.i18n("Name")
                        checkable: true
                        checked: root.sortMode === "name"
                        onTriggered: {
                            root.sortMode = "name";
                            checked = Qt.binding(() => root.sortMode === "name");
                        }
                    }
                    QQC2.MenuItem {
                        text: tx.i18n("Delay")
                        checkable: true
                        checked: root.sortMode === "delay"
                        onTriggered: {
                            root.sortMode = "delay";
                            checked = Qt.binding(() => root.sortMode === "delay");
                        }
                    }
                    QQC2.MenuSeparator {}
                    QQC2.MenuItem {
                        text: tx.i18n("Show unreachable")
                        checkable: true
                        checked: root.showUnreachable
                        onTriggered: {
                            root.showUnreachable = !root.showUnreachable;
                            checked = Qt.binding(() => root.showUnreachable);
                        }
                    }
                }
            }
        }

        Kirigami.Separator {
            Layout.fillWidth: true
            visible: kcm.client.groups.length > 0
        }

        ListView {
            Layout.fillWidth: true
            Layout.fillHeight: true
            clip: true
            visible: kcm.client.groups.length > 0
            model: nodeModel

            delegate: QQC2.RadioDelegate {
                id: nodeDelegate
                required property string name
                required property var delay_ms
                width: ListView.view ? ListView.view.width : 0
                visible: root.showUnreachable || delay_ms !== 0
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
