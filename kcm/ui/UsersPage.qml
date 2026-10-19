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

    function proxyText(state) {
        if (state === "ours") {
            return tx.i18n("Pointing at this core.");
        }
        if (state === "other") {
            return tx.i18n("Pointing somewhere else.");
        }
        return tx.i18n("Not set.");
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

        Kirigami.InlineMessage {
            Layout.fillWidth: true
            Layout.margins: Kirigami.Units.largeSpacing
            visible: kcm.client.accountsError.length > 0
            type: Kirigami.MessageType.Error
            text: kcm.client.accountsError
        }

        ColumnLayout {
            visible: !kcm.client.accountsUnlocked
            Layout.fillWidth: true
            spacing: 0

            FormCard.FormHeader {
                title: tx.i18n("This account")
                Layout.fillWidth: true
            }

            FormCard.FormCard {
                Layout.fillWidth: true

                FormCard.FormTextDelegate {
                    text: tx.i18n("KDE proxy")
                    description: root.proxyText(kcm.client.kdeProxy)

                    QQC2.ToolTip.visible: hovered
                    QQC2.ToolTip.delay: 700
                    QQC2.ToolTip.text: tx.i18n("Shows whether KDE is telling programs to use a proxy.")
                }

                FormCard.FormDelegateSeparator {}

                FormCard.FormTextDelegate {
                    text: tx.i18n("GNOME proxy")
                    description: root.proxyText(kcm.client.gnomeProxy)

                    QQC2.ToolTip.visible: hovered
                    QQC2.ToolTip.delay: 700
                    QQC2.ToolTip.text: tx.i18n("Shows whether GNOME is telling programs to use a proxy.")
                }

                FormCard.FormDelegateSeparator {}

                FormCard.FormSwitchDelegate {
                    text: tx.i18n("TUN")
                    checked: kcm.client.draftTunEnabled
                    onToggled: {
                        if (checked !== kcm.client.draftTunEnabled) {
                            kcm.client.draftTunEnabled = checked;
                        }
                    }

                    QQC2.ToolTip.visible: hovered
                    QQC2.ToolTip.delay: 700
                    QQC2.ToolTip.text: tx.i18n("This captures only this account's traffic, and the system proxy steps aside while it is on.")
                }
            }

            FormCard.FormHeader {
                title: tx.i18n("Connection")
                Layout.fillWidth: true
            }

            FormCard.FormCard {
                Layout.fillWidth: true

                FormCard.FormSwitchDelegate {
                    text: tx.i18n("Test delay automatically")
                    checked: kcm.client.draftAutoTestDelay
                    onToggled: {
                        if (checked !== kcm.client.draftAutoTestDelay) {
                            kcm.client.draftAutoTestDelay = checked;
                        }
                    }

                    QQC2.ToolTip.visible: hovered
                    QQC2.ToolTip.delay: 700
                    QQC2.ToolTip.text: tx.i18n("Tests the delay of the group you are looking at.")
                }

                FormCard.FormDelegateSeparator {}

                FormCard.FormSwitchDelegate {
                    text: tx.i18n("Blur the IP")
                    checked: kcm.client.draftBlurIp
                    onToggled: {
                        if (checked !== kcm.client.draftBlurIp) {
                            kcm.client.draftBlurIp = checked;
                        }
                    }

                    QQC2.ToolTip.visible: hovered
                    QQC2.ToolTip.delay: 700
                    QQC2.ToolTip.text: tx.i18n("Hides the latter half of the exit address.")
                }

                FormCard.FormDelegateSeparator {}

                FormCard.FormSwitchDelegate {
                    text: tx.i18n("Use the same sort for every group")
                    checked: kcm.client.draftSortGlobal
                    onToggled: {
                        if (checked !== kcm.client.draftSortGlobal) {
                            kcm.client.draftSortGlobal = checked;
                        }
                    }

                    QQC2.ToolTip.visible: hovered
                    QQC2.ToolTip.delay: 700
                    QQC2.ToolTip.text: tx.i18n("Keeps one sort and filter for every group and profile.")
                }

                FormCard.FormDelegateSeparator {}

                FormCard.FormSpinBoxDelegate {
                    label: tx.i18n("Mixed port")
                    from: 1
                    to: 65535
                    value: kcm.client.draftMixedPort
                    onValueModified: kcm.client.draftMixedPort = value

                    QQC2.ToolTip.visible: hovered
                    QQC2.ToolTip.delay: 700
                    QQC2.ToolTip.text: tx.i18n("HTTP and SOCKS share this port, and saving restarts a running core.")
                }

                FormCard.FormDelegateSeparator {}

                FormCard.FormSpinBoxDelegate {
                    label: tx.i18n("Controller port")
                    from: 1
                    to: 65535
                    value: kcm.client.draftControllerPort
                    onValueModified: kcm.client.draftControllerPort = value

                    QQC2.ToolTip.visible: hovered
                    QQC2.ToolTip.delay: 700
                    QQC2.ToolTip.text: tx.i18n("This page talks to the controller, which listens only on this computer.")
                }

                FormCard.FormDelegateSeparator {}

                FormCard.FormTextFieldDelegate {
                    label: tx.i18n("mihomo executable")
                    text: kcm.client.draftMihomoPath
                    onTextChanged: {
                        if (text !== kcm.client.draftMihomoPath) {
                            kcm.client.draftMihomoPath = text;
                        }
                    }

                    QQC2.ToolTip.visible: hovered
                    QQC2.ToolTip.delay: 700
                    QQC2.ToolTip.text: tx.i18n("This is the mihomo program already installed on the system.")
                }

                FormCard.FormDelegateSeparator {}

                FormCard.FormTextFieldDelegate {
                    label: tx.i18n("TUN device")
                    text: kcm.client.draftTunDevice
                    maximumLength: 15
                    onTextChanged: {
                        if (text !== kcm.client.draftTunDevice) {
                            kcm.client.draftTunDevice = text;
                        }
                    }

                    QQC2.ToolTip.visible: hovered
                    QQC2.ToolTip.delay: 700
                    QQC2.ToolTip.text: tx.i18n("Leave this empty to let the kernel choose the name, and applying a new name asks you to confirm.")
                }

                FormCard.FormDelegateSeparator {}

                FormCard.FormTextFieldDelegate {
                    id: overrideField
                    label: tx.i18n("Override file")
                    text: kcm.client.draftOverridePath
                    onTextChanged: {
                        if (text !== kcm.client.draftOverridePath) {
                            kcm.client.draftOverridePath = text;
                        }
                    }

                    QQC2.ToolTip.visible: hovered
                    QQC2.ToolTip.delay: 700
                    QQC2.ToolTip.text: tx.i18n("This file is placed over the subscription, while the listen address stays set here.")
                }

                FormCard.FormDelegateSeparator {}

                FormCard.FormButtonDelegate {
                    text: tx.i18n("Open file")
                    onClicked: kcm.client.openOverrideFile(overrideField.text)

                    QQC2.ToolTip.visible: hovered
                    QQC2.ToolTip.delay: 700
                    QQC2.ToolTip.text: tx.i18n("Opens the override file, creating it from the template when it is missing.")
                }

                FormCard.FormDelegateSeparator {}

                FormCard.FormButtonDelegate {
                    text: tx.i18n("Open folder")
                    onClicked: kcm.client.openOverrideFolder(overrideField.text)

                    QQC2.ToolTip.visible: hovered
                    QQC2.ToolTip.delay: 700
                    QQC2.ToolTip.text: tx.i18n("Opens the folder that contains the override file.")
                }
            }

            FormCard.FormHeader {
                title: tx.i18n("Accounts")
                Layout.fillWidth: true
            }

            FormCard.FormCard {
                Layout.fillWidth: true

                FormCard.FormButtonDelegate {
                    text: tx.i18n("Manage signed-in accounts")
                    description: tx.i18n("Asks for the administrator password, then lists the other signed-in accounts.")
                    enabled: !kcm.client.busy
                    onClicked: kcm.client.reloadAccounts()
                }
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
                    icon.name: "lock"
                    text: tx.i18n("Lock")
                    onClicked: {
                        root.detailUid = -1;
                        kcm.client.lockAccounts();
                    }
                }
            }

            FormCard.FormHeader {
                title: tx.i18n("This computer")
                Layout.fillWidth: true
            }

            FormCard.FormCard {
                Layout.fillWidth: true

                FormCard.FormSwitchDelegate {
                    id: machineSwitch
                    text: tx.i18n("Capture the whole computer")
                    description: tx.i18n("Sends every account and root through this account's configuration.")
                    checked: kcm.client.tunEnabled && kcm.client.tunMachine
                    enabled: !kcm.client.busy
                    onClicked: kcm.client.setMachineTun(!(kcm.client.tunEnabled && kcm.client.tunMachine))
                }
            }

            FormCard.FormHeader {
                title: tx.i18n("Signed-in accounts")
                Layout.fillWidth: true
            }

            FormCard.FormCard {
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
                title: root.detailAccount ? root.detailAccount.name : ""
                Layout.fillWidth: true
            }

            FormCard.FormCard {
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

                FormCard.FormTextDelegate {
                    text: tx.i18n("Exit")
                    description: {
                        if (!root.detailAccount) {
                            return "";
                        }
                        const formatted = kcm.client.formatExit(root.detailAccount.exit_region || "");
                        return formatted.length > 0 ? formatted : "—";
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
