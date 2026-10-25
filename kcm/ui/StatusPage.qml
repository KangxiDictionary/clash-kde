// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 clash-kde contributors

import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.kde.quickcharts as Charts
import org.kde.quickcharts.controls as ChartsControls

Item {
    id: root

    readonly property bool progress: kcm.client.phase === "starting" || kcm.client.phase === "downloading"
    readonly property bool failed: kcm.client.phase === "failed" || (kcm.client.message.length > 0 && !progress)
    readonly property bool starting: kcm.client.phase === "starting" || kcm.client.phase === "downloading"
                                  || (kcm.client.coreDesired && !kcm.client.coreRunning && !failed)
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

    function openDiagnostics() {
        diagnosticEdit.text = kcm.client.diagnostics();
        diagnosticsDialog.open();
    }

    Tx {
        id: tx
    }

    ListModel {
        id: connectionModel
    }

    function formatBytes(bytes) {
        const value = Number(bytes || 0);
        if (value < 1024) {
            return tx.i18n("%1 B", tx.formatNumber(value));
        }
        if (value < 1024 * 1024) {
            return tx.i18n("%1 KB", tx.formatNumber(value / 1024, 1));
        }
        if (value < 1024 * 1024 * 1024) {
            return tx.i18n("%1 MB", tx.formatNumber(value / 1024 / 1024, 1));
        }
        return tx.i18n("%1 GB", tx.formatNumber(value / 1024 / 1024 / 1024, 2));
    }

    function connectionRow(row) {
        return {
            connectionId: String(row.id || ""),
            process: String(row.process || ""),
            process_path: String(row.process_path || ""),
            host: String(row.host || ""),
            port: String(row.port || ""),
            destination_ip: String(row.destination_ip || ""),
            source_ip: String(row.source_ip || ""),
            source_port: String(row.source_port || ""),
            rule: String(row.rule || ""),
            rule_payload: String(row.rule_payload || ""),
            upload: Number(row.upload || 0),
            download: Number(row.download || 0)
        };
    }

    function syncConnections() {
        const rows = kcm.client.connections;
        let same = connectionModel.count === rows.length;
        for (let i = 0; same && i < rows.length; ++i) {
            if (connectionModel.get(i).connectionId !== String(rows[i].id || "")) {
                same = false;
            }
        }
        if (!same) {
            connectionModel.clear();
            for (let i = 0; i < rows.length; ++i) {
                connectionModel.append(connectionRow(rows[i]));
            }
            return;
        }
        for (let i = 0; i < rows.length; ++i) {
            const next = connectionRow(rows[i]);
            const current = connectionModel.get(i);
            if (current.download !== next.download || current.upload !== next.upload || current.rule !== next.rule || current.host !== next.host || current.process !== next.process || current.rule_payload !== next.rule_payload) {
                connectionModel.set(i, next);
            }
        }
    }

    Timer {
        interval: 2000
        repeat: true
        running: root.visible && kcm.client.coreRunning
        onTriggered: kcm.client.refreshConnections()
        onRunningChanged: {
            if (running) {
                kcm.client.refreshConnections();
            }
        }
    }

    Connections {
        target: kcm.client
        function onConnectionsChanged() {
            root.syncConnections();
        }
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

    function formatLatency(ms) {
        if (ms < 0) {
            return "—";
        }
        return tx.i18n("%1 ms", tx.formatNumber(ms));
    }

    component RateChart: ColumnLayout {
        id: chartRoot

        property bool live: false
        property real firstValue: 0
        property real secondValue: 0
        property var firstSeries: []
        property var secondSeries: []
        property string firstName: ""
        property string secondName: ""
        property real firstShown: 0
        property real secondShown: 0
        property bool firstPending: false
        property bool secondPending: false
        readonly property bool showFirst: live || firstSeries.length > 0
        readonly property bool showSecond: live || secondSeries.length > 0

        function secondColor() {
            const base = Kirigami.Theme.highlightColor;
            const hue = base.hsvHue < 0 ? 0.5 : (base.hsvHue / 360 + 0.5) % 1;
            return Qt.hsva(hue, base.hsvSaturation, base.hsvValue, base.a);
        }

        spacing: Kirigami.Units.largeSpacing

        Item {
            visible: false
            Layout.preferredWidth: 0
            Layout.preferredHeight: 0
            Layout.maximumHeight: 0

            Charts.SingleValueSource {
                id: firstNow
                value: chartRoot.firstValue
            }
            Charts.SingleValueSource {
                id: secondNow
                value: chartRoot.secondValue
            }
            Charts.HistoryProxySource {
                id: firstHistory
                source: firstNow
                interval: chartRoot.live ? 1000 : 0
                maximumHistory: 60
                fillMode: Charts.HistoryProxySource.FillFromEnd
            }
            Charts.HistoryProxySource {
                id: secondHistory
                source: secondNow
                interval: chartRoot.live ? 1000 : 0
                maximumHistory: 60
                fillMode: Charts.HistoryProxySource.FillFromEnd
            }
            Charts.ArraySource {
                id: firstArray
                array: chartRoot.firstSeries
            }
            Charts.ArraySource {
                id: secondArray
                array: chartRoot.secondSeries
            }
            Charts.ArraySource {
                id: nameSource
                array: {
                    const names = [];
                    if (chartRoot.showFirst) {
                        names.push(chartRoot.firstName);
                    }
                    if (chartRoot.showSecond) {
                        names.push(chartRoot.secondName);
                    }
                    return names;
                }
            }
            Charts.ColorGradientSource {
                id: pairedColors
                baseColor: Kirigami.Theme.highlightColor
                itemCount: 2
            }
            Charts.ArraySource {
                id: loneColor
                array: [chartRoot.showFirst ? Kirigami.Theme.highlightColor : chartRoot.secondColor()]
            }
            TextMetrics {
                id: rateWidth
                text: tx.i18n("%1 MB/s", tx.formatNumber(999.99, 2))
            }
        }

        RowLayout {
            spacing: Kirigami.Units.smallSpacing
            Layout.fillWidth: true
            Layout.preferredHeight: Kirigami.Units.gridUnit * 6
            Layout.minimumHeight: Kirigami.Units.gridUnit * 5
            Layout.topMargin: axisMetrics.height / 2
            Layout.bottomMargin: Layout.topMargin

            ChartsControls.AxisLabels {
                id: axisLabels
                Layout.fillHeight: true
                constrainToBounds: false
                direction: ChartsControls.AxisLabels.VerticalBottomTop
                delegate: QQC2.Label {
                    anchors.right: parent.right
                    font: Kirigami.Theme.smallFont
                    text: root.formatRate(Number(ChartsControls.AxisLabels.label))
                    opacity: 0.75
                }
                source: Charts.ChartAxisSource {
                    chart: lineChart
                    axis: Charts.ChartAxisSource.YAxis
                    itemCount: 5
                }
                TextMetrics {
                    id: axisMetrics
                    font: Kirigami.Theme.smallFont
                    text: root.formatRate(0)
                }
            }

            Charts.LineChart {
                id: lineChart
                Layout.fillWidth: true
                Layout.fillHeight: true
                direction: chartRoot.live ? Charts.XYChart.ZeroAtEnd : Charts.XYChart.ZeroAtStart
                interpolate: true
                fillOpacity: 0.1
                opacity: chartRoot.showFirst || chartRoot.showSecond ? 1 : 0
                valueSources: {
                    const sources = [];
                    if (chartRoot.showFirst) {
                        sources.push(chartRoot.live ? firstHistory : firstArray);
                    }
                    if (chartRoot.showSecond) {
                        sources.push(chartRoot.live ? secondHistory : secondArray);
                    }
                    return sources;
                }
                nameSource: nameSource
                colorSource: chartRoot.showFirst && chartRoot.showSecond ? pairedColors : loneColor
                highlight: rateLegend.highlightedIndex

                yRange {
                    from: 0
                    automatic: true
                    minimum: 1
                }

                ChartsControls.GridLines {
                    anchors.fill: parent
                    z: lineChart.z - 1
                    chart: lineChart
                    direction: ChartsControls.GridLines.Vertical
                    major.count: 3
                    major.lineWidth: 1
                    major.color: Kirigami.ColorUtils.linearInterpolation(Kirigami.Theme.backgroundColor, Kirigami.Theme.textColor, 0.2)
                    minor.visible: false
                }
            }
        }

        ChartsControls.Legend {
            id: rateLegend
            Layout.fillWidth: true
            chart: lineChart
            highlightEnabled: true
            horizontalSpacing: Kirigami.Units.gridUnit
            verticalSpacing: Kirigami.Units.smallSpacing
            maximumDelegateWidth: Kirigami.Units.gridUnit * 15
            formatValue: function(input, index) {
                const slot = chartRoot.showFirst && chartRoot.showSecond ? index : (chartRoot.showSecond ? 1 : 0);
                if ((slot === 0 && chartRoot.firstPending) || (slot === 1 && chartRoot.secondPending)) {
                    return "—";
                }
                const value = slot === 0 ? chartRoot.firstShown : chartRoot.secondShown;
                return root.formatRate(Number(value) || 0);
            }
            maximumValueWidth: function(input, index) {
                return rateWidth.advanceWidth;
            }
        }
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
                    text: tx.i18n("Diagnostics")
                    icon.name: "view-list-text"
                    onTriggered: root.openDiagnostics()
                },
                Kirigami.Action {
                    text: tx.i18n("Turn off system proxy")
                    icon.name: "network-disconnect"
                    onTriggered: kcm.client.restoreSystemProxy()
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
            text: tx.i18n("The firewall is on, and it can drop TUN traffic. Check Diagnostics for route details.")
            actions: [
                Kirigami.Action {
                    text: tx.i18n("Diagnostics")
                    icon.name: "view-list-text"
                    onTriggered: root.openDiagnostics()
                }
            ]
        }

        Kirigami.InlineMessage {
            Layout.fillWidth: true
            visible: kcm.client.tunEnabled && kcm.client.coreRunning
                && kcm.client.tunRouteKnown && !kcm.client.tunRouteReady
            type: Kirigami.MessageType.Error
            text: kcm.client.tunRouteDetail.length > 0
                ? tx.i18n("TUN route is not ready: %1", kcm.client.tunRouteDetail)
                : tx.i18n("TUN route is not ready.")
            actions: [
                Kirigami.Action {
                    text: tx.i18n("Diagnostics")
                    icon.name: "view-list-text"
                    onTriggered: root.openDiagnostics()
                }
            ]
        }

        Kirigami.InlineMessage {
            Layout.fillWidth: true
            visible: kcm.client.tunEnabled && kcm.client.coreRunning && !kcm.client.tunRouteKnown
            type: Kirigami.MessageType.Information
            text: tx.i18n("TUN route could not be checked. Install iproute2 or check the service logs.")
        }

        QQC2.Label {
            Layout.fillWidth: true
            visible: kcm.client.tunEnabled && kcm.client.coreRunning && kcm.client.tunRouteCheckedAt > 0
            text: tx.i18n("TUN route checked: %1", Qt.formatDateTime(new Date(Number(kcm.client.tunRouteCheckedAt) * 1000), "yyyy-MM-dd HH:mm:ss"))
            opacity: 0.7
            font: Kirigami.Theme.smallFont
            wrapMode: Text.WordWrap
            Kirigami.MnemonicData.enabled: false
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

                QQC2.Button {
                    Kirigami.MnemonicData.enabled: false
                    text: tx.i18n("Diagnostics")
                    icon.name: "view-list-text"
                    onClicked: root.openDiagnostics()

                    QQC2.ToolTip.visible: hovered
                    QQC2.ToolTip.delay: 700
                    QQC2.ToolTip.text: tx.i18n("Show the recent mihomo log, the service journal, and the paths in use.")
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
                    QQC2.ToolTip.text: tx.i18n("Press Apply to start or stop the local proxy.")
                }
            }
        }

        Kirigami.InlineMessage {
            Layout.fillWidth: true
            visible: kcm.client.draftCoreDesired !== kcm.client.coreDesired
            type: Kirigami.MessageType.Information
            text: tx.i18n("Press Apply to start or stop the core.")
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

            RateChart {
                Layout.fillWidth: true
                Layout.alignment: Qt.AlignTop
                live: true
                firstName: tx.i18nc("@title", "Download Rate")
                secondName: tx.i18nc("@title", "Upload Rate")
                firstValue: kcm.client.downloadBps
                secondValue: kcm.client.uploadBps
                firstShown: kcm.client.downloadBps
                secondShown: kcm.client.uploadBps
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

        RowLayout {
            Layout.fillWidth: true
            Layout.fillHeight: true
            spacing: Kirigami.Units.largeSpacing

            ColumnLayout {
                Layout.fillWidth: true
                Layout.fillHeight: true
                Layout.minimumWidth: Kirigami.Units.gridUnit * 16
                Layout.alignment: Qt.AlignTop
                spacing: Kirigami.Units.smallSpacing

                RowLayout {
                    Layout.fillWidth: true
                    spacing: Kirigami.Units.smallSpacing

                    Kirigami.Heading {
                        text: tx.i18n("Connections")
                        level: 3
                        Layout.fillWidth: true
                    }

                    QQC2.Button {
                        Kirigami.MnemonicData.enabled: false
                        text: tx.i18n("Close all")
                        enabled: kcm.client.coreRunning && connectionModel.count > 0
                        onClicked: closeAllDialog.open()

                        QQC2.ToolTip.visible: hovered
                        QQC2.ToolTip.delay: 700
                        QQC2.ToolTip.text: tx.i18n("Closes every connection going through the core.")
                    }
                }

                Item {
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    Layout.minimumHeight: Kirigami.Units.gridUnit * 8

                    QQC2.ScrollView {
                        anchors.fill: parent
                        Kirigami.StyleHints.showFramedBackground: true

                        ListView {
                        id: connectionList
                        clip: true
                        reuseItems: true
                        model: connectionModel
                        spacing: Kirigami.Units.smallSpacing
                        boundsBehavior: Flickable.StopAtBounds

                        delegate: QQC2.ItemDelegate {
                            id: connectionDelegate
                            required property int index
                            required property string connectionId
                            required property string process
                            required property string process_path
                            required property string host
                            required property string port
                            required property string destination_ip
                            required property string source_ip
                            required property string source_port
                            required property string rule
                            required property string rule_payload
                            required property var upload
                            required property var download
                            width: ListView.view ? ListView.view.width : 0
                            hoverEnabled: true
                            Kirigami.MnemonicData.enabled: false

                            background: Rectangle {
                                radius: Kirigami.Units.cornerRadius
                                color: connectionDelegate.hovered ? Kirigami.Theme.highlightColor : Kirigami.Theme.alternateBackgroundColor
                                opacity: connectionDelegate.hovered ? 0.35 : 1
                            }

                readonly property string destination: {
                    const raw = host.length > 0 ? host : destination_ip;
                    const shown = kcm.client.presentHost(raw, kcm.client.blurIp);
                    return port.length > 0 ? shown + ":" + port : shown;
                }
                readonly property string ruleText: rule_payload.length > 0 ? rule + " " + rule_payload : rule
                readonly property string tip: {
                    const parts = [];
                    if (process_path.length > 0) {
                        parts.push(process_path);
                    }
                    const source = kcm.client.presentHost(source_ip, kcm.client.blurIp);
                    if (source.length > 0) {
                        parts.push(source_port.length > 0 ? source + ":" + source_port : source);
                    }
                    return parts.join("\n");
                }

                            contentItem: RowLayout {
                    spacing: Kirigami.Units.smallSpacing

                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 0

                        QQC2.Label {
                            text: connectionDelegate.process.length > 0 ? connectionDelegate.process : tx.i18n("Unknown process")
                            font.bold: true
                            elide: Text.ElideRight
                            Layout.fillWidth: true
                            Kirigami.MnemonicData.enabled: false
                        }
                        QQC2.Label {
                            text: connectionDelegate.destination + (connectionDelegate.ruleText.length > 0 ? " · " + connectionDelegate.ruleText : "")
                            opacity: 0.7
                            font: Kirigami.Theme.smallFont
                            elide: Text.ElideRight
                            Layout.fillWidth: true
                            Kirigami.MnemonicData.enabled: false
                        }
                    }

                    ColumnLayout {
                        spacing: 0

                        QQC2.Label {
                            text: tx.i18n("Download") + " " + root.formatBytes(connectionDelegate.download)
                            font: Kirigami.Theme.smallFont
                            horizontalAlignment: Text.AlignRight
                            Layout.fillWidth: true
                            Kirigami.MnemonicData.enabled: false
                        }
                        QQC2.Label {
                            text: tx.i18n("Upload") + " " + root.formatBytes(connectionDelegate.upload)
                            font: Kirigami.Theme.smallFont
                            opacity: 0.7
                            horizontalAlignment: Text.AlignRight
                            Layout.fillWidth: true
                            Kirigami.MnemonicData.enabled: false
                        }
                    }

                    QQC2.ToolButton {
                        Kirigami.MnemonicData.enabled: false
                        icon.name: "edit-delete"
                        display: QQC2.AbstractButton.IconOnly
                        onClicked: kcm.client.closeConnection(connectionDelegate.connectionId)

                        QQC2.ToolTip.visible: hovered
                        QQC2.ToolTip.delay: 700
                        QQC2.ToolTip.text: tx.i18n("Closes this connection.")
                    }
                }

                            HoverHandler {
                                id: connectionHover
                            }

                            QQC2.ToolTip.visible: connectionHover.hovered && connectionDelegate.tip.length > 0
                            QQC2.ToolTip.delay: 700
                            QQC2.ToolTip.text: connectionDelegate.tip
                        }
                    }
                    }

                    QQC2.Label {
                        anchors.left: parent.left
                        anchors.top: parent.top
                        anchors.right: parent.right
                        anchors.margins: Kirigami.Units.largeSpacing
                        visible: !kcm.client.coreRunning || connectionModel.count === 0
                        text: kcm.client.coreRunning ? tx.i18n("No connections") : tx.i18n("Core is not running")
                        wrapMode: Text.WordWrap
                        opacity: 0.7
                        Kirigami.MnemonicData.enabled: false
                    }
                }
            }

            ColumnLayout {
                Layout.preferredWidth: Kirigami.Units.gridUnit * 22
                Layout.maximumWidth: Kirigami.Units.gridUnit * 28
                Layout.minimumWidth: Kirigami.Units.gridUnit * 16
                Layout.alignment: Qt.AlignTop
                spacing: Kirigami.Units.smallSpacing

                Kirigami.Heading {
                    text: tx.i18n("Speed test")
                    level: 3
                    Layout.fillWidth: true
                }

                Rectangle {
                    Layout.fillWidth: true
                    radius: Kirigami.Units.cornerRadius
                    color: Kirigami.Theme.alternateBackgroundColor
                    implicitHeight: speedColumn.implicitHeight + Kirigami.Units.largeSpacing * 2

                    ColumnLayout {
                        id: speedColumn
                        width: parent.width - Kirigami.Units.largeSpacing * 2
                        anchors.centerIn: parent
                        spacing: Kirigami.Units.largeSpacing

                        QQC2.Label {
                            text: tx.i18n("Downloads %1 MB from Cloudflare through the proxy and directly at the same time. Results are for reference only.", tx.formatNumber(kcm.client.speedTestMib))
                            wrapMode: Text.WordWrap
                            Layout.fillWidth: true
                            Kirigami.MnemonicData.enabled: false
                        }

                        RateChart {
                            Layout.fillWidth: true
                            firstName: tx.i18n("Proxy")
                            secondName: tx.i18n("Direct")
                            firstSeries: kcm.client.speedProxySamples
                            secondSeries: kcm.client.speedDirectSamples
                            firstShown: kcm.client.speedProxyBps
                            secondShown: kcm.client.speedDirectBps
                            firstPending: kcm.client.speedProxySamples.length === 0 && kcm.client.speedPhase !== "run"
                            secondPending: kcm.client.speedDirectSamples.length === 0 && kcm.client.speedPhase !== "run"
                        }

                        QQC2.Label {
                            text: tx.i18n("Latency") + "  " + tx.i18n("Proxy") + "  " + root.formatLatency(kcm.client.speedProxyLatencyMs) + "    " + tx.i18n("Direct") + "  " + root.formatLatency(kcm.client.speedDirectLatencyMs)
                            wrapMode: Text.WordWrap
                            font: Kirigami.Theme.smallFont
                            Layout.fillWidth: true
                            Kirigami.MnemonicData.enabled: false
                        }

                        QQC2.Label {
                            visible: kcm.client.speedError.length > 0
                            text: kcm.client.speedError
                            wrapMode: Text.WordWrap
                            color: Kirigami.Theme.negativeTextColor
                            Layout.fillWidth: true
                            Kirigami.MnemonicData.enabled: false
                        }

                        RowLayout {
                            Layout.fillWidth: true
                            spacing: Kirigami.Units.smallSpacing

                            QQC2.Button {
                                Kirigami.MnemonicData.enabled: false
                                Layout.fillWidth: true
                                text: kcm.client.speedRunning ? tx.i18n("Stop") : tx.i18n("Start test")
                                icon.name: kcm.client.speedRunning ? "media-playback-stop" : "media-playback-start"
                                onClicked: kcm.client.speedRunning ? kcm.client.stopSpeedTest() : kcm.client.startSpeedTest()
                            }

                            QQC2.Button {
                                Kirigami.MnemonicData.enabled: false
                                visible: !kcm.client.speedRunning && kcm.client.speedPhase === "done"
                                text: tx.i18n("Reset")
                                icon.name: "edit-clear"
                                onClicked: kcm.client.resetSpeedTest()
                            }
                        }
                    }
                }

                Item {
                    Layout.fillHeight: true
                }
            }
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

    Kirigami.PromptDialog {
        id: closeAllDialog
        standardButtons: Kirigami.Dialog.Ok | Kirigami.Dialog.Cancel
        title: tx.i18n("Close all connections?")
        onAccepted: kcm.client.closeAllConnections()
    }

    QQC2.Dialog {
        id: diagnosticsDialog
        title: tx.i18n("Diagnostics")
        modal: true
        standardButtons: QQC2.Dialog.Close
        padding: Kirigami.Units.largeSpacing
        implicitWidth: Kirigami.Units.gridUnit * 52
        implicitHeight: Kirigami.Units.gridUnit * 36

        contentItem: ColumnLayout {
            spacing: Kirigami.Units.smallSpacing

            Flickable {
                id: diagnosticFlick
                Layout.fillWidth: true
                Layout.fillHeight: true
                clip: true
                contentWidth: width
                contentHeight: diagnosticEdit.paintedHeight + Kirigami.Units.smallSpacing * 2
                boundsBehavior: Flickable.StopAtBounds
                QQC2.ScrollBar.vertical: QQC2.ScrollBar {
                    policy: QQC2.ScrollBar.AsNeeded
                }

                TextEdit {
                    id: diagnosticEdit
                    x: Kirigami.Units.smallSpacing
                    y: Kirigami.Units.smallSpacing
                    width: diagnosticFlick.width - Kirigami.Units.smallSpacing * 2
                    height: paintedHeight
                    readOnly: true
                    selectByMouse: true
                    wrapMode: TextEdit.WrapAtWordBoundaryOrAnywhere
                    textFormat: TextEdit.PlainText
                    font.family: "monospace"
                    color: Kirigami.Theme.textColor
                }
            }

            RowLayout {
                Layout.fillWidth: true
                spacing: Kirigami.Units.smallSpacing

                QQC2.Button {
                    text: tx.i18n("Refresh")
                    icon.name: "view-refresh"
                    onClicked: diagnosticEdit.text = kcm.client.diagnostics()
                }

                QQC2.Button {
                    text: tx.i18n("Copy")
                    icon.name: "edit-copy"
                    onClicked: {
                        kcm.client.copyText(diagnosticEdit.text);
                        kcm.client.showHint(tx.i18n("Copied."));
                    }
                }

                QQC2.Button {
                    text: tx.i18n("Open log folder")
                    icon.name: "folder-open"
                    onClicked: kcm.client.openLogFolder()
                }

                Item {
                    Layout.fillWidth: true
                }

                QQC2.Button {
                    text: tx.i18n("Turn off system proxy")
                    icon.name: "network-disconnect"
                    onClicked: kcm.client.restoreSystemProxy()
                }
            }
        }
    }
}
