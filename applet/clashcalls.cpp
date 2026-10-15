// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 clash-kde contributors

#include "clashcalls.h"

#include <KLocalizedString>
#include <QDBusConnection>
#include <QDBusInterface>
#include <QDBusPendingCallWatcher>
#include <QDBusPendingReply>
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
    QDBusConnection::sessionBus().connect(QStringLiteral("org.kde.clashkde"),
                                          QStringLiteral("/org/kde/clashkde"),
                                          QStringLiteral("org.kde.clashkde.Manager"),
                                          QStringLiteral("StatusChanged"),
                                          this,
                                          SLOT(refresh()));
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
    m_mode = object.value(QStringLiteral("mode")).toString(QStringLiteral("rule"));
    m_message = object.value(QStringLiteral("message")).toString();
    m_region = object.value(QStringLiteral("exit_region")).toString();
    m_listenAddress = QStringLiteral("127.0.0.1:%1").arg(object.value(QStringLiteral("mixed_port")).toInt(7890));
    m_uploadBps = object.value(QStringLiteral("upload_bps")).toDouble();
    m_downloadBps = object.value(QStringLiteral("download_bps")).toDouble();
    Q_EMIT changed();
}

void ClashCalls::fail(const QString &message)
{
    m_error = message.isEmpty() ? i18n("The service is not available.") : message;
    Q_EMIT changed();
}
