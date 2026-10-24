// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 clash-kde contributors

import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

Kirigami.Page {
    title: tx.i18n("Mihomo")
    padding: Kirigami.Units.smallSpacing

    Tx {
        id: tx
    }

    Connections {
        target: kcm.client
        function onDraftChanged() {
            kcm.needsSave = kcm.client.settingsDirty;
            kcm.representsDefaults = kcm.client.draftIsDefaults();
        }
    }

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: Kirigami.Units.smallSpacing

        Item {
            id: hintBanner
            Layout.fillWidth: true
            visible: kcm.client.hint.length > 0
            implicitHeight: visible ? hintMessage.implicitHeight : 0
            property real elapsed: 0
            property bool paused: false

            Timer {
                interval: 50
                repeat: true
                running: hintBanner.visible && !hintBanner.paused
                onTriggered: {
                    hintBanner.elapsed += 50;
                    if (hintBanner.elapsed >= 5000) {
                        kcm.client.clearHint();
                    }
                }
            }

            Connections {
                target: kcm.client
                function onHintChanged() {
                    hintBanner.elapsed = 0;
                }
            }

            HoverHandler {
                onHoveredChanged: hintBanner.paused = hovered
            }

            Kirigami.InlineMessage {
                id: hintMessage
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.top: parent.top
                type: Kirigami.MessageType.Information
                visible: hintBanner.visible
                text: kcm.client.hint
                showCloseButton: true
                onVisibleChanged: {
                    if (!visible && kcm.client.hint.length > 0) {
                        kcm.client.clearHint();
                    }
                }
            }

            QQC2.ProgressBar {
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.bottom: parent.bottom
                from: 0
                to: 5000
                value: Math.max(0, 5000 - hintBanner.elapsed)
                padding: 0
                implicitHeight: 4
                height: 4
                visible: hintBanner.visible
            }
        }

        RowLayout {
            QQC2.TabBar {
                id: tabs
                Layout.fillWidth: true
                onCurrentIndexChanged: {
                    if (currentIndex === 2) {
                        kcm.client.ensureProxies();
                    }
                }
                QQC2.TabButton { text: tx.i18n("Status"); Kirigami.MnemonicData.enabled: false }
                QQC2.TabButton { text: tx.i18n("Profiles"); Kirigami.MnemonicData.enabled: false }
                QQC2.TabButton { text: tx.i18n("Nodes"); Kirigami.MnemonicData.enabled: false }
                QQC2.TabButton { text: tx.i18n("Users"); Kirigami.MnemonicData.enabled: false }
                QQC2.TabButton { text: tx.i18n("Other"); Kirigami.MnemonicData.enabled: false }
            }
            QQC2.BusyIndicator {
                running: kcm.client.busy
                visible: kcm.client.busy
                Layout.preferredWidth: Kirigami.Units.gridUnit * 1.5
                Layout.preferredHeight: Kirigami.Units.gridUnit * 1.5
            }
        }

        StackLayout {
            Layout.fillWidth: true
            Layout.fillHeight: true
            currentIndex: tabs.currentIndex

            StatusPage {}
            ProfilesPage {}
            ProxiesPage {}
            UsersPage {}
            AdvancedPage {}
        }
    }
}
