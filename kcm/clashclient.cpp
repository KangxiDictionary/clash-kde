// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 clash-kde contributors

#include "clashclient.h"
#include "regionlabel.h"

#include <QDBusConnection>
#include <QDBusInterface>
#include <QDBusPendingCallWatcher>
#include <QDBusPendingReply>
#include <QDBusReply>
#include <QDateTime>
#include <QDir>
#include <QFile>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QProcess>
#include <QStandardPaths>
#include <QTemporaryFile>
#include <QTimer>
#include <QClipboard>
#include <QDesktopServices>
#include <QFileInfo>
#include <QGuiApplication>
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

bool tunDeviceAcceptable(const QString &name)
{
    if (name.isEmpty() || name.size() > 15) {
        return name.isEmpty();
    }
    for (const QChar &ch : name) {
        const char16_t code = ch.unicode();
        const bool ok = (code >= u'0' && code <= u'9') || (code >= u'A' && code <= u'Z') || (code >= u'a' && code <= u'z') || code == u'_' || code == u'-';
        if (!ok) {
            return false;
        }
    }
    return true;
}

QString canonicalOverride(const QString &text, const QString &fallback)
{
    const QString trimmed = text.trimmed();
    return trimmed.isEmpty() ? fallback : trimmed;
}

bool ufwEnabled()
{
    QFile file(QStringLiteral("/etc/ufw/ufw.conf"));
    if (!file.open(QIODevice::ReadOnly | QIODevice::Text)) {
        return false;
    }
    while (!file.atEnd()) {
        const QString line = QString::fromUtf8(file.readLine()).trimmed();
        if (line == QStringLiteral("ENABLED=yes")) {
            return true;
        }
    }
    return false;
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
                SLOT(noteProxiesChanged()));
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
    QTimer::singleShot(0, this, &ClashClient::reload);
}

bool ClashClient::coreDesired() const { return m_coreDesired; }
bool ClashClient::coreRunning() const { return m_coreRunning; }
bool ClashClient::systemProxyEnabled() const { return m_systemProxyEnabled; }
bool ClashClient::systemProxyApplied() const { return m_systemProxyApplied; }
bool ClashClient::tunEnabled() const { return m_tunEnabled; }
bool ClashClient::tunMachine() const { return m_tunMachine; }
QString ClashClient::kdeProxy() const { return m_kdeProxy; }
QString ClashClient::gnomeProxy() const { return m_gnomeProxy; }
bool ClashClient::accountsUnlocked() const { return m_accountsUnlocked; }
bool ClashClient::firewallActive() const { return m_firewallActive; }
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
QString ClashClient::exitRegion() const { return formatExitRegion(m_exitRegion, m_blurIp); }
QString ClashClient::exitRaw() const { return m_exitRegion; }
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
bool ClashClient::draftCoreDesired() const { return m_draftCoreDesired; }
bool ClashClient::draftSystemProxyEnabled() const { return m_draftSystemProxyEnabled; }
bool ClashClient::draftTunEnabled() const { return m_draftTunEnabled; }
QString ClashClient::draftMode() const { return m_draftMode; }
bool ClashClient::settingsDirty() const { return m_draftDirty; }

void ClashClient::setDraftMixedPort(int port)
{
    if (m_draftMixedPort == port) {
        return;
    }
    m_draftMixedPort = port;
    m_draftDirty = true;
    m_preferReset = false;
    Q_EMIT draftChanged();
}

void ClashClient::setDraftControllerPort(int port)
{
    if (m_draftControllerPort == port) {
        return;
    }
    m_draftControllerPort = port;
    m_draftDirty = true;
    m_preferReset = false;
    Q_EMIT draftChanged();
}

void ClashClient::setDraftMihomoPath(const QString &path)
{
    if (m_draftMihomoPath == path) {
        return;
    }
    m_draftMihomoPath = path;
    m_draftDirty = true;
    m_preferReset = false;
    Q_EMIT draftChanged();
}

void ClashClient::setDraftCoreDesired(bool desired)
{
    if (m_draftCoreDesired == desired) {
        return;
    }
    m_draftCoreDesired = desired;
    m_draftDirty = true;
    m_preferReset = false;
    Q_EMIT draftChanged();
}

void ClashClient::setDraftSystemProxyEnabled(bool enabled)
{
    if (m_draftSystemProxyEnabled == enabled) {
        return;
    }
    m_draftSystemProxyEnabled = enabled;
    m_draftDirty = true;
    m_preferReset = false;
    Q_EMIT draftChanged();
}

void ClashClient::setDraftTunEnabled(bool enabled)
{
    if (m_draftTunEnabled == enabled) {
        return;
    }
    m_draftTunEnabled = enabled;
    m_draftDirty = true;
    m_preferReset = false;
    Q_EMIT draftChanged();
}

