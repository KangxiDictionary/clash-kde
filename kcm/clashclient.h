// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 clash-kde contributors

#pragma once

#include <QList>
#include <QStringList>
#include <QObject>
#include <QVariantList>
#include <QVariantMap>

class QDBusInterface;
class QJsonObject;
class QProcess;

class ClashClient : public QObject
{
    Q_OBJECT
    Q_PROPERTY(bool coreDesired READ coreDesired NOTIFY statusChanged)
    Q_PROPERTY(bool coreRunning READ coreRunning NOTIFY statusChanged)
    Q_PROPERTY(bool systemProxyEnabled READ systemProxyEnabled NOTIFY statusChanged)
    Q_PROPERTY(bool systemProxyApplied READ systemProxyApplied NOTIFY statusChanged)
    Q_PROPERTY(bool tunEnabled READ tunEnabled NOTIFY statusChanged)
    Q_PROPERTY(bool tunMachine READ tunMachine NOTIFY statusChanged)
    Q_PROPERTY(bool tunRouteReady READ tunRouteReady NOTIFY statusChanged)
    Q_PROPERTY(bool tunRouteKnown READ tunRouteKnown NOTIFY statusChanged)
    Q_PROPERTY(QString tunRouteDetail READ tunRouteDetail NOTIFY statusChanged)
    Q_PROPERTY(qlonglong tunRouteCheckedAt READ tunRouteCheckedAt NOTIFY statusChanged)
    Q_PROPERTY(QString kdeProxy READ kdeProxy NOTIFY statusChanged)
    Q_PROPERTY(QString gnomeProxy READ gnomeProxy NOTIFY statusChanged)
    Q_PROPERTY(bool accountsUnlocked READ accountsUnlocked NOTIFY accountsChanged)
    Q_PROPERTY(bool firewallActive READ firewallActive NOTIFY statusChanged)
    Q_PROPERTY(QString mode READ mode NOTIFY statusChanged)
    Q_PROPERTY(QString listenAddress READ listenAddress NOTIFY statusChanged)
    Q_PROPERTY(int mixedPort READ mixedPort NOTIFY statusChanged)
    Q_PROPERTY(int controllerPort READ controllerPort NOTIFY statusChanged)
    Q_PROPERTY(QString mihomoPath READ mihomoPath NOTIFY statusChanged)
    Q_PROPERTY(QString overridePath READ overridePath NOTIFY statusChanged)
    Q_PROPERTY(qulonglong uploadBps READ uploadBps NOTIFY statusChanged)
    Q_PROPERTY(qulonglong downloadBps READ downloadBps NOTIFY statusChanged)
    Q_PROPERTY(QString exitRegion READ exitRegion NOTIFY statusChanged)
    Q_PROPERTY(QString exitRaw READ exitRaw NOTIFY statusChanged)
    Q_PROPERTY(QString activeProfile READ activeProfile NOTIFY statusChanged)
    Q_PROPERTY(QString message READ message NOTIFY statusChanged)
    Q_PROPERTY(QString messageLogTail READ messageLogTail NOTIFY statusChanged)
    Q_PROPERTY(QString phase READ phase NOTIFY statusChanged)
    Q_PROPERTY(bool busy READ busy NOTIFY busyChanged)
    Q_PROPERTY(bool delayTesting READ delayTesting NOTIFY delayTestingChanged)
    Q_PROPERTY(QVariantList profiles READ profiles NOTIFY profilesChanged)
    Q_PROPERTY(QVariantList groups READ groups NOTIFY proxiesChanged)
    Q_PROPERTY(QVariantList connections READ connections NOTIFY connectionsChanged)
    Q_PROPERTY(bool speedRunning READ speedRunning NOTIFY speedChanged)
    Q_PROPERTY(qulonglong speedProxyBps READ speedProxyBps NOTIFY speedChanged)
    Q_PROPERTY(qulonglong speedDirectBps READ speedDirectBps NOTIFY speedChanged)
    Q_PROPERTY(int speedProxyLatencyMs READ speedProxyLatencyMs NOTIFY speedChanged)
    Q_PROPERTY(int speedDirectLatencyMs READ speedDirectLatencyMs NOTIFY speedChanged)
    Q_PROPERTY(QVariantList speedProxySamples READ speedProxySamples NOTIFY speedChanged)
    Q_PROPERTY(QVariantList speedDirectSamples READ speedDirectSamples NOTIFY speedChanged)
    Q_PROPERTY(qreal speedProgress READ speedProgress NOTIFY speedChanged)
    Q_PROPERTY(QString speedPhase READ speedPhase NOTIFY speedChanged)
    Q_PROPERTY(QString speedError READ speedError NOTIFY speedChanged)
    Q_PROPERTY(int speedTestMib READ speedTestMib NOTIFY statusChanged)
    Q_PROPERTY(QVariantList proxyProviders READ proxyProviders NOTIFY providersChanged)
    Q_PROPERTY(QVariantList ownRules READ ownRules NOTIFY rulesChanged)
    Q_PROPERTY(QVariantList profileRules READ profileRules NOTIFY rulesChanged)
    Q_PROPERTY(QVariantList ruleProviders READ ruleProviders NOTIFY rulesChanged)
    Q_PROPERTY(QVariantList ruleTargets READ ruleTargets NOTIFY rulesChanged)
    Q_PROPERTY(QVariantList accounts READ accounts NOTIFY accountsChanged)
    Q_PROPERTY(QString accountsError READ accountsError NOTIFY accountsChanged)
    Q_PROPERTY(qulonglong ownUid READ ownUid CONSTANT)
    Q_PROPERTY(int draftMixedPort READ draftMixedPort WRITE setDraftMixedPort NOTIFY draftChanged)
    Q_PROPERTY(int draftControllerPort READ draftControllerPort WRITE setDraftControllerPort NOTIFY draftChanged)
    Q_PROPERTY(QString draftMihomoPath READ draftMihomoPath WRITE setDraftMihomoPath NOTIFY draftChanged)
    Q_PROPERTY(bool draftCoreDesired READ draftCoreDesired WRITE setDraftCoreDesired NOTIFY draftChanged)
    Q_PROPERTY(bool draftSystemProxyEnabled READ draftSystemProxyEnabled WRITE setDraftSystemProxyEnabled NOTIFY draftChanged)
    Q_PROPERTY(bool draftTunEnabled READ draftTunEnabled WRITE setDraftTunEnabled NOTIFY draftChanged)
    Q_PROPERTY(QString draftMode READ draftMode WRITE setDraftMode NOTIFY draftChanged)
    Q_PROPERTY(QString draftActiveProfile READ draftActiveProfile WRITE setDraftActiveProfile NOTIFY draftChanged)
    Q_PROPERTY(QVariantMap draftSelections READ draftSelections WRITE setDraftSelections NOTIFY draftChanged)
    Q_PROPERTY(bool draftSortGlobal READ draftSortGlobal WRITE setDraftSortGlobal NOTIFY draftChanged)
    Q_PROPERTY(bool draftAutoTestDelay READ draftAutoTestDelay WRITE setDraftAutoTestDelay NOTIFY draftChanged)
    Q_PROPERTY(bool draftBlurIp READ draftBlurIp WRITE setDraftBlurIp NOTIFY draftChanged)
    Q_PROPERTY(bool draftCompact READ draftCompact WRITE setDraftCompact NOTIFY draftChanged)
    Q_PROPERTY(bool draftShowWidgetHint READ draftShowWidgetHint WRITE setDraftShowWidgetHint NOTIFY draftChanged)
    Q_PROPERTY(int draftSpeedTestMib READ draftSpeedTestMib WRITE setDraftSpeedTestMib NOTIFY draftChanged)
    Q_PROPERTY(QString draftTunDevice READ draftTunDevice WRITE setDraftTunDevice NOTIFY draftChanged)
    Q_PROPERTY(QString draftOverridePath READ draftOverridePath WRITE setDraftOverridePath NOTIFY draftChanged)
    Q_PROPERTY(bool sortGlobal READ sortGlobal NOTIFY statusChanged)
    Q_PROPERTY(QString sortMode READ sortMode NOTIFY statusChanged)
    Q_PROPERTY(bool showUnreachable READ showUnreachable NOTIFY statusChanged)
    Q_PROPERTY(QVariantMap sortPrefs READ sortPrefs NOTIFY statusChanged)
    Q_PROPERTY(bool autoTestDelay READ autoTestDelay NOTIFY statusChanged)
    Q_PROPERTY(bool blurIp READ blurIp NOTIFY statusChanged)
    Q_PROPERTY(bool compact READ compact NOTIFY statusChanged)
    Q_PROPERTY(bool showWidgetHint READ showWidgetHint NOTIFY statusChanged)
    Q_PROPERTY(bool allowLan READ allowLan NOTIFY statusChanged)
    Q_PROPERTY(QString lanUsername READ lanUsername NOTIFY statusChanged)
    Q_PROPERTY(QString lanPassword READ lanPassword NOTIFY statusChanged)
    Q_PROPERTY(bool lanAllowed READ lanAllowed NOTIFY statusChanged)
    Q_PROPERTY(bool draftAllowLan READ draftAllowLan WRITE setDraftAllowLan NOTIFY draftChanged)
    Q_PROPERTY(QString draftLanUsername READ draftLanUsername WRITE setDraftLanUsername NOTIFY draftChanged)
    Q_PROPERTY(QString draftLanPassword READ draftLanPassword WRITE setDraftLanPassword NOTIFY draftChanged)
    Q_PROPERTY(bool machineLanAllowed READ machineLanAllowed NOTIFY accountsChanged)
    Q_PROPERTY(QVariantMap lanAccounts READ lanAccounts NOTIFY accountsChanged)
    Q_PROPERTY(QString hint READ hint NOTIFY hintChanged)
    Q_PROPERTY(bool settingsDirty READ settingsDirty NOTIFY draftChanged)

public:
    explicit ClashClient(QObject *parent = nullptr);

