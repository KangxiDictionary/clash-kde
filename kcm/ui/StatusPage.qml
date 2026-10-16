import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.kde.quickcharts.controls as ChartsControls

Item {
    id: root

    readonly property bool progress: kcm.client.message === "正在启动内核" || kcm.client.message === "正在下载地理数据"
    readonly property bool failed: kcm.client.message.length > 0 && !progress
    readonly property bool starting: kcm.client.coreDesired && !kcm.client.coreRunning && !failed
    property string dismissedMessage: ""
    onFailedChanged: {
        if (!failed) {
            dismissedMessage = "";
        }
    }
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
        if (progress) {
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
            id: errorBanner
            Layout.fillWidth: true
            visible: root.failed && kcm.client.message !== root.dismissedMessage
            type: Kirigami.MessageType.Error
            text: kcm.client.message
            showCloseButton: true
            actions: [
                Kirigami.Action {
                    text: tx.i18n("View log")
                    icon.name: "view-list-text"
                    onTriggered: {
                        const log = kcm.client.coreLog();
                        logView.text = log.length > 0 ? log : tx.i18n("There is no log yet.");
                        logDialog.open();
                    }
                }
            ]
            onVisibleChanged: {
                if (!visible && root.failed && kcm.client.message !== root.dismissedMessage) {
                    root.dismissedMessage = kcm.client.message;
                    visible = Qt.binding(function() {
                        return root.failed && kcm.client.message !== root.dismissedMessage;
                    });
                }
            }
        }

        Kirigami.InlineMessage {
            Layout.fillWidth: true
            visible: kcm.client.tunEnabled && kcm.client.firewallActive
            type: Kirigami.MessageType.Warning
            text: tx.i18n("The firewall is on, and it can drop TUN traffic.")
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
                        QQC2.ToolTip.text: tx.i18n("Other programs can use this address as it is.")
                    }
                }

                QQC2.Button {
                    text: tx.i18n("Restart")
                    icon.name: "view-refresh"
                    enabled: !kcm.client.busy
                    onClicked: kcm.client.restartCore()

                    QQC2.ToolTip.visible: hovered
                    QQC2.ToolTip.delay: 700
                    QQC2.ToolTip.text: tx.i18n("Restart the core with the configuration it already has.")
                }

                QQC2.Switch {
                    checked: kcm.client.draftCoreDesired
                    onToggled: {
                        if (checked !== kcm.client.draftCoreDesired) {
                            kcm.client.draftCoreDesired = checked;
                        }
                    }

                    QQC2.ToolTip.visible: hovered
                    QQC2.ToolTip.delay: 700
                    QQC2.ToolTip.text: tx.i18n("This switch only starts the local proxy.")
                }
            }
        }

        Kirigami.FormLayout {
            Layout.fillWidth: true

            Item {
                Kirigami.FormData.label: tx.i18n("System proxy")
                implicitWidth: proxySwitch.implicitWidth
                implicitHeight: proxySwitch.implicitHeight

                QQC2.Switch {
                    id: proxySwitch
                    anchors.fill: parent
                    checked: kcm.client.draftSystemProxyEnabled
                    enabled: checked || (kcm.client.draftCoreDesired && !kcm.client.draftTunEnabled)
                    onToggled: {
                        if (checked !== kcm.client.draftSystemProxyEnabled) {
                            kcm.client.draftSystemProxyEnabled = checked;
                        }
                    }
                }

                HoverHandler {
                    id: proxyHover
                }

                QQC2.ToolTip.visible: proxyHover.hovered
                QQC2.ToolTip.delay: 700
                QQC2.ToolTip.text: tx.i18n("This takes effect only while the core is running and TUN is off.")
            }

            QQC2.Switch {
                Kirigami.FormData.label: tx.i18n("TUN")
                checked: kcm.client.draftTunEnabled
                enabled: !kcm.client.busy
                onToggled: {
                    if (checked !== kcm.client.draftTunEnabled) {
                        kcm.client.draftTunEnabled = checked;
                    }
                }

                QQC2.ToolTip.visible: hovered
                QQC2.ToolTip.delay: 700
                QQC2.ToolTip.text: tx.i18n("This captures only this account's traffic, and the system proxy steps aside while it is on.")
            }

            QQC2.ComboBox {
                Kirigami.FormData.label: tx.i18n("Mode")
                model: [tx.i18n("Rule"), tx.i18n("Global"), tx.i18n("Direct")]
                readonly property var modes: ["rule", "global", "direct"]
                currentIndex: Math.max(0, modes.indexOf(kcm.client.draftMode))
                onActivated: index => {
                    const mode = modes[index];
                    if (mode !== kcm.client.draftMode) {
                        kcm.client.draftMode = mode;
                    }
                }
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
                Layout.fillWidth: true
                Layout.alignment: Qt.AlignTop
                spacing: Kirigami.Units.smallSpacing

                ChartsControls.LegendDelegate {
                    name: tx.i18n("Download")
                    value: root.formatRate(kcm.client.downloadBps)
                    color: Kirigami.Theme.positiveTextColor
                    maximumValueWidth: rateWidth.advanceWidth
                    Layout.fillWidth: true
                }

                ChartsControls.LegendDelegate {
                    name: tx.i18n("Upload")
                    value: root.formatRate(kcm.client.uploadBps)
                    color: Kirigami.Theme.visitedLinkColor
                    maximumValueWidth: rateWidth.advanceWidth
                    Layout.fillWidth: true
                }

                TextMetrics {
                    id: rateWidth
                    text: tx.i18n("%1 MB/s", "999.9")
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
                    QQC2.ToolTip.text: tx.i18n("See which region the traffic is leaving from.")
                }
            }
        }

        Item {
            Layout.fillHeight: true
        }

        QQC2.Label {
            Layout.fillWidth: true
            wrapMode: Text.WordWrap
            opacity: 0.65
            font: Kirigami.Theme.smallFont
            text: tx.i18n("There is a Mihomo widget for the panel.")
        }
    }

    QQC2.Dialog {
        id: logDialog
        title: tx.i18n("Core log")
        modal: true
        standardButtons: QQC2.Dialog.Close
        padding: Kirigami.Units.largeSpacing
        implicitWidth: Kirigami.Units.gridUnit * 40
        implicitHeight: Kirigami.Units.gridUnit * 28

        contentItem: Item {
            Flickable {
                id: logFlick
                anchors.fill: parent
                clip: true
                contentWidth: width
                contentHeight: logView.height
                boundsBehavior: Flickable.StopAtBounds
                QQC2.ScrollBar.vertical: QQC2.ScrollBar {
                    policy: QQC2.ScrollBar.AsNeeded
                }

                QQC2.TextArea {
                    id: logView
                    readOnly: true
                    selectByMouse: true
                    wrapMode: TextEdit.Wrap
                    width: logFlick.width
                    height: Math.max(logFlick.height, contentHeight)
                    padding: Kirigami.Units.smallSpacing

                    WheelHandler {
                        onWheel: function(event) {
                            const maxY = Math.max(0, logFlick.contentHeight - logFlick.height);
                            logFlick.contentY = Math.max(0, Math.min(maxY, logFlick.contentY - event.angleDelta.y));
                            event.accepted = true;
                        }
                    }
                }
            }
        }
    }
}
