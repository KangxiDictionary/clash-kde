// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 clash-kde contributors

#include "clashcalls.h"

#include <KLocalizedString>
#include <QDBusConnection>
#include <QDBusInterface>
#include <QDBusPendingCallWatcher>
#include <QDBusPendingReply>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>

ClashCalls::ClashCalls(QObject *parent)
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
                SLOT(refresh()));
    bus.connect(QStringLiteral("org.kde.clashkde"),
                QStringLiteral("/org/kde/clashkde"),
                QStringLiteral("org.kde.clashkde.Manager"),
                QStringLiteral("ProfilesChanged"),
                this,
                SLOT(refreshLists()));
    bus.connect(QStringLiteral("org.kde.clashkde"),
                QStringLiteral("/org/kde/clashkde"),
                QStringLiteral("org.kde.clashkde.Manager"),
                QStringLiteral("DelayUpdated"),
                this,
                SLOT(onDelayUpdated(QString,QString,qlonglong)));
}

namespace {
QVariantList jsonArray(const QString &text)
{
    QVariantList items;
    const auto values = QJsonDocument::fromJson(text.toUtf8()).array();
    for (const auto &value : values) {
        items.append(value.toObject().toVariantMap());
    }
    return items;
}
}

bool ClashCalls::ready() const
{
    return m_ready;
}

QString ClashCalls::error() const
{
    return m_error;
}

bool ClashCalls::coreDesired() const
{
    return m_coreDesired;
}

bool ClashCalls::coreRunning() const
{
    return m_coreRunning;
}

bool ClashCalls::systemProxyEnabled() const
{
    return m_systemProxyEnabled;
}

bool ClashCalls::systemProxyApplied() const
{
    return m_systemProxyApplied;
}

bool ClashCalls::tunEnabled() const
{
    return m_tunEnabled;
}

QString ClashCalls::mode() const
{
    return m_mode;
}

QString ClashCalls::message() const
{
    return m_message;
}

QString ClashCalls::region() const
{
    return m_region;
}

QString ClashCalls::listenAddress() const
{
    return m_listenAddress;
}

double ClashCalls::uploadBps() const
{
    return m_uploadBps;
}

double ClashCalls::downloadBps() const
{
    return m_downloadBps;
}

QString ClashCalls::activeProfile() const
{
    return m_activeProfile;
}

QVariantList ClashCalls::profiles() const
{
    return m_profiles;
}

QVariantList ClashCalls::groups() const
{
    return m_groups;
}

void ClashCalls::refresh()
{
    auto *watcher = new QDBusPendingCallWatcher(m_iface->asyncCall(QStringLiteral("GetStatus")), this);
    connect(watcher, &QDBusPendingCallWatcher::finished, this, [this](QDBusPendingCallWatcher *call) {
        call->deleteLater();
        const QDBusPendingReply<QString> reply = *call;
        if (reply.isError()) {
            fail(reply.error().message());
            return;
        }
        applyStatus(reply.value());
    });
}

void ClashCalls::setCoreDesired(bool desired)
{
    call(desired ? QStringLiteral("Start") : QStringLiteral("Stop"), {});
}

void ClashCalls::setSystemProxyEnabled(bool enabled)
{
    call(QStringLiteral("SetSystemProxy"), {enabled});
}

void ClashCalls::setMode(const QString &mode)
{
    call(QStringLiteral("SetMode"), {mode});
}

void ClashCalls::setActiveProfile(const QString &id)
{
    call(QStringLiteral("SetActiveProfile"), {id});
}

void ClashCalls::selectProxy(const QString &group, const QString &name)
{
    call(QStringLiteral("SelectProxy"), {group, name});
}

void ClashCalls::refreshLists()
{
    auto *profiles = new QDBusPendingCallWatcher(m_iface->asyncCall(QStringLiteral("GetProfiles")), this);
    connect(profiles, &QDBusPendingCallWatcher::finished, this, [this](QDBusPendingCallWatcher *call) {
        call->deleteLater();
        const QDBusPendingReply<QString> reply = *call;
        if (reply.isError()) {
            return;
        }
        const QVariantList next = jsonArray(reply.value());
        if (next != m_profiles) {
            m_profiles = next;
            Q_EMIT changed();
        }
    });

    auto *proxies = new QDBusPendingCallWatcher(m_iface->asyncCall(QStringLiteral("GetProxies")), this);
    connect(proxies, &QDBusPendingCallWatcher::finished, this, [this](QDBusPendingCallWatcher *call) {
        call->deleteLater();
        const QDBusPendingReply<QString> reply = *call;
        if (reply.isError()) {
            return;
        }
        const QVariantList next = jsonArray(reply.value());
        if (next != m_groups) {
            m_groups = next;
            Q_EMIT changed();
        }
    });
}

void ClashCalls::onDelayUpdated(const QString &group, const QString &name, qlonglong delayMs)
{
    for (int i = 0; i < m_groups.size(); ++i) {
        QVariantMap map = m_groups.at(i).toMap();
        if (map.value(QStringLiteral("name")).toString() != group) {
            continue;
        }
        QVariantList nodes = map.value(QStringLiteral("nodes")).toList();
        for (int j = 0; j < nodes.size(); ++j) {
            QVariantMap node = nodes.at(j).toMap();
            if (node.value(QStringLiteral("name")).toString() != name) {
                continue;
            }
            node.insert(QStringLiteral("delay_ms"), delayMs);
            nodes[j] = node;
            map.insert(QStringLiteral("nodes"), nodes);
            m_groups[i] = map;
            Q_EMIT changed();
            return;
        }
    }
}

void ClashCalls::call(const QString &member, const QVariantList &arguments)
{
    auto *watcher = new QDBusPendingCallWatcher(m_iface->asyncCallWithArgumentList(member, arguments), this);
    connect(watcher, &QDBusPendingCallWatcher::finished, this, [this](QDBusPendingCallWatcher *call) {
        call->deleteLater();
        const QDBusPendingReply<> reply = *call;
        if (reply.isError()) {
            fail(reply.error().message());
            return;
        }
        refresh();
        refreshLists();
    });
}

void ClashCalls::applyStatus(const QString &json)
{
    const QJsonObject object = QJsonDocument::fromJson(json.toUtf8()).object();
    if (object.isEmpty()) {
        fail(i18n("The status could not be read."));
        return;
    }
    m_ready = true;
    m_error.clear();
    m_coreDesired = object.value(QStringLiteral("core_desired")).toBool();
    m_coreRunning = object.value(QStringLiteral("core_running")).toBool();
    m_systemProxyEnabled = object.value(QStringLiteral("system_proxy_enabled")).toBool();
    m_systemProxyApplied = object.value(QStringLiteral("system_proxy_applied")).toBool();
    m_tunEnabled = object.value(QStringLiteral("tun_enabled")).toBool();
    m_mode = object.value(QStringLiteral("mode")).toString(QStringLiteral("rule"));
    m_message = object.value(QStringLiteral("message")).toString();
    m_region = object.value(QStringLiteral("exit_region")).toString();
    m_listenAddress = QStringLiteral("127.0.0.1:%1").arg(object.value(QStringLiteral("mixed_port")).toInt(7890));
    m_uploadBps = object.value(QStringLiteral("upload_bps")).toDouble();
    m_downloadBps = object.value(QStringLiteral("download_bps")).toDouble();
    m_activeProfile = object.value(QStringLiteral("active_profile")).toString();
    Q_EMIT changed();
}

void ClashCalls::fail(const QString &message)
{
    m_error = message.isEmpty() ? i18n("The service is not available.") : message;
    Q_EMIT changed();
}
