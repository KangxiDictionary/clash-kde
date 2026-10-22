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
        return tx.i18n("Off");
    }

    function proxyIcon(state) {
        if (state === "ours") {
            return "checkmark";
        }
        if (state === "other") {
            return "dialog-warning";
        }
        return "dialog-cancel";
    }

    function proxyColor(state) {
        if (state === "ours") {
            return Kirigami.Theme.positiveTextColor;
        }
        if (state === "other") {
            return Kirigami.Theme.neutralTextColor;
        }
        return Kirigami.Theme.disabledTextColor;
    }

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
            Layout.fillWidth: true
            spacing: 0

            FormCard.FormHeader {
                maximumWidth: Infinity
                title: tx.i18n("This account")
                Layout.fillWidth: true
            }

            FormCard.FormCard {
                maximumWidth: Infinity
                Layout.fillWidth: true

                RowLayout {
                    Layout.fillWidth: true
                    Layout.margins: Kirigami.Units.largeSpacing
                    spacing: Kirigami.Units.largeSpacing

                    RowLayout {
                        Layout.fillWidth: true
                        spacing: Kirigami.Units.largeSpacing

                        Kirigami.Icon {
                            source: root.proxyIcon(kcm.client.kdeProxy)
                            isMask: true
                            color: root.proxyColor(kcm.client.kdeProxy)
                            implicitWidth: Kirigami.Units.iconSizes.smallMedium
                            implicitHeight: Kirigami.Units.iconSizes.smallMedium
                        }

                        ColumnLayout {
                            Layout.fillWidth: true
                            spacing: 0

                            QQC2.Label {
                                text: tx.i18n("KDE proxy")
                                opacity: 0.65
                                font: Kirigami.Theme.smallFont
                                Kirigami.MnemonicData.enabled: false
                            }
                            QQC2.Label {
                                text: root.proxyText(kcm.client.kdeProxy)
                                wrapMode: Text.WordWrap
                                Layout.fillWidth: true
                                Kirigami.MnemonicData.enabled: false
                            }
                        }

                        HoverHandler { id: kdeHover }
                        QQC2.ToolTip.visible: kdeHover.hovered
                        QQC2.ToolTip.delay: 700
                        QQC2.ToolTip.text: tx.i18n("Shows whether KDE is telling programs to use a proxy.")
                    }

                    QQC2.ToolSeparator {
                        Layout.fillHeight: true
                    }

                    RowLayout {
                        Layout.fillWidth: true
                        spacing: Kirigami.Units.largeSpacing

                        Kirigami.Icon {
                            source: root.proxyIcon(kcm.client.gnomeProxy)
                            isMask: true
                            color: root.proxyColor(kcm.client.gnomeProxy)
                            implicitWidth: Kirigami.Units.iconSizes.smallMedium
                            implicitHeight: Kirigami.Units.iconSizes.smallMedium
                        }

                        ColumnLayout {
                            Layout.fillWidth: true
                            spacing: 0

                            QQC2.Label {
                                text: tx.i18n("GNOME proxy")
                                opacity: 0.65
                                font: Kirigami.Theme.smallFont
                                Kirigami.MnemonicData.enabled: false
                            }
                            QQC2.Label {
                                text: root.proxyText(kcm.client.gnomeProxy)
                                wrapMode: Text.WordWrap
                                Layout.fillWidth: true
                                Kirigami.MnemonicData.enabled: false
                            }
                        }

                        HoverHandler { id: gnomeHover }
                        QQC2.ToolTip.visible: gnomeHover.hovered
                        QQC2.ToolTip.delay: 700
                        QQC2.ToolTip.text: tx.i18n("Shows whether GNOME is telling programs to use a proxy.")
                    }
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
                maximumWidth: Infinity
                title: tx.i18n("Connection")
                Layout.fillWidth: true
            }

            FormCard.FormCard {
                maximumWidth: Infinity
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
                    text: tx.i18n("Allow LAN")
                    checked: kcm.client.draftAllowLan
                    enabled: kcm.client.lanAllowed
                    description: kcm.client.lanAllowed ? "" : tx.i18n("The administrator turned this off.")
                    onToggled: {
                        if (checked !== kcm.client.draftAllowLan) {
                            kcm.client.draftAllowLan = checked;
                        }
                    }

                    QQC2.ToolTip.visible: hovered
                    QQC2.ToolTip.delay: 700
                    QQC2.ToolTip.text: tx.i18n("Lets other computers on the local network use the mixed port. The controller stays on this computer.")
                }

                FormCard.FormDelegateSeparator {
                    visible: kcm.client.draftAllowLan
                }

                FormCard.FormTextFieldDelegate {
                    visible: kcm.client.draftAllowLan
                    label: tx.i18n("Username")
                    enabled: kcm.client.lanAllowed
                    text: kcm.client.draftLanUsername
                    onTextChanged: {
                        if (text !== kcm.client.draftLanUsername) {
                            kcm.client.draftLanUsername = text;
                        }
                    }
                }

                FormCard.FormDelegateSeparator {
                    visible: kcm.client.draftAllowLan
                }

                FormCard.FormTextFieldDelegate {
                    visible: kcm.client.draftAllowLan
                    label: tx.i18n("Password")
                    enabled: kcm.client.lanAllowed
                    echoMode: TextInput.Password
                    text: kcm.client.draftLanPassword
                    onTextChanged: {
                        if (text !== kcm.client.draftLanPassword) {
                            kcm.client.draftLanPassword = text;
                        }
                    }
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
        }
    }
}
