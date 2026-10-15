import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

Item {
    id: root

    readonly property bool failed: kcm.client.message.indexOf("\n") >= 0
    readonly property bool starting: kcm.client.coreDesired && !kcm.client.coreRunning && !failed
    readonly property string stateText: {
        if (kcm.client.coreRunning) {
            return tx.i18n("Running");
        }
        if (starting) {
            return tx.i18n("Waiting to start");
        }
        return tx.i18n("Stopped");
    }
    readonly property string detailText: {
        if (starting && kcm.client.message.length > 0) {
            return kcm.client.message;
        }
        const http = kcm.client.listenAddress;
        return http + "\n" + http.replace("http://", "socks5://");
    }

    Tx {
        id: tx
    }

    function formatRate(bps) {
        if (bps < 1024) {
            return tx.i18n("%1 B/s", bps);
        }
        if (bps < 1024 * 1024) {
            return tx.i18n("%1 KB/s", (bps / 1024).toFixed(1));
        }
        return tx.i18n("%1 MB/s", (bps / 1024 / 1024).toFixed(2));
    }

    ColumnLayout {
        anchors.fill: parent
        spacing: Kirigami.Units.largeSpacing

        Kirigami.InlineMessage {
            Layout.fillWidth: true
            visible: kcm.client.message.length > 0 && root.failed
            type: Kirigami.MessageType.Error
            text: kcm.client.message
        }

        Rectangle {
            Layout.fillWidth: true
            implicitHeight: bannerRow.implicitHeight + Kirigami.Units.largeSpacing * 2
            radius: Kirigami.Units.cornerRadius
            color: kcm.client.coreRunning ? Kirigami.Theme.positiveBackgroundColor : Kirigami.Theme.alternateBackgroundColor

            RowLayout {
                id: bannerRow
                width: parent.width - Kirigami.Units.largeSpacing * 2
                anchors.centerIn: parent
                spacing: Kirigami.Units.largeSpacing

                Kirigami.Icon {
                    source: kcm.client.coreRunning ? "media-playback-start" : "media-playback-stop"
                    Layout.preferredWidth: Kirigami.Units.iconSizes.medium
                    Layout.preferredHeight: Kirigami.Units.iconSizes.medium
                }

                ColumnLayout {
                    Layout.fillWidth: true
                    spacing: Kirigami.Units.smallSpacing

                    QQC2.Label {
                        text: root.stateText
                        font.bold: true
                        Layout.fillWidth: true
                    }

                    QQC2.Label {
                        text: root.detailText
                        wrapMode: Text.WordWrap
                        opacity: 0.75
                        font: Kirigami.Theme.smallFont
                        Layout.fillWidth: true

                        HoverHandler {
                            id: listenHover
                        }

                        QQC2.ToolTip.visible: listenHover.hovered
                        QQC2.ToolTip.delay: 700
                        QQC2.ToolTip.text: tx.i18n("Applications can use this address directly.")
                    }
                }

                QQC2.Button {
                    text: tx.i18n("Restart")
                    icon.name: "view-refresh"
                    enabled: !kcm.client.busy
                    onClicked: kcm.client.restartCore()

                    QQC2.ToolTip.visible: hovered
                    QQC2.ToolTip.delay: 700
                    QQC2.ToolTip.text: tx.i18n("Restart with the current configuration.")
                }

                QQC2.Switch {
                    checked: kcm.client.coreDesired
                    onToggled: {
                        if (checked !== kcm.client.coreDesired) {
                            kcm.client.setCoreDesired(checked);
                        }
                    }

                    QQC2.ToolTip.visible: hovered
                    QQC2.ToolTip.delay: 700
                    QQC2.ToolTip.text: tx.i18n("Only starts the local proxy. System proxy is a separate switch.")
                }
            }
        }

        RowLayout {
            Layout.fillWidth: true
            spacing: Kirigami.Units.largeSpacing

            QQC2.Label {
                text: tx.i18n("System proxy")
            }

            QQC2.Switch {
                checked: kcm.client.systemProxyEnabled
                onToggled: {
                    if (checked !== kcm.client.systemProxyEnabled) {
                        kcm.client.setSystemProxyEnabled(checked);
                    }
                }

                QQC2.ToolTip.visible: hovered
                QQC2.ToolTip.delay: 700
                QQC2.ToolTip.text: tx.i18n("While the core is running, KDE and browsers that follow the system proxy use it. Turning the core off withdraws it.")
            }

            QQC2.Label {
                text: kcm.client.systemProxyApplied ? tx.i18n("Applied") : tx.i18n("Not applied")
                opacity: 0.7
            }

            Item {
                Layout.fillWidth: true
            }

            QQC2.Label {
                text: tx.i18n("Mode")
            }

            QQC2.ComboBox {
                id: modeBox
                model: [tx.i18n("Rule"), tx.i18n("Global"), tx.i18n("Direct")]
                readonly property var modes: ["rule", "global", "direct"]
                currentIndex: Math.max(0, modes.indexOf(kcm.client.mode))
                onActivated: index => {
                    const mode = modes[index];
                    if (mode !== kcm.client.mode) {
                        kcm.client.setMode(mode);
                    }
                }

                QQC2.ToolTip.visible: hovered
                QQC2.ToolTip.delay: 700
                QQC2.ToolTip.text: tx.i18n("Rule follows the profile. Global sends everything through the proxy. Direct connects directly.")
            }
        }

        Kirigami.Separator {
            Layout.fillWidth: true
        }

        Kirigami.Heading {
            text: tx.i18n("Traffic")
            level: 3
            Layout.fillWidth: true
        }

        RowLayout {
            Layout.fillWidth: true
            spacing: Kirigami.Units.largeSpacing * 2

            ColumnLayout {
                spacing: Kirigami.Units.smallSpacing

                RowLayout {
                    spacing: Kirigami.Units.smallSpacing
                    Kirigami.Icon {
                        source: "arrow-up"
                        Layout.preferredWidth: Kirigami.Units.iconSizes.small
                        Layout.preferredHeight: Kirigami.Units.iconSizes.small
                    }
                    QQC2.Label {
                        text: tx.i18n("Upload")
                    }
                }
                Kirigami.Heading {
                    level: 2
                    text: root.formatRate(kcm.client.uploadBps)
                }
            }

            ColumnLayout {
                spacing: Kirigami.Units.smallSpacing

                RowLayout {
                    spacing: Kirigami.Units.smallSpacing
                    Kirigami.Icon {
                        source: "arrow-down"
                        Layout.preferredWidth: Kirigami.Units.iconSizes.small
                        Layout.preferredHeight: Kirigami.Units.iconSizes.small
                    }
                    QQC2.Label {
                        text: tx.i18n("Download")
                    }
                }
                Kirigami.Heading {
                    level: 2
                    text: root.formatRate(kcm.client.downloadBps)
                }
            }

            Item {
                Layout.fillWidth: true
            }

            ColumnLayout {
                spacing: Kirigami.Units.smallSpacing
                Layout.maximumWidth: Kirigami.Units.gridUnit * 16

                QQC2.Label {
                    text: tx.i18n("Exit region")
                }
                Kirigami.Heading {
                    level: 2
                    text: kcm.client.exitRegion.length > 0 ? kcm.client.exitRegion : tx.i18n("Not checked yet")
                    wrapMode: Text.WordWrap
                    Layout.fillWidth: true
                }
                QQC2.Button {
                    text: tx.i18n("Check region")
                    enabled: kcm.client.coreRunning && !kcm.client.busy
                    onClicked: kcm.client.refreshExitRegion()

                    QQC2.ToolTip.visible: hovered
                    QQC2.ToolTip.delay: 700
                    QQC2.ToolTip.text: tx.i18n("Look up the exit region through the current proxy.")
                }
            }
        }

        Item {
            Layout.fillHeight: true
        }
    }
}