    bool coreDesired() const;
    bool coreRunning() const;
    bool systemProxyEnabled() const;
    bool systemProxyApplied() const;
    bool tunEnabled() const;
    bool tunMachine() const;
    bool tunRouteReady() const;
    bool tunRouteKnown() const;
    QString tunRouteDetail() const;
    qlonglong tunRouteCheckedAt() const;
    QString kdeProxy() const;
    QString gnomeProxy() const;
    bool accountsUnlocked() const;
    bool firewallActive() const;
    QString mode() const;
    QString listenAddress() const;
    int mixedPort() const;
    int controllerPort() const;
    QString mihomoPath() const;
    QString overridePath() const;
    qulonglong uploadBps() const;
    qulonglong downloadBps() const;
    QString exitRegion() const;
    QString exitRaw() const;
    QString activeProfile() const;
    QString message() const;
    QString messageLogTail() const;
    QString phase() const;
    bool busy() const;
    bool delayTesting() const;
    QVariantList profiles() const;
    QVariantList groups() const;
    QVariantList connections() const;
    bool speedRunning() const { return m_speedRunning; }
    qulonglong speedProxyBps() const { return m_speedProxyBps; }
    qulonglong speedDirectBps() const { return m_speedDirectBps; }
    int speedProxyLatencyMs() const { return m_speedProxyLatencyMs; }
    int speedDirectLatencyMs() const { return m_speedDirectLatencyMs; }
    QVariantList speedProxySamples() const { return m_speedProxySamples; }
    QVariantList speedDirectSamples() const { return m_speedDirectSamples; }
    qreal speedProgress() const { return m_speedProgress; }
    QString speedPhase() const { return m_speedPhase; }
    QString speedError() const { return m_speedError; }
    int speedTestMib() const { return m_speedTestMib; }
    QVariantList proxyProviders() const;
    QVariantList ownRules() const;
    QVariantList profileRules() const;
    QVariantList ruleProviders() const;
    QVariantList ruleTargets() const;
    QVariantList accounts() const;
    QString accountsError() const;
    qulonglong ownUid() const;

