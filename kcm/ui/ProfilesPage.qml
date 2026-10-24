// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 clash-kde contributors

import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Dialogs
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

Item {
    id: root

    property string profileQuery: ""

    Tx {
        id: tx
    }

    Fuzzy {
        id: fuzzy
    }

    Connections {
        target: kcm.client
        function onProfilesChanged() {
            if (root.visible) {
                kcm.client.refreshProxyProviders();
            }
        }
    }

    onVisibleChanged: {
        if (visible) {
            kcm.client.refreshProxyProviders();
        }
    }

    readonly property var shownProfiles: {
        const query = root.profileQuery;
        const all = kcm.client.profiles;
        if (query.trim().length === 0) {
            return all;
        }
        const ranked = [];
        for (let i = 0; i < all.length; ++i) {
            const profile = all[i];
            const updated = formatUpdated(profile.updated_unix);
            const weight = fuzzy.best(query, [
                profile.name,
                profile.url,
                quotaText(profile),
                updated,
                tx.i18n("Last update: %1", updated)
            ]);
            if (weight >= 0) {
                ranked.push({
                    item: all[i],
                    weight: weight
                });
            }
        }
        ranked.sort((left, right) => right.weight - left.weight || String(left.item.name).localeCompare(String(right.item.name)));
        const out = [];
        for (let i = 0; i < ranked.length; ++i) {
            out.push(ranked[i].item);
        }
        return out;
    }

    function formatUpdated(unix) {
        if (!unix) {
            return tx.i18n("Not updated yet");
        }
        return new Date(unix * 1000).toLocaleString(Qt.locale(), Locale.ShortFormat);
    }

    function formatBytes(bytes) {
        if (bytes < 1024) {
            return tx.i18n("%1 B", tx.formatNumber(bytes));
        }
        if (bytes < 1024 * 1024) {
            return tx.i18n("%1 KB", tx.formatNumber(bytes / 1024, 1));
        }
        if (bytes < 1024 * 1024 * 1024) {
            return tx.i18n("%1 MB", tx.formatNumber(bytes / 1024 / 1024, 1));
        }
        return tx.i18n("%1 GB", tx.formatNumber(bytes / 1024 / 1024 / 1024, 2));
    }

    function quotaText(profile) {
        const parts = [];
        if (profile.total > 0) {
            const used = Number(profile.upload || 0) + Number(profile.download || 0);
            const remain = Math.max(0, Number(profile.total) - used);
            parts.push(tx.i18n("Remaining: %1", formatBytes(remain)));
        }
        if (profile.expire_unix > 0) {
            const date = new Date(profile.expire_unix * 1000).toLocaleDateString(Qt.locale(), Locale.ShortFormat);
            parts.push(tx.i18n("Expires: %1", date));
        }
        return parts.join(" · ");
    }

    QQC2.Dialog {
        id: addDialog
        title: tx.i18n("Add subscription")
        modal: true
        standardButtons: QQC2.Dialog.Ok | QQC2.Dialog.Cancel
        padding: Kirigami.Units.largeSpacing
        implicitWidth: Kirigami.Units.gridUnit * 24

        function validUrl(text) {
            return /^https?:\/\/\S+$/.test(text.trim());
        }

        function refreshOk() {
            const button = standardButton(QQC2.Dialog.Ok);
            if (button) {
                button.enabled = validUrl(urlField.text);
            }
        }

        onOpened: Qt.callLater(refreshOk)

        onAccepted: {
            if (!validUrl(urlField.text)) {
                return;
            }
            kcm.client.addSubscription(nameField.text, urlField.text.trim());
            nameField.text = "";
            urlField.text = "";
        }

        contentItem: Kirigami.FormLayout {
            QQC2.TextField {
                id: nameField
                Kirigami.FormData.label: tx.i18n("Name")
                placeholderText: tx.i18n("Optional")
                maximumLength: 128
                inputMethodHints: Qt.ImhNoPredictiveText
            }
            QQC2.TextField {
                id: urlField
                Kirigami.FormData.label: tx.i18n("URL")
                placeholderText: "https://"
                implicitWidth: Kirigami.Units.gridUnit * 18
                maximumLength: 2048
                inputMethodHints: Qt.ImhUrlCharactersOnly | Qt.ImhNoAutoUppercase
                validator: RegularExpressionValidator {
                    regularExpression: /^https?:\/\/\S+$/
                }
                onTextChanged: addDialog.refreshOk()
            }
            QQC2.Label {
                visible: urlField.text.length > 0 && !urlField.acceptableInput
                text: tx.i18n("Enter a complete http:// or https:// URL without spaces.")
                color: Kirigami.Theme.negativeTextColor
                font: Kirigami.Theme.smallFont
                wrapMode: Text.WordWrap
                Layout.fillWidth: true
                Kirigami.MnemonicData.enabled: false
            }
        }
    }

    QQC2.Dialog {
        id: metaDialog
        property string profileId: ""
        title: tx.i18n("Edit details")
        modal: true
        standardButtons: QQC2.Dialog.Ok | QQC2.Dialog.Cancel
        padding: Kirigami.Units.largeSpacing
        implicitWidth: Kirigami.Units.gridUnit * 24

        function validUrl(text) {
            const value = text.trim();
            return value.length === 0 || /^https?:\/\/\S+$/.test(value);
        }

        function refreshOk() {
            const button = standardButton(QQC2.Dialog.Ok);
            if (button) {
                button.enabled = metaName.text.trim().length > 0 && validUrl(metaUrl.text);
            }
        }

        onOpened: Qt.callLater(refreshOk)

        onAccepted: {
            if (metaName.text.trim().length === 0 || !validUrl(metaUrl.text)) {
                return;
            }
            kcm.client.setProfileMeta(profileId, metaName.text.trim(), metaUrl.text.trim());
        }

        contentItem: Kirigami.FormLayout {
            QQC2.TextField {
                id: metaName
                Kirigami.FormData.label: tx.i18n("Name")
                maximumLength: 128
                inputMethodHints: Qt.ImhNoPredictiveText
                onTextChanged: metaDialog.refreshOk()
            }
            QQC2.TextField {
                id: metaUrl
                Kirigami.FormData.label: tx.i18n("URL")
                placeholderText: "https://"
                implicitWidth: Kirigami.Units.gridUnit * 18
                maximumLength: 2048
                inputMethodHints: Qt.ImhUrlCharactersOnly | Qt.ImhNoAutoUppercase
                validator: RegularExpressionValidator {
                    regularExpression: /^$|^https?:\/\/\S+$/
                }
                onTextChanged: metaDialog.refreshOk()
            }
            QQC2.Label {
                visible: metaName.text.length === 0 || (metaUrl.text.length > 0 && !metaUrl.acceptableInput)
                text: metaName.text.length === 0
                    ? tx.i18n("A profile name is required.")
                    : tx.i18n("Enter a complete http:// or https:// URL without spaces, or leave it empty.")
                color: Kirigami.Theme.negativeTextColor
                font: Kirigami.Theme.smallFont
                wrapMode: Text.WordWrap
                Layout.fillWidth: true
                Kirigami.MnemonicData.enabled: false
            }
        }
    }

    QQC2.Dialog {
        id: removeDialog
        property string profileId: ""
        title: tx.i18n("Remove this profile?")
        modal: true
        standardButtons: QQC2.Dialog.Yes | QQC2.Dialog.No
        onAccepted: kcm.client.removeProfile(profileId)
    }

    FileDialog {
        id: importDialog
        title: tx.i18n("Import configuration")
        fileMode: FileDialog.OpenFile
        nameFilters: ["YAML (*.yaml *.yml)", "All files (*)"]
        onAccepted: kcm.client.importProfile("", selectedFile.toString())
    }

    Connections {
        target: kcm.client
        function onProfilesChanged() {
            const profiles = kcm.client.profiles;
            const draft = kcm.client.draftActiveProfile;
            if (draft.length === 0) {
                return;
            }
            for (let i = 0; i < profiles.length; ++i) {
                if (profiles[i].id === draft) {
                    return;
                }
            }
            kcm.client.draftActiveProfile = kcm.client.activeProfile;
        }
    }

    ColumnLayout {
        anchors.fill: parent
        spacing: Kirigami.Units.largeSpacing

        RowLayout {
            Layout.fillWidth: true

            Kirigami.Heading {
                text: tx.i18n("Profiles")
                level: 3
            }

            Item {
                Layout.fillWidth: true
            }

            QQC2.Button {
                Kirigami.MnemonicData.enabled: false
                text: tx.i18n("Import")
                icon.name: "document-import"
                enabled: !kcm.client.busy
                onClicked: importDialog.open()
            }

            QQC2.Button {
                Kirigami.MnemonicData.enabled: false
                text: tx.i18n("Add subscription")
                icon.name: "list-add"
                enabled: !kcm.client.busy
                onClicked: addDialog.open()
            }
        }

        Kirigami.Separator {
            Layout.fillWidth: true
        }

        RowLayout {
            Layout.fillWidth: true
            Layout.fillHeight: true
            spacing: Kirigami.Units.largeSpacing

            ColumnLayout {
                Layout.fillWidth: true
                Layout.fillHeight: true
                Layout.preferredWidth: Kirigami.Units.gridUnit * 22
                Layout.minimumWidth: Kirigami.Units.gridUnit * 14
                Layout.horizontalStretchFactor: 1
                spacing: Kirigami.Units.largeSpacing

                Kirigami.SearchField {
                    id: profileSearch
                    Layout.fillWidth: true
                    visible: kcm.client.profiles.length > 0
                    maximumLength: 128
                    onTextChanged: root.profileQuery = text
                }

                QQC2.Label {
                    visible: kcm.client.profiles.length === 0
                    text: tx.i18n("No profiles yet")
                    opacity: 0.7
                    Layout.fillWidth: true
                }

                QQC2.Label {
                    visible: kcm.client.profiles.length > 0 && root.shownProfiles.length === 0
                    text: tx.i18n("No matches")
                    opacity: 0.7
                    Layout.fillWidth: true
                }

                QQC2.ScrollView {
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    visible: root.shownProfiles.length > 0
                    Kirigami.StyleHints.showFramedBackground: true

                    ListView {
                clip: true
                reuseItems: true
                model: root.shownProfiles
                boundsBehavior: Flickable.StopAtBounds

                delegate: QQC2.ItemDelegate {
                    id: profileDelegate
                    required property var modelData
                    width: ListView.view ? ListView.view.width : 0
                    highlighted: modelData.id === kcm.client.draftActiveProfile
                    Kirigami.MnemonicData.enabled: false
                    onClicked: kcm.client.draftActiveProfile = modelData.id

                    contentItem: RowLayout {
                        spacing: Kirigami.Units.largeSpacing

                        QQC2.RadioButton {
                            checked: profileDelegate.highlighted
                            Kirigami.MnemonicData.enabled: false
                            onClicked: kcm.client.draftActiveProfile = profileDelegate.modelData.id
                        }

                        ColumnLayout {
                            Layout.fillWidth: true
                            spacing: 0

                            QQC2.Label {
                                text: profileDelegate.modelData.name
                                font.bold: true
                                elide: Text.ElideRight
                                Layout.fillWidth: true
                                Kirigami.MnemonicData.enabled: false
                            }
                            QQC2.Label {
                                visible: !kcm.client.compact && profileDelegate.modelData.url.length > 0
                                text: profileDelegate.modelData.url
                                opacity: 0.7
                                elide: Text.ElideMiddle
                                font: Kirigami.Theme.smallFont
                                Layout.fillWidth: true
                                Kirigami.MnemonicData.enabled: false
                            }
                            QQC2.Label {
                                readonly property string quota: quotaText(profileDelegate.modelData)
                                visible: !kcm.client.compact && quota.length > 0
                                text: quota
                                opacity: 0.7
                                font: Kirigami.Theme.smallFont
                                Layout.fillWidth: true
                                Kirigami.MnemonicData.enabled: false
                            }
                            QQC2.Label {
                                text: tx.i18n("Last update: %1", formatUpdated(profileDelegate.modelData.updated_unix))
                                visible: !kcm.client.compact
                                opacity: 0.7
                                font: Kirigami.Theme.smallFont
                                Layout.fillWidth: true
                                Kirigami.MnemonicData.enabled: false
                            }
                        }

                        QQC2.ToolButton {
                            icon.name: "document-edit"
                            display: QQC2.AbstractButton.IconOnly
                            enabled: !kcm.client.busy
                            Kirigami.MnemonicData.enabled: false
                            onClicked: {
                                metaDialog.profileId = profileDelegate.modelData.id;
                                metaName.text = profileDelegate.modelData.name;
                                metaUrl.text = profileDelegate.modelData.url;
                                metaDialog.open();
                            }
                            QQC2.ToolTip.text: tx.i18n("Edits the name and the subscription link.")
                            QQC2.ToolTip.visible: hovered
                        }

                        QQC2.ToolButton {
                            icon.name: "view-refresh"
                            display: QQC2.AbstractButton.IconOnly
                            enabled: !kcm.client.busy && profileDelegate.modelData.url.length > 0
                            Kirigami.MnemonicData.enabled: false
                            onClicked: kcm.client.updateProfile(profileDelegate.modelData.id)
                            QQC2.ToolTip.text: tx.i18n("Updates this profile from its subscription.")
                            QQC2.ToolTip.visible: hovered
                        }

                        QQC2.ToolButton {
                            icon.name: "list-remove"
                            display: QQC2.AbstractButton.IconOnly
                            enabled: !kcm.client.busy
                            Kirigami.MnemonicData.enabled: false
                            onClicked: {
                                removeDialog.profileId = profileDelegate.modelData.id;
                                removeDialog.open();
                            }
                            QQC2.ToolTip.text: tx.i18n("Removes this profile.")
                            QQC2.ToolTip.visible: hovered
                        }
                    }
                }
                    }
                }
            }

            ColumnLayout {
                Layout.fillWidth: true
                Layout.fillHeight: true
                Layout.preferredWidth: Kirigami.Units.gridUnit * 22
                Layout.minimumWidth: Kirigami.Units.gridUnit * 14
                Layout.horizontalStretchFactor: 1
                spacing: Kirigami.Units.largeSpacing

                Item {
                    Layout.fillWidth: true
                    Layout.preferredHeight: profileSearch.implicitHeight
                    visible: profileSearch.visible

                    Kirigami.Heading {
                        anchors.left: parent.left
                        anchors.right: parent.right
                        anchors.verticalCenter: parent.verticalCenter
                        text: tx.i18n("Proxy providers")
                        level: 3
                        elide: Text.ElideRight
                    }
                }

                Kirigami.Heading {
                    visible: !profileSearch.visible
                    text: tx.i18n("Proxy providers")
                    level: 3
                    Layout.fillWidth: true
                }

                Item {
                    Layout.fillWidth: true
                    Layout.fillHeight: true

                    QQC2.ScrollView {
                        anchors.fill: parent
                        Kirigami.StyleHints.showFramedBackground: true

                        ListView {
                            clip: true
                            reuseItems: true
                            model: kcm.client.proxyProviders
                            boundsBehavior: Flickable.StopAtBounds

                        delegate: QQC2.ItemDelegate {
                            id: providerDelegate
                            required property var modelData
                            width: ListView.view ? ListView.view.width : 0
                            Kirigami.MnemonicData.enabled: false

                            readonly property string updated: {
                                const text = String(modelData.updated_at || "");
                                if (text.length === 0) {
                                    return tx.i18n("Not updated yet");
                                }
                                const date = new Date(text);
                                if (isNaN(date.getTime()) || date.getFullYear() < 1970) {
                                    return tx.i18n("Not updated yet");
                                }
                                return date.toLocaleString(Qt.locale(), Locale.ShortFormat);
                            }

                            contentItem: RowLayout {
                                spacing: Kirigami.Units.largeSpacing

                                ColumnLayout {
                                    Layout.fillWidth: true
                                    spacing: 0

                                    QQC2.Label {
                                        text: providerDelegate.modelData.name
                                        font.bold: true
                                        elide: Text.ElideRight
                                        Layout.fillWidth: true
                                        Kirigami.MnemonicData.enabled: false
                                    }
                                    QQC2.Label {
                                        visible: !kcm.client.compact
                                        text: tx.i18n("Last update: %1", providerDelegate.updated)
                                        opacity: 0.7
                                        font: Kirigami.Theme.smallFont
                                        Layout.fillWidth: true
                                        Kirigami.MnemonicData.enabled: false
                                    }
                                    QQC2.Label {
                                        visible: !kcm.client.compact && providerDelegate.modelData.count >= 0
                                        text: tx.i18n("%1 nodes", tx.formatNumber(providerDelegate.modelData.count))
                                        opacity: 0.7
                                        font: Kirigami.Theme.smallFont
                                        Layout.fillWidth: true
                                        Kirigami.MnemonicData.enabled: false
                                    }
                                }

                                QQC2.ToolButton {
                                    icon.name: "view-refresh"
                                    display: QQC2.AbstractButton.IconOnly
                                    enabled: kcm.client.coreRunning && !kcm.client.busy
                                    Kirigami.MnemonicData.enabled: false
                                    onClicked: kcm.client.updateProxyProvider(providerDelegate.modelData.name)

                                    QQC2.ToolTip.visible: hovered
                                    QQC2.ToolTip.delay: 700
                                    QQC2.ToolTip.text: kcm.client.coreRunning ? tx.i18n("Updates this proxy provider.") : tx.i18n("Start the core first.")
                                }
                            }
                        }
                        }
                    }

                    QQC2.Label {
                        anchors.left: parent.left
                        anchors.top: parent.top
                        anchors.right: parent.right
                        anchors.margins: Kirigami.Units.largeSpacing
                        visible: kcm.client.proxyProviders.length === 0
                        text: tx.i18n("No proxy providers")
                        wrapMode: Text.WordWrap
                        opacity: 0.7
                        Kirigami.MnemonicData.enabled: false
                    }
                }
            }
        }
    }
}
