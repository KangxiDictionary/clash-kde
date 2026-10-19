import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

Item {
    id: root

    Tx {
        id: tx
    }

    component SheetRow: RowLayout {
        id: row
        required property string label
        property string tip: ""
        default property alias extras: controls.data

        Layout.fillWidth: true
        spacing: Kirigami.Units.largeSpacing

        QQC2.Label {
            text: row.label
            Layout.alignment: Qt.AlignVCenter

            HoverHandler {
                id: labelHover
            }

            QQC2.ToolTip.visible: row.tip.length > 0 && labelHover.hovered
            QQC2.ToolTip.delay: 700
            QQC2.ToolTip.text: row.tip
        }

        Item {
            Layout.fillWidth: true
        }

        RowLayout {
            id: controls
            Layout.alignment: Qt.AlignVCenter
            spacing: Kirigami.Units.smallSpacing
        }
    }

    ColumnLayout {
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: parent.top
        spacing: Kirigami.Units.largeSpacing

        Kirigami.Heading {
            text: tx.i18n("Connection")
            level: 3
            Layout.fillWidth: true
        }

        Kirigami.Separator {
            Layout.fillWidth: true
        }

        SheetRow {
            label: tx.i18n("Test delay automatically")
            tip: tx.i18n("Tests the delay of the group you are looking at.")

            QQC2.Switch {
                checked: kcm.client.draftAutoTestDelay
                onToggled: kcm.client.draftAutoTestDelay = checked
            }
        }

        SheetRow {
            label: tx.i18n("Blur the IP")
            tip: tx.i18n("Hides the latter half of the exit address.")

            QQC2.Switch {
                checked: kcm.client.draftBlurIp
                onToggled: kcm.client.draftBlurIp = checked
            }
        }

        SheetRow {
            label: tx.i18n("Use the same sort for every group")
            tip: tx.i18n("Keeps one sort and filter for every group and profile.")

            QQC2.Switch {
                checked: kcm.client.draftSortGlobal
                onToggled: kcm.client.draftSortGlobal = checked
            }
        }

        SheetRow {
            label: tx.i18n("Mixed port")
            tip: tx.i18n("HTTP and SOCKS share this port, and saving restarts a running core.")

            QQC2.SpinBox {
                from: 1
                to: 65535
                editable: true
                value: kcm.client.draftMixedPort
                onValueModified: kcm.client.draftMixedPort = value
            }
        }

        SheetRow {
            label: tx.i18n("Controller port")
            tip: tx.i18n("This page talks to the controller, which listens only on this computer.")

            QQC2.SpinBox {
                from: 1
                to: 65535
                editable: true
                value: kcm.client.draftControllerPort
                onValueModified: kcm.client.draftControllerPort = value
            }
        }

        SheetRow {
            label: tx.i18n("mihomo executable")
            tip: tx.i18n("This is the mihomo program already installed on the system.")

            QQC2.TextField {
                text: kcm.client.draftMihomoPath
                implicitWidth: Kirigami.Units.gridUnit * 12
                onTextEdited: kcm.client.draftMihomoPath = text
            }
        }

        SheetRow {
            label: tx.i18n("TUN device")
            tip: tx.i18n("Leave this empty to let the kernel choose the name, and applying a new name asks you to confirm.")

            QQC2.TextField {
                text: kcm.client.draftTunDevice
                implicitWidth: Kirigami.Units.gridUnit * 8
                maximumLength: 15
                onTextEdited: kcm.client.draftTunDevice = text
            }
        }

        SheetRow {
            label: tx.i18n("Override file")
            tip: tx.i18n("This file is placed over the subscription, while the listen address stays set here.")

            QQC2.TextField {
                id: overrideField
                text: kcm.client.draftOverridePath
                implicitWidth: Kirigami.Units.gridUnit * 12
                onTextEdited: kcm.client.draftOverridePath = text
            }

            QQC2.Button {
                text: tx.i18n("Open file")
                onClicked: kcm.client.openOverrideFile(overrideField.text)

                QQC2.ToolTip.visible: hovered
                QQC2.ToolTip.delay: 700
                QQC2.ToolTip.text: tx.i18n("Opens the override file, creating it from the template when it is missing.")
            }

            QQC2.Button {
                text: tx.i18n("Open folder")
                onClicked: kcm.client.openOverrideFolder(overrideField.text)

                QQC2.ToolTip.visible: hovered
                QQC2.ToolTip.delay: 700
                QQC2.ToolTip.text: tx.i18n("Opens the folder that contains the override file.")
            }
        }

        Kirigami.Heading {
            text: tx.i18n("About")
            level: 3
            Layout.fillWidth: true
        }

        Kirigami.Separator {
            Layout.fillWidth: true
        }

        SheetRow {
            label: tx.i18n("Name")

            QQC2.Label {
                text: tx.i18n("Mihomo")
            }
        }

        SheetRow {
            label: tx.i18n("Version")

            QQC2.Label {
                text: "1.0.1"
            }
        }

        SheetRow {
            label: tx.i18n("License")

            QQC2.Label {
                text: "GNU GPL v3"
            }
        }

        SheetRow {
            label: tx.i18n("Website")

            QQC2.Label {
                text: "<a href=\"https://github.com/KangxiDictionary/clash-kde\">github.com/KangxiDictionary/clash-kde</a>"
                textFormat: Text.RichText
                onLinkActivated: link => Qt.openUrlExternally(link)
            }
        }

        Item {
            Layout.fillHeight: true
        }
    }
}
