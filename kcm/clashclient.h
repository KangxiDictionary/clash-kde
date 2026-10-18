// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 clash-kde contributors

#pragma once

#include <QObject>
#include <QVariantList>
#include <QVariantMap>

class QDBusInterface;

class ClashClient : public QObject
{
    Q_OBJECT
    Q_PROPERTY(bool coreDesired READ coreDesired NOTIFY statusChanged)
    Q_PROPERTY(bool coreRunning READ coreRunning NOTIFY statusChanged)
    Q_PROPERTY(bool systemProxyEnabled READ systemProxyEnabled NOTIFY statusChanged)
    Q_PROPERTY(bool systemProxyApplied READ systemProxyApplied NOTIFY statusChanged)
    Q_PROPERTY(bool tunEnabled READ tunEnabled NOTIFY statusChanged)
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
    Q_PROPERTY(QString activeProfile READ activeProfile NOTIFY statusChanged)
    Q_PROPERTY(QString message READ message NOTIFY statusChanged)
    Q_PROPERTY(bool busy READ busy NOTIFY busyChanged)
    Q_PROPERTY(bool delayTesting READ delayTesting NOTIFY delayTestingChanged)
    Q_PROPERTY(QVariantList profiles READ profiles NOTIFY profilesChanged)
    Q_PROPERTY(QVariantList groups READ groups NOTIFY proxiesChanged)
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
    Q_PROPERTY(bool sortGlobal READ sortGlobal NOTIFY statusChanged)
    Q_PROPERTY(QString sortMode READ sortMode NOTIFY statusChanged)
    Q_PROPERTY(bool showUnreachable READ showUnreachable NOTIFY statusChanged)
    Q_PROPERTY(QVariantMap sortPrefs READ sortPrefs NOTIFY statusChanged)
    Q_PROPERTY(bool autoTestDelay READ autoTestDelay NOTIFY statusChanged)
    Q_PROPERTY(bool settingsDirty READ settingsDirty NOTIFY draftChanged)

public:
    explicit ClashClient(QObject *parent = nullptr);

    bool coreDesired() const;
    bool coreRunning() const;
    bool systemProxyEnabled() const;
    bool systemProxyApplied() const;
    bool tunEnabled() const;
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
    QString activeProfile() const;
    QString message() const;
    bool busy() const;
    bool delayTesting() const;
    QVariantList profiles() const;
    QVariantList groups() const;
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
    bool sortGlobal() const;
    QString sortMode() const;
    bool showUnreachable() const;
    QVariantMap sortPrefs() const;
    bool autoTestDelay() const;
    bool settingsDirty() const;

    Q_INVOKABLE void setCoreDesired(bool desired);
    Q_INVOKABLE void restartCore();
    Q_INVOKABLE void setSystemProxyEnabled(bool enabled);
    Q_INVOKABLE void setTunEnabled(bool enabled);
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
    Q_INVOKABLE bool openBodyInEditor(const QString &body);
    Q_INVOKABLE void setProfileBody(const QString &id, const QString &body);
    Q_INVOKABLE void setProfileMeta(const QString &id, const QString &name, const QString &url);
    Q_INVOKABLE void importProfile(const QString &name, const QString &fileUrl);
    Q_INVOKABLE void refreshExitRegion();
    Q_INVOKABLE void reload();
    bool commitSettings();
    void revertDraft();
    void applyDefaultsToDraft();
    bool draftIsDefaults() const;

Q_SIGNALS:
    void statusChanged();
    void profilesChanged();
    void proxiesChanged();
    void accountsChanged();
    void busyChanged();
    void delayTestingChanged();
    void delayUpdated(const QString &group, const QString &name, qlonglong delayMs);
    void draftChanged();
    void errorOccurred(const QString &message);
    void editorTextReady(const QString &body);

private Q_SLOTS:
    void refreshStatus();
    void refreshProfiles();
    void refreshProxies();
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
    void asyncCall(const QString &method, const QVariantList &args, bool refreshProfilesAfter, bool refreshProxiesAfter, bool markBusy = true);
    void accountCall(const QString &method, qulonglong uid);

    QDBusInterface *m_iface = nullptr;
    bool m_coreDesired = false;
    bool m_coreRunning = false;
    bool m_systemProxyEnabled = false;
    bool m_systemProxyApplied = false;
    bool m_tunEnabled = false;
    bool m_firewallActive = false;
    qint64 m_firewallCheckedAt = 0;
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
    QVariantMap m_selections;
    bool m_sortGlobal = false;
    QString m_sortMode = QStringLiteral("profile");
    bool m_showUnreachable = true;
    QVariantMap m_sortPrefs;
    bool m_autoTestDelay = false;
    bool m_blurIp = false;
    QVariantList m_profiles;
    QVariantList m_groups;
    QVariantList m_accounts;
    QString m_accountsError;
    QDBusInterface *m_admin = nullptr;
};
