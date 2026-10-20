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

    property bool filling: false
    property bool revealSecret: false
    property bool chosenActive: false
    property string chosenMode: "profile"
    property bool chosenShow: true

    ListModel {
        id: shownModel
    }

    function showPicked() {
        if (root.picked.length === 0) {
            root.detail = {};
            return;
        }
        root.filling = true;
        const staged = kcm.client.stagedRoute(root.picked);
        root.detail = staged && staged.name ? staged : kcm.client.routeDetail(root.picked);
        detailName.text = root.detail.name || "";
        const kind = String(root.detail.kind || "http").toLowerCase();
        let kindIndex = 0;
        for (let i = 0; i < detailKind.count; ++i) {
            if (String(detailKind.textAt(i)).toLowerCase() === kind) {
                kindIndex = i;
                break;
            }
        }
        detailKind.currentIndex = kindIndex;
        detailServer.text = root.detail.server || "";
        detailPort.value = root.detail.port > 0 ? root.detail.port : 443;
        detailSecret.text = root.detail.secret || "";
        root.filling = false;
    }

    function pushRoute() {
        if (root.filling || root.detail.in_file !== true || root.picked.length === 0) {
            return;
        }
        kcm.client.stageRoute(root.picked, detailName.text, detailKind.currentText, detailServer.text, detailPort.value, detailSecret.text);
    }

    function rebuildShown() {
        const pref = root.viewPref();
        const titles = [tx.i18n("Current"), tx.i18n("Available"), tx.i18n("Timeout")];
        const rows = [];
        for (let i = 0; i < nodeModel.count; ++i) {
            const row = nodeModel.get(i);
            if ((!pref.show && row.delay_ms === 0) || !root.nodeMatches(row)) {
                continue;
            }
            rows.push({
                name: row.name,
                delay_ms: row.delay_ms,
                order: row.order
            });
        }
        rows.sort((left, right) => root.compareNodes(left, right, pref.mode));
        const buckets = [[], [], []];
        for (let i = 0; i < rows.length; ++i) {
            const row = rows[i];
            const current = row.name === root.groupNow && root.groupNow.length > 0;
            const section = current ? 0 : (row.delay_ms === 0 ? 2 : 1);
            buckets[section].push(row);
        }
        shownModel.clear();
        for (let section = 0; section < buckets.length; ++section) {
            for (let i = 0; i < buckets[section].length; ++i) {
                const row = buckets[section][i];
                shownModel.append({
                    name: row.name,
                    delay_ms: row.delay_ms,
                    section: titles[section]
                });
            }
        }
    }

    Tx {
        id: tx
    }

    Fuzzy {
        id: fuzzy
    }

    function nodeMatches(row) {
        return fuzzy.best(root.nodeQuery, [row.name, root.delayText(row.delay_ms)]) >= 0;
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
            if ((showUnreachable || row.delay_ms !== 0) && root.nodeMatches(row)) {
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
        return tx.i18n("%1 ms", tx.formatNumber(ms));
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

    function savedPref() {
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

    function viewPref() {
        if (root.chosenActive) {
            return {
                mode: root.chosenMode,
                show: root.chosenShow
            };
        }
        return root.savedPref();
    }

    function rememberSort(mode, show) {
        root.chosenMode = mode;
        root.chosenShow = show;
        root.chosenActive = true;
        kcm.client.setViewPref(kcm.client.draftActiveProfile, root.selectedGroup, mode, show, kcm.client.draftSortGlobal);
        root.rebuildShown();
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

    function compareNodes(left, right, mode) {
        if (mode === "name") {
            return String(left.name).localeCompare(String(right.name));
        }
        if (mode === "delay") {
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
            return String(left.name).localeCompare(String(right.name));
        }
        return left.order - right.order;
    }

    function resort() {
        root.rebuildShown();
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
            root.chosenActive = false;
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
        root.rebuildShown();
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
            root.rebuildShown();
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
                root.rebuildShown();
            }
        }
        function onStatusChanged() {
            if (root.chosenActive) {
                const saved = root.savedPref();
                if (saved.mode === root.chosenMode && saved.show === root.chosenShow) {
                    root.chosenActive = false;
                }
            }
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
                Kirigami.MnemonicData.enabled: false
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
                Kirigami.MnemonicData.enabled: false
                text: tx.i18n("Test delays")
                enabled: kcm.client.coreRunning && kcm.client.draftActiveProfile === kcm.client.activeProfile && !kcm.client.delayTesting && root.selectedGroup.length > 0
                onClicked: kcm.client.testGroupDelay(root.selectedGroup)
            }

            QQC2.ToolButton {
                Kirigami.MnemonicData.enabled: false
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
                        Kirigami.MnemonicData.enabled: false
                        text: tx.i18n("Profile order")
                        checkable: true
                        checked: root.viewPref().mode === "profile"
                        onTriggered: {
                            root.rememberSort("profile", root.viewPref().show);
                            checked = Qt.binding(() => root.viewPref().mode === "profile");
                        }
                    }
                    QQC2.MenuItem {
                        Kirigami.MnemonicData.enabled: false
                        text: tx.i18n("Name")
                        checkable: true
                        checked: root.viewPref().mode === "name"
                        onTriggered: {
                            root.rememberSort("name", root.viewPref().show);
                            checked = Qt.binding(() => root.viewPref().mode === "name");
                        }
                    }
                    QQC2.MenuItem {
                        Kirigami.MnemonicData.enabled: false
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
                        Kirigami.MnemonicData.enabled: false
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
                Kirigami.MnemonicData.enabled: false
                onTextChanged: {
                    root.nodeQuery = text;
                    root.rebuildShown();
                }
                Keys.onDownPressed: event => {
                    routeList.currentIndex = 0;
                    event.accepted = false;
                }
                KeyNavigation.down: routeList
            }

            QQC2.Label {
                anchors.top: routeSearch.bottom
                anchors.topMargin: Kirigami.Units.largeSpacing
                anchors.left: parent.left
                visible: root.nodeQuery.trim().length > 0 && shownModel.count === 0
                text: tx.i18n("No matches")
                opacity: 0.7
                Kirigami.MnemonicData.enabled: false
            }

            QQC2.SplitView {
                anchors.top: routeSearch.bottom
                anchors.topMargin: Kirigami.Units.smallSpacing
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.bottom: buttonRow.top
                anchors.bottomMargin: Kirigami.Units.smallSpacing
                orientation: Qt.Horizontal

                QQC2.ScrollView {
                    QQC2.SplitView.preferredWidth: Kirigami.Units.gridUnit * 16
                    QQC2.SplitView.minimumWidth: Kirigami.Units.gridUnit * 10
                    QQC2.SplitView.fillHeight: true
                    Kirigami.StyleHints.showFramedBackground: true

                    ListView {
                        id: routeList
                        clip: true
                        focus: true
                        model: shownModel
                        boundsBehavior: Flickable.StopAtBounds
                        KeyNavigation.up: routeSearch
                        section.property: "section"
                        section.criteria: ViewSection.FullString
                        section.delegate: Kirigami.ListSectionHeader {
                            required property string section
                            width: ListView.view ? ListView.view.width : 0
                            text: section
                        }

                        delegate: QQC2.ItemDelegate {
                            id: nodeDelegate
                            required property string name
                            required property var delay_ms
                            width: ListView.view ? ListView.view.width : 0
                            highlighted: root.groupNow === name
                            Kirigami.MnemonicData.enabled: false
                            onClicked: {
                                root.picked = name;
                                root.showPicked();
                                root.chooseNode(name);
                            }

                            contentItem: Kirigami.IconTitleSubtitle {
                                title: nodeDelegate.name
                                subtitle: kcm.client.compact ? "" : root.delayText(nodeDelegate.delay_ms)
                                selected: nodeDelegate.highlighted
                            }
                        }
                    }
                }

                Flickable {
                    id: detailFlick
                    QQC2.SplitView.fillWidth: true
                    QQC2.SplitView.minimumWidth: Kirigami.Units.gridUnit * 14
                    QQC2.SplitView.fillHeight: true
                    contentWidth: width
                    contentHeight: detailColumn.implicitHeight
                    clip: true

                    ColumnLayout {
                        id: detailColumn
                        width: detailFlick.width
                        spacing: Kirigami.Units.largeSpacing

                        QQC2.Label {
                            visible: root.picked.length === 0
                            text: tx.i18n("Select a route to see its details.")
                            opacity: 0.7
                            wrapMode: Text.WordWrap
                            Layout.fillWidth: true
                            Kirigami.MnemonicData.enabled: false
                        }

                        ColumnLayout {
                            visible: root.picked.length > 0
                            Layout.fillWidth: true
                            spacing: Kirigami.Units.smallSpacing

                            QQC2.Label {
                                text: tx.i18n("Name")
                                font.bold: true
                                Kirigami.MnemonicData.enabled: false
                            }
                            QQC2.Label {
                                text: tx.i18n("The name shown in the list. Renaming it updates the groups that use it.")
                                wrapMode: Text.WordWrap
                                opacity: 0.7
                                font: Kirigami.Theme.smallFont
                                Layout.fillWidth: true
                                Kirigami.MnemonicData.enabled: false
                            }
                            QQC2.TextField {
                                id: detailName
                                Layout.fillWidth: true
                                enabled: root.detail.in_file === true
                                Kirigami.MnemonicData.enabled: false
                                onTextChanged: root.pushRoute()
                            }

                            QQC2.Label {
                                text: tx.i18n("Type")
                                font.bold: true
                                Kirigami.MnemonicData.enabled: false
                            }
                            QQC2.Label {
                                text: tx.i18n("The protocol this route uses.")
                                wrapMode: Text.WordWrap
                                opacity: 0.7
                                font: Kirigami.Theme.smallFont
                                Layout.fillWidth: true
                                Kirigami.MnemonicData.enabled: false
                            }
                            QQC2.ComboBox {
                                id: detailKind
                                Layout.fillWidth: true
                                enabled: root.detail.in_file === true
                                model: ["http", "socks5", "ss", "vmess", "trojan"]
                                Kirigami.MnemonicData.enabled: false
                                onActivated: root.pushRoute()
                            }

                            QQC2.Label {
                                text: tx.i18n("Server")
                                font.bold: true
                                Kirigami.MnemonicData.enabled: false
                            }
                            QQC2.Label {
                                text: tx.i18n("The server this route connects to.")
                                wrapMode: Text.WordWrap
                                opacity: 0.7
                                font: Kirigami.Theme.smallFont
                                Layout.fillWidth: true
                                Kirigami.MnemonicData.enabled: false
                            }
                            QQC2.TextField {
                                id: detailServer
                                Layout.fillWidth: true
                                enabled: root.detail.in_file === true
                                Kirigami.MnemonicData.enabled: false
                                onTextChanged: root.pushRoute()
                            }

                            QQC2.Label {
                                text: tx.i18n("Port")
                                font.bold: true
                                Kirigami.MnemonicData.enabled: false
                            }
                            QQC2.Label {
                                text: tx.i18n("The server port.")
                                wrapMode: Text.WordWrap
                                opacity: 0.7
                                font: Kirigami.Theme.smallFont
                                Layout.fillWidth: true
                                Kirigami.MnemonicData.enabled: false
                            }
                            QQC2.SpinBox {
                                id: detailPort
                                Layout.fillWidth: true
                                enabled: root.detail.in_file === true
                                from: 1
                                to: 65535
                                editable: true
                                Kirigami.MnemonicData.enabled: false
                                onValueModified: root.pushRoute()
                            }

                            QQC2.Label {
                                text: tx.i18n("Password")
                                font.bold: true
                                Kirigami.MnemonicData.enabled: false
                            }
                            QQC2.Label {
                                text: tx.i18n("The password for this route. For VMess this is the UUID.")
                                wrapMode: Text.WordWrap
                                opacity: 0.7
                                font: Kirigami.Theme.smallFont
                                Layout.fillWidth: true
                                Kirigami.MnemonicData.enabled: false
                            }
                            RowLayout {
                                Layout.fillWidth: true
                                QQC2.TextField {
                                    id: detailSecret
                                    Layout.fillWidth: true
                                    enabled: root.detail.in_file === true
                                    echoMode: root.revealSecret ? TextInput.Normal : TextInput.Password
                                    Kirigami.MnemonicData.enabled: false
                                    onTextChanged: root.pushRoute()
                                }
                                QQC2.ToolButton {
                                    icon.name: "password-show-on"
                                    display: QQC2.AbstractButton.IconOnly
                                    enabled: root.detail.in_file === true
                                    Kirigami.MnemonicData.enabled: false
                                    onPressed: root.revealSecret = true
                                    onReleased: root.revealSecret = false
                                    onCanceled: root.revealSecret = false
                                    QQC2.ToolTip.visible: hovered
                                    QQC2.ToolTip.delay: 700
                                    QQC2.ToolTip.text: tx.i18n("Hold to show the password.")
                                }
                            }
                        }
                    }
                }
            }

            Row {
                id: buttonRow
                anchors.right: parent.right
                anchors.bottom: parent.bottom
                spacing: Kirigami.Units.smallSpacing

                QQC2.ToolButton {
                    icon.name: "document-edit"
                    display: QQC2.AbstractButton.IconOnly
                    text: QQC2.ToolTip.text
                    enabled: kcm.client.draftActiveProfile.length > 0
                    Kirigami.MnemonicData.enabled: false
                    onClicked: {
                        editorDialog.profileId = kcm.client.draftActiveProfile;
                        editorText.text = kcm.client.profileBody(kcm.client.draftActiveProfile);
                        editorDialog.open();
                    }
                    QQC2.ToolTip.text: tx.i18n("Opens this profile in the editor. A subscription update replaces the whole file.")
                    QQC2.ToolTip.visible: hovered
                }

                QQC2.ToolButton {
                    icon.name: "list-add"
                    display: QQC2.AbstractButton.IconOnly
                    text: QQC2.ToolTip.text
                    enabled: root.selectedGroup.length > 0
                    Kirigami.MnemonicData.enabled: false
                    onClicked: addDialog.open()
                    QQC2.ToolTip.text: tx.i18n("Adds a route to the current profile, which a subscription update replaces.")
                    QQC2.ToolTip.visible: hovered
                }

                QQC2.ToolButton {
                    icon.name: "list-remove"
                    display: QQC2.AbstractButton.IconOnly
                    text: QQC2.ToolTip.text
                    enabled: root.detail.in_file === true
                    Kirigami.MnemonicData.enabled: false
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
                Kirigami.MnemonicData.enabled: false
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
                root.showPicked();
            }
        }
    }

    QQC2.Dialog {
        id: editorDialog
        property string profileId: ""
        title: tx.i18n("Edit configuration")
        modal: true
        standardButtons: QQC2.Dialog.Ok | QQC2.Dialog.Cancel
        padding: Kirigami.Units.largeSpacing
        implicitWidth: Kirigami.Units.gridUnit * 36
        implicitHeight: Kirigami.Units.gridUnit * 28

        onAccepted: kcm.client.setProfileBody(profileId, editorText.text)
        onOpened: {
            editorNote.text = "";
            editorText.forceActiveFocus();
        }

        contentItem: Item {
            ColumnLayout {
                anchors.fill: parent
                spacing: Kirigami.Units.smallSpacing

                Kirigami.InlineMessage {
                    id: editorNote
                    Layout.fillWidth: true
                    visible: text.length > 0
                }

                RowLayout {
                    QQC2.Button {
                        text: tx.i18n("Check")
                        Kirigami.MnemonicData.enabled: false
                        onClicked: {
                            const result = kcm.client.checkProfileText(editorText.text);
                            editorNote.text = result.length === 0 ? tx.i18n("This configuration can be read.") : result;
                            editorNote.type = result.length === 0 ? Kirigami.MessageType.Positive : Kirigami.MessageType.Error;
                        }
                    }
                    QQC2.Button {
                        text: tx.i18n("Open in editor")
                        Kirigami.MnemonicData.enabled: false
                        onClicked: {
                            if (!kcm.client.openBodyInEditor(editorText.text)) {
                                editorNote.text = tx.i18n("No text editor was found.");
                                editorNote.type = Kirigami.MessageType.Error;
                            }
                        }
                    }
                    Item {
                        Layout.fillWidth: true
                    }
                }

                Flickable {
                    id: editorFlick
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    clip: true
                    contentWidth: width
                    contentHeight: editorText.height
                    boundsBehavior: Flickable.StopAtBounds
                    QQC2.ScrollBar.vertical: QQC2.ScrollBar {
                        policy: QQC2.ScrollBar.AsNeeded
                    }

                    function revealCursor() {
                        const rect = editorText.cursorRectangle;
                        const margin = Kirigami.Units.smallSpacing;
                        if (rect.y < contentY) {
                            contentY = Math.max(0, rect.y - margin);
                        } else if (rect.y + rect.height > contentY + height) {
                            contentY = rect.y + rect.height - height + margin;
                        }
                    }

                    QQC2.TextArea {
                        id: editorText
                        width: editorFlick.width
                        height: Math.max(editorFlick.height, contentHeight)
                        wrapMode: TextEdit.Wrap
                        selectByMouse: true
                        padding: Kirigami.Units.smallSpacing
                        onCursorRectangleChanged: editorFlick.revealCursor()

                        WheelHandler {
                            onWheel: function(event) {
                                const maxY = Math.max(0, editorFlick.contentHeight - editorFlick.height);
                                editorFlick.contentY = Math.max(0, Math.min(maxY, editorFlick.contentY - event.angleDelta.y));
                                event.accepted = true;
                            }
                        }

                        HoverHandler {
                            id: editorHover
                        }

                        QQC2.ToolTip.visible: editorHover.hovered
                        QQC2.ToolTip.delay: 700
                        QQC2.ToolTip.text: tx.i18n("A subscription update replaces the text written here.")
                    }
                }
            }
        }

        Connections {
            target: kcm.client
            function onEditorTextReady(body) {
                editorText.text = body;
            }
        }
    }
}
