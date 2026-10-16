// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 clash-kde contributors

#pragma once

#include <QLocale>
#include <QString>
#include <QStringList>

#include <unicode/uloc.h>
#include <unicode/ustring.h>

inline QString flagEmoji(const QString &code)
{
    if (code.size() != 2) {
        return QStringLiteral("❓");
    }
    const char first = code.at(0).toUpper().toLatin1();
    const char second = code.at(1).toUpper().toLatin1();
    if (first < 'A' || first > 'Z' || second < 'A' || second > 'Z') {
        return QStringLiteral("❓");
    }
    if (QLocale::codeToTerritory(code) == QLocale::AnyTerritory) {
        return QStringLiteral("❓");
    }
    const char32_t points[] = {
        0x1F1E6 + static_cast<char32_t>(first - 'A'),
        0x1F1E6 + static_cast<char32_t>(second - 'A'),
    };
    return QString::fromUcs4(points, 2);
}

inline QString territoryName(const QString &code)
{
    if (QLocale::codeToTerritory(code) == QLocale::AnyTerritory) {
        return {};
    }
    const QByteArray id = QByteArrayLiteral("und_") + code.toLatin1();
    const QByteArray display = QLocale().name().toUtf8();
    UChar buffer[96];
    UErrorCode error = U_ZERO_ERROR;
    const int32_t length = uloc_getDisplayCountry(id.constData(), display.constData(), buffer, 96, &error);
    if (U_FAILURE(error) || length <= 0) {
        return {};
    }
    return QString::fromUtf16(reinterpret_cast<const char16_t *>(buffer), length);
}

inline QString formatExitRegion(const QString &raw)
{
    if (raw.isEmpty() || raw == QStringLiteral("正在检测")) {
        return raw;
    }
    const QStringList parts = raw.split(QLatin1Char('\t'));
    if (parts.size() != 3 || parts.at(0).size() != 2) {
        return QStringLiteral("❓ ") + raw;
    }
    const QString country = territoryName(parts.at(0));
    if (country.isEmpty()) {
        return QStringLiteral("❓");
    }
    QString place = country;
    if (!parts.at(1).isEmpty()) {
        place += QStringLiteral(" · ") + parts.at(1);
    }
    if (!parts.at(2).isEmpty()) {
        place = QStringLiteral("%1 (%2)").arg(place, parts.at(2));
    }
    return flagEmoji(parts.at(0)) + QLatin1Char(' ') + place;
}
