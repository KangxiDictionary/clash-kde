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
    setButtons(Apply | Default);
    setSupportsInstantApply(false);
    connect(m_client, &ClashClient::settingsApplied, this, [this]() {
        setNeedsSave(false);
    });
    connect(m_client, &ClashClient::settingsFailed, this, [this](const QString &) {
        setNeedsSave(true);
    });
}

ClashClient *ClashKcm::client() const
{
    return m_client;
}

void ClashKcm::load()
{
    m_client->revertDraft();
    setNeedsSave(false);
    setRepresentsDefaults(m_client->draftIsDefaults());
}

void ClashKcm::save()
{
    if (m_client->commitSettings()) {
        setNeedsSave(false);
        setRepresentsDefaults(m_client->draftIsDefaults());
    }
}

void ClashKcm::defaults()
{
    m_client->applyDefaultsToDraft();
    setNeedsSave(m_client->settingsDirty());
    setRepresentsDefaults(m_client->draftIsDefaults());
}

#include "kcm.moc"
