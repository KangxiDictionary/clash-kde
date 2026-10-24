// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 clash-kde contributors

import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.kde.kirigamiaddons.formcard as FormCard

Flickable {
    id: root

    contentWidth: width
    contentHeight: body.implicitHeight
    clip: true

    property int detailUid: -1
    property var otherAccounts: []

    readonly property bool showingList: kcm.client.accountsUnlocked && detailUid < 0
    readonly property bool showingDetail: kcm.client.accountsUnlocked && detailUid >= 0
    readonly property var detailAccount: {
        const accounts = kcm.client.accounts;
        for (let i = 0; i < accounts.length; ++i) {
            if (accounts[i].uid === detailUid) {
                return accounts[i];
            }
        }
        return null;
    }

    Tx {
        id: tx
    }

    function captureText(enabled, machine) {
        if (!enabled) {
            return tx.i18n("Off");
        }
        return machine ? tx.i18n("This computer") : tx.i18n("This account");
    }

    function refreshOthers() {
        const next = [];
        const accounts = kcm.client.accounts;
        for (let i = 0; i < accounts.length; ++i) {
            if (accounts[i].uid !== kcm.client.ownUid) {
                next.push(accounts[i]);
            }
        }
        otherAccounts = next;
    }

    function followMachine() {
        machineSwitch.checked = Qt.binding(function() {
            return kcm.client.tunEnabled && kcm.client.tunMachine;
        });
    }

    Connections {
        target: kcm.client
        function onAccountsChanged() {
            root.refreshOthers();
            if (!kcm.client.accountsUnlocked) {
                root.detailUid = -1;
            }
        }
        function onStatusChanged() {
            root.followMachine();
        }
        function onErrorOccurred() {
            root.followMachine();
        }
    }

    Component.onCompleted: refreshOthers()

    ColumnLayout {
        id: body
        width: root.width
        spacing: 0

        FormCard.FormHeader {
            maximumWidth: Infinity
            title: tx.i18n("About")
            Layout.fillWidth: true
        }

        FormCard.FormCard {
            maximumWidth: Infinity
            Layout.fillWidth: true

            FormCard.FormTextDelegate {
                text: tx.i18n("Name")
                description: tx.i18n("Mihomo")
                Kirigami.MnemonicData.enabled: false
            }

            FormCard.FormDelegateSeparator {}

            FormCard.FormTextDelegate {
                text: tx.i18n("Version")
                description: "1.0.3"
                Kirigami.MnemonicData.enabled: false
            }

            FormCard.FormDelegateSeparator {}

            FormCard.FormTextDelegate {
                text: tx.i18n("License")
                description: "GNU GPL v3"
                Kirigami.MnemonicData.enabled: false
            }

            FormCard.FormDelegateSeparator {}

            FormCard.FormLinkDelegate {
                text: tx.i18n("Website")
                description: "github.com/KangxiDictionary/clash-kde"
                url: "https://github.com/KangxiDictionary/clash-kde"
                Kirigami.MnemonicData.enabled: false
            }
        }

        FormCard.FormHeader {
            maximumWidth: Infinity
            title: tx.i18n("Appearance")
            Layout.fillWidth: true
        }

        FormCard.FormCard {
            maximumWidth: Infinity
            Layout.fillWidth: true

            FormCard.FormSwitchDelegate {
                text: tx.i18n("Compact mode")
                checked: kcm.client.draftCompact
                Kirigami.MnemonicData.enabled: false
                onToggled: {
                    if (checked !== kcm.client.draftCompact) {
                        kcm.client.draftCompact = checked;
                    }
                }

                QQC2.ToolTip.visible: hovered
                QQC2.ToolTip.delay: 700
                QQC2.ToolTip.text: tx.i18n("Shows only the name in the profile list and the route list.")
            }

            FormCard.FormDelegateSeparator {}

            FormCard.FormSwitchDelegate {
                text: tx.i18n("Show the panel widget note")
                checked: kcm.client.draftShowWidgetHint
                Kirigami.MnemonicData.enabled: false
                onToggled: {
                    if (checked !== kcm.client.draftShowWidgetHint) {
                        kcm.client.draftShowWidgetHint = checked;
                    }
                }

                QQC2.ToolTip.visible: hovered
                QQC2.ToolTip.delay: 700
                QQC2.ToolTip.text: tx.i18n("Shows a note on the Status page that this program includes a panel widget.")
            }

            FormCard.FormDelegateSeparator {}

            FormCard.FormSwitchDelegate {
                text: tx.i18n("Blur the IP")
                checked: kcm.client.draftBlurIp
                Kirigami.MnemonicData.enabled: false
                onToggled: {
                    if (checked !== kcm.client.draftBlurIp) {
                        kcm.client.draftBlurIp = checked;
                    }
                }

                QQC2.ToolTip.visible: hovered
                QQC2.ToolTip.delay: 700
                QQC2.ToolTip.text: tx.i18n("Hides the latter half of the exit address; right-click copies the full address, and three quick clicks show it.")
            }
        }

        FormCard.FormHeader {
            maximumWidth: Infinity
            title: tx.i18n("Speed test")
            Layout.fillWidth: true
        }

        FormCard.FormCard {
            maximumWidth: Infinity
            Layout.fillWidth: true

            FormCard.FormSpinBoxDelegate {
                id: speedSizeBox
                label: tx.i18n("Download size")
                from: 1
                to: 100
                stepSize: 1
                validator: IntValidator {
                    bottom: 1
                    top: 100
                }
                textFromValue: function(value, locale) {
                    return tx.i18n("%1 MB", tx.formatNumber(value));
                }
                onValueModified: {
                    const bounded = Math.max(1, Math.min(100, value));
                    if (bounded !== value) {
                        speedSizeBox.value = bounded;
                    }
                    if (bounded !== kcm.client.draftSpeedTestMib) {
                        kcm.client.draftSpeedTestMib = bounded;
                    }
                }
                Kirigami.MnemonicData.enabled: false

                QQC2.ToolTip.visible: hovered
                QQC2.ToolTip.delay: 700
                QQC2.ToolTip.text: tx.i18n("How much each speed test downloads through the proxy and directly, between 1 and 100 MB.")

                Component.onCompleted: speedSizeBox.value = kcm.client.draftSpeedTestMib

                Connections {
                    target: kcm.client
                    function onDraftChanged() {
                        if (!speedSizeBox.fieldActiveFocus && speedSizeBox.value !== kcm.client.draftSpeedTestMib) {
                            speedSizeBox.value = kcm.client.draftSpeedTestMib;
                        }
                    }
                }
            }

            FormCard.FormSectionText {
                text: tx.i18n("Allowed range: 1 to 100 MB per run.")
            }
        }

        Kirigami.InlineMessage {
            Layout.fillWidth: true
            Layout.margins: Kirigami.Units.largeSpacing
            visible: kcm.client.accountsError.length > 0
            type: Kirigami.MessageType.Error
            text: kcm.client.accountsError
        }

        FormCard.FormHeader {
            maximumWidth: Infinity
            title: tx.i18n("Accounts")
            Layout.fillWidth: true
        }

        FormCard.FormCard {
            maximumWidth: Infinity
            Layout.fillWidth: true
            visible: !kcm.client.accountsUnlocked

            FormCard.FormButtonDelegate {
                text: tx.i18n("Manage signed-in accounts")
                description: tx.i18n("Asks for the administrator password, then lists the other signed-in accounts.")
                enabled: !kcm.client.busy
                Kirigami.MnemonicData.enabled: false
                onClicked: kcm.client.reloadAccounts()
            }
        }

        ColumnLayout {
            visible: root.showingList
            Layout.fillWidth: true
            spacing: 0

            RowLayout {
                Layout.fillWidth: true
                Layout.leftMargin: Kirigami.Units.largeSpacing
                Layout.rightMargin: Kirigami.Units.largeSpacing
                Layout.topMargin: Kirigami.Units.largeSpacing

                Item {
                    Layout.fillWidth: true
                }

                QQC2.ToolButton {
                    Kirigami.MnemonicData.enabled: false
                    icon.name: "lock"
                    text: tx.i18n("Lock")
                    onClicked: {
                        root.detailUid = -1;
                        kcm.client.lockAccounts();
                    }
                }
            }

            FormCard.FormHeader {
                maximumWidth: Infinity
                title: tx.i18n("This computer")
                Layout.fillWidth: true
            }

            FormCard.FormCard {
                maximumWidth: Infinity
                Layout.fillWidth: true

                FormCard.FormSwitchDelegate {
                    id: machineSwitch
                    text: tx.i18n("Capture the whole computer")
                    description: tx.i18n("Sends every account and root through this account's configuration.")
                    checked: kcm.client.tunEnabled && kcm.client.tunMachine
                    enabled: !kcm.client.busy
                    onClicked: kcm.client.setMachineTun(!(kcm.client.tunEnabled && kcm.client.tunMachine))
                }

                FormCard.FormDelegateSeparator {}

                FormCard.FormSwitchDelegate {
                    text: tx.i18n("Allow LAN")
                    description: tx.i18n("Lets every account open its mixed port to the local network.")
                    checked: kcm.client.machineLanAllowed
                    enabled: !kcm.client.busy
                    onClicked: kcm.client.setMachineLan(!kcm.client.machineLanAllowed)
                }

                FormCard.FormDelegateSeparator {}

                FormCard.FormSwitchDelegate {
                    text: tx.i18n("Allow LAN for this account")
                    description: tx.i18n("When this is off, this account's mixed port stays on this computer.")
                    checked: kcm.client.lanAccounts[String(kcm.client.ownUid)] !== false
                    enabled: !kcm.client.busy
                    onClicked: kcm.client.setAccountLan(kcm.client.ownUid, kcm.client.lanAccounts[String(kcm.client.ownUid)] === false)
                }
            }

            FormCard.FormHeader {
                maximumWidth: Infinity
                title: tx.i18n("Signed-in accounts")
                Layout.fillWidth: true
            }

            FormCard.FormCard {
                maximumWidth: Infinity
                Layout.fillWidth: true
                visible: root.otherAccounts.length > 0

                Repeater {
                    model: root.otherAccounts

                    delegate: ColumnLayout {
                        id: accountRow
                        required property var modelData
                        required property int index
                        Layout.fillWidth: true
                        spacing: 0

                        FormCard.FormDelegateSeparator {
                            visible: accountRow.index > 0
                        }

                        FormCard.FormButtonDelegate {
                            text: accountRow.modelData.name
                            description: root.captureText(accountRow.modelData.tun_enabled, accountRow.modelData.tun_machine)
                            Layout.fillWidth: true
                            onClicked: root.detailUid = accountRow.modelData.uid
                        }
                    }
                }
            }

            FormCard.FormSectionText {
                Layout.maximumWidth: Infinity
                visible: root.otherAccounts.length === 0
                text: tx.i18n("No other signed-in accounts.")
                Layout.fillWidth: true
            }
        }

        ColumnLayout {
            visible: root.showingDetail && root.detailAccount
            Layout.fillWidth: true
            spacing: 0

            FormCard.FormHeader {
                maximumWidth: Infinity
                title: root.detailAccount ? root.detailAccount.name : ""
                Layout.fillWidth: true
            }

            FormCard.FormCard {
                maximumWidth: Infinity
                Layout.fillWidth: true

                FormCard.FormButtonDelegate {
                    text: tx.i18n("Back")
                    icon.name: "go-previous"
                    onClicked: root.detailUid = -1
                }

                FormCard.FormDelegateSeparator {}

                FormCard.FormTextDelegate {
                    text: tx.i18n("Core")
                    description: root.detailAccount && root.detailAccount.core_running ? tx.i18n("The core is running.") : tx.i18n("The core is stopped.")
                }

                FormCard.FormDelegateSeparator {}

                FormCard.FormTextDelegate {
                    text: tx.i18n("System proxy")
                    description: root.detailAccount && root.detailAccount.system_proxy_enabled ? tx.i18n("On") : tx.i18n("Off")
                }

                FormCard.FormDelegateSeparator {}

                FormCard.FormTextDelegate {
                    text: tx.i18n("Traffic capture")
                    description: root.detailAccount ? root.captureText(root.detailAccount.tun_enabled, root.detailAccount.tun_machine) : ""
                }

                FormCard.FormDelegateSeparator {}

                FormCard.FormSwitchDelegate {
                    text: tx.i18n("Allow LAN")
                    description: tx.i18n("When this is off, that account's mixed port stays on this computer.")
                    enabled: root.detailAccount && !kcm.client.busy
                    checked: root.detailAccount ? kcm.client.lanAccounts[String(root.detailAccount.uid)] !== false : true
                    onClicked: {
                        if (!root.detailAccount) {
                            return;
                        }
                        kcm.client.setAccountLan(root.detailAccount.uid, kcm.client.lanAccounts[String(root.detailAccount.uid)] === false);
                    }
                }

                FormCard.FormDelegateSeparator {}

                FormCard.FormTextDelegate {
                    id: exitLine
                    readonly property string rawExit: root.detailAccount ? (root.detailAccount.exit_region || "") : ""
                    property bool revealAddress: false
                    onRawExitChanged: revealAddress = false
                    text: tx.i18n("Exit")
                    description: {
                        if (rawExit.length === 0) {
                            return "—";
                        }
                        const formatted = kcm.client.presentExit(rawExit, false);
                        return formatted.length > 0 ? formatted : "—";
                    }

                    MouseArea {
                        anchors.fill: parent
                        acceptedButtons: Qt.LeftButton | Qt.RightButton
                        property int clicks: 0
                        onPressed: mouse => {
                            const address = kcm.client.addressOf(exitLine.rawExit);
                            if (mouse.button === Qt.RightButton && address.length > 0) {
                                kcm.client.copyText(address);
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
                                exitLine.revealAddress = true;
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

                FormCard.FormDelegateSeparator {}

                FormCard.FormButtonDelegate {
                    enabled: root.detailAccount && !kcm.client.busy
                    text: root.detailAccount && root.detailAccount.core_running ? tx.i18n("Stop this account's core") : tx.i18n("Start this account's core")
                    icon.name: root.detailAccount && root.detailAccount.core_running ? "media-playback-stop" : "media-playback-start"
                    onClicked: {
                        if (!root.detailAccount) {
                            return;
                        }
                        if (root.detailAccount.core_running) {
                            kcm.client.stopAccount(root.detailAccount.uid);
                        } else {
                            kcm.client.startAccount(root.detailAccount.uid);
                        }
                    }
                }
            }
        }
    }
}
