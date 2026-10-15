// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 clash-kde contributors

#include "clashclient.h"

#include <QDBusConnection>
#include <QDBusInterface>
#include <QDBusPendingCallWatcher>
#include <QDBusPendingReply>
#include <QDBusReply>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QTimer>

namespace {
QVariantList jsonArray(const QString &text)
{
    const auto document = QJsonDocument::fromJson(text.toUtf8());
    QVariantList items;
    for (const auto &value : document.array()) {
        items.append(value.toObject().toVariantMap());
    }
    return items;
}
}

ClashClient::ClashClient(QObject *parent)
    : QObject(parent)
    , m_iface(new QDBusInterface(QStringLiteral("org.kde.clashkde"),
                                 QStringLiteral("/org/kde/clashkde"),
                                 QStringLiteral("org.kde.clashkde.Manager"),
                                 QDBusConnection::sessionBus(),
                                 this))
{
    auto bus = QDBusConnection::sessionBus();
    bus.connect(QStringLiteral("org.kde.clashkde"),
                 QStringLiteral("/org/kde/clashkde"),
                 QStringLiteral("org.kde.clashkde.Manager"),
                 QStringLiteral("StatusChanged"),
                 this,
                 SLOT(refreshStatus()));
    bus.connect(QStringLiteral("org.kde.clashkde"),
                QStringLiteral("/org/kde/clashkde"),
                QStringLiteral("org.kde.clashkde.Manager"),
                QStringLiteral("ProfilesChanged"),
                this,
                SLOT(refreshProfiles()));
    bus.connect(QStringLiteral("org.kde.clashkde"),
                 QStringLiteral("/org/kde/clashkde"),
                 QStringLiteral("org.kde.clashkde.Manager"),
                 QStringLiteral("ProxiesChanged"),
                 this,
                 SLOT(refreshProxies()));

    auto *timer = new QTimer(this);
    timer->setInterval(1000);
    connect(timer, &QTimer::timeout, this, &ClashClient::refreshStatus);
    timer->start();
    reload();
}

bool ClashClient::coreDesired() const { return m_coreDesired; }
bool ClashClient::coreRunning() const { return m_coreRunning; }
bool ClashClient::systemProxyEnabled() const { return m_systemProxyEnabled; }
bool ClashClient::systemProxyApplied() const { return m_systemProxyApplied; }
QString ClashClient::mode() const { return m_mode; }
QString ClashClient::listenAddress() const
{
    return QStringLiteral("http://%1:%2").arg(m_listenHost).arg(m_mixedPort);
}
int ClashClient::mixedPort() const { return m_mixedPort; }
int ClashClient::controllerPort() const { return m_controllerPort; }
QString ClashClient::mihomoPath() const { return m_mihomoPath; }
QString ClashClient::overridePath() const { return m_overridePath; }
qulonglong ClashClient::uploadBps() const { return m_uploadBps; }
qulonglong ClashClient::downloadBps() const { return m_downloadBps; }
QString ClashClient::exitRegion() const { return m_exitRegion; }
QString ClashClient::activeProfile() const { return m_activeProfile; }
QString ClashClient::message() const { return m_message; }
bool ClashClient::busy() const { return m_busy; }
QVariantList ClashClient::profiles() const { return m_profiles; }
QVariantList ClashClient::groups() const { return m_groups; }
int ClashClient::draftMixedPort() const { return m_draftMixedPort; }
int ClashClient::draftControllerPort() const { return m_draftControllerPort; }
QString ClashClient::draftMihomoPath() const { return m_draftMihomoPath; }

void ClashClient::setDraftMixedPort(int port)
{
    if (m_draftMixedPort == port) {
        return;
    }
    m_draftMixedPort = port;
    m_draftDirty = true;
    Q_EMIT draftChanged();
}

void ClashClient::setDraftControllerPort(int port)
{
    if (m_draftControllerPort == port) {
        return;
    }
    m_draftControllerPort = port;
    m_draftDirty = true;
    Q_EMIT draftChanged();
}

void ClashClient::setDraftMihomoPath(const QString &path)
{
    if (m_draftMihomoPath == path) {
        return;
    }
    m_draftMihomoPath = path;
    m_draftDirty = true;
    Q_EMIT draftChanged();
}

void ClashClient::setCoreDesired(bool desired)
{
    asyncCall(desired ? QStringLiteral("Start") : QStringLiteral("Stop"), {}, false, true);
}

void ClashClient::restartCore()
{
    asyncCall(QStringLiteral("Restart"), {}, false, true);
}

void ClashClient::setSystemProxyEnabled(bool enabled)
{
    asyncCall(QStringLiteral("SetSystemProxy"), {enabled}, false, false);
}

void ClashClient::setMode(const QString &mode)
{
    asyncCall(QStringLiteral("SetMode"), {mode}, false, false);
}

void ClashClient::setActiveProfile(const QString &id)
{
    asyncCall(QStringLiteral("SetActiveProfile"), {id}, true, true);
}

void ClashClient::addSubscription(const QString &name, const QString &url)
{
    asyncCall(QStringLiteral("AddSubscription"), {name, url}, true, false);
}

void ClashClient::removeProfile(const QString &id)
{
    asyncCall(QStringLiteral("RemoveProfile"), {id}, true, true);
}