    int draftMixedPort() const;
    void setDraftMixedPort(int port);
    int draftControllerPort() const;
    void setDraftControllerPort(int port);
    QString draftMihomoPath() const;
    void setDraftMihomoPath(const QString &path);
    bool draftCoreDesired() const;
    void setDraftCoreDesired(bool desired);
    bool draftSystemProxyEnabled() const;
    void setDraftSystemProxyEnabled(bool enabled);
    bool draftTunEnabled() const;
    void setDraftTunEnabled(bool enabled);
    QString draftMode() const;
    void setDraftMode(const QString &mode);
    QString draftActiveProfile() const;
    void setDraftActiveProfile(const QString &id);
    QVariantMap draftSelections() const;
    void setDraftSelections(const QVariantMap &selections);
    bool draftSortGlobal() const;
    void setDraftSortGlobal(bool global);
    bool draftAutoTestDelay() const;
    void setDraftAutoTestDelay(bool enabled);
    bool draftBlurIp() const;
    void setDraftBlurIp(bool enabled);
    bool draftCompact() const;
    void setDraftCompact(bool enabled);
    bool draftShowWidgetHint() const;
    void setDraftShowWidgetHint(bool enabled);
    int draftSpeedTestMib() const;
    void setDraftSpeedTestMib(int mib);
    QString draftTunDevice() const;
    void setDraftTunDevice(const QString &name);
    QString draftOverridePath() const;
    void setDraftOverridePath(const QString &path);
    bool sortGlobal() const;
    QString sortMode() const;
    bool showUnreachable() const;
    QVariantMap sortPrefs() const;
    bool autoTestDelay() const;
    bool blurIp() const;
    bool compact() const;
    bool showWidgetHint() const;
    bool allowLan() const;
    QString lanUsername() const;
    QString lanPassword() const;
    bool lanAllowed() const;
    bool draftAllowLan() const;
    void setDraftAllowLan(bool enabled);
    QString draftLanUsername() const;
    void setDraftLanUsername(const QString &username);
    QString draftLanPassword() const;
    void setDraftLanPassword(const QString &password);
    bool machineLanAllowed() const;
    QVariantMap lanAccounts() const;
    QString hint() const;
    bool settingsDirty() const;

