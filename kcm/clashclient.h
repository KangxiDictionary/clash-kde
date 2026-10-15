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
    Q_PROPERTY(int draftMixedPort READ draftMixedPort WRITE setDraftMixedPort NOTIFY draftChanged)
    Q_PROPERTY(int draftControllerPort READ draftControllerPort WRITE setDraftControllerPort NOTIFY draftChanged)
    Q_PROPERTY(QString draftMihomoPath READ draftMihomoPath WRITE setDraftMihomoPath NOTIFY draftChanged)

public:
    explicit ClashClient(QObject *parent = nullptr);

    bool coreDesired() const;
    bool coreRunning() const;
    bool systemProxyEnabled() const;
    bool systemProxyApplied() const;
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

    int draftMixedPort() const;
    void setDraftMixedPort(int port);
    int draftControllerPort() const;
    void setDraftControllerPort(int port);
    QString draftMihomoPath() const;
    void setDraftMihomoPath(const QString &path);

    Q_INVOKABLE void setCoreDesired(bool desired);
    Q_INVOKABLE void restartCore();
    Q_INVOKABLE void setSystemProxyEnabled(bool enabled);
    Q_INVOKABLE void setMode(const QString &mode);
    Q_INVOKABLE void setActiveProfile(const QString &id);
    Q_INVOKABLE void addSubscription(const QString &name, const QString &url);
    Q_INVOKABLE void removeProfile(const QString &id);
    Q_INVOKABLE void updateProfile(const QString &id);
    Q_INVOKABLE void selectProxy(const QString &group, const QString &name);
    Q_INVOKABLE void testGroupDelay(const QString &group);
    Q_INVOKABLE QString profileBody(const QString &id);
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
    void busyChanged();
    void delayTestingChanged();
    void delayUpdated(const QString &group, const QString &name, qlonglong delayMs);
    void draftChanged();
    void errorOccurred(const QString &message);

private Q_SLOTS:
    void refreshStatus();
    void refreshProfiles();
    void refreshProxies();
    void onDelayUpdated(const QString &group, const QString &name, qlonglong delayMs);

private:
    void setBusy(bool busy);
    void setDelayTesting(bool testing);
    void applyStatus(const QVariantMap &status);
    void reportError(const QString &message);
    void asyncCall(const QString &method, const QVariantList &args, bool refreshProfilesAfter, bool refreshProxiesAfter, bool markBusy = true);

    QDBusInterface *m_iface = nullptr;
    bool m_coreDesired = false;
    bool m_coreRunning = false;
    bool m_systemProxyEnabled = false;
    bool m_systemProxyApplied = false;
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
    int m_draftMixedPort = 7890;
    int m_draftControllerPort = 9090;
    QString m_draftMihomoPath = QStringLiteral("mihomo");
    QVariantList m_profiles;
    QVariantList m_groups;
};
