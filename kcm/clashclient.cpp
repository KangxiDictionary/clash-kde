// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 clash-kde contributors

#include "clashclient.h"

#include <QDBusConnection>
#include <QDBusInterface>
#include <QDBusPendingCallWatcher>
#include <QDBusPendingReply>
#include <QDBusReply>
#include <QDir>
#include <QFile>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QProcess>
#include <QStandardPaths>
#include <QTemporaryFile>
#include <QTimer>
#include <QUrl>
#include <unistd.h>

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
    , m_admin(new QDBusInterface(QStringLiteral("org.kde.clashkde.Admin"),
                                 QStringLiteral("/org/kde/clashkde"),
                                 QStringLiteral("org.kde.clashkde.Admin"),
                                 QDBusConnection::systemBus(),
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
    bus.connect(QStringLiteral("org.kde.clashkde"),
                QStringLiteral("/org/kde/clashkde"),
                QStringLiteral("org.kde.clashkde.Manager"),
                QStringLiteral("DelayUpdated"),
                this,
                SLOT(onDelayUpdated(QString,QString,qlonglong)));

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
bool ClashClient::tunEnabled() const { return m_tunEnabled; }
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
bool ClashClient::delayTesting() const { return m_delayTesting; }
QVariantList ClashClient::profiles() const { return m_profiles; }
QVariantList ClashClient::groups() const { return m_groups; }
QVariantList ClashClient::accounts() const { return m_accounts; }
QString ClashClient::accountsError() const { return m_accountsError; }
qulonglong ClashClient::ownUid() const { return ::getuid(); }
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

void ClashClient::setTunEnabled(bool enabled)
{
    asyncCall(QStringLiteral("SetTun"), {enabled}, false, false);
}

void ClashClient::reloadAccounts()
{
    setBusy(true);
    auto *watcher = new QDBusPendingCallWatcher(m_admin->asyncCall(QStringLiteral("List")), this);
    connect(watcher, &QDBusPendingCallWatcher::finished, this, [this](QDBusPendingCallWatcher *call) {
        call->deleteLater();
        setBusy(false);
        const QDBusPendingReply<QString> reply = *call;
        if (!reply.isValid()) {
            m_accountsError = reply.error().message();
            m_accounts.clear();
        } else {
            m_accountsError.clear();
            m_accounts = jsonArray(reply.value());
            overlayCurrentAccount();
        }
        Q_EMIT accountsChanged();
    });
}

void ClashClient::startAccount(qulonglong uid)
{
    if (uid == ownUid()) {
        setCoreDesired(true);
        return;
    }
    accountCall(QStringLiteral("Start"), uid);
}

void ClashClient::stopAccount(qulonglong uid)
{
    if (uid == ownUid()) {
        setCoreDesired(false);
        return;
    }
    accountCall(QStringLiteral("Stop"), uid);
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
    asyncCall(QStringLiteral("SelectProxy"), {group, name}, false, true, false);
}

void ClashClient::testGroupDelay(const QString &group)
{
    setDelayTesting(true);
    auto *watcher = new QDBusPendingCallWatcher(m_iface->asyncCall(QStringLiteral("TestGroupDelay"), group), this);
    connect(watcher, &QDBusPendingCallWatcher::finished, this, [this](QDBusPendingCallWatcher *call) {
        call->deleteLater();
        setDelayTesting(false);
        const QDBusPendingReply<> reply = *call;
        if (!reply.isValid()) {
            reportError(reply.error().message());
        }
    });
}

QString ClashClient::profileBody(const QString &id)
{
    const QDBusReply<QString> reply = m_iface->call(QStringLiteral("GetProfileBody"), id);
    if (!reply.isValid()) {
        reportError(reply.error().message());
        return {};
    }
    return reply.value();
}

QString ClashClient::checkProfileText(const QString &body)
{
    const QDBusReply<QString> reply = m_iface->call(QStringLiteral("CheckProfileText"), body);
    if (!reply.isValid()) {
        return reply.error().message();
    }
    return reply.value();
}

QString ClashClient::coreLog() const
{
    QFile file(QDir::homePath() + QStringLiteral("/.local/share/clash-kde/runtime/mihomo.log"));
    if (!file.open(QIODevice::ReadOnly)) {
        return {};
    }
    constexpr qint64 keep = 48 * 1024;
    if (file.size() > keep) {
        file.seek(file.size() - keep);
        file.readLine();
    }
    return QString::fromUtf8(file.readAll());
}

bool ClashClient::openBodyInEditor(const QString &body)
{
    QString editor = QStandardPaths::findExecutable(QStringLiteral("kwrite"));
    if (editor.isEmpty()) {
        editor = QStandardPaths::findExecutable(QStringLiteral("kate"));
    }
    if (editor.isEmpty()) {
        return false;
    }

    auto *file = new QTemporaryFile(QDir::tempPath() + QStringLiteral("/clash-kde-XXXXXX.yaml"));
    file->setAutoRemove(true);
    if (!file->open() || file->write(body.toUtf8()) < 0) {
        delete file;
        return false;
    }
    file->flush();
    const QString path = file->fileName();
    file->close();

    auto *process = new QProcess(this);
    file->setParent(process);

    connect(process, &QProcess::finished, this, [this, process, path]() {
        QFile saved(path);
        if (saved.open(QIODevice::ReadOnly)) {
            Q_EMIT editorTextReady(QString::fromUtf8(saved.readAll()));
        }
        process->deleteLater();
    });
    connect(process, &QProcess::errorOccurred, process, [process](QProcess::ProcessError error) {
        if (error == QProcess::FailedToStart) {
            process->deleteLater();
        }
    });
    process->start(editor, {path});
    return true;
}

void ClashClient::setProfileBody(const QString &id, const QString &body)
{
    asyncCall(QStringLiteral("SetProfileBody"), {id, body}, false, true);
}

void ClashClient::setProfileMeta(const QString &id, const QString &name, const QString &url)
{
    asyncCall(QStringLiteral("SetProfileMeta"), {id, name, url}, true, false);
}

void ClashClient::importProfile(const QString &name, const QString &fileUrl)
{
    const QUrl url(fileUrl);
    const QString path = url.isLocalFile() ? url.toLocalFile() : fileUrl;
    asyncCall(QStringLiteral("ImportProfile"), {name, path}, true, false);
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

void ClashClient::revertDraft()
{
    m_draftDirty = false;
    m_draftMixedPort = m_mixedPort;
    m_draftControllerPort = m_controllerPort;
    m_draftMihomoPath = m_mihomoPath;
    Q_EMIT draftChanged();
}

void ClashClient::applyDefaultsToDraft()
{
    m_draftDirty = true;
    m_draftMixedPort = 7890;
    m_draftControllerPort = 9090;
    m_draftMihomoPath = QStringLiteral("mihomo");
    Q_EMIT draftChanged();
}

bool ClashClient::draftIsDefaults() const
{
    return m_draftMixedPort == 7890 && m_draftControllerPort == 9090 && m_draftMihomoPath == QStringLiteral("mihomo");
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

void ClashClient::setDelayTesting(bool testing)
{
    if (m_delayTesting == testing) {
        return;
    }
    m_delayTesting = testing;
    Q_EMIT delayTestingChanged();
}

void ClashClient::onDelayUpdated(const QString &group, const QString &name, qlonglong delayMs)
{
    Q_EMIT delayUpdated(group, name, delayMs);
}

void ClashClient::applyStatus(const QVariantMap &status)
{
    m_coreDesired = status.value(QStringLiteral("core_desired")).toBool();
    m_coreRunning = status.value(QStringLiteral("core_running")).toBool();
    m_systemProxyEnabled = status.value(QStringLiteral("system_proxy_enabled")).toBool();
    m_systemProxyApplied = status.value(QStringLiteral("system_proxy_applied")).toBool();
    m_tunEnabled = status.value(QStringLiteral("tun_enabled")).toBool();
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
    const QString dbusPrefix = QStringLiteral("org.freedesktop.DBus.Error.Failed: ");
    if (m_message.startsWith(dbusPrefix)) {
        m_message = m_message.mid(dbusPrefix.size());
    }
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
    overlayCurrentAccount();
}

void ClashClient::overlayCurrentAccount()
{
    const uint uid = static_cast<uint>(ownUid());
    bool changed = false;
    for (int i = 0; i < m_accounts.size(); ++i) {
        QVariantMap map = m_accounts.at(i).toMap();
        if (map.value(QStringLiteral("uid")).toUInt() != uid) {
            continue;
        }
        const bool same = map.value(QStringLiteral("core_running")).toBool() == m_coreRunning
            && map.value(QStringLiteral("tun_enabled")).toBool() == m_tunEnabled
            && map.value(QStringLiteral("system_proxy_enabled")).toBool() == m_systemProxyEnabled
            && map.value(QStringLiteral("mode")).toString() == m_mode
            && map.value(QStringLiteral("error")).toString().isEmpty();
        if (same) {
            continue;
        }
        map.insert(QStringLiteral("core_running"), m_coreRunning);
        map.insert(QStringLiteral("tun_enabled"), m_tunEnabled);
        map.insert(QStringLiteral("system_proxy_enabled"), m_systemProxyEnabled);
        map.insert(QStringLiteral("mode"), m_mode);
        map.insert(QStringLiteral("error"), QString());
        m_accounts[i] = map;
        changed = true;
    }
    if (changed) {
        Q_EMIT accountsChanged();
    }
}

void ClashClient::reportError(const QString &message)
{
    QString text = message;
    const QString prefix = QStringLiteral("org.freedesktop.DBus.Error.Failed: ");
    if (text.startsWith(prefix)) {
        text = text.mid(prefix.size());
    }
    m_message = text;
    Q_EMIT statusChanged();
    Q_EMIT errorOccurred(message);
}

void ClashClient::asyncCall(const QString &method, const QVariantList &args, bool refreshProfilesAfter, bool refreshProxiesAfter, bool markBusy)
{
    if (markBusy) {
        setBusy(true);
    }
    auto *watcher = new QDBusPendingCallWatcher(m_iface->asyncCallWithArgumentList(method, args), this);
    connect(watcher, &QDBusPendingCallWatcher::finished, this, [this, refreshProfilesAfter, refreshProxiesAfter, markBusy](QDBusPendingCallWatcher *call) {
        call->deleteLater();
        if (markBusy) {
            setBusy(false);
        }
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

void ClashClient::accountCall(const QString &method, qulonglong uid)
{
    setBusy(true);
    auto *watcher = new QDBusPendingCallWatcher(m_admin->asyncCall(method, static_cast<uint>(uid)), this);
    connect(watcher, &QDBusPendingCallWatcher::finished, this, [this](QDBusPendingCallWatcher *call) {
        call->deleteLater();
        setBusy(false);
        const QDBusPendingReply<> reply = *call;
        if (!reply.isValid()) {
            m_accountsError = reply.error().message();
            Q_EMIT accountsChanged();
            return;
        }
        reloadAccounts();
    });
}