    Q_INVOKABLE void setCoreDesired(bool desired);
    Q_INVOKABLE void restartCore();
    Q_INVOKABLE void setSystemProxyEnabled(bool enabled);
    Q_INVOKABLE void setTunEnabled(bool enabled);
    Q_INVOKABLE void setMachineTun(bool enabled);
    Q_INVOKABLE void lockAccounts();
    Q_INVOKABLE QString formatExit(const QString &raw) const;
    Q_INVOKABLE QString presentExit(const QString &raw, bool blur) const;
    Q_INVOKABLE QString presentPlace(const QString &raw) const;
    Q_INVOKABLE QString presentAddress(const QString &raw, bool blur) const;
    Q_INVOKABLE QString presentHost(const QString &host, bool blur) const;
    Q_INVOKABLE QString addressOf(const QString &raw) const;
    Q_INVOKABLE void ensureProxies();
    Q_INVOKABLE void copyText(const QString &text) const;
    Q_INVOKABLE QString i18n(const QString &text, const QVariantList &args) const;
    Q_INVOKABLE QString i18nc(const QString &context, const QString &text, const QVariantList &args) const;
    Q_INVOKABLE void showHint(const QString &text);
    Q_INVOKABLE void clearHint();
    Q_INVOKABLE void reloadAccounts();
    Q_INVOKABLE void startAccount(qulonglong uid);
    Q_INVOKABLE void stopAccount(qulonglong uid);
    Q_INVOKABLE void setMode(const QString &mode);
    Q_INVOKABLE void setActiveProfile(const QString &id);
    Q_INVOKABLE void addSubscription(const QString &name, const QString &url);
    Q_INVOKABLE void removeProfile(const QString &id);
    Q_INVOKABLE void updateProfile(const QString &id);
    Q_INVOKABLE void selectProxy(const QString &group, const QString &name);
    Q_INVOKABLE void setViewPref(const QString &profile, const QString &group, const QString &mode, bool showUnreachable, bool global);
    Q_INVOKABLE QVariantList profileGroups(const QString &id);
    Q_INVOKABLE void testGroupDelay(const QString &group);
    Q_INVOKABLE QString profileBody(const QString &id);
    Q_INVOKABLE QString checkProfileText(const QString &body);
    Q_INVOKABLE QString coreLog() const;
    Q_INVOKABLE QString serviceLog() const;
    Q_INVOKABLE void refreshDiagnostics();
    Q_INVOKABLE void restoreSystemProxy();
    Q_INVOKABLE void openLogFolder();
    Q_INVOKABLE bool openBodyInEditor(const QString &body);
    Q_INVOKABLE void setProfileBody(const QString &id, const QString &body);
    Q_INVOKABLE void setProfileMeta(const QString &id, const QString &name, const QString &url);
    Q_INVOKABLE void importProfile(const QString &name, const QString &fileUrl);
    Q_INVOKABLE void refreshExitRegion();
    Q_INVOKABLE void setCompact(bool enabled);
    Q_INVOKABLE QVariantMap routeDetail(const QString &name);
    Q_INVOKABLE void stageRoute(const QString &previous, const QString &name, const QString &kind, const QString &server, int port, const QString &secret);
    Q_INVOKABLE bool restoreRoute(const QString &name);
    Q_INVOKABLE bool restoreProfileRule(int index);
    Q_INVOKABLE QString noteFor(const QString &key) const;
    Q_INVOKABLE void stageNote(const QString &key, const QString &text);
    Q_INVOKABLE void resetSpeedTest();
    Q_INVOKABLE QVariantMap stagedRoute(const QString &previous) const;
    Q_INVOKABLE bool addRoute(const QString &group, const QString &name, const QString &kind, const QString &server, int port, const QString &secret);
    Q_INVOKABLE void removeRoute(const QString &name);
    Q_INVOKABLE bool saveRoute(const QString &previous, const QString &name, const QString &kind, const QString &server, int port, const QString &secret);
    Q_INVOKABLE void setMachineLan(bool enabled);
    Q_INVOKABLE void setAccountLan(qulonglong uid, bool enabled);
    Q_INVOKABLE void openOverrideFile(const QString &path);
    Q_INVOKABLE void openOverrideFolder(const QString &path);
    Q_INVOKABLE void refreshConnections();
    Q_INVOKABLE void startSpeedTest();
    Q_INVOKABLE void stopSpeedTest();
    Q_INVOKABLE void closeConnection(const QString &id);
    Q_INVOKABLE void closeAllConnections();
    Q_INVOKABLE void refreshProxyProviders();
    Q_INVOKABLE void updateProxyProvider(const QString &name);
    Q_INVOKABLE void refreshRules();
    Q_INVOKABLE void updateRuleProvider(const QString &name);
    Q_INVOKABLE void stageRule(int index, const QString &type, const QString &payload, const QString &target, bool enabled);
    Q_INVOKABLE void stageForkedRule(const QString &type, const QString &payload, const QString &target, bool enabled);
    Q_INVOKABLE bool addOwnRule(const QString &type, const QString &payload, const QString &target);
    Q_INVOKABLE void removeOwnRule(int index);
    Q_INVOKABLE int openRulesSpreadsheet();
    Q_INVOKABLE QString finishRulesSpreadsheet();
    Q_INVOKABLE void cancelRulesSpreadsheet();
    Q_INVOKABLE void reload();
    bool commitSettings();
    void revertDraft();
    void applyDefaultsToDraft();
    Q_INVOKABLE bool draftIsDefaults() const;

Q_SIGNALS:
    void statusChanged();
    void profilesChanged();
    void proxiesChanged();
    void accountsChanged();
    void busyChanged();
    void delayTestingChanged();
    void delayUpdated(const QString &group, const QString &name, qlonglong delayMs);
    void draftChanged();
    void routeDraftDiscarded();
    void ruleDraftDiscarded();
    void connectionsChanged();
    void speedChanged();
    void notesChanged();
    void providersChanged();
    void rulesChanged();
    void hintChanged();
    void errorOccurred(const QString &message);
    void editorTextReady(const QString &body);
    void diagnosticsReady(const QString &text);
    void settingsApplied();
    void settingsFailed(const QString &message);

private Q_SLOTS:
    void refreshStatus();
    void refreshProfiles();
    void refreshProxies();
    void noteProxiesChanged();
    void onDelayUpdated(const QString &group, const QString &name, qlonglong delayMs);

private:
    void setBusy(bool busy);
    void setDelayTesting(bool testing);
    void applyStatus(const QVariantMap &status);
    void copyLiveToDraft();
    bool draftMatchesLive() const;
    void refreshFirewall();
    void overlayCurrentAccount();
    void reportError(const QString &message);
    QString prepareOverride(const QString &path);
    bool ruleDirty() const;
    QString rulesJson() const;
    void clearRuleDraft();
    void pullRules();
    void applyRuleView(const QVariantMap &view);
    void discardSheetFile();
    struct SpeedLeg {
        class QNetworkReply *probe = nullptr;
        class QNetworkReply *download = nullptr;
        qint64 markMs = 0;
        qint64 received = 0;
        qint64 sampleBytes = 0;
        qint64 sampleAt = 0;
        bool probeSettled = false;
        bool probeOk = false;
        bool downloading = false;
        bool downloadSettled = false;
        QList<double> raw;
    };