void ClashClient::setDraftMode(const QString &mode)
{
    if (m_draftMode == mode) {
        return;
    }
    m_draftMode = mode;
    m_draftDirty = true;
    m_preferReset = false;
    Q_EMIT draftChanged();
}

QString ClashClient::draftActiveProfile() const { return m_draftActiveProfile; }
QVariantMap ClashClient::draftSelections() const { return m_draftSelections; }
bool ClashClient::draftSortGlobal() const { return m_draftSortGlobal; }
bool ClashClient::draftAutoTestDelay() const { return m_draftAutoTestDelay; }
bool ClashClient::draftBlurIp() const { return m_draftBlurIp; }
bool ClashClient::sortGlobal() const { return m_sortGlobal; }
QString ClashClient::sortMode() const { return m_sortMode; }
bool ClashClient::showUnreachable() const { return m_showUnreachable; }
QVariantMap ClashClient::sortPrefs() const { return m_sortPrefs; }
bool ClashClient::autoTestDelay() const { return m_autoTestDelay; }
bool ClashClient::blurIp() const { return m_blurIp; }
bool ClashClient::allowLan() const { return m_allowLan; }
QString ClashClient::lanUsername() const { return m_lanUsername; }
QString ClashClient::lanPassword() const { return m_lanPassword; }
bool ClashClient::lanAllowed() const { return m_lanAllowed; }
bool ClashClient::draftAllowLan() const { return m_draftAllowLan; }
QString ClashClient::draftLanUsername() const { return m_draftLanUsername; }
QString ClashClient::draftLanPassword() const { return m_draftLanPassword; }
bool ClashClient::machineLanAllowed() const { return m_machineLanAllowed; }
QVariantMap ClashClient::lanAccounts() const { return m_lanAccounts; }

void ClashClient::setDraftAllowLan(bool enabled)
{
    if (m_draftAllowLan == enabled) {
        return;
    }
    m_draftAllowLan = enabled;
    m_preferReset = false;
    m_draftDirty = !draftMatchesLive();
    Q_EMIT draftChanged();
}

void ClashClient::setDraftLanUsername(const QString &username)
{
    if (m_draftLanUsername == username) {
        return;
    }
    m_draftLanUsername = username;
    m_preferReset = false;
    m_draftDirty = !draftMatchesLive();
    Q_EMIT draftChanged();
}

void ClashClient::setDraftLanPassword(const QString &password)
{
    if (m_draftLanPassword == password) {
        return;
    }
    m_draftLanPassword = password;
    m_preferReset = false;
    m_draftDirty = !draftMatchesLive();
    Q_EMIT draftChanged();
}
QString ClashClient::hint() const { return m_hint; }

void ClashClient::showHint(const QString &text)
{
    if (m_hint == text) {
        return;
    }
    m_hint = text;
    Q_EMIT hintChanged();
}

void ClashClient::clearHint()
{
    showHint(QString());
}

void ClashClient::setDraftActiveProfile(const QString &id)
{
    if (m_draftActiveProfile == id) {
        return;
    }
    m_draftActiveProfile = id;
    m_draftDirty = true;
    m_preferReset = false;
    Q_EMIT draftChanged();
}

void ClashClient::setDraftSelections(const QVariantMap &selections)
{
    if (m_draftSelections == selections) {
        return;
    }
    m_draftSelections = selections;
    m_draftDirty = true;
    m_preferReset = false;
    Q_EMIT draftChanged();
}

void ClashClient::setDraftSortGlobal(bool global)
{
    if (m_draftSortGlobal == global) {
        return;
    }
    m_draftSortGlobal = global;
    m_draftDirty = true;
    m_preferReset = false;
    Q_EMIT draftChanged();
}

void ClashClient::setDraftAutoTestDelay(bool enabled)
{
    if (m_draftAutoTestDelay == enabled) {
        return;
    }
    m_draftAutoTestDelay = enabled;
    m_draftDirty = true;
    m_preferReset = false;
    Q_EMIT draftChanged();
}

void ClashClient::setDraftBlurIp(bool enabled)
{
    if (m_draftBlurIp == enabled) {
        return;
    }
    m_draftBlurIp = enabled;
    m_draftDirty = true;
    m_preferReset = false;
    Q_EMIT draftChanged();
}

QString ClashClient::draftTunDevice() const { return m_draftTunDevice; }
QString ClashClient::draftOverridePath() const { return m_draftOverridePath; }

void ClashClient::setDraftTunDevice(const QString &name)
{
    if (m_draftTunDevice == name) {
        return;
    }
    m_draftTunDevice = name;
    m_draftDirty = true;
    m_preferReset = false;
    Q_EMIT draftChanged();
}

