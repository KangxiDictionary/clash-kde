import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

Item {
    id: root

    property int selected: -1
    property string selectedSource: ""
    property bool filling: false
    property bool sheetMissing: false
    property string sheetError: ""
    property var targetChoices: []
    property string ruleSignature: ""

    readonly property var ruleTypes: [
        { value: "DOMAIN", label: tx.i18n("Domain") },
        { value: "DOMAIN-SUFFIX", label: tx.i18n("Domain suffix") },
        { value: "DOMAIN-KEYWORD", label: tx.i18n("Domain keyword") },
        { value: "IP-CIDR", label: tx.i18n("IP range") },
        { value: "GEOSITE", label: "GEOSITE" },
        { value: "GEOIP", label: "GEOIP" },
        { value: "PROCESS-NAME", label: tx.i18n("Process name") },
        { value: "DST-PORT", label: tx.i18n("Destination port") },
        { value: "MATCH", label: tx.i18n("Final match") }
    ]

    Tx {
        id: tx
    }

    Fuzzy {
        id: fuzzy
    }

    property string ruleQuery: ""

    ListModel {
        id: ruleModel
    }

    function typeLabel(value) {
        for (let i = 0; i < root.ruleTypes.length; ++i) {
            if (root.ruleTypes[i].value === value) {
                return root.ruleTypes[i].label;
            }
        }
        return value;
    }

    function typeIndex(value) {
        for (let i = 0; i < root.ruleTypes.length; ++i) {
            if (root.ruleTypes[i].value === value) {
                return i;
            }
        }
        return 0;
    }

    function currentType() {
        const index = Math.max(0, ruleKind.currentIndex);
        return root.ruleTypes[index].value;
    }

    function appendRule(section, source, ruleIndex, type, payload, target, enabled, name, updated, count) {
        ruleModel.append({
            section: section,
            source: source,
            ruleIndex: ruleIndex,
            type: type,
            payload: payload,
            target: target,
            ruleOn: enabled,
            name: name,
            updated: updated,
            count: count
        });
    }

    function ruleSignatureOf() {
        const own = kcm.client.ownRules;
        const profile = kcm.client.profileRules;
        const providers = kcm.client.ruleProviders;
        let text = root.ruleQuery.trim().toLowerCase() + "\n" + String(own.length) + ":" + String(profile.length) + ":" + String(providers.length);
        for (let i = 0; i < own.length; ++i) {
            text += "\n" + String(own[i].type || "") + "\t" + String(own[i].payload || "") + "\t" + String(own[i].target || "") + "\t" + (own[i].enabled !== false ? "1" : "0");
        }
        for (let i = 0; i < profile.length; ++i) {
            text += "\n" + String(profile[i].type || "") + "\t" + String(profile[i].payload || "") + "\t" + String(profile[i].target || "");
        }
        for (let i = 0; i < providers.length; ++i) {
            text += "\n" + String(providers[i].name || "") + "\t" + String(providers[i].count) + "\t" + String(providers[i].updated_at || "");
        }
        return text;
    }

    function rebuild() {
        const signature = root.ruleSignatureOf();
        if (signature === root.ruleSignature && ruleModel.count > 0) {
            return;
        }
        root.ruleSignature = signature;
        const keepSource = root.selectedSource;
        const keepIndex = root.selected;
        root.filling = true;
        ruleModel.clear();
        const own = kcm.client.ownRules;
        for (let i = 0; i < own.length; ++i) {
            const type = String(own[i].type || "");
            const payload = String(own[i].payload || "");
            const target = String(own[i].target || "");
            if (fuzzy.best(root.ruleQuery, [root.typeLabel(type), payload, target]) < 0) {
                continue;
            }
            root.appendRule(tx.i18n("Your rules"), "own", i, type, payload, target, own[i].enabled !== false, "", "", -1);
        }
        const profile = kcm.client.profileRules;
        for (let i = 0; i < profile.length; ++i) {
            const type = String(profile[i].type || "");
            const payload = String(profile[i].payload || "");
            const target = String(profile[i].target || "");
            if (fuzzy.best(root.ruleQuery, [root.typeLabel(type), payload, target]) < 0) {
                continue;
            }
            root.appendRule(tx.i18n("Profile rules"), "profile", i, type, payload, target, true, "", "", -1);
        }
        const providers = kcm.client.ruleProviders;
        for (let i = 0; i < providers.length; ++i) {
            const name = String(providers[i].name || "");
            if (fuzzy.best(root.ruleQuery, [name]) < 0) {
                continue;
            }
            root.appendRule(tx.i18n("Rule sets"), "provider", i, "", "", "", true, name, String(providers[i].updated_at || ""), Number(providers[i].count));
        }
        let found = -1;
        for (let i = 0; i < ruleModel.count; ++i) {
            const row = ruleModel.get(i);
            if (row.source === keepSource && row.ruleIndex === keepIndex) {
                found = i;
                break;
            }
        }
        ruleList.currentIndex = found;
        if (found < 0) {
            root.selected = -1;
            root.selectedSource = "";
        }
        root.filling = false;
    }

    function showSelected() {
        const row = ruleList.currentIndex >= 0 ? ruleModel.get(ruleList.currentIndex) : null;
        root.filling = true;
        ruleNote.text = root.noteKey().length > 0 ? kcm.client.noteFor(root.noteKey()) : "";
        if (!row || row.source === "provider") {
            rulePayload.text = "";
            ruleKind.currentIndex = 0;
            ruleEnabled.checked = true;
            root.filling = false;
            return;
        }
        ruleKind.currentIndex = root.typeIndex(row.type);
        rulePayload.text = row.type === "MATCH" ? "" : row.payload;
        const names = root.targetNames(row.target);
        let targetIndex = 0;
        for (let i = 0; i < names.length; ++i) {
            if (names[i] === row.target) {
                targetIndex = i;
                break;
            }
        }
        root.targetChoices = names;
        ruleTarget.currentIndex = targetIndex;
        ruleEnabled.checked = row.ruleOn !== false;
        root.filling = false;
    }

    function targetNames(extra) {
        const names = [];
        const listed = kcm.client.ruleTargets;
        for (let i = 0; i < listed.length; ++i) {
            names.push(String(listed[i]));
        }
        if (extra && names.indexOf(extra) < 0) {
            names.unshift(extra);
        }
        return names;
    }

    function pushRule() {
        if (root.filling || root.selected < 0) {
            return;
        }
        const type = root.currentType();
        const payload = type === "MATCH" ? "" : rulePayload.text;
        const target = ruleTarget.currentText;
        const enabled = ruleEnabled.checked;
        if (root.selectedSource === "own") {
            kcm.client.stageRule(root.selected, type, payload, target, enabled);
            return;
        }
        if (root.selectedSource !== "profile") {
            return;
        }
        const row = ruleList.currentIndex >= 0 ? ruleModel.get(ruleList.currentIndex) : null;
        if (row && row.type === type && (type === "MATCH" || row.payload === payload) && row.target === target && enabled) {
            return;
        }
        root.selectedSource = "own";
        root.selected = kcm.client.ownRules.length;
        kcm.client.stageForkedRule(type, payload, target, enabled);
    }

    function openSheet() {
        root.sheetMissing = false;
        root.sheetError = "";
        const code = kcm.client.openRulesSpreadsheet();
        if (code === 1) {
            sheetDialog.open();
        } else if (code === 0) {
            root.sheetMissing = true;
        }
    }

    function finishSheet() {
        const result = kcm.client.finishRulesSpreadsheet();
        if (result === "same") {
            sheetDialog.close();
            return;
        }
        if (result.length > 0) {
            root.sheetError = result;
            return;
        }
        sheetDialog.close();
    }

    function providerUpdated(text) {
        if (!text) {
            return tx.i18n("Not updated yet");
        }
        const date = new Date(text);
        if (isNaN(date.getTime())) {
            return tx.i18n("Not updated yet");
        }
        if (date.getFullYear() < 1970) {
            return tx.i18n("Not updated yet");
        }
        return date.toLocaleString(Qt.locale(), Locale.ShortFormat);
    }

    function profileSubscribed() {
        const id = kcm.client.draftActiveProfile;
        const profiles = kcm.client.profiles || [];
        for (let i = 0; i < profiles.length; ++i) {
            if (profiles[i].id === id) {
                return String(profiles[i].url || "").length > 0;
            }
        }
        return false;
    }

    function ruleChanged() {
        const rules = kcm.client.profileRules || [];
        if (root.selected < 0 || root.selected >= rules.length) {
            return false;
        }
        return rules[root.selected].changed === true;
    }

    function noteKey() {
        if (root.selectedSource === "provider") {
            const row = ruleList.currentIndex >= 0 ? ruleModel.get(ruleList.currentIndex) : null;
            return row ? "rule\nprovider\n" + row.name : "";
        }
        if (root.selected < 0 || root.selectedSource.length === 0) {
            return "";
        }
        return "rule\n" + root.selectedSource + "\n" + root.selected;
    }

    Connections {
        target: kcm.client
        function onRulesChanged() {
            root.rebuild();
        }
        function onRuleDraftDiscarded() {
            root.showSelected();
        }
        function onNotesChanged() {
            if (ruleNote.activeFocus || root.filling || root.noteKey().length === 0) {
                return;
            }
            const next = kcm.client.noteFor(root.noteKey());
            if (next === ruleNote.text) {
                return;
            }
            root.filling = true;
            ruleNote.text = next;
            root.filling = false;
        }
    }

    onVisibleChanged: {
        if (visible) {
            kcm.client.refreshRules();
        }
    }

    ColumnLayout {
        anchors.fill: parent
        spacing: Kirigami.Units.largeSpacing

        Kirigami.SearchField {
            Layout.fillWidth: true
            Kirigami.MnemonicData.enabled: false
            onTextChanged: {
                root.ruleQuery = text;
                root.rebuild();
            }
        }

        QQC2.Label {
            visible: root.ruleQuery.trim().length > 0 && ruleModel.count === 0
            text: tx.i18n("No matches")
            opacity: 0.7
            Layout.fillWidth: true
            Kirigami.MnemonicData.enabled: false
        }

        QQC2.Label {
            visible: root.sheetMissing
            text: tx.i18n("No spreadsheet program was found.")
            color: Kirigami.Theme.negativeTextColor
            wrapMode: Text.WordWrap
            Layout.fillWidth: true
            Kirigami.MnemonicData.enabled: false
        }

        Item {
            Layout.fillWidth: true
            Layout.fillHeight: true

            QQC2.SplitView {
                anchors.top: parent.top
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.bottom: parent.bottom
                orientation: Qt.Horizontal

                ColumnLayout {
                    spacing: Kirigami.Units.smallSpacing
                    QQC2.SplitView.preferredWidth: Kirigami.Units.gridUnit * 16
                    QQC2.SplitView.minimumWidth: Kirigami.Units.gridUnit * 10
                    QQC2.SplitView.fillHeight: true

                QQC2.ScrollView {
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    Kirigami.StyleHints.showFramedBackground: true

                    ListView {
                        id: ruleList
                        clip: true
                        reuseItems: true
                        model: ruleModel
                        currentIndex: -1
                        boundsBehavior: Flickable.StopAtBounds
                        section.property: "section"
                        section.criteria: ViewSection.FullString
                        section.delegate: Kirigami.ListSectionHeader {
                            required property string section
                            width: ListView.view ? ListView.view.width : 0
                            text: section
                        }
                        onCurrentIndexChanged: {
                            if (root.filling) {
                                return;
                            }
                            const row = currentIndex >= 0 ? ruleModel.get(currentIndex) : null;
                            root.selected = row ? row.ruleIndex : -1;
                            root.selectedSource = row ? row.source : "";
                            root.showSelected();
                        }

                        delegate: QQC2.ItemDelegate {
                            id: ruleDelegate
                            required property int index
                            required property string source
                            required property var ruleIndex
                            required property string type
                            required property string payload
                            required property string target
                            required property bool ruleOn
                            required property string name
                            required property string updated
                            required property var count
                            width: ListView.view ? ListView.view.width : 0
                            highlighted: ruleList.currentIndex === index
                            Kirigami.MnemonicData.enabled: false
                            onClicked: ruleList.currentIndex = index

                            contentItem: Kirigami.IconTitleSubtitle {
                                title: ruleDelegate.source === "provider" ? ruleDelegate.name : root.typeLabel(ruleDelegate.type) + (ruleDelegate.payload.length > 0 ? " · " + ruleDelegate.payload : "")
                                subtitle: kcm.client.compact ? "" : (ruleDelegate.source === "provider"
                                                                     ? (ruleDelegate.count >= 0 ? tx.i18n("%1 rules", tx.formatNumber(ruleDelegate.count)) : tx.i18n("Not updated yet"))
                                                                     : ruleDelegate.target)
                                selected: ruleDelegate.highlighted
                                opacity: ruleDelegate.ruleOn ? 1 : 0.45
                            }
                        }
                    }
                }

                RowLayout {
                    Layout.alignment: Qt.AlignRight
                    spacing: Kirigami.Units.smallSpacing

                    QQC2.ToolButton {
                        icon.name: "x-office-spreadsheet"
                        display: QQC2.AbstractButton.IconOnly
                        text: QQC2.ToolTip.text
                        enabled: !kcm.client.busy
                        Kirigami.MnemonicData.enabled: false
                        onClicked: root.openSheet()
                        QQC2.ToolTip.text: tx.i18n("Opens these rules in a spreadsheet. Press Apply after they are read back.")
                        QQC2.ToolTip.visible: hovered
                    }

                    QQC2.ToolButton {
                        icon.name: "list-add"
                        display: QQC2.AbstractButton.IconOnly
                        text: QQC2.ToolTip.text
                        enabled: !kcm.client.busy
                        Kirigami.MnemonicData.enabled: false
                        onClicked: addRuleDialog.open()
                        QQC2.ToolTip.text: tx.i18n("Adds a rule now, before the profile rules. A subscription update does not remove it.")
                        QQC2.ToolTip.visible: hovered
                    }

                    QQC2.ToolButton {
                        icon.name: "list-remove"
                        display: QQC2.AbstractButton.IconOnly
                        text: QQC2.ToolTip.text
                        enabled: root.selectedSource === "own" && root.selected >= 0 && !kcm.client.busy
                        Kirigami.MnemonicData.enabled: false
                        onClicked: removeRuleDialog.open()
                        QQC2.ToolTip.text: tx.i18n("Removes the selected rule now.")
                        QQC2.ToolTip.visible: hovered
                    }
                }
                }

                Item {
                    QQC2.SplitView.fillWidth: true
                    QQC2.SplitView.minimumWidth: Kirigami.Units.gridUnit * 14
                    QQC2.SplitView.fillHeight: true

            Flickable {
                id: detailFlick
                anchors.top: parent.top
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.bottom: restoreRow.top
                anchors.bottomMargin: Kirigami.Units.smallSpacing
                contentWidth: width
                contentHeight: detailColumn.implicitHeight
                clip: true

                ColumnLayout {
                    id: detailColumn
                    width: detailFlick.width
                    spacing: Kirigami.Units.largeSpacing

                    QQC2.Label {
                        visible: root.selectedSource.length === 0
                        text: tx.i18n("Select a rule to see its details.")
                        opacity: 0.7
                        wrapMode: Text.WordWrap
                        Layout.fillWidth: true
                        Kirigami.MnemonicData.enabled: false
                    }

                    QQC2.Label {
                        visible: root.selectedSource === "profile"
                        text: tx.i18n("Editing this profile rule saves your own copy, which a subscription update does not remove.")
                        wrapMode: Text.WordWrap
                        Layout.fillWidth: true
                        Kirigami.MnemonicData.enabled: false
                    }

                    QQC2.Label {
                        visible: root.selectedSource === "provider"
                        text: tx.i18n("This rule set comes with the profile. It can be updated, not edited here.")
                        wrapMode: Text.WordWrap
                        Layout.fillWidth: true
                        Kirigami.MnemonicData.enabled: false
                    }

                    ColumnLayout {
                        visible: root.selectedSource === "provider"
                        Layout.fillWidth: true
                        spacing: Kirigami.Units.smallSpacing

                        QQC2.Label {
                            readonly property var row: ruleList.currentIndex >= 0 ? ruleModel.get(ruleList.currentIndex) : null
                            text: row ? row.name : ""
                            font.bold: true
                            Layout.fillWidth: true
                            Kirigami.MnemonicData.enabled: false
                        }
                        QQC2.Label {
                            readonly property var row: ruleList.currentIndex >= 0 ? ruleModel.get(ruleList.currentIndex) : null
                            visible: row && row.count >= 0
                            text: row ? tx.i18n("%1 rules", tx.formatNumber(row.count)) : ""
                            opacity: 0.7
                            Layout.fillWidth: true
                            Kirigami.MnemonicData.enabled: false
                        }
                        QQC2.Label {
                            readonly property var row: ruleList.currentIndex >= 0 ? ruleModel.get(ruleList.currentIndex) : null
                            text: row ? tx.i18n("Last update: %1", root.providerUpdated(row.updated)) : ""
                            opacity: 0.7
                            Layout.fillWidth: true
                            Kirigami.MnemonicData.enabled: false
                        }
                        QQC2.Button {
                            readonly property var row: ruleList.currentIndex >= 0 ? ruleModel.get(ruleList.currentIndex) : null
                            Kirigami.MnemonicData.enabled: false
                            text: tx.i18n("Update now")
                            enabled: row && kcm.client.coreRunning && !kcm.client.busy
                            onClicked: {
                                if (row) {
                                    kcm.client.updateRuleProvider(row.name);
                                }
                            }

                            QQC2.ToolTip.visible: hovered
                            QQC2.ToolTip.delay: 700
                            QQC2.ToolTip.text: kcm.client.coreRunning ? tx.i18n("Updates this rule set.") : tx.i18n("Start the core first.")
                        }
                    }

                    ColumnLayout {
                        id: ruleFields
                        visible: root.selectedSource === "own" || root.selectedSource === "profile"
                        Layout.fillWidth: true
                        spacing: Kirigami.Units.smallSpacing

                        readonly property bool editable: root.selectedSource === "own" || root.selectedSource === "profile"

                        QQC2.Label {
                            text: tx.i18n("Type")
                            font.bold: true
                            Kirigami.MnemonicData.enabled: false
                        }
                        QQC2.Label {
                            text: tx.i18n("The kind of match this rule uses.")
                            wrapMode: Text.WordWrap
                            opacity: 0.7
                            font: Kirigami.Theme.smallFont
                            Layout.fillWidth: true
                            Kirigami.MnemonicData.enabled: false
                        }
                        QQC2.ComboBox {
                            id: ruleKind
                            Kirigami.MnemonicData.enabled: false
                            Layout.fillWidth: true
                            enabled: ruleFields.editable
                            model: root.ruleTypes
                            textRole: "label"
                            onActivated: {
                                if (root.currentType() === "MATCH") {
                                    rulePayload.text = "";
                                }
                                root.pushRule();
                            }
                        }

                        QQC2.Label {
                            text: tx.i18n("Match")
                            font.bold: true
                            Kirigami.MnemonicData.enabled: false
                        }
                        QQC2.Label {
                            text: tx.i18n("What this rule compares against. A final match leaves this empty.")
                            wrapMode: Text.WordWrap
                            opacity: 0.7
                            font: Kirigami.Theme.smallFont
                            Layout.fillWidth: true
                            Kirigami.MnemonicData.enabled: false
                        }
                        QQC2.TextField {
                            id: rulePayload
                            Layout.fillWidth: true
                            enabled: ruleFields.editable && root.currentType() !== "MATCH"
                            Kirigami.MnemonicData.enabled: false
                            onTextChanged: root.pushRule()
                        }

                        QQC2.Label {
                            text: tx.i18n("Target")
                            font.bold: true
                            Kirigami.MnemonicData.enabled: false
                        }
                        QQC2.Label {
                            text: tx.i18n("The group, DIRECT, or REJECT that handles a match.")
                            wrapMode: Text.WordWrap
                            opacity: 0.7
                            font: Kirigami.Theme.smallFont
                            Layout.fillWidth: true
                            Kirigami.MnemonicData.enabled: false
                        }
                        QQC2.ComboBox {
                            id: ruleTarget
                            Kirigami.MnemonicData.enabled: false
                            Layout.fillWidth: true
                            enabled: ruleFields.editable
                            model: root.targetChoices
                            onActivated: root.pushRule()
                        }

                        QQC2.Label {
                            text: tx.i18n("Enabled")
                            font.bold: true
                            Kirigami.MnemonicData.enabled: false
                        }
                        QQC2.Label {
                            text: tx.i18n("A disabled rule stays in the list and is left out of the running config.")
                            wrapMode: Text.WordWrap
                            opacity: 0.7
                            font: Kirigami.Theme.smallFont
                            Layout.fillWidth: true
                            Kirigami.MnemonicData.enabled: false
                        }
                        QQC2.Switch {
                            id: ruleEnabled
                            Kirigami.MnemonicData.enabled: false
                            enabled: ruleFields.editable
                            onToggled: root.pushRule()
                        }

                        QQC2.Label {
                            text: tx.i18n("Comment")
                            font.bold: true
                            visible: root.selectedSource.length > 0
                            Kirigami.MnemonicData.enabled: false
                        }
                        QQC2.Label {
                            text: tx.i18n("A note kept by this program, separate from the profile.")
                            visible: root.selectedSource.length > 0
                            wrapMode: Text.WordWrap
                            opacity: 0.7
                            font: Kirigami.Theme.smallFont
                            Layout.fillWidth: true
                            Kirigami.MnemonicData.enabled: false
                        }
                        QQC2.TextArea {
                            id: ruleNote
                            visible: root.selectedSource.length > 0
                            Layout.fillWidth: true
                            readonly property real noteLimit: Kirigami.Units.gridUnit * 12
                            Layout.preferredHeight: Math.min(noteLimit, Math.max(implicitBackgroundHeight, contentHeight + topPadding + bottomPadding))
                            Layout.maximumHeight: noteLimit
                            wrapMode: TextEdit.Wrap
                            clip: true
                            Kirigami.MnemonicData.enabled: false
                            QQC2.ScrollBar.vertical: QQC2.ScrollBar {
                                policy: QQC2.ScrollBar.AsNeeded
                            }
                            onTextChanged: {
                                if (root.filling || root.noteKey().length === 0) {
                                    return;
                                }
                                kcm.client.stageNote(root.noteKey(), text);
                            }
                        }
                    }
                }
            }

                RowLayout {
                    id: restoreRow
                    anchors.right: parent.right
                    anchors.bottom: parent.bottom
                    QQC2.Button {
                        text: tx.i18n("Restore")
                        icon.name: "edit-undo"
                        enabled: root.profileSubscribed() && root.selectedSource === "profile" && root.ruleChanged()
                        Kirigami.MnemonicData.enabled: false
                        onClicked: {
                            if (root.selectedSource !== "profile") {
                                kcm.client.showHint(tx.i18n("This item is not in the subscription."));
                                return;
                            }
                            if (kcm.client.restoreProfileRule(root.selected)) {
                                root.showSelected();
                            }
                        }
                        QQC2.ToolTip.visible: hovered && !root.profileSubscribed()
                        QQC2.ToolTip.delay: 700
                        QQC2.ToolTip.text: tx.i18n("This profile has no subscription, so this item cannot be restored.")
                    }
                }
            }
        }
    }

    QQC2.Dialog {
        id: addRuleDialog
        title: tx.i18n("Add a rule")
        modal: true
        standardButtons: QQC2.Dialog.Ok | QQC2.Dialog.Cancel
        anchors.centerIn: parent
        width: Kirigami.Units.gridUnit * 24

        contentItem: ColumnLayout {
            spacing: Kirigami.Units.smallSpacing

            QQC2.ComboBox {
                id: addKind
                Kirigami.MnemonicData.enabled: false
                Layout.fillWidth: true
                model: root.ruleTypes
                textRole: "label"
            }
            QQC2.TextField {
                id: addPayload
                Layout.fillWidth: true
                enabled: root.ruleTypes[addKind.currentIndex].value !== "MATCH"
                placeholderText: tx.i18n("Match")
                Kirigami.MnemonicData.enabled: false
            }
            QQC2.ComboBox {
                id: addTarget
                Kirigami.MnemonicData.enabled: false
                Layout.fillWidth: true
                model: root.targetNames("DIRECT")
            }
        }

        onAccepted: {
            const type = root.ruleTypes[addKind.currentIndex].value;
            if (kcm.client.addOwnRule(type, addPayload.text, addTarget.currentText)) {
                root.selectedSource = "own";
                root.selected = Math.max(0, kcm.client.ownRules.length - 1);
                root.rebuild();
                root.showSelected();
            }
        }
    }

    Kirigami.PromptDialog {
        id: removeRuleDialog
        standardButtons: Kirigami.Dialog.Ok | Kirigami.Dialog.Cancel
        title: tx.i18n("Remove this rule?")
        onAccepted: {
            const index = root.selected;
            root.selected = -1;
            root.selectedSource = "";
            kcm.client.removeOwnRule(index);
        }
    }

    QQC2.Dialog {
        id: sheetDialog
        title: tx.i18n("Edit in a spreadsheet")
        modal: true
        standardButtons: QQC2.Dialog.NoButton
        anchors.centerIn: parent
        width: Kirigami.Units.gridUnit * 28
        closePolicy: QQC2.Popup.NoAutoClose

        contentItem: ColumnLayout {
            spacing: Kirigami.Units.largeSpacing

            QQC2.Label {
                text: tx.i18n("Save over this CSV file, then press Done. A separate spreadsheet file is not read back, and LibreOffice does not edit the YAML.")
                wrapMode: Text.WordWrap
                Layout.fillWidth: true
                Kirigami.MnemonicData.enabled: false
            }
            QQC2.Label {
                visible: root.sheetError.length > 0
                text: root.sheetError
                wrapMode: Text.WordWrap
                color: Kirigami.Theme.negativeTextColor
                Layout.fillWidth: true
                Kirigami.MnemonicData.enabled: false
            }
        }

        footer: QQC2.DialogButtonBox {
            QQC2.Button {
                Kirigami.MnemonicData.enabled: false
                text: tx.i18n("Done")
                QQC2.DialogButtonBox.buttonRole: QQC2.DialogButtonBox.AcceptRole
                onClicked: root.finishSheet()
            }
            QQC2.Button {
                Kirigami.MnemonicData.enabled: false
                text: tx.i18n("Cancel")
                QQC2.DialogButtonBox.buttonRole: QQC2.DialogButtonBox.RejectRole
                onClicked: sheetDialog.close()
            }
        }

        onClosed: kcm.client.cancelRulesSpreadsheet()
    }
}


}
