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
#include <QDateTime>
#include <QNetworkAccessManager>
#include <QNetworkProxy>
#include <QNetworkReply>
#include <QNetworkRequest>
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

QVariantMap ruleMap(const QString &type, const QString &payload, const QString &target, bool enabled)
{
    QVariantMap map;
    map.insert(QStringLiteral("type"), type);
    map.insert(QStringLiteral("payload"), type == QLatin1String("MATCH") ? QString() : payload);
    map.insert(QStringLiteral("target"), target);
    map.insert(QStringLiteral("enabled"), enabled);
    return map;
}

bool sameRule(const QVariantMap &left, const QVariantMap &right)
{
    return left.value(QStringLiteral("type")).toString() == right.value(QStringLiteral("type")).toString()
        && left.value(QStringLiteral("payload")).toString() == right.value(QStringLiteral("payload")).toString()
        && left.value(QStringLiteral("target")).toString() == right.value(QStringLiteral("target")).toString()
        && left.value(QStringLiteral("enabled")).toBool() == right.value(QStringLiteral("enabled")).toBool();
}

bool sameRules(const QVariantList &left, const QVariantList &right)
{
    if (left.size() != right.size()) {
        return false;
    }
    for (int index = 0; index < left.size(); ++index) {
        if (!sameRule(left.at(index).toMap(), right.at(index).toMap())) {
            return false;
        }
    }
    return true;
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
bool ClashClient::settingsDirty() const { return m_draftDirty || !m_routeEdits.isEmpty() || ruleDirty() || !m_noteEdits.isEmpty(); }

void ClashClient::setDraftMixedPort(int port)
{
    if (m_draftMixedPort == port) {
        return;
    }
    m_draftMixedPort = port;
    m_preferReset = false;
    m_draftDirty = !draftMatchesLive();
    Q_EMIT draftChanged();
}

void ClashClient::setDraftControllerPort(int port)
{
    if (m_draftControllerPort == port) {
        return;
    }
    m_draftControllerPort = port;
    m_preferReset = false;
    m_draftDirty = !draftMatchesLive();
    Q_EMIT draftChanged();
}

void ClashClient::setDraftMihomoPath(const QString &path)
{
    if (m_draftMihomoPath == path) {
        return;
    }
    m_draftMihomoPath = path;
    m_preferReset = false;
    m_draftDirty = !draftMatchesLive();
    Q_EMIT draftChanged();
}

void ClashClient::setDraftCoreDesired(bool desired)
{
    if (m_draftCoreDesired == desired) {
        return;
    }
    m_draftCoreDesired = desired;
    m_preferReset = false;
    m_draftDirty = !draftMatchesLive();
    Q_EMIT draftChanged();
}

void ClashClient::setDraftSystemProxyEnabled(bool enabled)
{
    if (m_draftSystemProxyEnabled == enabled) {
        return;
    }
    m_draftSystemProxyEnabled = enabled;
    m_preferReset = false;
    m_draftDirty = !draftMatchesLive();
    Q_EMIT draftChanged();
}

void ClashClient::setDraftTunEnabled(bool enabled)
{
    if (m_draftTunEnabled == enabled) {
        return;
    }
    m_draftTunEnabled = enabled;
    m_preferReset = false;
    m_draftDirty = !draftMatchesLive();
    Q_EMIT draftChanged();
}

void ClashClient::setDraftMode(const QString &mode)
{
    if (m_draftMode == mode) {
        return;
    }
    m_draftMode = mode;
    m_preferReset = false;
    m_draftDirty = !draftMatchesLive();
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
bool ClashClient::compact() const { return m_compact; }
bool ClashClient::showWidgetHint() const { return m_showWidgetHint; }
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
    m_preferReset = false;
    m_draftDirty = !draftMatchesLive();
    Q_EMIT draftChanged();
}

void ClashClient::setDraftSelections(const QVariantMap &selections)
{
    if (m_draftSelections == selections) {
        return;
    }
    m_draftSelections = selections;
    m_preferReset = false;
    m_draftDirty = !draftMatchesLive();
    Q_EMIT draftChanged();
}

void ClashClient::setDraftSortGlobal(bool global)
{
    if (m_draftSortGlobal == global) {
        return;
    }
    m_draftSortGlobal = global;
    m_preferReset = false;
    m_draftDirty = !draftMatchesLive();
    Q_EMIT draftChanged();
}

void ClashClient::setDraftAutoTestDelay(bool enabled)
{
    if (m_draftAutoTestDelay == enabled) {
        return;
    }
    m_draftAutoTestDelay = enabled;
    m_preferReset = false;
    m_draftDirty = !draftMatchesLive();
    Q_EMIT draftChanged();
}

void ClashClient::setDraftBlurIp(bool enabled)
{
    if (m_draftBlurIp == enabled) {
        return;
    }
    m_draftBlurIp = enabled;
    m_preferReset = false;
    m_draftDirty = !draftMatchesLive();
    Q_EMIT draftChanged();
}

bool ClashClient::draftCompact() const { return m_draftCompact; }

void ClashClient::setDraftCompact(bool enabled)
{
    if (m_draftCompact == enabled) {
        return;
    }
    m_draftCompact = enabled;
    m_preferReset = false;
    m_draftDirty = !draftMatchesLive();
    Q_EMIT draftChanged();
}

bool ClashClient::draftShowWidgetHint() const { return m_draftShowWidgetHint; }

void ClashClient::setDraftShowWidgetHint(bool enabled)
{
    if (m_draftShowWidgetHint == enabled) {
        return;
    }
    m_draftShowWidgetHint = enabled;
    m_preferReset = false;
    m_draftDirty = !draftMatchesLive();
    Q_EMIT draftChanged();
}

int ClashClient::draftSpeedTestMib() const
{
    return m_draftSpeedTestMib;
}

void ClashClient::setDraftSpeedTestMib(int mib)
{
    mib = qBound(1, mib, 100);
    if (m_draftSpeedTestMib == mib) {
        return;
    }
    m_draftSpeedTestMib = mib;
    m_preferReset = false;
    m_draftDirty = !draftMatchesLive();
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
    m_preferReset = false;
    m_draftDirty = !draftMatchesLive();
    Q_EMIT draftChanged();
}

void ClashClient::setDraftOverridePath(const QString &path)
{
    if (m_draftOverridePath == path) {
        return;
    }
    m_draftOverridePath = path;
    m_preferReset = false;
    m_draftDirty = !draftMatchesLive();
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

QString ClashClient::presentHost(const QString &host, bool blur) const
{
    return ::presentHost(host, blur);
}

QVariantList ClashClient::connections() const { return m_connections; }

QVariantList ClashClient::proxyProviders() const { return m_proxyProviders; }

QVariantList ClashClient::ownRules() const
{
    if (m_ruleImporting) {
        return m_ruleImport;
    }
    QVariantList shown = m_savedRules;
    for (auto it = m_ruleEdits.cbegin(); it != m_ruleEdits.cend(); ++it) {
        bool ok = false;
        const int index = it.key().toInt(&ok);
        if (ok && index >= 0 && index < shown.size()) {
            shown[index] = it.value();
        }
    }
    return shown;
}

QVariantList ClashClient::profileRules() const { return m_profileRules; }

QVariantList ClashClient::ruleProviders() const { return m_ruleProviders; }

QVariantList ClashClient::ruleTargets() const { return m_ruleTargets; }

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

void ClashClient::setCompact(bool enabled)
{
    if (m_compact == enabled) {
        return;
    }
    const QDBusReply<void> reply = m_iface->call(QStringLiteral("SetCompact"), enabled);
    if (!reply.isValid()) {
        reportError(reply.error().message());
        return;
    }
    m_compact = enabled;
    Q_EMIT statusChanged();
}

QVariantMap ClashClient::stagedRoute(const QString &previous) const
{
    return m_routeEdits.value(previous).toMap();
}

void ClashClient::stageRoute(const QString &previous, const QString &name, const QString &kind, const QString &server, int port, const QString &secret)
{
    if (previous.isEmpty()) {
        return;
    }
    if (m_routeProfile.isEmpty() || m_routeEdits.isEmpty()) {
        m_routeProfile = m_draftActiveProfile;
    }
    if (!m_routeBaselines.contains(previous)) {
        const QDBusReply<QString> reply = m_iface->call(QStringLiteral("RouteDetail"), m_routeProfile, previous);
        if (!reply.isValid()) {
            return;
        }
        const QJsonDocument document = QJsonDocument::fromJson(reply.value().toUtf8());
        m_routeBaselines.insert(previous, document.object().toVariantMap());
    }
    const QVariantMap saved = m_routeBaselines.value(previous).toMap();
    const bool same = saved.value(QStringLiteral("in_file")).toBool()
        && saved.value(QStringLiteral("name")).toString() == name
        && saved.value(QStringLiteral("kind")).toString() == kind
        && saved.value(QStringLiteral("server")).toString() == server
        && saved.value(QStringLiteral("port")).toInt() == port
        && saved.value(QStringLiteral("secret")).toString() == secret;
    if (same) {
        m_routeEdits.remove(previous);
    } else {
        m_preferReset = false;
        QVariantMap edit;
        edit.insert(QStringLiteral("name"), name);
        edit.insert(QStringLiteral("kind"), kind);
        edit.insert(QStringLiteral("server"), server);
        edit.insert(QStringLiteral("port"), port);
        edit.insert(QStringLiteral("secret"), secret);
        edit.insert(QStringLiteral("in_file"), true);
        m_routeEdits.insert(previous, edit);
    }
    if (m_routeEdits.isEmpty()) {
        m_routeProfile.clear();
    }
    Q_EMIT draftChanged();
}

QString ClashClient::noteFor(const QString &key) const
{
    if (m_noteEdits.contains(key)) {
        return m_noteEdits.value(key).toString();
    }
    return m_savedNotes.value(key).toString();
}

void ClashClient::stageNote(const QString &key, const QString &text)
{
    if (key.isEmpty()) {
        return;
    }
    const QString saved = m_savedNotes.value(key).toString();
    if (text == saved) {
        m_noteEdits.remove(key);
    } else {
        m_preferReset = false;
        m_noteEdits.insert(key, text);
    }
    Q_EMIT draftChanged();
    Q_EMIT notesChanged();
}

void ClashClient::pullNotes()
{
    const QDBusReply<QString> reply = m_iface->call(QStringLiteral("GetNotes"));
    if (!reply.isValid()) {
        return;
    }
    const QJsonObject object = QJsonDocument::fromJson(reply.value().toUtf8()).object();
    const QVariantMap next = object.toVariantMap();
    if (next == m_savedNotes) {
        return;
    }
    m_savedNotes = next;
    Q_EMIT notesChanged();
}

QString dbusHint(const QDBusError &error)
{
    QString text = error.message();
    const QString prefix = QStringLiteral("org.freedesktop.DBus.Error.Failed: ");
    if (text.startsWith(prefix)) {
        text = text.mid(prefix.size());
    }
    return text;
}

bool ClashClient::restoreRoute(const QString &name)
{
    const QDBusReply<void> reply = m_iface->call(QStringLiteral("RestoreRoute"), m_draftActiveProfile, name);
    if (!reply.isValid()) {
        showHint(dbusHint(reply.error()));
        return false;
    }
    m_routeEdits.remove(name);
    m_routeBaselines.remove(name);
    if (m_routeEdits.isEmpty()) {
        m_routeProfile.clear();
    }
    Q_EMIT draftChanged();
    refreshProxies();
    return true;
}

bool ClashClient::restoreProfileRule(int index)
{
    if (index < 0) {
        return false;
    }
    const QDBusReply<void> reply = m_iface->call(QStringLiteral("RestoreProfileRule"), m_draftActiveProfile, static_cast<quint32>(index));
    if (!reply.isValid()) {
        showHint(dbusHint(reply.error()));
        return false;
    }
    pullRules();
    return true;
}

void ClashClient::shiftOwnRuleNotes(int removed)
{
    const QString prefix = QStringLiteral("rule\nown\n");
    auto shift = [prefix, removed](QVariantMap &map) {
        QVariantMap next;
        for (auto it = map.cbegin(); it != map.cend(); ++it) {
            if (!it.key().startsWith(prefix)) {
                next.insert(it.key(), it.value());
                continue;
            }
            bool ok = false;
            int index = it.key().mid(prefix.size()).toInt(&ok);
            if (!ok || index == removed) {
                continue;
            }
            if (index > removed) {
                index -= 1;
            }
            next.insert(prefix + QString::number(index), it.value());
        }
        map = next;
    };
    const QVariantMap before = m_savedNotes;
    shift(m_savedNotes);
    shift(m_noteEdits);
    if (m_savedNotes != before) {
        const QString body = QString::fromUtf8(QJsonDocument(QJsonObject::fromVariantMap(m_savedNotes)).toJson(QJsonDocument::Compact));
        m_iface->call(QStringLiteral("SetNotes"), body);
    }
    Q_EMIT notesChanged();
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
    m_routeEdits.remove(name);
    m_routeBaselines.remove(name);
    if (m_routeEdits.isEmpty()) {
        m_routeProfile.clear();
    }
    Q_EMIT draftChanged();
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

bool ClashClient::ruleDirty() const
{
    if (m_ruleImporting) {
        return !sameRules(m_ruleImport, m_savedRules);
    }
    return !m_ruleEdits.isEmpty();
}

QString ClashClient::rulesJson() const
{
    QJsonArray array;
    const QVariantList rules = ownRules();
    for (const QVariant &item : rules) {
        const QVariantMap map = item.toMap();
        QJsonObject object;
        object.insert(QStringLiteral("type"), map.value(QStringLiteral("type")).toString());
        object.insert(QStringLiteral("payload"), map.value(QStringLiteral("payload")).toString());
        object.insert(QStringLiteral("target"), map.value(QStringLiteral("target")).toString());
        object.insert(QStringLiteral("enabled"), map.value(QStringLiteral("enabled")).toBool());
        array.append(object);
    }
    return QString::fromUtf8(QJsonDocument(array).toJson(QJsonDocument::Compact));
}

void ClashClient::clearRuleDraft()
{
    m_ruleEdits.clear();
    m_ruleImport.clear();
    m_ruleImporting = false;
}

void ClashClient::pullRules()
{
    const QDBusReply<QString> reply = m_iface->call(QStringLiteral("GetRuleView"));
    if (!reply.isValid()) {
        reportError(reply.error().message());
        return;
    }
    applyRuleView(QJsonDocument::fromJson(reply.value().toUtf8()).object().toVariantMap());
}

void ClashClient::applyRuleView(const QVariantMap &view)
{
    m_savedRules = view.value(QStringLiteral("own")).toList();
    QVariantList profile = view.value(QStringLiteral("profile")).toList();
    const QVariantList changed = view.value(QStringLiteral("changed")).toList();
    for (int i = 0; i < profile.size(); ++i) {
        QVariantMap row = profile.at(i).toMap();
        row.insert(QStringLiteral("changed"), i < changed.size() && changed.at(i).toBool());
        profile[i] = row;
    }
    m_profileRules = profile;
    m_ruleProviders = view.value(QStringLiteral("providers")).toList();
    m_ruleTargets = view.value(QStringLiteral("targets")).toList();
    Q_EMIT rulesChanged();
}

void ClashClient::discardSheetFile()
{
    if (m_sheetPath.isEmpty()) {
        return;
    }
    QFile::remove(m_sheetPath);
    m_sheetPath.clear();
}

void ClashClient::refreshConnections()
{
    if (!m_coreRunning) {
        if (!m_connections.isEmpty()) {
            m_connections.clear();
            Q_EMIT connectionsChanged();
        }
        return;
    }
    if (m_connectionsPending) {
        return;
    }
    m_connectionsPending = true;
    auto *watcher = new QDBusPendingCallWatcher(m_iface->asyncCall(QStringLiteral("GetConnections")), this);
    connect(watcher, &QDBusPendingCallWatcher::finished, this, [this](QDBusPendingCallWatcher *call) {
        call->deleteLater();
        m_connectionsPending = false;
        const QDBusPendingReply<QString> reply = *call;
        if (!reply.isValid()) {
            return;
        }
        const QVariantList next = jsonArray(reply.value());
        if (next == m_connections) {
            return;
        }
        m_connections = next;
        Q_EMIT connectionsChanged();
    });
}

void ClashClient::closeConnection(const QString &id)
{
    auto *watcher = new QDBusPendingCallWatcher(m_iface->asyncCall(QStringLiteral("CloseConnection"), id), this);
    connect(watcher, &QDBusPendingCallWatcher::finished, this, [this](QDBusPendingCallWatcher *call) {
        call->deleteLater();
        const QDBusPendingReply<> reply = *call;
        if (!reply.isValid()) {
            reportError(reply.error().message());
            return;
        }
        refreshConnections();
    });
}

void ClashClient::closeAllConnections()
{
    auto *watcher = new QDBusPendingCallWatcher(m_iface->asyncCall(QStringLiteral("CloseAllConnections")), this);
    connect(watcher, &QDBusPendingCallWatcher::finished, this, [this](QDBusPendingCallWatcher *call) {
        call->deleteLater();
        const QDBusPendingReply<> reply = *call;
        if (!reply.isValid()) {
            reportError(reply.error().message());
            return;
        }
        refreshConnections();
    });
}

namespace {
constexpr qint64 kProbeBytes = 1000;
constexpr int kSampleMs = 100;
constexpr int kMaxSamples = 6000;
constexpr int kLiveSamples = 80;
constexpr int kSmooth = 5;

int binCount(int samples)
{
    if (samples <= 0) {
        return 0;
    }
    return qBound(8, samples / 5, 48);
}

QVariantList liveWindow(const QList<double> &raw)
{
    if (raw.isEmpty()) {
        return {};
    }
    const int begin = qMax(0, raw.size() - kLiveSamples);
    QVariantList out;
    out.reserve(raw.size() - begin);
    for (int i = begin; i < raw.size(); ++i) {
        const int from = qMax(begin, i - kSmooth + 1);
        double sum = 0;
        for (int j = from; j <= i; ++j) {
            sum += raw.at(j);
        }
        out.append(sum / (i - from + 1));
    }
    return out;
}

void trimToSharedTail(QVariantList &first, QVariantList &second)
{
    if (first.isEmpty() || second.isEmpty() || first.size() == second.size()) {
        return;
    }
    const int count = qMin(first.size(), second.size());
    if (first.size() != count) {
        first = first.mid(first.size() - count);
    }
    if (second.size() != count) {
        second = second.mid(second.size() - count);
    }
}

QVariantList resample(const QList<double> &raw, int bins)
{
    if (raw.isEmpty() || bins <= 0) {
        return {};
    }
    QVariantList out;
    out.reserve(bins);
    for (int bin = 0; bin < bins; ++bin) {
        int from = int(double(bin) * raw.size() / bins);
        int to = int(double(bin + 1) * raw.size() / bins);
        from = qBound(0, from, raw.size() - 1);
        to = qBound(from + 1, to, raw.size());
        double sum = 0;
        for (int i = from; i < to; ++i) {
            sum += raw.at(i);
        }
        out.append(sum / (to - from));
    }
    return out;
}

QNetworkRequest speedRequest(const QUrl &url, int timeoutMs)
{
    QNetworkRequest request(url);
    request.setTransferTimeout(timeoutMs);
    request.setHeader(QNetworkRequest::UserAgentHeader, QStringLiteral("clash-kde"));
    request.setAttribute(QNetworkRequest::RedirectPolicyAttribute, QNetworkRequest::ManualRedirectPolicy);
    request.setAttribute(QNetworkRequest::CacheLoadControlAttribute, QNetworkRequest::AlwaysNetwork);
    request.setAttribute(QNetworkRequest::CacheSaveControlAttribute, false);
    return request;
}

qulonglong pace(qint64 bytes, qint64 startedMs)
{
    const qint64 elapsed = qMax(qint64(1), QDateTime::currentMSecsSinceEpoch() - startedMs);
    return qulonglong(qMax(qint64(0), bytes) * 1000 / elapsed);
}

bool speedOk(QNetworkReply *reply)
{
    const int status = reply->attribute(QNetworkRequest::HttpStatusCodeAttribute).toInt();
    return reply->error() == QNetworkReply::NoError && (status == 0 || status == 200);
}

QString speedErrorText(QNetworkReply *reply)
{
    if (reply->error() != QNetworkReply::NoError) {
        return reply->errorString();
    }
    return QString::number(reply->attribute(QNetworkRequest::HttpStatusCodeAttribute).toInt());
}
}

void ClashClient::noteSpeed(bool force)
{
    const qint64 now = QDateTime::currentMSecsSinceEpoch();
    if (!force && now - m_speedNotedMs < 100) {
        return;
    }
    m_speedNotedMs = now;
    Q_EMIT speedChanged();
}

QNetworkAccessManager *ClashClient::speedNam(bool throughProxy)
{
    if (!throughProxy) {
        if (!m_directNam) {
            m_directNam = new QNetworkAccessManager(this);
            m_directNam->setProxy(QNetworkProxy(QNetworkProxy::NoProxy));
        }
        return m_directNam;
    }
    if (!m_proxyNam) {
        m_proxyNam = new QNetworkAccessManager(this);
    }
    QString host = m_listenHost;
    if (host.isEmpty() || host == QLatin1String("0.0.0.0") || host == QLatin1String("::")) {
        host = QStringLiteral("127.0.0.1");
    }
    m_proxyNam->setProxy(QNetworkProxy(QNetworkProxy::HttpProxy, host, m_mixedPort));
    return m_proxyNam;
}

qint64 ClashClient::speedDownloadBytes() const
{
    return qint64(qBound(1, m_speedTestMib, 100)) * 1000000;
}

void ClashClient::noteSpeedError(const QString &which, const QString &message)
{
    if (message.isEmpty()) {
        return;
    }
    const QString line = which + QStringLiteral(": ") + message;
    if (m_speedError.isEmpty()) {
        m_speedError = line;
    } else if (!m_speedError.contains(line)) {
        m_speedError = m_speedError + QLatin1Char('\n') + line;
    }
}

void ClashClient::abortSpeedReply(QNetworkReply *&reply)
{
    if (!reply) {
        return;
    }
    auto *current = reply;
    reply = nullptr;
    current->disconnect(this);
    current->abort();
    current->deleteLater();
}

void ClashClient::takeSpeedSample(SpeedLeg &leg, qint64 now, bool force)
{
    if (leg.received <= 0 || leg.raw.size() >= kMaxSamples) {
        return;
    }
    if (!force && !leg.downloading) {
        return;
    }
    if (force && leg.raw.isEmpty()) {
        leg.raw.append(double(pace(leg.received, leg.markMs)));
        leg.sampleBytes = leg.received;
        leg.sampleAt = now;
        return;
    }
    if (leg.raw.isEmpty() && leg.sampleBytes == 0) {
        leg.sampleBytes = leg.received;
        leg.sampleAt = now;
        return;
    }
    const qint64 dt = qMax(qint64(1), now - leg.sampleAt);
    if (!force && dt < kSampleMs) {
        return;
    }
    const qint64 delta = qMax(qint64(0), leg.received - leg.sampleBytes);
    leg.sampleBytes = leg.received;
    leg.sampleAt = now;
    if (delta == 0) {
        return;
    }
    leg.raw.append(double(delta) * 1000.0 / double(dt));
}

void ClashClient::publishSpeedSeries(bool force)
{
    if (m_speedRunning) {
        auto proxy = liveWindow(m_proxyLeg.raw);
        auto direct = liveWindow(m_directLeg.raw);
        trimToSharedTail(proxy, direct);
        m_speedProxySamples = proxy;
        m_speedDirectSamples = direct;
    } else {
        const int bins = qMax(binCount(m_proxyLeg.raw.size()), binCount(m_directLeg.raw.size()));
        m_speedProxySamples = resample(m_proxyLeg.raw, bins);
        m_speedDirectSamples = resample(m_directLeg.raw, bins);
    }
    noteSpeed(force);
}

void ClashClient::startSpeedSampler()
{
    if (!m_speedSampleTimer) {
        m_speedSampleTimer = new QTimer(this);
        m_speedSampleTimer->setInterval(kSampleMs);
        connect(m_speedSampleTimer, &QTimer::timeout, this, &ClashClient::sampleSpeedTick);
    }
    if (!m_speedSampleTimer->isActive()) {
        m_speedSampleTimer->start();
    }
}

void ClashClient::stopSpeedSampler()
{
    if (m_speedSampleTimer) {
        m_speedSampleTimer->stop();
    }
}

void ClashClient::sampleSpeedTick()
{
    if (!m_speedRunning) {
        return;
    }
    const qint64 now = QDateTime::currentMSecsSinceEpoch();
    const bool before = m_proxyLeg.raw.isEmpty() && m_directLeg.raw.isEmpty();
    takeSpeedSample(m_proxyLeg, now, false);
    takeSpeedSample(m_directLeg, now, false);
    if (m_proxyLeg.downloading && m_proxyLeg.received > 0) {
        m_speedProxyBps = pace(m_proxyLeg.received, m_proxyLeg.markMs);
    }
    if (m_directLeg.downloading && m_directLeg.received > 0) {
        m_speedDirectBps = pace(m_directLeg.received, m_directLeg.markMs);
    }
    if (before && m_proxyLeg.raw.isEmpty() && m_directLeg.raw.isEmpty()) {
        return;
    }
    publishSpeedSeries(false);
}

void ClashClient::finishSpeedTest()
{
    if (!m_speedRunning || !m_proxyLeg.downloadSettled || !m_directLeg.downloadSettled) {
        return;
    }
    stopSpeedSampler();
    m_speedRunning = false;
    m_speedPhase = QStringLiteral("done");
    publishSpeedSeries(true);
}

void ClashClient::resetSpeedTest()
{
    if (m_speedRunning) {
        return;
    }
    stopSpeedSampler();
    ++m_speedToken;
    abortSpeedReply(m_proxyLeg.probe);
    abortSpeedReply(m_proxyLeg.download);
    abortSpeedReply(m_directLeg.probe);
    abortSpeedReply(m_directLeg.download);
    m_proxyLeg = SpeedLeg{};
    m_directLeg = SpeedLeg{};
    m_speedProxyBps = 0;
    m_speedDirectBps = 0;
    m_speedProxyLatencyMs = -1;
    m_speedDirectLatencyMs = -1;
    m_speedProxySamples.clear();
    m_speedDirectSamples.clear();
    m_speedProgress = 0;
    m_speedPhase.clear();
    m_speedError.clear();
    noteSpeed(true);
}

void ClashClient::stopSpeedTest()
{
    const bool busy = m_speedRunning || m_proxyLeg.probe || m_proxyLeg.download || m_directLeg.probe || m_directLeg.download;
    if (!busy) {
        return;
    }
    stopSpeedSampler();
    ++m_speedToken;
    m_speedRunning = false;
    abortSpeedReply(m_proxyLeg.probe);
    abortSpeedReply(m_proxyLeg.download);
    abortSpeedReply(m_directLeg.probe);
    abortSpeedReply(m_directLeg.download);
    m_proxyLeg.downloading = false;
    m_directLeg.downloading = false;
    if (!m_proxyLeg.raw.isEmpty() || !m_directLeg.raw.isEmpty()) {
        m_speedPhase = QStringLiteral("done");
        publishSpeedSeries(true);
        return;
    }
    m_speedPhase = m_speedError.isEmpty() ? QString() : QStringLiteral("done");
    m_speedProgress = 0;
    noteSpeed(true);
}

void ClashClient::startSpeedTest()
{
    if (m_speedRunning) {
        return;
    }
    stopSpeedSampler();
    ++m_speedToken;
    abortSpeedReply(m_proxyLeg.probe);
    abortSpeedReply(m_proxyLeg.download);
    abortSpeedReply(m_directLeg.probe);
    abortSpeedReply(m_directLeg.download);
    m_proxyLeg = SpeedLeg{};
    m_directLeg = SpeedLeg{};
    m_speedRunning = true;
    m_speedProxyBps = 0;
    m_speedDirectBps = 0;
    m_speedProxyLatencyMs = -1;
    m_speedDirectLatencyMs = -1;
    m_speedProxySamples.clear();
    m_speedDirectSamples.clear();
    m_speedProgress = 0;
    m_speedPhase = QStringLiteral("run");
    m_speedError.clear();
    noteSpeed(true);
    beginSpeedProbes();
}

void ClashClient::beginSpeedProbes()
{
    if (!m_speedRunning) {
        return;
    }
    const auto startProbe = [this](SpeedLeg &leg, bool throughProxy) {
        leg.markMs = QDateTime::currentMSecsSinceEpoch();
        const quint64 token = m_speedToken;
        leg.probe = speedNam(throughProxy)->get(speedRequest(QUrl(QStringLiteral("https://speed.cloudflare.com/__down?bytes=%1").arg(kProbeBytes)), 15000));
        connect(leg.probe, &QNetworkReply::readyRead, leg.probe, [reply = leg.probe] {
            reply->readAll();
        });
        connect(leg.probe, &QNetworkReply::finished, this, [this, token, throughProxy] {
            if (token != m_speedToken) {
                return;
            }
            SpeedLeg &current = throughProxy ? m_proxyLeg : m_directLeg;
            auto *reply = current.probe;
            if (!reply) {
                return;
            }
            current.probe = nullptr;
            reply->disconnect(this);
            reply->deleteLater();
            const bool ok = speedOk(reply);
            current.probeSettled = true;
            current.probeOk = ok;
            if (!ok) {
                noteSpeedError(throughProxy ? QStringLiteral("Proxy") : QStringLiteral("Direct"), speedErrorText(reply));
                current.downloadSettled = true;
            } else {
                const int latency = int(qBound(qint64(0), QDateTime::currentMSecsSinceEpoch() - current.markMs, qint64(600000)));
                if (throughProxy) {
                    m_speedProxyLatencyMs = latency;
                } else {
                    m_speedDirectLatencyMs = latency;
                }
            }
            if (!m_proxyLeg.probeSettled || !m_directLeg.probeSettled) {
                noteSpeed(true);
                return;
            }
            if (!m_proxyLeg.probeOk && !m_directLeg.probeOk) {
                finishSpeedTest();
                return;
            }
            beginSpeedDownloads();
        });
    };
    startProbe(m_proxyLeg, true);
    startProbe(m_directLeg, false);
}

void ClashClient::beginSpeedDownloads()
{
    if (!m_speedRunning) {
        return;
    }
    const qint64 now = QDateTime::currentMSecsSinceEpoch();
    if (m_proxyLeg.probeOk) {
        m_proxyLeg.markMs = now;
        beginSpeedDownload(m_proxyLeg);
    }
    if (m_directLeg.probeOk) {
        m_directLeg.markMs = now;
        beginSpeedDownload(m_directLeg);
    }
    startSpeedSampler();
    noteSpeed(true);
}

void ClashClient::beginSpeedDownload(SpeedLeg &leg)
{
    const bool throughProxy = &leg == &m_proxyLeg;
    leg.received = 0;
    leg.sampleBytes = 0;
    leg.sampleAt = leg.markMs;
    leg.downloading = true;
    const quint64 token = m_speedToken;
    const qint64 bytes = speedDownloadBytes();
    const int timeoutMs = qBound(180000, m_speedTestMib * 20000, 900000);
    leg.download = speedNam(throughProxy)->get(speedRequest(QUrl(QStringLiteral("https://speed.cloudflare.com/__down?bytes=%1").arg(bytes)), timeoutMs));
    connect(leg.download, &QNetworkReply::readyRead, this, [this, token, throughProxy, reply = leg.download] {
        const QByteArray chunk = reply->readAll();
        if (token != m_speedToken || chunk.isEmpty()) {
            return;
        }
        SpeedLeg &current = throughProxy ? m_proxyLeg : m_directLeg;
        current.received += chunk.size();
        auto &average = throughProxy ? m_speedProxyBps : m_speedDirectBps;
        average = pace(current.received, current.markMs);
        noteSpeed(false);
    });
    connect(leg.download, &QNetworkReply::finished, this, [this, token, throughProxy] {
        if (token != m_speedToken) {
            return;
        }
        SpeedLeg &current = throughProxy ? m_proxyLeg : m_directLeg;
        auto *reply = current.download;
        if (!reply) {
            return;
        }
        const QByteArray tail = reply->readAll();
        current.download = nullptr;
        reply->disconnect(this);
        reply->deleteLater();
        if (!tail.isEmpty()) {
            current.received += tail.size();
        }
        if (speedOk(reply) && current.received > 0) {
            takeSpeedSample(current, QDateTime::currentMSecsSinceEpoch(), true);
            auto &average = throughProxy ? m_speedProxyBps : m_speedDirectBps;
            average = pace(current.received, current.markMs);
        } else if (!speedOk(reply)) {
            noteSpeedError(throughProxy ? QStringLiteral("Proxy") : QStringLiteral("Direct"), speedErrorText(reply));
            if (current.received > 0) {
                takeSpeedSample(current, QDateTime::currentMSecsSinceEpoch(), true);
            }
        }
        current.downloading = false;
        current.downloadSettled = true;
        finishSpeedTest();
    });
}

void ClashClient::refreshProxyProviders()
{
    if (m_providersPending) {
        return;
    }
    m_providersPending = true;
    auto *watcher = new QDBusPendingCallWatcher(m_iface->asyncCall(QStringLiteral("GetProxyProviders")), this);
    connect(watcher, &QDBusPendingCallWatcher::finished, this, [this](QDBusPendingCallWatcher *call) {
        call->deleteLater();
        m_providersPending = false;
        const QDBusPendingReply<QString> reply = *call;
        if (!reply.isValid()) {
            reportError(reply.error().message());
            return;
        }
        const QVariantList next = jsonArray(reply.value());
        if (next == m_proxyProviders) {
            return;
        }
        m_proxyProviders = next;
        Q_EMIT providersChanged();
    });
}

void ClashClient::updateProxyProvider(const QString &name)
{
    setBusy(true);
    auto *watcher = new QDBusPendingCallWatcher(m_iface->asyncCall(QStringLiteral("UpdateProxyProvider"), name), this);
    connect(watcher, &QDBusPendingCallWatcher::finished, this, [this](QDBusPendingCallWatcher *call) {
        call->deleteLater();
        setBusy(false);
        const QDBusPendingReply<> reply = *call;
        if (!reply.isValid()) {
            reportError(reply.error().message());
            return;
        }
        refreshProxyProviders();
    });
}

void ClashClient::refreshRules()
{
    if (m_rulesPending) {
        return;
    }
    m_rulesPending = true;
    auto *watcher = new QDBusPendingCallWatcher(m_iface->asyncCall(QStringLiteral("GetRuleView")), this);
    connect(watcher, &QDBusPendingCallWatcher::finished, this, [this](QDBusPendingCallWatcher *call) {
        call->deleteLater();
        m_rulesPending = false;
        const QDBusPendingReply<QString> reply = *call;
        if (!reply.isValid()) {
            reportError(reply.error().message());
            return;
        }
        applyRuleView(QJsonDocument::fromJson(reply.value().toUtf8()).object().toVariantMap());
    });
}

void ClashClient::updateRuleProvider(const QString &name)
{
    setBusy(true);
    auto *watcher = new QDBusPendingCallWatcher(m_iface->asyncCall(QStringLiteral("UpdateRuleProvider"), name), this);
    connect(watcher, &QDBusPendingCallWatcher::finished, this, [this](QDBusPendingCallWatcher *call) {
        call->deleteLater();
        setBusy(false);
        const QDBusPendingReply<> reply = *call;
        if (!reply.isValid()) {
            reportError(reply.error().message());
            return;
        }
        refreshRules();
    });
}

void ClashClient::stageRule(int index, const QString &type, const QString &payload, const QString &target, bool enabled)
{
    const QVariantList shown = ownRules();
    if (index < 0 || index >= shown.size()) {
        return;
    }
    const QVariantMap next = ruleMap(type, payload, target.trimmed(), enabled);
    if (m_ruleImporting) {
        if (sameRule(m_ruleImport.at(index).toMap(), next)) {
            return;
        }
        m_ruleImport[index] = next;
        if (sameRules(m_ruleImport, m_savedRules)) {
            m_ruleImporting = false;
            m_ruleImport.clear();
        } else {
            m_preferReset = false;
        }
        Q_EMIT rulesChanged();
        Q_EMIT draftChanged();
        return;
    }
    const QString key = QString::number(index);
    const QVariantMap saved = index < m_savedRules.size() ? m_savedRules.at(index).toMap() : QVariantMap();
    if (sameRule(saved, next)) {
        if (!m_ruleEdits.contains(key)) {
            return;
        }
        m_ruleEdits.remove(key);
    } else if (m_ruleEdits.contains(key) && sameRule(m_ruleEdits.value(key).toMap(), next)) {
        return;
    } else {
        m_preferReset = false;
        m_ruleEdits.insert(key, next);
    }
    Q_EMIT rulesChanged();
    Q_EMIT draftChanged();
}

void ClashClient::stageForkedRule(const QString &type, const QString &payload, const QString &target, bool enabled)
{
    QVariantList base = ownRules();
    base.append(ruleMap(type, payload, target.trimmed(), enabled));
    if (sameRules(base, m_savedRules)) {
        return;
    }
    m_ruleImport = base;
    m_ruleImporting = true;
    m_ruleEdits.clear();
    m_preferReset = false;
    Q_EMIT rulesChanged();
    Q_EMIT draftChanged();
}

bool ClashClient::addOwnRule(const QString &type, const QString &payload, const QString &target)
{
    const QString kind = type.trimmed();
    const QString value = kind == QLatin1String("MATCH") ? QString() : payload.trimmed();
    const QString policy = target.trimmed();
    if (kind != QLatin1String("MATCH") && value.isEmpty()) {
        reportError(QStringLiteral("匹配内容不能为空"));
        return false;
    }
    if (policy.isEmpty()) {
        reportError(QStringLiteral("目标不能为空"));
        return false;
    }
    if (m_ruleImporting) {
        m_ruleImport.append(ruleMap(kind, value, policy, true));
        m_preferReset = false;
        Q_EMIT rulesChanged();
        Q_EMIT draftChanged();
        return true;
    }
    const QDBusReply<void> reply = m_iface->call(QStringLiteral("AddOwnRule"), kind, value, policy);
    if (!reply.isValid()) {
        reportError(reply.error().message());
        return false;
    }
    pullRules();
    return true;
}

void ClashClient::removeOwnRule(int index)
{
    if (m_ruleImporting) {
        if (index < 0 || index >= m_ruleImport.size()) {
            return;
        }
        m_ruleImport.removeAt(index);
        if (sameRules(m_ruleImport, m_savedRules)) {
            m_ruleImporting = false;
            m_ruleImport.clear();
        } else {
            m_preferReset = false;
        }
        Q_EMIT rulesChanged();
        Q_EMIT draftChanged();
        return;
    }
    if (index < 0 || index >= m_savedRules.size()) {
        return;
    }
    const QDBusReply<void> reply = m_iface->call(QStringLiteral("RemoveOwnRule"), static_cast<quint32>(index));
    if (!reply.isValid()) {
        reportError(reply.error().message());
        return;
    }
    QVariantMap shifted;
    for (auto it = m_ruleEdits.cbegin(); it != m_ruleEdits.cend(); ++it) {
        bool ok = false;
        int key = it.key().toInt(&ok);
        if (!ok || key == index) {
            continue;
        }
        if (key > index) {
            key -= 1;
        }
        shifted.insert(QString::number(key), it.value());
    }
    m_ruleEdits = shifted;
    shiftOwnRuleNotes(index);
    pullRules();
    Q_EMIT draftChanged();
}

int ClashClient::openRulesSpreadsheet()
{
    const QDBusReply<QString> csv = m_iface->call(QStringLiteral("RulesCsv"), rulesJson());
    if (!csv.isValid()) {
        reportError(csv.error().message());
        return -1;
    }
    QString program = QStandardPaths::findExecutable(QStringLiteral("libreoffice"));
    if (program.isEmpty()) {
        program = QStandardPaths::findExecutable(QStringLiteral("soffice"));
    }
    QStringList arguments;
    if (!program.isEmpty()) {
        arguments << QStringLiteral("--nologo") << QStringLiteral("--norestore") << QStringLiteral("--calc")
                  << QStringLiteral("--infilter=CSV:44,34,76");
    } else {
        program = QStandardPaths::findExecutable(QStringLiteral("xdg-open"));
        if (program.isEmpty()) {
            return 0;
        }
    }
    discardSheetFile();
    auto *file = new QTemporaryFile(QDir::tempPath() + QStringLiteral("/clash-kde-rules-XXXXXX.csv"));
    file->setAutoRemove(false);
    if (!file->open() || file->write(csv.value().toUtf8()) < 0) {
        delete file;
        reportError(QStringLiteral("无法写入表格"));
        return -1;
    }
    const QString path = file->fileName();
    file->close();
    delete file;
    arguments << path;
    auto *process = new QProcess(this);
    connect(process, &QProcess::finished, process, &QObject::deleteLater);
    process->start(program, arguments);
    if (!process->waitForStarted(3000)) {
        QFile::remove(path);
        process->deleteLater();
        reportError(QStringLiteral("无法打开表格"));
        return -1;
    }
    m_sheetPath = path;
    return 1;
}

QString ClashClient::finishRulesSpreadsheet()
{
    if (m_sheetPath.isEmpty()) {
        return QStringLiteral("无法读取表格");
    }
    QFile file(m_sheetPath);
    if (!file.open(QIODevice::ReadOnly)) {
        return QStringLiteral("无法读取表格");
    }
    const QString text = QString::fromUtf8(file.readAll());
    file.close();
    const QDBusReply<QString> parsed = m_iface->call(QStringLiteral("ParseRulesCsv"), text);
    if (!parsed.isValid()) {
        return parsed.error().message();
    }
    const QVariantList incoming = jsonArray(parsed.value());
    discardSheetFile();
    if (sameRules(incoming, ownRules()) || sameRules(incoming, m_savedRules)) {
        if (sameRules(incoming, m_savedRules)) {
            clearRuleDraft();
            Q_EMIT rulesChanged();
            Q_EMIT draftChanged();
        }
        return QStringLiteral("same");
    }
    m_ruleImport = incoming;
    m_ruleImporting = true;
    m_ruleEdits.clear();
    m_preferReset = false;
    Q_EMIT rulesChanged();
    Q_EMIT draftChanged();
    Q_EMIT ruleDraftDiscarded();
    return {};
}

void ClashClient::cancelRulesSpreadsheet()
{
    discardSheetFile();
}

void ClashClient::reload()
{
    refreshStatus();
    refreshProfiles();
    pullNotes();
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
    m_draftCompact = m_compact;
    m_draftShowWidgetHint = m_showWidgetHint;
    m_draftSpeedTestMib = m_speedTestMib;
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
        && m_draftCompact == m_compact
        && m_draftShowWidgetHint == m_showWidgetHint
        && m_draftSpeedTestMib == m_speedTestMib
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
    m_routeEdits.clear();
    m_routeBaselines.clear();
    m_routeProfile.clear();
    clearRuleDraft();
    m_noteEdits.clear();
    copyLiveToDraft();
    Q_EMIT draftChanged();
    Q_EMIT routeDraftDiscarded();
    Q_EMIT rulesChanged();
    Q_EMIT ruleDraftDiscarded();
    Q_EMIT notesChanged();
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
    m_draftCompact = false;
    m_draftShowWidgetHint = true;
    m_draftSpeedTestMib = 20;
    m_draftTunDevice.clear();
    m_draftOverridePath = m_overrideDefault;
    m_draftAllowLan = false;
    m_draftLanUsername.clear();
    m_draftLanPassword.clear();
    m_draftActiveProfile = m_activeProfile;
    m_draftSelections = m_selections;
    m_routeEdits.clear();
    m_routeBaselines.clear();
    m_routeProfile.clear();
    clearRuleDraft();
    m_noteEdits.clear();
    m_draftDirty = !draftMatchesLive();
    m_preferReset = true;
    Q_EMIT draftChanged();
    Q_EMIT routeDraftDiscarded();
    Q_EMIT rulesChanged();
    Q_EMIT ruleDraftDiscarded();
    Q_EMIT notesChanged();
}

bool ClashClient::draftIsDefaults() const
{
    return m_draftMixedPort == 7890 && m_draftControllerPort == 9090 && m_draftMihomoPath == QStringLiteral("mihomo")
        && !m_draftCoreDesired && !m_draftSystemProxyEnabled && !m_draftTunEnabled && m_draftMode == QStringLiteral("rule")
        && !m_draftSortGlobal && !m_draftAutoTestDelay && !m_draftBlurIp && !m_draftCompact
        && m_draftShowWidgetHint
        && m_draftSpeedTestMib == 20
        && m_draftTunDevice.trimmed().isEmpty()
        && canonicalOverride(m_draftOverridePath, m_overrideDefault) == m_overrideDefault
        && !m_draftAllowLan && m_draftLanUsername.isEmpty() && m_draftLanPassword.isEmpty()
        && m_routeEdits.isEmpty()
        && m_noteEdits.isEmpty()
        && !ruleDirty();
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
        m_routeEdits.clear();
        m_routeBaselines.clear();
        m_routeProfile.clear();
        clearRuleDraft();
        Q_EMIT rulesChanged();
        Q_EMIT ruleDraftDiscarded();
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
    if (ruleDirty()) {
        const QVariantList written = ownRules();
        const QDBusReply<void> rulesReply = m_iface->call(QStringLiteral("SetOwnRules"), rulesJson());
        if (!rulesReply.isValid()) {
            return failed(rulesReply.error());
        }
        m_savedRules = written;
        clearRuleDraft();
        Q_EMIT rulesChanged();
        Q_EMIT ruleDraftDiscarded();
    }
    const QString routeProfile = m_routeProfile;
    const QVariantMap routeEdits = m_routeEdits;
    QVariantMap notes = m_savedNotes;
    for (auto it = m_noteEdits.cbegin(); it != m_noteEdits.cend(); ++it) {
        notes.insert(it.key(), it.value());
    }
    bool notesMoved = false;
    for (auto it = routeEdits.cbegin(); it != routeEdits.cend(); ++it) {
        const QVariantMap edit = it.value().toMap();
        const int port = edit.value(QStringLiteral("port")).toInt();
        if (port < 1 || port > 65535) {
            reportError(QStringLiteral("端口无效"));
            return false;
        }
        const QString renamed = edit.value(QStringLiteral("name")).toString();
        const QDBusReply<void> reply = m_iface->call(QStringLiteral("UpdateRoute"),
                                                     routeProfile,
                                                     it.key(),
                                                     renamed,
                                                     edit.value(QStringLiteral("kind")).toString(),
                                                     edit.value(QStringLiteral("server")).toString(),
                                                     static_cast<quint16>(port),
                                                     edit.value(QStringLiteral("secret")).toString());
        if (!reply.isValid()) {
            return failed(reply.error());
        }
        if (renamed != it.key()) {
            const QString oldKey = QStringLiteral("route\n%1\n%2").arg(routeProfile, it.key());
            const QString newKey = QStringLiteral("route\n%1\n%2").arg(routeProfile, renamed);
            if (notes.contains(oldKey)) {
                notes.insert(newKey, notes.value(oldKey));
                notes.remove(oldKey);
                notesMoved = true;
            }
        }
        m_routeEdits.remove(it.key());
        m_routeBaselines.remove(it.key());
    }
    if (!m_noteEdits.isEmpty() || notesMoved) {
        QVariantMap clean;
        for (auto it = notes.cbegin(); it != notes.cend(); ++it) {
            const QString text = it.value().toString().trimmed();
            if (!text.isEmpty()) {
                clean.insert(it.key(), text);
            }
        }
        const QString body = QString::fromUtf8(QJsonDocument(QJsonObject::fromVariantMap(clean)).toJson(QJsonDocument::Compact));
        const QDBusReply<void> notesReply = m_iface->call(QStringLiteral("SetNotes"), body);
        if (!notesReply.isValid()) {
            return failed(notesReply.error());
        }
        m_savedNotes = clean;
        m_noteEdits.clear();
        Q_EMIT notesChanged();
    }
    if (m_routeEdits.isEmpty()) {
        m_routeProfile.clear();
        m_routeBaselines.clear();
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
    if (m_draftCompact != m_compact) {
        const QDBusReply<void> reply = m_iface->call(QStringLiteral("SetCompact"), m_draftCompact);
        if (!reply.isValid()) {
            return failed(reply.error());
        }
    }
    if (m_draftSortGlobal != m_sortGlobal || m_draftAutoTestDelay != m_autoTestDelay || m_draftBlurIp != m_blurIp || m_draftShowWidgetHint != m_showWidgetHint
        || m_draftSpeedTestMib != m_speedTestMib) {
        const QDBusReply<void> reply = m_iface->call(QStringLiteral("SetBehavior"), m_draftSortGlobal, m_draftAutoTestDelay, m_draftBlurIp, m_draftShowWidgetHint, m_draftSpeedTestMib);
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
    const bool compact = status.value(QStringLiteral("compact")).toBool();
    const bool showWidgetHint = !status.contains(QStringLiteral("show_widget_hint")) || status.value(QStringLiteral("show_widget_hint")).toBool();
    const int speedTestMib = status.contains(QStringLiteral("speed_test_mib")) ? qBound(1, status.value(QStringLiteral("speed_test_mib")).toInt(), 100) : 20;
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
        && m_autoTestDelay == autoTestDelay && m_blurIp == blurIp && m_compact == compact
        && m_showWidgetHint == showWidgetHint && m_speedTestMib == speedTestMib && m_allowLan == allowLan
        && m_lanUsername == lanUsername && m_lanPassword == lanPassword && m_lanAllowed == lanAllowed
        && m_message == message) {
        if (!coreRunning && !m_connections.isEmpty()) {
            m_connections.clear();
            Q_EMIT connectionsChanged();
        }
        return;
    }
    m_coreDesired = coreDesired;
    m_coreRunning = coreRunning;
    if (!m_coreRunning && !m_connections.isEmpty()) {
        m_connections.clear();
        Q_EMIT connectionsChanged();
    }
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
    m_compact = compact;
    m_showWidgetHint = showWidgetHint;
    m_speedTestMib = speedTestMib;
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
