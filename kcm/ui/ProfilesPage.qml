import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

Item {
    Tx {
        id: tx
    }

    function formatUpdated(unix) {
        if (!unix) {
            return tx.i18n("Not updated yet");
        }
        return new Date(unix * 1000).toLocaleString(Qt.locale(), Locale.ShortFormat);
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
        id: removeDialog
        property string profileId: ""
        title: tx.i18n("Remove this subscription?")
        modal: true
        standardButtons: QQC2.Dialog.Yes | QQC2.Dialog.No
        onAccepted: kcm.client.removeProfile(profileId)
    }

    ColumnLayout {
        anchors.fill: parent
        spacing: Kirigami.Units.largeSpacing

        RowLayout {
            Layout.fillWidth: true

            Kirigami.Heading {
                text: tx.i18n("Subscriptions")
                level: 3
            }

            Item {
                Layout.fillWidth: true
            }

            QQC2.Button {
                text: tx.i18n("Add subscription")
                icon.name: "list-add"
                enabled: !kcm.client.busy
                onClicked: addDialog.open()
            }
        }

        Kirigami.Separator {
            Layout.fillWidth: true
        }

        QQC2.Label {
            visible: kcm.client.profiles.length === 0
            text: tx.i18n("No subscriptions yet")
            opacity: 0.7
            Layout.fillWidth: true
        }

        ListView {
            Layout.fillWidth: true
            Layout.fillHeight: true
            clip: true
            model: kcm.client.profiles

            delegate: QQC2.ItemDelegate {
                width: ListView.view.width
                required property var modelData
                highlighted: modelData.id === kcm.client.activeProfile

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
                            text: modelData.url
                            opacity: 0.7
                            elide: Text.ElideMiddle
                            font: Kirigami.Theme.smallFont
                            Layout.fillWidth: true
                        }
                        QQC2.Label {
                            text: tx.i18n("Last update: %1", formatUpdated(modelData.updated_unix))
                            opacity: 0.7
                            font: Kirigami.Theme.smallFont
                            Layout.fillWidth: true
                        }
                    }

                    QQC2.Button {
                        text: tx.i18n("Use")
                        visible: modelData.id !== kcm.client.activeProfile
                        enabled: !kcm.client.busy
                        onClicked: kcm.client.setActiveProfile(modelData.id)
                    }
                    QQC2.Button {
                        text: tx.i18n("Update")
                        enabled: !kcm.client.busy
                        onClicked: kcm.client.updateProfile(modelData.id)
                    }
                    QQC2.Button {
                        text: tx.i18n("Remove")
                        enabled: !kcm.client.busy
                        onClicked: {
                            removeDialog.profileId = modelData.id;
                            removeDialog.open();
                        }
                    }
                }
            }
        }
    }
}