void ClashClient::setDraftOverridePath(const QString &path)
{
    if (m_draftOverridePath == path) {
        return;
    }
    m_draftOverridePath = path;
    m_draftDirty = true;
    m_preferReset = false;
    Q_EMIT draftChanged();
}

void ClashClient::setViewPref(const QString &profile, const QString &group, const QString &mode, bool showUnreachable, bool global)
{
    const QDBusReply<void> reply = m_iface->call(QStringLiteral("SetViewPref"), profile, group, mode, showUnreachable, global);
    if (!reply.isValid()) {
        reportError(reply.error().message());
        return;
    }
    refreshStatus();
}

QVariantList ClashClient::profileGroups(const QString &id)
{
    if (id.isEmpty() || m_iface == nullptr) {
        return {};
    }
    const QDBusReply<QString> reply = m_iface->call(QStringLiteral("GetProfileGroups"), id);
    if (!reply.isValid()) {
        return {};
    }
    return jsonArray(reply.value());
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

void ClashClient::setMachineTun(bool enabled)
{
    asyncCall(QStringLiteral("SetMachineTun"), {enabled}, false, false);
}

void ClashClient::lockAccounts()
{
    m_accountsUnlocked = false;
    m_accountsError.clear();
    Q_EMIT accountsChanged();
}

QString ClashClient::formatExit(const QString &raw) const
{
    return formatExitRegion(raw, m_blurIp);
}

QString ClashClient::presentExit(const QString &raw, bool blur) const
{
    return formatExitRegion(raw, blur);
}

QString ClashClient::presentPlace(const QString &raw) const
{
    return formatExitPlace(raw);
}

QString ClashClient::presentAddress(const QString &raw, bool blur) const
{
    return formatExitAddress(raw, blur);
}

QString ClashClient::addressOf(const QString &raw) const
{
    return exitAddress(raw);
}

void ClashClient::copyText(const QString &text) const
{
    const QString value = text.trimmed();
    if (value.isEmpty() || QGuiApplication::clipboard() == nullptr) {
        return;
    }
    QGuiApplication::clipboard()->setText(value);
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
            m_accountsUnlocked = true;
            const QDBusReply<QString> policy = m_admin->call(QStringLiteral("LanPolicy"));
            if (policy.isValid()) {
                const QJsonObject object = QJsonDocument::fromJson(policy.value().toUtf8()).object();
                m_machineLanAllowed = object.value(QStringLiteral("global")).toBool(true);
                QVariantMap accounts;
                const QJsonArray rows = object.value(QStringLiteral("accounts")).toArray();
                for (const QJsonValue &row : rows) {
                    const QJsonObject item = row.toObject();
                    accounts.insert(QString::number(item.value(QStringLiteral("uid")).toVariant().toULongLong()), item.value(QStringLiteral("allow")).toBool(true));
                }
                m_lanAccounts = accounts;
            }
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

QString ClashClient::prepareOverride(const QString &path)
{
    const QDBusReply<QString> reply = m_iface->call(QStringLiteral("EnsureOverride"), path);
    if (!reply.isValid()) {
        reportError(reply.error().message());
        return {};
    }
    return reply.value();
}

void ClashClient::openOverrideFile(const QString &path)
{
    const QString ready = prepareOverride(path);
    if (ready.isEmpty()) {
        return;
    }
    QDesktopServices::openUrl(QUrl::fromLocalFile(ready));
}

void ClashClient::openOverrideFolder(const QString &path)
{
    const QString ready = prepareOverride(path);
    if (ready.isEmpty()) {
        return;
    }
    QDesktopServices::openUrl(QUrl::fromLocalFile(QFileInfo(ready).absolutePath()));
}

void ClashClient::refreshExitRegion()
{
    asyncCall(QStringLiteral("RefreshExitRegion"), {}, false, false);
}

QVariantMap ClashClient::routeDetail(const QString &name)
{
    const QDBusReply<QString> reply = m_iface->call(QStringLiteral("RouteDetail"), m_draftActiveProfile, name);
    if (!reply.isValid()) {
        reportError(reply.error().message());
        return {};
    }
    const QJsonDocument document = QJsonDocument::fromJson(reply.value().toUtf8());
    return document.object().toVariantMap();
}

bool ClashClient::addRoute(const QString &group, const QString &name, const QString &kind, const QString &server, int port, const QString &secret)
{
    if (port < 1 || port > 65535) {
        reportError(QStringLiteral("端口无效"));
        return false;
    }
    const QDBusReply<void> reply = m_iface->call(QStringLiteral("AddRoute"),
                                                 m_draftActiveProfile,
                                                 group,
                                                 name,
                                                 kind,
                                                 server,
                                                 static_cast<quint16>(port),
                                                 secret);
    if (!reply.isValid()) {
        reportError(reply.error().message());
        return false;
    }
    refreshProxies();
    return true;
}

void ClashClient::removeRoute(const QString &name)
{
    const QDBusReply<void> reply = m_iface->call(QStringLiteral("RemoveRoute"), m_draftActiveProfile, name);
    if (!reply.isValid()) {
        reportError(reply.error().message());
        return;
    }
    refreshProxies();
}

bool ClashClient::saveRoute(const QString &previous, const QString &name, const QString &kind, const QString &server, int port, const QString &secret)
{
    if (port < 1 || port > 65535) {
        reportError(QStringLiteral("端口无效"));
        return false;
    }
    const QDBusReply<void> reply = m_iface->call(QStringLiteral("UpdateRoute"),
                                                 m_draftActiveProfile,
                                                 previous,
                                                 name,
                                                 kind,
                                                 server,
                                                 static_cast<quint16>(port),
                                                 secret);
    if (!reply.isValid()) {
        reportError(reply.error().message());
        return false;
    }
    refreshProxies();
    return true;
}

void ClashClient::setMachineLan(bool enabled)
{
    const QDBusReply<void> reply = m_admin->call(QStringLiteral("SetMachineLan"), enabled);
    if (!reply.isValid()) {
        reportError(reply.error().message());
        return;
    }
    m_machineLanAllowed = enabled;
    refreshStatus();
    reloadAccounts();
}

void ClashClient::setAccountLan(qulonglong uid, bool enabled)
{
    const QDBusReply<void> reply = m_admin->call(QStringLiteral("SetAccountLan"), static_cast<quint32>(uid), enabled);
    if (!reply.isValid()) {
        reportError(reply.error().message());
        return;
    }
    refreshStatus();
    reloadAccounts();
}

void ClashClient::reload()
{
    refreshStatus();
    refreshProfiles();
}

void ClashClient::ensureProxies()
{
    m_proxiesWanted = true;
    if (!m_proxiesLoaded || m_proxiesStale) {
        refreshProxies();
    }
}

void ClashClient::noteProxiesChanged()
{
    if (m_proxiesWanted) {
        refreshProxies();
    } else {
        m_proxiesStale = true;
    }
}

void ClashClient::copyLiveToDraft()
{
    m_draftMixedPort = m_mixedPort;
    m_draftControllerPort = m_controllerPort;
    m_draftMihomoPath = m_mihomoPath;
    m_draftCoreDesired = m_coreDesired;
    m_draftSystemProxyEnabled = m_systemProxyEnabled;
    m_draftTunEnabled = m_tunEnabled && !m_tunMachine;
    m_draftMode = m_mode;
    m_draftActiveProfile = m_activeProfile;
    m_draftSelections = m_selections;
    m_draftSortGlobal = m_sortGlobal;
    m_draftAutoTestDelay = m_autoTestDelay;
    m_draftBlurIp = m_blurIp;
    m_draftTunDevice = m_tunDevice;
    m_draftOverridePath = m_overridePath;
    m_draftAllowLan = m_allowLan;
    m_draftLanUsername = m_lanUsername;
    m_draftLanPassword = m_lanPassword;
}

bool ClashClient::draftMatchesLive() const
{
    return m_draftMixedPort == m_mixedPort
        && m_draftControllerPort == m_controllerPort
        && m_draftMihomoPath == m_mihomoPath
        && m_draftCoreDesired == m_coreDesired
        && m_draftSystemProxyEnabled == m_systemProxyEnabled
        && m_draftTunEnabled == (m_tunEnabled && !m_tunMachine)
        && m_draftMode == m_mode
        && m_draftActiveProfile == m_activeProfile
        && m_draftSelections == m_selections
        && m_draftSortGlobal == m_sortGlobal
        && m_draftAutoTestDelay == m_autoTestDelay
        && m_draftBlurIp == m_blurIp
        && m_draftTunDevice.trimmed() == m_tunDevice
        && canonicalOverride(m_draftOverridePath, m_overrideDefault) == canonicalOverride(m_overridePath, m_overrideDefault)
        && m_draftAllowLan == m_allowLan
        && m_draftLanUsername == m_lanUsername
        && m_draftLanPassword == m_lanPassword;
}

void ClashClient::revertDraft()
{
    m_draftDirty = false;
    m_preferReset = false;
    copyLiveToDraft();
    Q_EMIT draftChanged();
}

void ClashClient::applyDefaultsToDraft()
{
    m_draftMixedPort = 7890;
    m_draftControllerPort = 9090;
    m_draftMihomoPath = QStringLiteral("mihomo");
    m_draftCoreDesired = false;
    m_draftSystemProxyEnabled = false;
    m_draftTunEnabled = false;
    m_draftMode = QStringLiteral("rule");
    m_draftSortGlobal = false;
    m_draftAutoTestDelay = false;
    m_draftBlurIp = false;
    m_draftTunDevice.clear();
    m_draftOverridePath = m_overrideDefault;
    m_draftAllowLan = false;
    m_draftLanUsername.clear();
    m_draftLanPassword.clear();
    m_draftDirty = !draftMatchesLive();
    m_preferReset = true;
    Q_EMIT draftChanged();
}

bool ClashClient::draftIsDefaults() const
{
    return m_draftMixedPort == 7890 && m_draftControllerPort == 9090 && m_draftMihomoPath == QStringLiteral("mihomo")
        && !m_draftCoreDesired && !m_draftSystemProxyEnabled && !m_draftTunEnabled && m_draftMode == QStringLiteral("rule")
        && !m_draftSortGlobal && !m_draftAutoTestDelay && !m_draftBlurIp
        && m_draftTunDevice.trimmed().isEmpty()
        && canonicalOverride(m_draftOverridePath, m_overrideDefault) == m_overrideDefault
        && !m_draftAllowLan && m_draftLanUsername.isEmpty() && m_draftLanPassword.isEmpty();
}

bool ClashClient::commitSettings()
{
    auto failed = [this](const QDBusError &error) {
        reportError(error.message());
        return false;
    };
    const QString lanUser = m_draftLanUsername.trimmed();
    const QString lanPassword = m_draftLanPassword;
    const bool lanEmpty = lanUser.isEmpty() && lanPassword.isEmpty();
    if (!lanEmpty && (lanUser.isEmpty() || lanPassword.isEmpty() || lanUser.contains(QLatin1Char(':')) || lanPassword.contains(QLatin1Char(':')))) {
        reportError(QStringLiteral("局域网账号和密码要一起填写，且不能包含冒号"));
        return false;
    }
    if (m_preferReset && draftIsDefaults()) {
        if (!m_tunDevice.isEmpty()) {
            const QDBusReply<void> auth = m_admin->call(QStringLiteral("AuthorizeTun"));
            if (!auth.isValid()) {
                return failed(auth.error());
            }
        }
        const QDBusReply<void> reply = m_iface->call(QStringLiteral("ResetPreferences"));
        if (!reply.isValid()) {
            return failed(reply.error());
        }
        m_draftDirty = false;
        m_preferReset = false;
        refreshStatus();
        return true;
    }
    const QString draftDevice = m_draftTunDevice.trimmed();
    const QString draftOverride = m_draftOverridePath.trimmed();
    if (!tunDeviceAcceptable(draftDevice)) {
        reportError(QStringLiteral("网卡名称只允许 1 到 15 个字母、数字、下划线或连字符"));
        return false;
    }
    if (!draftOverride.isEmpty()) {
        const QFileInfo info(draftOverride);
        if (!info.isAbsolute()) {
            reportError(QStringLiteral("覆盖文件需要绝对路径"));
            return false;
        }
        if (info.fileName().isEmpty()) {
            reportError(QStringLiteral("覆盖文件需要文件名"));
            return false;
        }
        if (info.isDir()) {
            reportError(QStringLiteral("覆盖路径是文件夹"));
            return false;
        }
    }
    const bool personalOn = m_tunEnabled && !m_tunMachine;
    if (draftDevice != m_tunDevice || (m_draftTunEnabled && !personalOn)) {
        const QDBusReply<void> auth = m_admin->call(QStringLiteral("AuthorizeTun"));
        if (!auth.isValid()) {
            return failed(auth.error());
        }
    }
    if (!m_draftCoreDesired && m_coreDesired) {
        const QDBusReply<void> reply = m_iface->call(QStringLiteral("Stop"));
        if (!reply.isValid()) {
            return failed(reply.error());
        }
    }
    const bool settingsDiffer = m_draftMixedPort != m_mixedPort || m_draftControllerPort != m_controllerPort || m_draftMihomoPath != m_mihomoPath
        || draftDevice != m_tunDevice
        || canonicalOverride(draftOverride, m_overrideDefault) != canonicalOverride(m_overridePath, m_overrideDefault);
    if (settingsDiffer) {
        const QDBusReply<void> reply = m_iface->call(QStringLiteral("SetSettings"),
                                                     static_cast<quint32>(m_draftMixedPort),
                                                     static_cast<quint32>(m_draftControllerPort),
                                                     m_draftMihomoPath,
                                                     draftDevice,
                                                     draftOverride);
        if (!reply.isValid()) {
            return failed(reply.error());
        }
    }
    if (m_draftMode != m_mode) {
        const QDBusReply<void> reply = m_iface->call(QStringLiteral("SetMode"), m_draftMode);
        if (!reply.isValid()) {
            return failed(reply.error());
        }
    }
    if (m_draftTunEnabled != (m_tunEnabled && !m_tunMachine)) {
        const QDBusReply<void> reply = m_iface->call(QStringLiteral("SetTun"), m_draftTunEnabled);
        if (!reply.isValid()) {
            return failed(reply.error());
        }
    }
    if (m_draftSystemProxyEnabled != m_systemProxyEnabled) {
        const QDBusReply<void> reply = m_iface->call(QStringLiteral("SetSystemProxy"), m_draftSystemProxyEnabled);
        if (!reply.isValid()) {
            return failed(reply.error());
        }
    }
    if (m_draftActiveProfile != m_activeProfile || m_draftSelections != m_selections) {
        QJsonObject object;
        for (auto it = m_draftSelections.cbegin(); it != m_draftSelections.cend(); ++it) {
            object.insert(it.key(), it.value().toString());
        }
        const QDBusReply<void> reply = m_iface->call(QStringLiteral("SetChoices"),
                                                     m_draftActiveProfile,
                                                     QString::fromUtf8(QJsonDocument(object).toJson(QJsonDocument::Compact)));
        if (!reply.isValid()) {
            return failed(reply.error());
        }
    }
    if (m_draftSortGlobal != m_sortGlobal || m_draftAutoTestDelay != m_autoTestDelay || m_draftBlurIp != m_blurIp) {
        const QDBusReply<void> reply = m_iface->call(QStringLiteral("SetBehavior"), m_draftSortGlobal, m_draftAutoTestDelay, m_draftBlurIp);
        if (!reply.isValid()) {
            return failed(reply.error());
        }
    }
    if (m_draftAllowLan != m_allowLan || m_draftLanUsername != m_lanUsername || m_draftLanPassword != m_lanPassword) {
        const QDBusReply<void> reply = m_iface->call(QStringLiteral("SetLan"), m_draftAllowLan, lanUser, lanPassword);
        if (!reply.isValid()) {
            return failed(reply.error());
        }
    }
    if (m_draftCoreDesired && !m_coreDesired) {
        const QDBusReply<void> reply = m_iface->call(QStringLiteral("Start"));
        if (!reply.isValid()) {
            return failed(reply.error());
        }
    }
    m_draftDirty = false;
    refreshStatus();
    return true;
}

void ClashClient::refreshFirewall()
{
    const qint64 now = QDateTime::currentMSecsSinceEpoch();
    if (m_firewallBusy || (m_firewallCheckedAt != 0 && now - m_firewallCheckedAt < 30000)) {
        return;
    }
    m_firewallCheckedAt = now;
    if (ufwEnabled()) {
        if (!m_firewallActive) {
            m_firewallActive = true;
            Q_EMIT statusChanged();
        }
        return;
    }
    m_firewallBusy = true;
    auto *process = new QProcess(this);
    connect(process, &QProcess::finished, this, [this, process](int code, QProcess::ExitStatus status) {
        m_firewallBusy = false;
        process->deleteLater();
        const bool active = status == QProcess::NormalExit && code == 0;
        if (m_firewallActive != active) {
            m_firewallActive = active;
            Q_EMIT statusChanged();
        }
    });
    process->start(QStringLiteral("systemctl"),
                   {QStringLiteral("is-active"), QStringLiteral("--quiet"), QStringLiteral("firewalld")});
}

void ClashClient::refreshStatus()
{
    if (m_statusPending) {
        return;
    }
    m_statusPending = true;
    refreshFirewall();
    auto *watcher = new QDBusPendingCallWatcher(m_iface->asyncCall(QStringLiteral("GetStatus")), this);
    connect(watcher, &QDBusPendingCallWatcher::finished, this, [this](QDBusPendingCallWatcher *call) {
        call->deleteLater();
        m_statusPending = false;
        const QDBusPendingReply<QString> reply = *call;
        if (!reply.isValid()) {
            if (m_message != reply.error().message()) {
                m_message = reply.error().message();
                Q_EMIT statusChanged();
            }
            return;
        }
        const auto document = QJsonDocument::fromJson(reply.value().toUtf8());
        applyStatus(document.object().toVariantMap());
    });
}

void ClashClient::refreshProfiles()
{
    if (m_profilesPending) {
        return;
    }
    m_profilesPending = true;
    auto *watcher = new QDBusPendingCallWatcher(m_iface->asyncCall(QStringLiteral("GetProfiles")), this);
    connect(watcher, &QDBusPendingCallWatcher::finished, this, [this](QDBusPendingCallWatcher *call) {
        call->deleteLater();
        m_profilesPending = false;
        const QDBusPendingReply<QString> reply = *call;
        if (!reply.isValid()) {
            reportError(reply.error().message());
            return;
        }
        const QVariantList next = jsonArray(reply.value());
        if (next != m_profiles) {
            m_profiles = next;
            Q_EMIT profilesChanged();
        }
    });
}

void ClashClient::refreshProxies()
{
    if (m_proxiesPending) {
        return;
    }
    m_proxiesPending = true;
    auto *watcher = new QDBusPendingCallWatcher(m_iface->asyncCall(QStringLiteral("GetProxies")), this);
    connect(watcher, &QDBusPendingCallWatcher::finished, this, [this](QDBusPendingCallWatcher *call) {
        call->deleteLater();
        m_proxiesPending = false;
        const QDBusPendingReply<QString> reply = *call;
        if (!reply.isValid()) {
            reportError(reply.error().message());
            return;
        }
        m_proxiesLoaded = true;
        m_proxiesStale = false;
        const QVariantList next = jsonArray(reply.value());
        if (next != m_groups) {
            m_groups = next;
            Q_EMIT proxiesChanged();
        }
    });
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
    for (int i = 0; i < m_groups.size(); ++i) {
        auto entry = m_groups.at(i).toMap();
        if (entry.value(QStringLiteral("name")).toString() != group) {
            continue;
        }
        auto nodes = entry.value(QStringLiteral("nodes")).toList();
        for (int nodeIndex = 0; nodeIndex < nodes.size(); ++nodeIndex) {
            auto node = nodes.at(nodeIndex).toMap();
            if (node.value(QStringLiteral("name")).toString() != name) {
                continue;
            }
            node.insert(QStringLiteral("delay_ms"), delayMs);
            nodes[nodeIndex] = node;
            entry.insert(QStringLiteral("nodes"), nodes);
            m_groups[i] = entry;
            break;
        }
        break;
    }
    Q_EMIT delayUpdated(group, name, delayMs);
}

void ClashClient::applyStatus(const QVariantMap &status)
{
    const bool proxyWasEnabled = m_systemProxyEnabled;
    const bool coreDesired = status.value(QStringLiteral("core_desired")).toBool();
    const bool coreRunning = status.value(QStringLiteral("core_running")).toBool();
    const bool systemProxyEnabled = status.value(QStringLiteral("system_proxy_enabled")).toBool();
    const bool systemProxyApplied = status.value(QStringLiteral("system_proxy_applied")).toBool();
    const bool tunEnabled = status.value(QStringLiteral("tun_enabled")).toBool();
    const bool tunMachine = status.value(QStringLiteral("tun_machine")).toBool();
    QString kdeProxy = status.value(QStringLiteral("kde_proxy")).toString();
    if (kdeProxy.isEmpty()) {
        kdeProxy = QStringLiteral("off");
    }
    QString gnomeProxy = status.value(QStringLiteral("gnome_proxy")).toString();
    if (gnomeProxy.isEmpty()) {
        gnomeProxy = QStringLiteral("off");
    }
    const QString mode = status.value(QStringLiteral("mode")).toString();
    const QString listenHost = status.value(QStringLiteral("listen_host"), QStringLiteral("127.0.0.1")).toString();
    const int mixedPort = status.value(QStringLiteral("mixed_port")).toInt();
    const int controllerPort = status.value(QStringLiteral("controller_port")).toInt();
    const QString mihomoPath = status.value(QStringLiteral("mihomo_path")).toString();
    const QString overridePath = status.value(QStringLiteral("override_path")).toString();
    QString overrideDefault = status.value(QStringLiteral("override_default")).toString();
    if (overrideDefault.isEmpty()) {
        overrideDefault = overridePath;
    }
    const QString tunDevice = status.value(QStringLiteral("tun_device")).toString();
    const qulonglong uploadBps = status.value(QStringLiteral("upload_bps")).toULongLong();
    const qulonglong downloadBps = status.value(QStringLiteral("download_bps")).toULongLong();
    const QString exitRegion = status.value(QStringLiteral("exit_region")).toString();
    const QString activeProfile = status.value(QStringLiteral("active_profile")).toString();
    const QVariantMap selections = status.value(QStringLiteral("selections")).toMap();
    const bool sortGlobal = status.value(QStringLiteral("sort_global")).toBool();
    const QString sortMode = status.value(QStringLiteral("sort_mode"), QStringLiteral("profile")).toString();
    const bool showUnreachable = status.value(QStringLiteral("show_unreachable"), true).toBool();
    const QVariantMap sortPrefs = status.value(QStringLiteral("sort_prefs")).toMap();
    const bool autoTestDelay = status.value(QStringLiteral("auto_test_delay")).toBool();
    const bool blurIp = status.value(QStringLiteral("blur_ip")).toBool();
    const bool allowLan = status.value(QStringLiteral("allow_lan")).toBool();
    const QString lanUsername = status.value(QStringLiteral("lan_username")).toString();
    const QString lanPassword = status.value(QStringLiteral("lan_password")).toString();
    const bool lanAllowed = status.value(QStringLiteral("lan_allowed"), true).toBool();
    QString message = status.value(QStringLiteral("message")).toString();
    const QString dbusPrefix = QStringLiteral("org.freedesktop.DBus.Error.Failed: ");
    if (message.startsWith(dbusPrefix)) {
        message = message.mid(dbusPrefix.size());
    }
    if (m_coreDesired == coreDesired && m_coreRunning == coreRunning && m_systemProxyEnabled == systemProxyEnabled
        && m_systemProxyApplied == systemProxyApplied && m_tunEnabled == tunEnabled && m_tunMachine == tunMachine
        && m_kdeProxy == kdeProxy && m_gnomeProxy == gnomeProxy && m_mode == mode && m_listenHost == listenHost
        && m_mixedPort == mixedPort && m_controllerPort == controllerPort && m_mihomoPath == mihomoPath
        && m_overridePath == overridePath && m_overrideDefault == overrideDefault && m_tunDevice == tunDevice
        && m_uploadBps == uploadBps && m_downloadBps == downloadBps && m_exitRegion == exitRegion
        && m_activeProfile == activeProfile && m_selections == selections && m_sortGlobal == sortGlobal
        && m_sortMode == sortMode && m_showUnreachable == showUnreachable && m_sortPrefs == sortPrefs
        && m_autoTestDelay == autoTestDelay && m_blurIp == blurIp && m_allowLan == allowLan
        && m_lanUsername == lanUsername && m_lanPassword == lanPassword && m_lanAllowed == lanAllowed
        && m_message == message) {
        return;
    }
    m_coreDesired = coreDesired;
    m_coreRunning = coreRunning;
    m_systemProxyEnabled = systemProxyEnabled;
    m_systemProxyApplied = systemProxyApplied;
    m_tunEnabled = tunEnabled;
    m_tunMachine = tunMachine;
    m_kdeProxy = kdeProxy;
    m_gnomeProxy = gnomeProxy;
    m_mode = mode;
    m_listenHost = listenHost;
    m_mixedPort = mixedPort;
    m_controllerPort = controllerPort;
    m_mihomoPath = mihomoPath;
    m_overridePath = overridePath;
    m_overrideDefault = overrideDefault;
    m_tunDevice = tunDevice;
    m_uploadBps = uploadBps;
    m_downloadBps = downloadBps;
    m_exitRegion = exitRegion;
    m_activeProfile = activeProfile;
    m_selections = selections;
    m_sortGlobal = sortGlobal;
    m_sortMode = sortMode;
    m_showUnreachable = showUnreachable;
    m_sortPrefs = sortPrefs;
    m_autoTestDelay = autoTestDelay;
    m_blurIp = blurIp;
    m_allowLan = allowLan;
    m_lanUsername = lanUsername;
    m_lanPassword = lanPassword;
    m_lanAllowed = lanAllowed;
    m_message = message;
    const bool releasedOutside = proxyWasEnabled && !m_systemProxyEnabled;
    if (!m_draftDirty && !draftMatchesLive()) {
        copyLiveToDraft();
        Q_EMIT draftChanged();
    } else if (releasedOutside && m_draftSystemProxyEnabled) {
        m_draftSystemProxyEnabled = false;
        m_preferReset = false;
        m_draftDirty = !draftMatchesLive();
        Q_EMIT draftChanged();
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
            && map.value(QStringLiteral("tun_machine")).toBool() == m_tunMachine
            && map.value(QStringLiteral("system_proxy_enabled")).toBool() == m_systemProxyEnabled
            && map.value(QStringLiteral("mode")).toString() == m_mode
            && map.value(QStringLiteral("exit_region")).toString() == m_exitRegion
            && map.value(QStringLiteral("error")).toString().isEmpty();
        if (same) {
            continue;
        }
        map.insert(QStringLiteral("core_running"), m_coreRunning);
        map.insert(QStringLiteral("tun_enabled"), m_tunEnabled);
        map.insert(QStringLiteral("tun_machine"), m_tunMachine);
        map.insert(QStringLiteral("system_proxy_enabled"), m_systemProxyEnabled);
        map.insert(QStringLiteral("exit_region"), m_exitRegion);
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