void ClashClient::updateProfile(const QString &id)
{
    asyncCall(QStringLiteral("UpdateProfile"), {id}, true, true);
}

void ClashClient::selectProxy(const QString &group, const QString &name)
{
    asyncCall(QStringLiteral("SelectProxy"), {group, name}, false, true);
}

void ClashClient::testGroupDelay(const QString &group)
{
    asyncCall(QStringLiteral("TestGroupDelay"), {group}, false, true);
}

void ClashClient::refreshExitRegion()
{
    asyncCall(QStringLiteral("RefreshExitRegion"), {}, false, false);
}

void ClashClient::reload()
{
    refreshStatus();
    refreshProfiles();
    refreshProxies();
}

bool ClashClient::commitSettings()
{
    QDBusReply<void> reply = m_iface->call(QStringLiteral("SetSettings"),
                                           static_cast<quint32>(m_draftMixedPort),
                                           static_cast<quint32>(m_draftControllerPort),
                                           m_draftMihomoPath);
    if (!reply.isValid()) {
        reportError(reply.error().message());
        return false;
    }
    m_draftDirty = false;
    refreshStatus();
    return true;
}

void ClashClient::refreshStatus()
{
    QDBusReply<QString> reply = m_iface->call(QStringLiteral("GetStatus"));
    if (!reply.isValid()) {
        if (m_message != reply.error().message()) {
            m_message = reply.error().message();
            Q_EMIT statusChanged();
        }
        return;
    }
    const auto document = QJsonDocument::fromJson(reply.value().toUtf8());
    applyStatus(document.object().toVariantMap());
}

void ClashClient::refreshProfiles()
{
    QDBusReply<QString> reply = m_iface->call(QStringLiteral("GetProfiles"));
    if (!reply.isValid()) {
        reportError(reply.error().message());
        return;
    }
    m_profiles = jsonArray(reply.value());
    Q_EMIT profilesChanged();
}

void ClashClient::refreshProxies()
{
    QDBusReply<QString> reply = m_iface->call(QStringLiteral("GetProxies"));
    if (!reply.isValid()) {
        reportError(reply.error().message());
        return;
    }
    m_groups = jsonArray(reply.value());
    Q_EMIT proxiesChanged();
}

void ClashClient::setBusy(bool busy)
{
    if (m_busy == busy) {
        return;
    }
    m_busy = busy;
    Q_EMIT busyChanged();
}

void ClashClient::applyStatus(const QVariantMap &status)
{
    m_coreDesired = status.value(QStringLiteral("core_desired")).toBool();
    m_coreRunning = status.value(QStringLiteral("core_running")).toBool();
    m_systemProxyEnabled = status.value(QStringLiteral("system_proxy_enabled")).toBool();
    m_systemProxyApplied = status.value(QStringLiteral("system_proxy_applied")).toBool();
    m_mode = status.value(QStringLiteral("mode")).toString();
    m_listenHost = status.value(QStringLiteral("listen_host"), QStringLiteral("127.0.0.1")).toString();
    m_mixedPort = status.value(QStringLiteral("mixed_port")).toInt();
    m_controllerPort = status.value(QStringLiteral("controller_port")).toInt();
    m_mihomoPath = status.value(QStringLiteral("mihomo_path")).toString();
    m_overridePath = status.value(QStringLiteral("override_path")).toString();
    m_uploadBps = status.value(QStringLiteral("upload_bps")).toULongLong();
    m_downloadBps = status.value(QStringLiteral("download_bps")).toULongLong();
    m_exitRegion = status.value(QStringLiteral("exit_region")).toString();
    m_activeProfile = status.value(QStringLiteral("active_profile")).toString();
    m_message = status.value(QStringLiteral("message")).toString();
    if (!m_draftDirty) {
        const bool changed = m_draftMixedPort != m_mixedPort || m_draftControllerPort != m_controllerPort || m_draftMihomoPath != m_mihomoPath;
        m_draftMixedPort = m_mixedPort;
        m_draftControllerPort = m_controllerPort;
        m_draftMihomoPath = m_mihomoPath;
        if (changed) {
            Q_EMIT draftChanged();
        }
    }
    Q_EMIT statusChanged();
}

void ClashClient::reportError(const QString &message)
{
    m_message = message;
    Q_EMIT statusChanged();
    Q_EMIT errorOccurred(message);
}

void ClashClient::asyncCall(const QString &method, const QVariantList &args, bool refreshProfilesAfter, bool refreshProxiesAfter)
{
    setBusy(true);
    auto *watcher = new QDBusPendingCallWatcher(m_iface->asyncCallWithArgumentList(method, args), this);
    connect(watcher, &QDBusPendingCallWatcher::finished, this, [this, refreshProfilesAfter, refreshProxiesAfter](QDBusPendingCallWatcher *call) {
        call->deleteLater();
        setBusy(false);
        QDBusPendingReply<> reply = *call;
        if (!reply.isValid()) {
            reportError(reply.error().message());
        }
        refreshStatus();
        if (refreshProfilesAfter) {
            refreshProfiles();
        }
        if (refreshProxiesAfter) {
            refreshProxies();
        }
    });
}
