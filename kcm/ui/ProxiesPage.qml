import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

Item {
    id: root

    Tx {
        id: tx
    }

    function delayText(ms) {
        if (ms < 0) {
            return tx.i18n("Not tested");
        }
        if (ms === 0) {
            return tx.i18n("Timeout");
        }
        return tx.i18n("%1 ms", ms);
    }

    function currentGroup() {
        const groups = kcm.client.groups;
        for (let i = 0; i < groups.length; ++i) {
            if (groups[i].name === groupBox.currentText) {
                return groups[i];
            }
        }
        return groups.length > 0 ? groups[0] : null;
    }

    ColumnLayout {
        anchors.fill: parent
        spacing: Kirigami.Units.largeSpacing

        QQC2.Label {
            visible: kcm.client.groups.length === 0
            text: kcm.client.coreRunning ? tx.i18n("No proxy groups") : tx.i18n("Core is not running")
            opacity: 0.7
            Layout.fillWidth: true
        }

        RowLayout {
            Layout.fillWidth: true
            visible: kcm.client.groups.length > 0
            spacing: Kirigami.Units.largeSpacing

            QQC2.Label {
                text: tx.i18n("Group")
            }

            QQC2.ComboBox {
                id: groupBox
                model: kcm.client.groups
                textRole: "name"
                Layout.fillWidth: true
                Layout.maximumWidth: Kirigami.Units.gridUnit * 20
            }

            Item {
                Layout.fillWidth: true
            }

            QQC2.Button {
                text: tx.i18n("Test delays")
                enabled: kcm.client.coreRunning && !kcm.client.busy && groupBox.currentText.length > 0
                onClicked: kcm.client.testGroupDelay(groupBox.currentText)
            }
        }

        Kirigami.Separator {
            Layout.fillWidth: true
            visible: kcm.client.groups.length > 0
        }

        ListView {
            Layout.fillWidth: true
            Layout.fillHeight: true
            clip: true
            visible: kcm.client.groups.length > 0
            model: {
                const group = root.currentGroup();
                return group && group.nodes ? group.nodes : [];
            }

            delegate: QQC2.ItemDelegate {
                width: ListView.view.width
                required property var modelData
                highlighted: {
                    const group = root.currentGroup();
                    return group && modelData.name === group.now;
                }
                enabled: {
                    const group = root.currentGroup();
                    return group && group["type"] === "Selector" && !kcm.client.busy;
                }
                onClicked: {
                    const group = root.currentGroup();
                    if (group) {
                        kcm.client.selectProxy(group.name, modelData.name);
                    }
                }

                contentItem: RowLayout {
                    spacing: Kirigami.Units.largeSpacing

                    QQC2.Label {
                        text: modelData.name
                        elide: Text.ElideRight
                        Layout.fillWidth: true
                    }
                    QQC2.Label {
                        text: root.delayText(modelData.delay_ms)
                        opacity: 0.75
                    }
                }
            }
        }
    }
}
