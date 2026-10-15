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
    const bool unchanged = m_client->draftMixedPort() == m_client->mixedPort()
        && m_client->draftControllerPort() == m_client->controllerPort()
        && m_client->draftMihomoPath() == m_client->mihomoPath();
    setNeedsSave(!unchanged);
    setRepresentsDefaults(true);
}

#include "kcm.moc"
