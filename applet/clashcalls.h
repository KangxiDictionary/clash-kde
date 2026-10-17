// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 clash-kde contributors

#pragma once

#include <QObject>
#include <QVariantList>
#include <qqmlregistration.h>

class QDBusInterface;

class ClashCalls : public QObject
{
    Q_OBJECT
    QML_ELEMENT

    Q_PROPERTY(bool ready READ ready NOTIFY changed)
    Q_PROPERTY(QString error READ error NOTIFY changed)
    Q_PROPERTY(bool coreDesired READ coreDesired NOTIFY changed)
    Q_PROPERTY(bool coreRunning READ coreRunning NOTIFY changed)
    Q_PROPERTY(bool systemProxyEnabled READ systemProxyEnabled NOTIFY changed)
    Q_PROPERTY(bool systemProxyApplied READ systemProxyApplied NOTIFY changed)
    Q_PROPERTY(bool tunEnabled READ tunEnabled NOTIFY changed)
    Q_PROPERTY(QString mode READ mode NOTIFY changed)
    Q_PROPERTY(QString message READ message NOTIFY changed)
    Q_PROPERTY(QString region READ region NOTIFY changed)
    Q_PROPERTY(QString listenAddress READ listenAddress NOTIFY changed)
    Q_PROPERTY(double uploadBps READ uploadBps NOTIFY changed)
    Q_PROPERTY(double downloadBps READ downloadBps NOTIFY changed)
    Q_PROPERTY(QString activeProfile READ activeProfile NOTIFY changed)
    Q_PROPERTY(bool autoTestDelay READ autoTestDelay NOTIFY changed)
    Q_PROPERTY(QVariantList profiles READ profiles NOTIFY changed)
    Q_PROPERTY(QVariantList groups READ groups NOTIFY changed)

public:
    explicit ClashCalls(QObject *parent = nullptr);

    bool ready() const;
    QString error() const;
    bool coreDesired() const;
    bool coreRunning() const;
    bool systemProxyEnabled() const;
    bool systemProxyApplied() const;
    bool tunEnabled() const;
    QString mode() const;
    QString message() const;
    QString region() const;
    QString listenAddress() const;
    double uploadBps() const;
    double downloadBps() const;
    QString activeProfile() const;
    bool autoTestDelay() const;
    QVariantList profiles() const;
    QVariantList groups() const;

    Q_INVOKABLE void setCoreDesired(bool desired);
    Q_INVOKABLE void setSystemProxyEnabled(bool enabled);
    Q_INVOKABLE void setMode(const QString &mode);
    Q_INVOKABLE void setActiveProfile(const QString &id);
    Q_INVOKABLE void selectProxy(const QString &group, const QString &name);
    Q_INVOKABLE void testGroupDelay(const QString &group);
    Q_INVOKABLE bool copyTerminalProxyCommand();

public Q_SLOTS:
    void refresh();
    void refreshLists();
    void onDelayUpdated(const QString &group, const QString &name, qlonglong delayMs);

Q_SIGNALS:
    void changed();

private:
    void call(const QString &member, const QVariantList &arguments);
    void applyStatus(const QString &json);
    void fail(const QString &message);

    QDBusInterface *m_iface = nullptr;
    bool m_ready = false;
    QString m_error;
    bool m_coreDesired = false;
    bool m_coreRunning = false;
    bool m_systemProxyEnabled = false;
    bool m_systemProxyApplied = false;
    bool m_tunEnabled = false;
    QString m_mode = QStringLiteral("rule");
    QString m_message;
    QString m_region;
    QString m_listenAddress;
    double m_uploadBps = 0;
    double m_downloadBps = 0;
    QString m_activeProfile;
    bool m_autoTestDelay = false;
    QVariantList m_profiles;
    QVariantList m_groups;
};
