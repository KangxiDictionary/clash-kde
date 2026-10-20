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

    readonly property var shownProfiles: {
        const query = root.profileQuery;
        const all = kcm.client.profiles;
        if (query.trim().length === 0) {
            return all;
        }
        const ranked = [];
        for (let i = 0; i < all.length; ++i) {
            const weight = fuzzy.score(query, all[i].name);
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
            return tx.i18n("%1 B", bytes);
        }
        if (bytes < 1024 * 1024) {
            return tx.i18n("%1 KB", (bytes / 1024).toFixed(1));
        }
        if (bytes < 1024 * 1024 * 1024) {
            return tx.i18n("%1 MB", (bytes / 1024 / 1024).toFixed(1));
        }
        return tx.i18n("%1 GB", (bytes / 1024 / 1024 / 1024).toFixed(2));
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

        onAccepted: {
            kcm.client.addSubscription(nameField.text, urlField.text);
            nameField.text = "";
            urlField.text = "";
        }

        contentItem: Kirigami.FormLayout {
            QQC2.TextField {
                id: nameField
                Kirigami.FormData.label: tx.i18n("Name")
                placeholderText: tx.i18n("Optional")
            }
            QQC2.TextField {
                id: urlField
                Kirigami.FormData.label: tx.i18n("URL")
                placeholderText: "https://"
                implicitWidth: Kirigami.Units.gridUnit * 18
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

        onAccepted: kcm.client.setProfileMeta(profileId, metaName.text, metaUrl.text)

        contentItem: Kirigami.FormLayout {
            QQC2.TextField {
                id: metaName
                Kirigami.FormData.label: tx.i18n("Name")
            }
            QQC2.TextField {
                id: metaUrl
                Kirigami.FormData.label: tx.i18n("URL")
                placeholderText: "https://"
                implicitWidth: Kirigami.Units.gridUnit * 18
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

        Kirigami.SearchField {
            Layout.fillWidth: true
            visible: kcm.client.profiles.length > 0
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

        ListView {
            Layout.fillWidth: true
            Layout.fillHeight: true
            clip: true
            model: root.shownProfiles

            delegate: QQC2.RadioDelegate {
                id: profileDelegate
                width: ListView.view.width
                required property var modelData
                checked: modelData.id === kcm.client.draftActiveProfile
                onClicked: kcm.client.draftActiveProfile = modelData.id

                contentItem: RowLayout {
                    spacing: Kirigami.Units.largeSpacing

                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 0

                        QQC2.Label {
                            text: modelData.name
                            font.bold: true
                            elide: Text.ElideRight
                            Layout.fillWidth: true
                        }
                        QQC2.Label {
                            visible: !kcm.client.compact && modelData.url.length > 0
                            text: modelData.url
                            opacity: 0.7
                            elide: Text.ElideMiddle
                            font: Kirigami.Theme.smallFont
                            Layout.fillWidth: true
                        }
                        QQC2.Label {
                            readonly property string quota: quotaText(modelData)
                            visible: !kcm.client.compact && quota.length > 0
                            text: quota
                            opacity: 0.7
                            font: Kirigami.Theme.smallFont
                            Layout.fillWidth: true
                        }
                        QQC2.Label {
                            text: tx.i18n("Last update: %1", formatUpdated(modelData.updated_unix))
                            visible: !kcm.client.compact
                            opacity: 0.7
                            font: Kirigami.Theme.smallFont
                            Layout.fillWidth: true
                        }
                    }

                    QQC2.ToolButton {
                        Kirigami.MnemonicData.enabled: false
                        id: moreButton
                        icon.name: "overflow-menu"
                        display: QQC2.AbstractButton.IconOnly
                        enabled: !kcm.client.busy
                        onClicked: profileMenu.popup(moreButton, 0, height)

                        QQC2.ToolTip.visible: hovered
                        QQC2.ToolTip.delay: 700
                        QQC2.ToolTip.text: tx.i18n("More actions")

                        QQC2.Menu {
                            id: profileMenu
                            QQC2.MenuItem {
                                Kirigami.MnemonicData.enabled: false
                                text: tx.i18n("Update")
                                enabled: modelData.url.length > 0
                                onTriggered: kcm.client.updateProfile(modelData.id)
                            }
                            QQC2.MenuItem {
                                Kirigami.MnemonicData.enabled: false
                                text: tx.i18n("Edit details")
                                onTriggered: {
                                    metaDialog.profileId = modelData.id;
                                    metaName.text = modelData.name;
                                    metaUrl.text = modelData.url;
                                    metaDialog.open();
                                }
                            }
                            QQC2.MenuItem {
                                Kirigami.MnemonicData.enabled: false
                                text: tx.i18n("Remove")
                                onTriggered: {
                                    removeDialog.profileId = modelData.id;
                                    removeDialog.open();
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
