import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

Item {
    id: root

    property string selectedGroup: ""
    property string autoTested: ""
    property string loadedProfile: ""
    property string loadedMode: ""
    property string sourceKey: ""
    property bool sawAutoTest: false
    property string groupSignature: ""
    property string nodeSignature: ""
    property string groupNow: ""
    property string groupType: ""
    property string nodeQuery: ""
    property string picked: ""
    property bool detailsOpen: false
    property var detail: ({})

    function refreshDetail() {
        root.detail = root.picked.length === 0 ? {} : kcm.client.routeDetail(root.picked);
        detailName.text = root.detail.name || "";
        detailKind.currentIndex = Math.max(0, detailKind.indexOfValue(String(root.detail.kind || "http").toLowerCase()));
        detailServer.text = root.detail.server || "";
        detailPort.value = root.detail.port > 0 ? root.detail.port : 443;
        detailSecret.text = root.detail.secret || "";
    }

    Tx {
        id: tx
    }

    Fuzzy {
        id: fuzzy
    }

    function nodeMatches(name) {
        return fuzzy.score(root.nodeQuery, name) >= 0;
    }

    readonly property bool anyNodeShown: {
        const query = root.nodeQuery;
        const signature = root.nodeSignature;
        const showUnreachable = root.viewPref().show;
        if (query.trim().length === 0) {
            return signature.length >= 0;
        }
        for (let i = 0; i < nodeModel.count; ++i) {
            const row = nodeModel.get(i);
            if ((showUnreachable || row.delay_ms !== 0) && fuzzy.score(query, row.name) >= 0) {
                return true;
            }
        }
        return false;
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
        const groups = kcm.client.coreRunning && kcm.client.draftActiveProfile === kcm.client.activeProfile
                ? kcm.client.groups : kcm.client.profileGroups(kcm.client.draftActiveProfile);
        // GLOBAL only carries traffic while the mode is global. Rule mode uses the groups from the profile.
        const out = [];
        for (let i = 0; i < groups.length; ++i) {
            const synthetic = groups[i].name === "GLOBAL";
            if (kcm.client.draftMode === "global" ? !synthetic : synthetic) {
                continue;
            }
            out.push(groups[i]);
        }
        return out.length > 0 ? out : groups;
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
            let index = -1;
            for (let i = 0; i < groups.length; ++i) {
                if (groups[i].name === keep) {
                    index = i;
                    break;
                }
            }
            if (index < 0) {
                for (let i = 0; i < groups.length; ++i) {
                    if (groups[i]["type"] === "Selector") {
                        index = i;
                        break;
                    }
                }
            }
            if (index < 0) {
                index = 0;
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
                if (nodes[i].delay_ms < 0) {
                    continue;
                }
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
            if (root.loadedProfile !== kcm.client.draftActiveProfile || root.loadedMode !== kcm.client.draftMode) {
                root.loadedProfile = kcm.client.draftActiveProfile;
                root.loadedMode = kcm.client.draftMode;
                root.groupSignature = "";
                root.nodeSignature = "";
                root.syncFromClient();
                return;
            }
            const group = root.currentGroup();
            if (group) {
                root.groupNow = root.chosenName(group);
            }
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

        Item {
            Layout.fillWidth: true
            Layout.fillHeight: true
            visible: root.selectedGroup.length > 0

            Kirigami.SearchField {
                id: routeSearch
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.top: parent.top
                onTextChanged: root.nodeQuery = text
                Keys.onDownPressed: event => {
                    routeList.currentIndex = 0;
                    event.accepted = false;
                }
                KeyNavigation.down: routeScroll
            }

            QQC2.Label {
                anchors.top: routeSearch.bottom
                anchors.topMargin: Kirigami.Units.largeSpacing
                anchors.left: parent.left
                visible: root.nodeQuery.trim().length > 0 && !root.anyNodeShown
                text: tx.i18n("No matches")
                opacity: 0.7
            }

            QQC2.ScrollView {
                id: routeScroll
                Kirigami.StyleHints.showFramedBackground: true
                anchors.top: routeSearch.bottom
                anchors.topMargin: Kirigami.Units.smallSpacing
                anchors.left: parent.left
                anchors.bottom: buttonRow.top
                anchors.bottomMargin: Kirigami.Units.smallSpacing
                anchors.right: detailPane.left
                anchors.rightMargin: root.detailsOpen ? Kirigami.Units.largeSpacing : 0
                visible: root.anyNodeShown || root.nodeQuery.trim().length === 0

                ListView {
                    id: routeList
                    clip: true
                    focus: true
                    model: nodeModel
                    boundsBehavior: Flickable.StopAtBounds
                    KeyNavigation.up: routeSearch

                    delegate: QQC2.ItemDelegate {
                        id: nodeDelegate
                        required property string name
                        required property var delay_ms
                        width: ListView.view ? ListView.view.width : 0
                        visible: (root.viewPref().show || delay_ms !== 0) && root.nodeMatches(name)
                        height: visible ? implicitHeight : 0
                        highlighted: root.groupNow === name
                        onClicked: {
                            root.picked = name;
                            root.refreshDetail();
                            root.chooseNode(name);
                        }

                        contentItem: Kirigami.IconTitleSubtitle {
                            title: nodeDelegate.name
                            subtitle: root.delayText(nodeDelegate.delay_ms)
                            selected: nodeDelegate.highlighted
                        }
                    }
                }
            }

            ColumnLayout {
                id: detailPane
                anchors.top: routeSearch.bottom
                anchors.topMargin: Kirigami.Units.smallSpacing
                anchors.right: parent.right
                anchors.bottom: buttonRow.top
                width: root.detailsOpen ? Kirigami.Units.gridUnit * 16 : 0
                visible: root.detailsOpen
                spacing: Kirigami.Units.smallSpacing

                QQC2.Label {
                    text: tx.i18n("Route")
                    Layout.fillWidth: true
                }

                QQC2.TextField {
                    id: detailName
                    Layout.fillWidth: true
                    enabled: root.detail.in_file === true
                    placeholderText: tx.i18n("Name")
                }

                QQC2.ComboBox {
                    id: detailKind
                    Layout.fillWidth: true
                    enabled: root.detail.in_file === true
                    model: ["http", "socks5", "ss", "vmess", "trojan"]
                }

                QQC2.TextField {
                    id: detailServer
                    Layout.fillWidth: true
                    enabled: root.detail.in_file === true
                    placeholderText: tx.i18n("Server")
                }

                QQC2.SpinBox {
                    id: detailPort
                    Layout.fillWidth: true
                    enabled: root.detail.in_file === true
                    from: 1
                    to: 65535
                    editable: true
                }

                QQC2.TextField {
                    id: detailSecret
                    Layout.fillWidth: true
                    enabled: root.detail.in_file === true
                    echoMode: TextInput.Password
                    placeholderText: tx.i18n("Password")
                }

                QQC2.Button {
                    text: tx.i18n("Save")
                    enabled: root.detail.in_file === true && root.picked.length > 0
                    Layout.alignment: Qt.AlignRight
                    onClicked: {
                        if (kcm.client.saveRoute(root.picked, detailName.text, detailKind.currentText, detailServer.text, detailPort.value, detailSecret.text)) {
                            root.picked = detailName.text;
                            root.refreshDetail();
                        }
                    }
                }
            }

            Row {
                id: detailButtonRow
                anchors.left: parent.left
                anchors.bottom: parent.bottom
                spacing: Kirigami.Units.smallSpacing

                QQC2.ToolButton {
                    icon.name: root.detailsOpen ? "view-right-close" : "view-right-new"
                    display: QQC2.AbstractButton.IconOnly
                    text: QQC2.ToolTip.text
                    onClicked: root.detailsOpen = !root.detailsOpen
                    QQC2.ToolTip.text: root.detailsOpen ? tx.i18n("Hide details") : tx.i18n("Show details")
                    QQC2.ToolTip.visible: hovered
                }
            }

            Row {
                id: buttonRow
                anchors.right: parent.right
                anchors.bottom: parent.bottom
                spacing: Kirigami.Units.smallSpacing

                QQC2.ToolButton {
                    icon.name: "list-add"
                    display: QQC2.AbstractButton.IconOnly
                    text: QQC2.ToolTip.text
                    enabled: root.selectedGroup.length > 0
                    onClicked: addDialog.open()
                    QQC2.ToolTip.text: tx.i18n("Adds a route to the current profile, which a subscription update replaces.")
                    QQC2.ToolTip.visible: hovered
                }

                QQC2.ToolButton {
                    icon.name: "list-remove"
                    display: QQC2.AbstractButton.IconOnly
                    text: QQC2.ToolTip.text
                    enabled: root.detail.in_file === true
                    onClicked: removeDialog.open()
                    QQC2.ToolTip.text: tx.i18n("Removes the selected route from the current profile, which a subscription update replaces.")
                    QQC2.ToolTip.visible: hovered
                }
            }
        }
    }

    Kirigami.PromptDialog {
        id: removeDialog
        standardButtons: Kirigami.Dialog.Ok | Kirigami.Dialog.Cancel
        title: tx.i18n("Remove this route?")
        subtitle: root.picked
        onAccepted: {
            kcm.client.removeRoute(root.picked);
            root.picked = "";
            root.detail = {};
        }
    }

    QQC2.Dialog {
        id: addDialog
        title: tx.i18n("Add a route")
        modal: true
        standardButtons: QQC2.Dialog.Ok | QQC2.Dialog.Cancel
        anchors.centerIn: parent
        width: Kirigami.Units.gridUnit * 22

        contentItem: ColumnLayout {
            spacing: Kirigami.Units.smallSpacing

            QQC2.TextField {
                id: addName
                Layout.fillWidth: true
                placeholderText: tx.i18n("Name")
            }
            QQC2.ComboBox {
                id: addKind
                Layout.fillWidth: true
                model: ["http", "socks5", "ss", "vmess", "trojan"]
            }
            QQC2.TextField {
                id: addServer
                Layout.fillWidth: true
                placeholderText: tx.i18n("Server")
            }
            QQC2.SpinBox {
                id: addPort
                Layout.fillWidth: true
                from: 1
                to: 65535
                value: 443
                editable: true
            }
            QQC2.TextField {
                id: addSecret
                Layout.fillWidth: true
                echoMode: TextInput.Password
                placeholderText: tx.i18n("Password")
            }
        }

        onAccepted: {
            if (kcm.client.addRoute(root.selectedGroup, addName.text, addKind.currentText, addServer.text, addPort.value, addSecret.text)) {
                root.picked = addName.text;
                root.refreshDetail();
            }
        }
    }
}
