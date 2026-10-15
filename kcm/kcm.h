// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 clash-kde contributors

#pragma once

#include <KQuickConfigModule>

class ClashClient;

class ClashKcm : public KQuickConfigModule
{
    Q_OBJECT
    Q_PROPERTY(ClashClient *client READ client CONSTANT)

public:
    ClashKcm(QObject *parent, const KPluginMetaData &data);
    ClashClient *client() const;
    void load() override;
    void save() override;
    void defaults() override;

private:
    ClashClient *m_client = nullptr;
};
