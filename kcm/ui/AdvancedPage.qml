import QtQuick
import QtQuick.Layouts
import org.kde.kirigamiaddons.formcard as FormCard

Flickable {
    id: root

    contentWidth: width
    contentHeight: body.implicitHeight
    clip: true

    Tx {
        id: tx
    }

    ColumnLayout {
        id: body
        width: root.width
        spacing: 0

        FormCard.FormHeader {
            title: tx.i18n("About")
            Layout.fillWidth: true
        }

        FormCard.FormCard {
            Layout.fillWidth: true

            FormCard.FormTextDelegate {
                text: tx.i18n("Name")
                description: tx.i18n("Mihomo")
            }

            FormCard.FormDelegateSeparator {}

            FormCard.FormTextDelegate {
                text: tx.i18n("Version")
                description: "1.0.1"
            }

            FormCard.FormDelegateSeparator {}

            FormCard.FormTextDelegate {
                text: tx.i18n("License")
                description: "GNU GPL v3"
            }

            FormCard.FormDelegateSeparator {}

            FormCard.FormLinkDelegate {
                text: tx.i18n("Website")
                description: "github.com/KangxiDictionary/clash-kde"
                url: "https://github.com/KangxiDictionary/clash-kde"
            }
        }
    }
}
