// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 clash-kde contributors

import QtQuick

QtObject {
    function score(query, text) {
        const needle = String(query).trim().toLowerCase();
        if (needle.length === 0) {
            return 0;
        }
        const hay = String(text).toLowerCase();
        let score = 0;
        let at = 0;
        let streak = 0;
        for (let i = 0; i < needle.length; ++i) {
            const found = hay.indexOf(needle[i], at);
            if (found < 0) {
                return -1;
            }
            streak = found === at ? streak + 1 : 1;
            score += 8 + streak * 6 - (found - at);
            at = found + 1;
        }
        if (hay.startsWith(needle)) {
            score += 20;
        }
        return score;
    }

    function best(query, fields) {
        let best = -1;
        for (let i = 0; i < fields.length; ++i) {
            const weight = score(query, fields[i]);
            if (weight > best) {
                best = weight;
            }
        }
        return best;
    }
}
