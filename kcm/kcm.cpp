// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 clash-kde contributors

#include "kcm.h"

#include "clashclient.h"

#include <KPluginFactory>

K_PLUGIN_CLASS_WITH_JSON(ClashKcm, "kcm_clashkde.json")

ClashKcm::ClashKcm(QObject *parent, const KPluginMetaData &data)
    : KQuickConfigModule(parent, data)
    , m_client(new ClashClient(this))
{
    setButtons(Apply);
}

ClashClient *ClashKcm::client() const
{
    return m_client;
}

void ClashKcm::save()
{
    if (m_client->commitSettings()) {
        setNeedsSave(false);
    }
}

#include "kcm.moc"
