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
            return tx.i18n("%1 B/s", tx.formatNumber(bps));
        }
        if (bps < 1024 * 1024) {
            return tx.i18n("%1 KB/s", tx.formatNumber(bps / 1024, 1));
        }
        return tx.i18n("%1 MB/s", tx.formatNumber(bps / 1024 / 1024, 2));
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
                    Kirigami.MnemonicData.enabled: false
                    text: tx.i18n("Restart")
                    icon.name: "view-refresh"
                    enabled: kcm.client.coreRunning && !kcm.client.busy
                    onClicked: kcm.client.restartCore()

                    QQC2.ToolTip.visible: hovered
                    QQC2.ToolTip.delay: 700
                    QQC2.ToolTip.text: tx.i18n("Restart the core with the configuration it already has.")
                }

                QQC2.Switch {
                    Kirigami.MnemonicData.enabled: false
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

        RowLayout {
            Layout.fillWidth: true
            spacing: Kirigami.Units.largeSpacing

            QQC2.Label {
                text: (kcm.client.tunEnabled && kcm.client.tunMachine) || kcm.client.draftTunEnabled ? tx.i18n("Traffic capture") : tx.i18n("System proxy")
                Kirigami.MnemonicData.enabled: false
            }

            QQC2.Switch {
                id: proxySwitch
                readonly property bool machineCapture: kcm.client.tunEnabled && kcm.client.tunMachine
                readonly property bool personalCapture: kcm.client.draftTunEnabled && !machineCapture
                checked: machineCapture || personalCapture ? true : kcm.client.draftSystemProxyEnabled
                enabled: !machineCapture && (personalCapture || checked || (kcm.client.draftCoreDesired && !kcm.client.draftTunEnabled && !kcm.client.tunMachine))
                Kirigami.MnemonicData.enabled: false
                onToggled: {
                    if (machineCapture) {
                        return;
                    }
                    if (personalCapture) {
                        if (!checked) {
                            kcm.client.draftTunEnabled = false;
                        }
                        return;
                    }
                    if (checked !== kcm.client.draftSystemProxyEnabled) {
                        kcm.client.draftSystemProxyEnabled = checked;
                    }
                }

                QQC2.ToolTip.visible: hovered
                QQC2.ToolTip.delay: 700
                QQC2.ToolTip.text: {
                    if (machineCapture) {
                        return tx.i18n("This computer's capture stays on until it is turned off from the Other page.");
                    }
                    if (personalCapture) {
                        return tx.i18n("System proxy does nothing while traffic capture is on.");
                    }
                    return tx.i18n("This takes effect only while the core is running and TUN is off.");
                }
            }

            QQC2.Label {
                visible: proxySwitch.machineCapture || proxySwitch.personalCapture
                text: proxySwitch.machineCapture
                      ? tx.i18n("This computer's capture stays on until it is turned off from the Other page.")
                      : tx.i18n("System proxy does nothing while traffic capture is on.")
                wrapMode: Text.WordWrap
                opacity: 0.75
                Layout.fillWidth: true
                Kirigami.MnemonicData.enabled: false
            }

            Item {
                Layout.fillWidth: true
            }

            QQC2.Label {
                text: tx.i18n("Mode")
            }

            QQC2.ComboBox {
                Kirigami.MnemonicData.enabled: false
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
                    text: tx.i18n("%1 MB/s", tx.formatNumber(999.9, 1))
                }
            }

            Item {
                Layout.fillWidth: true
            }

            ColumnLayout {
                id: exitColumn
                spacing: Kirigami.Units.smallSpacing
                Layout.fillWidth: true
                Layout.maximumWidth: Kirigami.Units.gridUnit * 22
                Layout.alignment: Qt.AlignTop

                QQC2.Label {
                    text: tx.i18n("Exit region")
                    Layout.fillWidth: true
                }
                Kirigami.Heading {
                    id: exitHeading
                    property bool revealAddress: false
                    readonly property string rawExit: kcm.client.exitRaw
                    readonly property bool hideAddress: kcm.client.blurIp && !revealAddress
                    readonly property string fullAddress: kcm.client.addressOf(rawExit)
                    onRawExitChanged: revealAddress = false
                    level: 2
                    text: rawExit.length === 0 ? tx.i18n("Not checked yet") : kcm.client.presentPlace(rawExit)
                    wrapMode: Text.WordWrap
                    Layout.fillWidth: true
                }
                QQC2.Label {
                    visible: exitHeading.fullAddress.length > 0
                    text: kcm.client.presentAddress(exitHeading.rawExit, exitHeading.hideAddress)
                    wrapMode: Text.WrapAnywhere
                    Layout.fillWidth: true

                    MouseArea {
                        anchors.fill: parent
                        acceptedButtons: Qt.LeftButton | Qt.RightButton
                        property int clicks: 0
                        onPressed: mouse => {
                            if (mouse.button === Qt.RightButton && exitHeading.fullAddress.length > 0) {
                                kcm.client.copyText(exitHeading.fullAddress);
                                kcm.client.showHint(tx.i18n("Copied the address."));
                            }
                        }
                        onClicked: mouse => {
                            if (mouse.button !== Qt.LeftButton) {
                                return;
                            }
                            clicks += 1;
                            exitClicks.restart();
                            if (clicks >= 3) {
                                exitHeading.revealAddress = true;
                                clicks = 0;
                            }
                        }
                        Timer {
                            id: exitClicks
                            interval: 450
                            onTriggered: parent.clicks = 0
                        }
                    }
                }
                QQC2.Button {
                    Kirigami.MnemonicData.enabled: false
                    text: tx.i18n("Check region")
                    enabled: !kcm.client.busy
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
            visible: kcm.client.showWidgetHint
            Layout.fillWidth: true
            wrapMode: Text.WordWrap
            opacity: 0.65
            font: Kirigami.Theme.smallFont
            text: tx.i18n("This program includes a Mihomo widget you can add to the panel.")
            Kirigami.MnemonicData.enabled: false
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
