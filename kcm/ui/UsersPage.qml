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
        return "—";
    }

    readonly property bool coreError: {
        const message = kcm.client.message;
        return message.length > 0 && message !== "正在启动内核" && message !== "正在下载地理数据";
    }

    QQC2.Label {
        Layout.fillWidth: true
        wrapMode: Text.WordWrap
        text: tx.i18n("This page starts or stops the core of a signed-in account.")
    }

    Kirigami.InlineMessage {
        Layout.fillWidth: true
        visible: kcm.client.accountsError.length > 0 || coreError
        type: Kirigami.MessageType.Error
        text: kcm.client.accountsError.length > 0 ? kcm.client.accountsError : kcm.client.message
    }

    QQC2.Label {
        Layout.fillWidth: true
        visible: kcm.client.accounts.length === 0 && kcm.client.accountsError.length === 0
        text: tx.i18n("No signed-in accounts.")
        opacity: 0.65
    }

    Repeater {
        model: kcm.client.accounts

        Rectangle {
            id: card
            required property var modelData
            Layout.fillWidth: true
            implicitHeight: cardBody.implicitHeight + Kirigami.Units.largeSpacing * 2
            radius: Kirigami.Units.cornerRadius
            color: Kirigami.Theme.alternateBackgroundColor

            ColumnLayout {
                id: cardBody
                width: parent.width - Kirigami.Units.largeSpacing * 2
                anchors.centerIn: parent
                spacing: Kirigami.Units.largeSpacing

                RowLayout {
                    Layout.fillWidth: true
                    spacing: Kirigami.Units.largeSpacing

                    Kirigami.Icon {
                        source: "user-identity"
                        Layout.preferredWidth: Kirigami.Units.iconSizes.medium
                        Layout.preferredHeight: Kirigami.Units.iconSizes.medium
                    }

                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: Kirigami.Units.smallSpacing

                        RowLayout {
                            QQC2.Label {
                                text: card.modelData.name
                                font.bold: true
                                elide: Text.ElideRight
                            }
                            QQC2.Label {
                                visible: card.modelData.uid === kcm.client.ownUid
                                text: tx.i18n("This account")
                                opacity: 0.65
                            }
                        }

                        QQC2.Label {
                            text: card.modelData.core_running ? tx.i18n("The core is running.") : tx.i18n("The core is stopped.")
                            color: card.modelData.core_running ? Kirigami.Theme.positiveTextColor : Kirigami.Theme.neutralTextColor
                        }
                    }
                }

                Kirigami.InlineMessage {
                    Layout.fillWidth: true
                    visible: card.modelData.error !== undefined && card.modelData.error.length > 0
                    type: Kirigami.MessageType.Error
                    text: card.modelData.error || ""
                }

                GridLayout {
                    Layout.fillWidth: true
                    columns: 4
                    columnSpacing: Kirigami.Units.largeSpacing
                    rowSpacing: Kirigami.Units.smallSpacing

                    QQC2.Label {
                        text: tx.i18n("Traffic capture")
                        opacity: 0.65
                    }
                    QQC2.Label {
                        text: card.modelData.tun_enabled ? tx.i18n("On") : tx.i18n("Off")
                    }
                    QQC2.Label {
                        text: tx.i18n("System proxy")
                        opacity: 0.65
                    }
                    QQC2.Label {
                        text: card.modelData.system_proxy_enabled ? tx.i18n("On") : tx.i18n("Off")
                    }
                    QQC2.Label {
                        text: tx.i18n("Mode")
                        opacity: 0.65
                    }
                    QQC2.Label {
                        text: modeText(card.modelData.mode)
                        Layout.columnSpan: 3
                    }
                }

                QQC2.Button {
                    Layout.fillWidth: true
                    text: card.modelData.core_running ? tx.i18n("Stop this account's core") : tx.i18n("Start this account's core")
                    icon.name: card.modelData.core_running ? "media-playback-stop" : "media-playback-start"
                    enabled: !kcm.client.busy
                    onClicked: card.modelData.core_running ? kcm.client.stopAccount(card.modelData.uid) : kcm.client.startAccount(card.modelData.uid)
                }
            }
        }
    }

    Item {
        Layout.fillHeight: true
    }
}
