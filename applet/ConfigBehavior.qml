/*
    SPDX-FileCopyrightText: 2026 clash-kde contributors
    SPDX-License-Identifier: GPL-3.0-or-later
*/

import QtQuick
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami

Kirigami.FormLayout {
    id: root

    property string cfg_clickAction
    property string cfg_middleClickAction
    property string cfg_wheelAction
    property string cfg_routeSort
    property bool cfg_showUnreachable

    readonly property var clickActions: [
        { label: i18nc("@item:inlistbox", "Open the popup"), value: "popup" },
        { label: i18nc("@item:inlistbox", "Toggle the core"), value: "toggleCore" },
        { label: i18nc("@item:inlistbox", "Toggle the system proxy"), value: "toggleSystemProxy" },
        { label: i18nc("@item:inlistbox", "Cycle mode"), value: "cycleMode" },
        { label: i18nc("@item:inlistbox", "Open settings"), value: "openSettings" },
        { label: i18nc("@item:inlistbox", "Do nothing"), value: "none" }
    ]
    readonly property var wheelActions: [
        { label: i18nc("@item:inlistbox", "Do nothing"), value: "none" },
        { label: i18nc("@item:inlistbox", "Cycle mode"), value: "cycleMode" },
        { label: i18nc("@item:inlistbox", "Toggle the core"), value: "toggleCore" },
        { label: i18nc("@item:inlistbox", "Toggle the system proxy"), value: "toggleSystemProxy" }
    ]

    QQC2.ComboBox {
        Kirigami.FormData.label: i18nc("@label:listbox", "Left click")
        textRole: "label"
        valueRole: "value"
        model: root.clickActions
        Component.onCompleted: currentIndex = Math.max(0, indexOfValue(root.cfg_clickAction))
        onActivated: root.cfg_clickAction = currentValue
    }

    QQC2.ComboBox {
        Kirigami.FormData.label: i18nc("@label:listbox", "Middle click")
        textRole: "label"
        valueRole: "value"
        model: root.clickActions
        Component.onCompleted: currentIndex = Math.max(0, indexOfValue(root.cfg_middleClickAction))
        onActivated: root.cfg_middleClickAction = currentValue
    }

    QQC2.ComboBox {
        Kirigami.FormData.label: i18nc("@label:listbox", "Scroll wheel")
        textRole: "label"
        valueRole: "value"
        model: root.wheelActions
        Component.onCompleted: currentIndex = Math.max(0, indexOfValue(root.cfg_wheelAction))
        onActivated: root.cfg_wheelAction = currentValue
    }

    QQC2.ComboBox {
        Kirigami.FormData.label: i18nc("@label:listbox", "Route order")
        textRole: "label"
        valueRole: "value"
        model: [
            { label: i18nc("@item:inlistbox", "Profile order"), value: "profile" },
            { label: i18nc("@item:inlistbox", "Name"), value: "name" },
            { label: i18nc("@item:inlistbox", "Delay"), value: "delay" }
        ]
        Component.onCompleted: currentIndex = Math.max(0, indexOfValue(root.cfg_routeSort))
        onActivated: root.cfg_routeSort = currentValue
    }

    QQC2.CheckBox {
        text: i18nc("@option:check", "Show unreachable routes")
        checked: root.cfg_showUnreachable
        onToggled: root.cfg_showUnreachable = checked
    }
}