    void beginSpeedProbes();
    void beginSpeedDownloads();
    void beginSpeedDownload(SpeedLeg &leg);
    void takeSpeedSample(SpeedLeg &leg, qint64 now, bool force);
    void startSpeedSampler();
    void stopSpeedSampler();
    void sampleSpeedTick();
    void finishSpeedTest();
    void abortSpeedReply(class QNetworkReply *&reply);
    void publishSpeedSeries(bool force);
    void noteSpeedError(const QString &which, const QString &message);
    qint64 speedDownloadBytes() const;
    void noteSpeed(bool force = false);
    void pullNotes();
    void shiftOwnRuleNotes(int removed);
    class QNetworkAccessManager *speedNam(bool throughProxy);
    QString buildDiagnostics(const QString &version, const QString &systemState, const QString &journal) const;
    QString translateServiceError(const QString &message) const;
    QString translateCode(const QString &code, const QJsonObject &params, const QString &detail) const;
    void startDiagnosticsStage(quint64 generation);
    void startDiagnosticsProcess(quint64 generation, const QString &program, const QStringList &args, int timeoutMs);
    void finishDiagnosticsStage(quint64 generation, const QString &output);

    void asyncCall(const QString &method, const QVariantList &args, bool refreshProfilesAfter, bool refreshProxiesAfter, bool markBusy = true);
    void accountCall(const QString &method, qulonglong uid);
    void runRestoreProxyCommand();

