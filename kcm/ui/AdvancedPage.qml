import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

Item {
    id: root

    function noteDraftEdited() {
        kcm.needsSave = true;
        kcm.representsDefaults = kcm.client.draftMixedPort === 7890
            && kcm.client.draftControllerPort === 9090
            && kcm.client.draftMihomoPath === "mihomo";
    }

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
                    root.noteDraftEdited();
                }

                QQC2.ToolTip.visible: hovered
                QQC2.ToolTip.delay: 700
                QQC2.ToolTip.text: tx.i18n("HTTP and SOCKS share this port, and saving restarts a running core.")
            }

            QQC2.SpinBox {
                Kirigami.FormData.label: tx.i18n("Controller port")
                from: 1
                to: 65535
                editable: true
                value: kcm.client.draftControllerPort
                onValueModified: {
                    kcm.client.draftControllerPort = value;
                    root.noteDraftEdited();
                }

                QQC2.ToolTip.visible: hovered
                QQC2.ToolTip.delay: 700
                QQC2.ToolTip.text: tx.i18n("This page talks to the controller, which listens only on this computer.")
            }

            QQC2.TextField {
                Kirigami.FormData.label: tx.i18n("mihomo executable")
                text: kcm.client.draftMihomoPath
                implicitWidth: Kirigami.Units.gridUnit * 16
                onTextEdited: {
                    kcm.client.draftMihomoPath = text;
                    root.noteDraftEdited();
                }

                QQC2.ToolTip.visible: hovered
                QQC2.ToolTip.delay: 700
                QQC2.ToolTip.text: tx.i18n("This is the mihomo program already installed on the system.")
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
                QQC2.ToolTip.text: tx.i18n("This file is placed over the subscription, while the listen address stays set here.")
            }
        }

        Kirigami.Heading {
            text: tx.i18n("About")
            level: 3
            Layout.fillWidth: true
            Layout.topMargin: Kirigami.Units.largeSpacing
        }

        Kirigami.Separator {
            Layout.fillWidth: true
        }

        Kirigami.FormLayout {
            Layout.fillWidth: true

            QQC2.Label {
                Kirigami.FormData.label: tx.i18n("Name")
                text: tx.i18n("Mihomo")
            }

            QQC2.Label {
                Kirigami.FormData.label: tx.i18n("Version")
                text: "0.1.0"
            }

            QQC2.Label {
                Kirigami.FormData.label: tx.i18n("License")
                text: "GNU GPL v3"
            }

            QQC2.Label {
                Kirigami.FormData.label: tx.i18n("Website")
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
