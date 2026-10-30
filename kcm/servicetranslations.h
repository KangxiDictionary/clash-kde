// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 clash-kde contributors

#pragma once

#include <QJsonObject>
#include <QString>

QString translateServiceCode(const QString &code, const QJsonObject &params, const QString &detail);