    QDBusInterface *m_iface = nullptr;
    bool m_coreDesired = false;
    bool m_coreRunning = false;
    bool m_systemProxyEnabled = false;
    bool m_systemProxyApplied = false;
    bool m_tunEnabled = false;
    bool m_tunMachine = false;
    bool m_tunRouteReady = true;
    bool m_tunRouteKnown = true;
    QString m_tunRouteDetail;
    qlonglong m_tunRouteCheckedAt = 0;
    QString m_kdeProxy = QStringLiteral("off");
    QString m_gnomeProxy = QStringLiteral("off");
    bool m_accountsUnlocked = false;
    bool m_firewallActive = false;
    qint64 m_firewallCheckedAt = 0;
    bool m_firewallBusy = false;
    bool m_tunPending = false;
    bool m_startPending = false;
    QProcess *m_diagnosticsProcess = nullptr;
    quint64 m_diagnosticsGeneration = 0;
    int m_diagnosticsStage = 0;
    QString m_diagnosticsResolved;
    QString m_diagnosticsVersion;
    QString m_diagnosticsSystem;
    QString m_diagnosticsJournal;
    bool m_statusPending = false;
    bool m_profilesPending = false;
    bool m_proxiesPending = false;
    bool m_proxiesWanted = false;
    bool m_proxiesLoaded = false;
    bool m_proxiesStale = true;
    QString m_mode = QStringLiteral("rule");
    QString m_listenHost = QStringLiteral("127.0.0.1");
    int m_mixedPort = 7890;
    int m_controllerPort = 9090;
    QString m_mihomoPath = QStringLiteral("mihomo");
    QString m_overridePath;
    qulonglong m_uploadBps = 0;
    qulonglong m_downloadBps = 0;
    QString m_exitRegion;
    QString m_activeProfile;
    QString m_message;
    QString m_messageLogTail;
    QString m_phase = QStringLiteral("stopped");
    bool m_busy = false;
    bool m_delayTesting = false;
    bool m_draftDirty = false;
    bool m_preferReset = false;
    int m_draftMixedPort = 7890;
    int m_draftControllerPort = 9090;
    QString m_draftMihomoPath = QStringLiteral("mihomo");
    bool m_draftCoreDesired = false;
    bool m_draftSystemProxyEnabled = false;
    bool m_draftTunEnabled = false;
    QString m_draftMode = QStringLiteral("rule");
    QString m_draftActiveProfile;
    QVariantMap m_draftSelections;
    bool m_draftSortGlobal = false;
    bool m_draftAutoTestDelay = false;
    bool m_draftBlurIp = false;
    bool m_draftCompact = false;
    bool m_draftShowWidgetHint = true;
    int m_draftSpeedTestMib = 20;
    QString m_draftTunDevice;
    QString m_draftOverridePath;
    QString m_tunDevice;
    QString m_overrideDefault;
    QVariantMap m_selections;
    bool m_sortGlobal = false;
    QString m_sortMode = QStringLiteral("profile");
    bool m_showUnreachable = true;
    QVariantMap m_sortPrefs;
    bool m_autoTestDelay = false;
    bool m_blurIp = false;
    bool m_compact = false;
    bool m_showWidgetHint = true;
    int m_speedTestMib = 20;
    QString m_routeProfile;
    QVariantMap m_routeEdits;
    QVariantMap m_routeBaselines;
    QVariantMap m_savedNotes;
    QVariantMap m_noteEdits;
    QVariantList m_connections;
    bool m_connectionsPending = false;
    class QNetworkAccessManager *m_directNam = nullptr;
    class QNetworkAccessManager *m_proxyNam = nullptr;
    class QTimer *m_speedSampleTimer = nullptr;
    SpeedLeg m_proxyLeg;
    SpeedLeg m_directLeg;
    bool m_speedRunning = false;
    qulonglong m_speedProxyBps = 0;
    qulonglong m_speedDirectBps = 0;
    int m_speedProxyLatencyMs = -1;
    int m_speedDirectLatencyMs = -1;
    QVariantList m_speedProxySamples;
    QVariantList m_speedDirectSamples;
    qreal m_speedProgress = 0;
    QString m_speedPhase;
    QString m_speedError;
    qint64 m_speedNotedMs = 0;
    quint64 m_speedToken = 0;
    QVariantList m_proxyProviders;
    bool m_providersPending = false;
    QVariantList m_savedRules;
    QVariantList m_ruleImport;
    bool m_ruleImporting = false;
    QVariantMap m_ruleEdits;
    QVariantList m_profileRules;
    QVariantList m_ruleProviders;
    QVariantList m_ruleTargets;
    bool m_rulesPending = false;
    QString m_sheetPath;
    bool m_allowLan = false;
    QString m_lanUsername;
    QString m_lanPassword;
    bool m_lanAllowed = true;
    bool m_draftAllowLan = false;
    QString m_draftLanUsername;
    QString m_draftLanPassword;
    bool m_machineLanAllowed = true;
    QVariantMap m_lanAccounts;
    QString m_hint;
    QVariantList m_profiles;
    QVariantList m_groups;
    QVariantList m_accounts;
    QString m_accountsError;
    QDBusInterface *m_admin = nullptr;
};
