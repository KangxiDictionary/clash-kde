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

inline QString blurAddress(const QString &address)
{
    const QString text = address.trimmed();
    if (text.isEmpty()) {
        return text;
    }
    if (!text.contains(QLatin1Char(':')) && text.count(QLatin1Char('.')) == 3) {
        const QStringList parts = text.split(QLatin1Char('.'));
        bool ipv4 = parts.size() == 4;
        for (const QString &part : parts) {
            bool ok = false;
            const int value = part.toInt(&ok);
            if (!ok || value < 0 || value > 255) {
                ipv4 = false;
                break;
            }
        }
        if (ipv4) {
            return parts.at(0) + QLatin1Char('.') + parts.at(1) + QStringLiteral(".*.*");
        }
    }
    if (text.contains(QLatin1Char(':')) && !text.contains(QLatin1Char('.'))) {
        const QStringList groups = text.split(QLatin1Char(':'));
        if (groups.size() >= 2) {
            return groups.at(0) + QLatin1Char(':') + groups.at(1) + QStringLiteral(":*");
        }
    }
    return QStringLiteral("*");
}

inline bool looksLikeIp(const QString &address)
{
    const QString text = address.trimmed();
    if (!text.contains(QLatin1Char(':')) && text.count(QLatin1Char('.')) == 3) {
        const QStringList parts = text.split(QLatin1Char('.'));
        if (parts.size() != 4) {
            return false;
        }
        for (const QString &part : parts) {
            bool ok = false;
            const int value = part.toInt(&ok);
            if (!ok || value < 0 || value > 255) {
                return false;
            }
        }
        return true;
    }
    return text.contains(QLatin1Char(':')) && !text.contains(QLatin1Char('.')) && text.count(QLatin1Char(':')) >= 2;
}

inline QString presentHost(const QString &host, bool blurIp)
{
    const QString text = host.trimmed();
    if (text.isEmpty() || !blurIp || !looksLikeIp(text)) {
        return text;
    }
    return blurAddress(text);
}

inline QString exitAddress(const QString &raw)
{
    const QString text = raw.trimmed();
    if (text.isEmpty() || text == QStringLiteral("正在检测")) {
        return {};
    }
    const QStringList parts = text.split(QLatin1Char('\t'));
    if (parts.size() == 3 && parts.at(0).size() == 2) {
        return parts.at(2).trimmed();
    }
    return {};
}

inline QString formatExitPlace(const QString &raw)
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
    QString place = flagEmoji(parts.at(0)) + QLatin1Char(' ') + country;
    if (!parts.at(1).isEmpty()) {
        place += QStringLiteral(" · ") + parts.at(1);
    }
    return place;
}

inline QString formatExitAddress(const QString &raw, bool blurIp)
{
    const QString address = exitAddress(raw);
    if (address.isEmpty()) {
        return {};
    }
    return blurIp ? blurAddress(address) : address;
}

inline QString formatExitRegion(const QString &raw, bool blurIp = false)
{
    const QString place = formatExitPlace(raw);
    if (place.isEmpty() || place == QStringLiteral("正在检测") || place.startsWith(QStringLiteral("❓"))) {
        return place;
    }
    const QString address = formatExitAddress(raw, blurIp);
    if (address.isEmpty()) {
        return place;
    }
    return place + QLatin1Char('\n') + address;
}
