import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

ColumnLayout {
    spacing: Kirigami.Units.largeSpacing

    Tx {
        id: tx
    }

    function modeText(mode) {
        if (mode === "global") {
            return tx.i18n("Global");
        }
        if (mode === "direct") {
            return tx.i18n("Direct");
        }
        if (mode === "rule") {
            return tx.i18n("Rule");
        }
        return "";
    }

    QQC2.Label {
        Layout.fillWidth: true
        wrapMode: Text.WordWrap
        text: tx.i18n("This page starts and stops Mihomo for accounts that are signed in.")
    }

    Kirigami.InlineMessage {
        Layout.fillWidth: true
        visible: kcm.client.accountsError.length > 0
        type: Kirigami.MessageType.Error
        text: kcm.client.accountsError
    }

    RowLayout {
        Layout.fillWidth: true
        spacing: Kirigami.Units.largeSpacing

        QQC2.Label {
            Layout.preferredWidth: Kirigami.Units.gridUnit * 6
            text: tx.i18n("Account")
            opacity: 0.65
        }
        QQC2.Label {
            Layout.preferredWidth: Kirigami.Units.gridUnit * 4
            text: tx.i18n("Core")
            opacity: 0.65
        }
        QQC2.Label {
            Layout.preferredWidth: Kirigami.Units.gridUnit * 5
            text: tx.i18n("TUN")
            opacity: 0.65
        }
        QQC2.Label {
            Layout.fillWidth: true
            text: tx.i18n("System proxy")
            opacity: 0.65
        }
        QQC2.Label {
            Layout.preferredWidth: Kirigami.Units.gridUnit * 4
            text: tx.i18n("Mode")
            opacity: 0.65
        }
    }

    Column {
        Layout.fillWidth: true
        spacing: Kirigami.Units.smallSpacing

        Repeater {
            model: kcm.client.accounts

            RowLayout {
                required property var modelData
                width: parent.width
                spacing: Kirigami.Units.largeSpacing

                QQC2.Label {
                    Layout.preferredWidth: Kirigami.Units.gridUnit * 6
                    text: modelData.name
                    elide: Text.ElideRight
                }
                QQC2.Label {
                    Layout.preferredWidth: Kirigami.Units.gridUnit * 4
                    text: modelData.core_running ? tx.i18n("Running") : tx.i18n("Stopped")
                }
                QQC2.Label {
                    Layout.preferredWidth: Kirigami.Units.gridUnit * 5
                    text: modelData.tun_enabled ? tx.i18n("On") : tx.i18n("Off")
                }
                QQC2.Label {
                    Layout.fillWidth: true
                    text: modelData.system_proxy_enabled ? tx.i18n("On") : tx.i18n("Off")
                }
                QQC2.Label {
                    Layout.preferredWidth: Kirigami.Units.gridUnit * 4
                    text: modeText(modelData.mode)
                }
                QQC2.Button {
                    text: tx.i18n("Start")
                    enabled: !kcm.client.busy
                    onClicked: kcm.client.startAccount(modelData.uid)
                }
                QQC2.Button {
                    text: tx.i18n("Stop")
                    enabled: !kcm.client.busy
                    onClicked: kcm.client.stopAccount(modelData.uid)
                }
            }
        }
    }

    Item {
        Layout.fillHeight: true
    }
}
