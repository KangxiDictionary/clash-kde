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

        RowLayout {
            QQC2.TabBar {
                id: tabs
                Layout.fillWidth: true
                QQC2.TabButton { text: tx.i18n("Status") }
                QQC2.TabButton { text: tx.i18n("Profiles") }
                QQC2.TabButton { text: tx.i18n("Proxies") }
                QQC2.TabButton { text: tx.i18n("Users") }
                QQC2.TabButton { text: tx.i18n("Advanced") }
                onCurrentIndexChanged: {
                    if (currentIndex === 3) {
                        kcm.client.reloadAccounts();
                    }
                }
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
