// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 clash-kde contributors

import QtQuick

// Translation goes through ClashClient so the page does not need the
// org.kde.ki18n QML module at runtime. That module is not available in
// Debian 13 even though KF6 itself is.
QtObject {
    function i18n(text) {
        let args = [];
        for (let i = 1; i < arguments.length; ++i) {
            args.push(arguments[i]);
        }
        return kcm.client.i18n(text, args);
    }

    function i18nc(context, text) {
        let args = [];
        for (let i = 2; i < arguments.length; ++i) {
            args.push(arguments[i]);
        }
        return kcm.client.i18nc(context, text, args);
    }

    // Grouping, separators, and the decimal mark come from the user's formats.
    function formatNumber(value, decimals) {
        const loc = Qt.locale();
        if (decimals === undefined || decimals === 0) {
            return loc.toString(Math.round(value));
        }
        return loc.toString(value, "f", decimals);
    }
}
