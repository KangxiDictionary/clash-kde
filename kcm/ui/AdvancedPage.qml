import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

Item {
    Tx {
        id: tx
    }

    ColumnLayout {
        anchors.fill: parent
        spacing: Kirigami.Units.largeSpacing

        Kirigami.Heading {
            text: tx.i18n("Connection")
            level: 3
            Layout.fillWidth: true
        }

        Kirigami.Separator {
            Layout.fillWidth: true
        }

        Kirigami.FormLayout {
            Layout.fillWidth: true

            QQC2.SpinBox {
                Kirigami.FormData.label: tx.i18n("Mixed port")
                from: 1
                to: 65535
                editable: true
                value: kcm.client.draftMixedPort
                onValueModified: {
                    kcm.client.draftMixedPort = value;
                    kcm.needsSave = true;
                }

                QQC2.ToolTip.visible: hovered
                QQC2.ToolTip.delay: 700
                QQC2.ToolTip.text: tx.i18n("HTTP and SOCKS share this port. Applying restarts a running core.")
            }

            QQC2.SpinBox {
                Kirigami.FormData.label: tx.i18n("Controller port")
                from: 1
                to: 65535
                editable: true
                value: kcm.client.draftControllerPort
                onValueModified: {
                    kcm.client.draftControllerPort = value;
                    kcm.needsSave = true;
                }

                QQC2.ToolTip.visible: hovered
                QQC2.ToolTip.delay: 700
                QQC2.ToolTip.text: tx.i18n("Used by this page. It listens only on this computer.")
            }

            QQC2.TextField {
                Kirigami.FormData.label: tx.i18n("mihomo executable")
                text: kcm.client.draftMihomoPath
                implicitWidth: Kirigami.Units.gridUnit * 16
                onTextEdited: {
                    kcm.client.draftMihomoPath = text;
                    kcm.needsSave = true;
                }

                QQC2.ToolTip.visible: hovered
                QQC2.ToolTip.delay: 700
                QQC2.ToolTip.text: tx.i18n("The mihomo program installed on the system.")
            }

            Kirigami.SelectableLabel {
                Kirigami.FormData.label: tx.i18n("Override file")
                text: kcm.client.overridePath
                wrapMode: Text.WrapAnywhere

                HoverHandler {
                    id: overrideHover
                }

                QQC2.ToolTip.visible: overrideHover.hovered
                QQC2.ToolTip.delay: 700
                QQC2.ToolTip.text: tx.i18n("Merged on top of the subscription. The listen address is still set here.")
            }
        }

        Item {
            Layout.fillHeight: true
        }
    }
}
