// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 clash-kde contributors

#include "clashcalls.h"
#include "regionlabel.h"

#include <KLocalizedString>
#include <QDBusConnection>
#include <QDBusInterface>
#include <QDBusPendingCallWatcher>
#include <QDBusPendingReply>
#include <QClipboard>
#include <QFileInfo>
#include <QGuiApplication>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QProcess>
#include <QStandardPaths>

namespace {
QString configValue(const QString &file, const QString &group, const QString &key)
{
    QProcess process;
    process.start(QStringLiteral("kreadconfig6"), {QStringLiteral("--file"), file, QStringLiteral("--group"), group, QStringLiteral("--key"), key});
    if (!process.waitForFinished(1500) || process.exitCode() != 0) {
        return {};
    }
    return QString::fromUtf8(process.readAllStandardOutput()).trimmed();
}

QString shellName()
{
    const QString terminal = configValue(QStringLiteral("kdeglobals"), QStringLiteral("General"), QStringLiteral("TerminalApplication"));
    const QString terminalBase = QFileInfo(terminal).fileName();
    const bool konsole = terminal.isEmpty() || terminalBase == QStringLiteral("konsole") || terminalBase.startsWith(QStringLiteral("konsole"));
    if (konsole) {
        const QString profile = configValue(QStringLiteral("konsolerc"), QStringLiteral("Desktop Entry"), QStringLiteral("DefaultProfile"));
        if (!profile.isEmpty()) {
            const QString path = QStandardPaths::locate(QStandardPaths::GenericDataLocation, QStringLiteral("konsole/") + profile);
            const QString command = path.isEmpty() ? QString() : configValue(path, QStringLiteral("General"), QStringLiteral("Command"));
            const QString base = QFileInfo(command.section(QLatin1Char(' '), 0, 0)).fileName();
            if (base == QStringLiteral("fish") || base == QStringLiteral("nu") || base == QStringLiteral("nushell") || base == QStringLiteral("csh") || base == QStringLiteral("tcsh")
                || base == QStringLiteral("bash") || base == QStringLiteral("zsh")) {
                return base;
            }
        }
    }
    return QFileInfo(qEnvironmentVariable("SHELL")).fileName();
}

QString proxyCommand(const QString &shell, bool wayland, int port)
{
    const QString http = QStringLiteral("http://127.0.0.1:%1").arg(port);
    // Wayland terminals usually resolve names locally, so the proxy must receive the hostname.
    // X11 libraries have long accepted socks5 without the hostname flag.
    const QString socks = QStringLiteral("%1://127.0.0.1:%2").arg(wayland ? QStringLiteral("socks5h") : QStringLiteral("socks5"), QString::number(port));
    const QString bypass = QStringLiteral("localhost,127.0.0.1,::1");
    if (shell == QStringLiteral("fish")) {
        return QStringLiteral("set -gx http_proxy %1; set -gx https_proxy %2; set -gx all_proxy %3; set -gx no_proxy %4").arg(http, http, socks, bypass);
    }
    if (shell == QStringLiteral("nu") || shell == QStringLiteral("nushell")) {
        return QStringLiteral("$env.http_proxy = \"%1\"; $env.https_proxy = \"%2\"; $env.all_proxy = \"%3\"; $env.no_proxy = \"%4\"").arg(http, http, socks, bypass);
    }
    if (shell == QStringLiteral("csh") || shell == QStringLiteral("tcsh")) {
        return QStringLiteral("setenv http_proxy %1; setenv https_proxy %2; setenv all_proxy %3; setenv no_proxy %4").arg(http, http, socks, bypass);
    }
    if (wayland) {
        return QStringLiteral("export http_proxy=%1 https_proxy=%2 all_proxy=%3 no_proxy=%4").arg(http, http, socks, bypass);
    }
    return QStringLiteral("export http_proxy=%1 https_proxy=%2 HTTP_PROXY=%3 HTTPS_PROXY=%4 all_proxy=%5 ALL_PROXY=%6 no_proxy=%7 NO_PROXY=%8")
        .arg(http, http, http, http, socks, socks, bypass, bypass);
}
}

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
    return formatExitRegion(m_region, m_blurIp);
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

bool ClashCalls::autoTestDelay() const
{
    return m_autoTestDelay;
}

bool ClashCalls::blurIp() const
{
    return m_blurIp;
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

bool ClashCalls::groupDelayTesting(const QString &group) const
{
    return m_testingGroups.contains(group);
}

void ClashCalls::testGroupDelay(const QString &group)
{
    if (group.isEmpty() || m_testingGroups.contains(group)) {
        return;
    }
    m_testingGroups.insert(group);
    auto *watcher = new QDBusPendingCallWatcher(m_iface->asyncCall(QStringLiteral("TestGroupDelay"), group), this);
    connect(watcher, &QDBusPendingCallWatcher::finished, this, [this, group](QDBusPendingCallWatcher *call) {
        call->deleteLater();
        m_testingGroups.remove(group);
        const QDBusPendingReply<> reply = *call;
        if (reply.isError()) {
            fail(reply.error().message());
            return;
        }
        refresh();
        refreshLists();
    });
}

bool ClashCalls::copyTerminalProxyCommand()
{
    const int port = m_listenAddress.section(QLatin1Char(':'), -1).toInt();
    if (port <= 0 || QGuiApplication::clipboard() == nullptr) {
        return false;
    }
    const QByteArray sessionType = qgetenv("XDG_SESSION_TYPE");
    const bool wayland = sessionType == "wayland" || (sessionType.isEmpty() && !qEnvironmentVariableIsEmpty("WAYLAND_DISPLAY"));
    QGuiApplication::clipboard()->setText(proxyCommand(shellName(), wayland, port));
    return true;
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
    m_autoTestDelay = object.value(QStringLiteral("auto_test_delay")).toBool();
    m_blurIp = object.value(QStringLiteral("blur_ip")).toBool();
    Q_EMIT changed();
}

void ClashCalls::fail(const QString &message)
{
    m_error = message.isEmpty() ? i18n("The service is not available.") : message;
    Q_EMIT changed();
}
