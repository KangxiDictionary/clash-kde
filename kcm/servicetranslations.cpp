// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 clash-kde contributors

#include "servicetranslations.h"

#include <KLocalizedString>
#include <QJsonDocument>
#include <QJsonObject>

namespace {
QString localized(const char *text)
{
    return ki18nd("kcm_clashkde", text).toString();
}
}

QString translateServiceCode(const QString &code, const QJsonObject &params, const QString &detail)
{
    const auto p = [&params](const char *key) { return params.value(QLatin1String(key)).toString(); };
    const auto text = [](const char *source) { return localized(source); };

    if (code == QLatin1String("settings.mihomo_path_empty")) return text("The mihomo executable path cannot be empty.");
    if (code == QLatin1String("settings.mihomo_path_too_long")) return text("The mihomo executable path cannot exceed %1 characters.").arg(p("limit"));
    if (code == QLatin1String("settings.override_too_long")) return text("The override path cannot exceed %1 characters.").arg(p("limit"));
    if (code == QLatin1String("settings.speed_test_range")) return text("The speed test size must be between 1 and 100 MB.");
    if (code == QLatin1String("settings.notes_too_large")) return text("The note is too large.");
    if (code == QLatin1String("settings.notes_parse")) return text("The notes could not be parsed.");
    if (code == QLatin1String("profile.name_empty")) return text("The name cannot be empty.");
    if (code == QLatin1String("profile.name_too_long")) return text("The name cannot exceed %1 characters.").arg(p("limit"));
    if (code == QLatin1String("profile.url_too_long")) return text("The subscription URL cannot exceed %1 characters.").arg(p("limit"));
    if (code == QLatin1String("profile.url_scheme")) return text("The subscription URL must start with http:// or https://.");
    if (code == QLatin1String("profile.too_large")) return text("The profile is too large.");
    if (code == QLatin1String("profile.import_read")) return text("The imported profile could not be read.");
    if (code == QLatin1String("rule.index_missing")) return text("This rule no longer exists.");
    if (code == QLatin1String("controller.group_missing")) return text("The proxy group was not found.");
    if (code == QLatin1String("tun.revoke_timeout")) return text("TUN authorization could not be revoked in time.");
    if (code == QLatin1String("core.not_running")) return text("The core is not running.");
    if (code == QLatin1String("core.check_failed")) return text("The core state could not be checked.");
    if (code == QLatin1String("core.exited")) return text("mihomo exited.");
    if (code == QLatin1String("core.stop_timeout")) return text("The core did not stop in time.");
    if (code == QLatin1String("core.port_busy")) return text("%1 is already in use; stop the other program or choose another port.").arg(p("ports"));
    if (code == QLatin1String("core.restore_failed")) return text("Could not restore the previous core state.");
    if (code == QLatin1String("core.start_failed")) return text("The core failed to start.");
    if (code == QLatin1String("core.stop_failed")) return text("The old core could not be stopped.");
    if (code == QLatin1String("core.restart")) return text("The core exited unexpectedly; retrying.");
    if (code == QLatin1String("core.geodata_downloading")) return text("Downloading geodata.");
    if (code == QLatin1String("geodata.prepare_failed")) return text("The geodata directory could not be prepared.");
    if (code == QLatin1String("geodata.copy_failed")) return text("Geodata could not be copied.");
    if (code == QLatin1String("proxy.release_failed")) return text("The edited system proxy could not be released.");
    if (code == QLatin1String("proxy.restore_failed")) return text("The KDE proxy could not be restored.");
    if (code == QLatin1String("proxy.save_failed")) return text("The system-proxy preference could not be saved.");
    if (code == QLatin1String("settings.config_parse")) return text("The configuration could not be parsed.");
    if (code == QLatin1String("tun.route_degraded")) return text("The TUN policy route is not ready.");
    if (code == QLatin1String("tun.route_unknown")) return text("The TUN policy route could not be verified.");
    if (code == QLatin1String("cli.admin")) return text("The admin service failed.");
    if (code == QLatin1String("cli.restore_proxy")) return text("The proxy restore command failed.");
    if (code == QLatin1String("cli.session")) return text("The session service failed.");
    if (code == QLatin1String("admin.account_missing")) return text("No account was specified.");
    if (code == QLatin1String("admin.action_missing")) return text("No action was specified.");
    if (code == QLatin1String("admin.switch_failed")) return text("Could not switch to this account.");
    if (code == QLatin1String("admin.bridge_failed")) return text("Could not contact this account's Mihomo.");
    if (code == QLatin1String("admin.launch_failed")) return text("The Mihomo helper could not be started.");
    if (code == QLatin1String("admin.caller_missing")) return text("The D-Bus caller could not be identified.");
    if (code == QLatin1String("admin.not_root")) return text("The system service requires root.");
    if (code == QLatin1String("admin.polkit_ask")) return text("Authorization could not be requested.");
    if (code == QLatin1String("admin.polkit_denied")) return text("Authorization was denied.");
    if (code == QLatin1String("tun.not_root")) return text("TUN requires the system service.");
    if (code == QLatin1String("tun.switch_failed")) return text("Could not switch to this account.");
    if (code == QLatin1String("tun.bad_account_name")) return text("The account name is invalid.");
    if (code == QLatin1String("core.starting")) return text("Starting the core.");
    if (code == QLatin1String("core.geodata_downloading")) return text("Downloading geodata.");
    if (code == QLatin1String("core.controller_timeout")) return text("The controller did not respond in time.");
    if (code == QLatin1String("core.tun_listen_error")) return text("TUN could not start.");
    if (code == QLatin1String("controller.mode_switch")) return text("Could not switch the mode.");
    if (code == QLatin1String("controller.select_proxy")) return text("Could not select the proxy.");
    if (code == QLatin1String("controller.read_groups")) return text("Could not read the proxy groups.");
    if (code == QLatin1String("controller.parse_groups")) return text("The proxy group response could not be parsed.");
    if (code == QLatin1String("controller.read_traffic")) return text("Could not read traffic.");
    if (code == QLatin1String("controller.read_connections")) return text("Could not read connections.");
    if (code == QLatin1String("controller.parse_connections")) return text("The connection response could not be parsed.");
    if (code == QLatin1String("controller.close_connection")) return text("Could not close the connection.");
    if (code == QLatin1String("controller.no_connection")) return text("That connection no longer exists.");
    if (code == QLatin1String("controller.read_providers")) return text("Could not read the provider list.");
    if (code == QLatin1String("controller.parse_providers")) return text("The provider list could not be parsed.");
    if (code == QLatin1String("controller.update_provider")) return text("Could not update the provider.");
    if (code == QLatin1String("controller.subscription_download")) return text("The subscription could not be downloaded.");
    if (code == QLatin1String("controller.subscription_read")) return text("The subscription could not be read.");
    if (code == QLatin1String("controller.region_failed")) return text("The exit region could not be detected.");
    if (code == QLatin1String("controller.region_empty")) return text("The exit region service returned no result.");
    if (code == QLatin1String("controller.region_http")) return text("The exit region service returned an error.");
    if (code == QLatin1String("controller.region_parse")) return text("The exit region response could not be parsed.");
    if (code == QLatin1String("controller.region_proxy")) return text("The exit-region proxy could not be configured.");
    if (code == QLatin1String("io.not_found")) return text("The file was not found.");
    if (code == QLatin1String("io.denied")) return text("Permission denied.");
    if (code == QLatin1String("io.failed")) return text("The file operation failed.");
    if (code == QLatin1String("settings.port_invalid")) return text("Invalid port.");
    if (code == QLatin1String("settings.port_same")) return text("The mixed and controller ports must differ.");
    if (code == QLatin1String("settings.mode_invalid")) return text("The mode must be rule, global, or direct.");
    if (code == QLatin1String("settings.tun_device_invalid")) return text("The interface name must be 1 to 15 letters, digits, underscores, or hyphens.");
    if (code == QLatin1String("settings.override_absolute")) return text("The override file needs an absolute path.");
    if (code == QLatin1String("settings.override_filename")) return text("The override path needs a file name.");
    if (code == QLatin1String("settings.override_is_dir")) return text("The override path is a folder.");
    if (code == QLatin1String("lan.auth_both")) return text("Fill in both the LAN username and password.");
    if (code == QLatin1String("lan.auth_colon")) return text("The LAN username and password cannot contain colons.");
    if (code == QLatin1String("lan.username_too_long")) return text("The LAN username cannot exceed %1 characters.").arg(p("limit"));
    if (code == QLatin1String("lan.password_too_long")) return text("The LAN password cannot exceed %1 characters.").arg(p("limit"));
    if (code == QLatin1String("account.not_found")) return text("The account was not found.");
    if (code == QLatin1String("tun.binary_not_system")) return text("TUN can only use the system's /usr/bin/mihomo.");
    if (code == QLatin1String("tun.system_binary_missing")) return text("The system's /usr/bin/mihomo is missing.");
    if (code == QLatin1String("tun.binary_missing")) return text("mihomo could not be found.");
    if (code == QLatin1String("tun.config_not_owned")) return text("TUN can only use this account's own runtime configuration.");
    if (code == QLatin1String("tun.config_parse")) return text("The TUN runtime configuration could not be parsed.");
    if (code == QLatin1String("tun.config_missing")) return text("The runtime configuration has no TUN section.");
    if (code == QLatin1String("tun.config_disabled")) return text("TUN is not enabled in the runtime configuration.");
    if (code == QLatin1String("tun.machine_include_uid")) return text("Machine-wide capture cannot restrict accounts.");
    if (code == QLatin1String("tun.machine_exclude_uid")) return text("Machine-wide capture cannot exclude this account.");
    if (code == QLatin1String("tun.include_uid_invalid")) return text("TUN may only include this account.");
    if (code == QLatin1String("tun.already_other_user")) return text("Another account already has its own TUN running.");
    if (code == QLatin1String("tun.log_symlink")) return text("The TUN log cannot be a symbolic link.");
    if (code == QLatin1String("tun.log_check")) return text("The TUN log could not be checked.");
    if (code == QLatin1String("tun.not_authorized")) return text("TUN has not been authorized yet.");
    if (code == QLatin1String("profile.not_found")) return text("The profile was not found.");
    if (code == QLatin1String("profile.no_url")) return text("This profile has no subscription URL.");
    if (code == QLatin1String("profile.yaml_error")) return text("The subscription is not valid YAML.");
    if (code == QLatin1String("profile.empty")) return text("The profile cannot be empty.");
    if (code == QLatin1String("profile.not_mapping")) return text("The profile must be a YAML mapping.");
    if (code == QLatin1String("profile.override_not_mapping")) return text("The local override must be a YAML mapping.");
    if (code == QLatin1String("profile.override_parse")) return text("The local override could not be parsed.");
    if (code == QLatin1String("profile.config_parse")) return text("The profile YAML could not be parsed.");
    if (code == QLatin1String("profile.runtime_write")) return text("The runtime configuration could not be written.");
    if (code == QLatin1String("route.name_empty")) return text("The route name cannot be empty or a built-in name.");
    if (code == QLatin1String("route.name_too_long")) return text("The route name cannot exceed %1 characters.").arg(p("limit"));
    if (code == QLatin1String("route.kind_unsupported")) return text("This route type is not supported.");
    if (code == QLatin1String("route.server_empty")) return text("The server cannot be empty.");
    if (code == QLatin1String("route.server_too_long")) return text("The server address cannot exceed %1 characters.").arg(p("limit"));
    if (code == QLatin1String("route.secret_too_long")) return text("The password or UUID cannot exceed %1 characters.").arg(p("limit"));
    if (code == QLatin1String("route.port_invalid")) return text("Invalid port.");
    if (code == QLatin1String("route.duplicate")) return text("A route with this name already exists.");
    if (code == QLatin1String("route.group_duplicate")) return text("A proxy group with this name already exists.");
    if (code == QLatin1String("route.cannot_remove")) return text("This route cannot be removed.");
    if (code == QLatin1String("route.not_found")) return text("This route is not in the configuration.");
    if (code == QLatin1String("route.cannot_edit")) return text("This route cannot be edited.");
    if (code == QLatin1String("route.cannot_restore")) return text("This route cannot be restored.");
    if (code == QLatin1String("route.subscription_missing")) return text("This route is not in the subscription.");
    if (code == QLatin1String("profile.subscription_unparseable")) return text("The subscription snapshot could not be parsed.");
    if (code == QLatin1String("rule.subscription_missing")) return text("This rule is not in the subscription.");
    if (code == QLatin1String("rule.not_found")) return text("This rule is not in the configuration.");
    if (code == QLatin1String("rule.no_rules")) return text("The configuration has no rules.");
    if (code == QLatin1String("rules.read_failed")) return text("The custom rules could not be read.");
    if (code == QLatin1String("rules.write_failed")) return text("The custom rules could not be written.");
    if (code == QLatin1String("rules.parse_failed")) return text("The custom rules could not be parsed.");
    if (code == QLatin1String("rules.json_parse")) return text("The rules JSON could not be read.");
    if (code == QLatin1String("rules.csv_header")) return text("The spreadsheet header is invalid.");
    if (code == QLatin1String("rules.csv_row")) return text("Row %1 could not be split.").arg(p("row"));
    if (code == QLatin1String("rules.type_unknown")) return text("Unknown rule type.");
    if (code == QLatin1String("rules.comma")) return text("The match value cannot contain a comma.");
    if (code == QLatin1String("rules.payload_too_long")) return text("The match value is too long.");
    if (code == QLatin1String("rules.target_too_long")) return text("The target name is too long.");
    if (code == QLatin1String("rules.target_empty")) return text("The target cannot be empty.");
    if (code == QLatin1String("rules.target_unknown")) return text("The target does not exist.");
    if (code == QLatin1String("rules.payload_empty")) return text("The match value cannot be empty.");
    if (code == QLatin1String("rules.cidr")) return text("The IP range is invalid.");
    if (code == QLatin1String("rules.port")) return text("The port is invalid.");
    if (code == QLatin1String("rules.geoip")) return text("The country code is invalid.");
    if (code == QLatin1String("rules.csv_kind")) return text("The type in the spreadsheet is unknown.");
    if (code == QLatin1String("rules.csv_enabled")) return text("The enabled column must be true or false.");
    if (code == QLatin1String("rules.row_prefix")) {
        const QJsonObject inner = QJsonDocument::fromJson(p("inner_params").toUtf8()).object();
        return text("Row %1: %2").arg(p("row"), translateServiceCode(p("inner_code"), inner, detail));
    }
    if (code == QLatin1String("core.log_open")) return text("The core log could not be opened.");
    if (code == QLatin1String("core.spawn_failed")) return text("mihomo could not be started.");
    if (code == QLatin1String("core.no_pid")) return text("mihomo has no process ID.");
    if (code == QLatin1String("core.binary_missing")) return text("mihomo could not be found. Install the mihomo package first.");
    if (code == QLatin1String("controller.subscription_download")) return text("The subscription could not be downloaded.");
    if (code == QLatin1String("controller.subscription_read")) return text("The subscription could not be read.");
    if (code == QLatin1String("controller.region_failed")) return text("The exit region could not be detected.");
    if (code == QLatin1String("controller.region_empty")) return text("The exit region service returned no result.");
    if (code == QLatin1String("error.legacy")) {
        return detail.isEmpty() ? text("The operation failed.") : detail;
    }
    if (detail.isEmpty()) return text("The operation failed (%1).").arg(code);
    return text("The operation failed (%1):\n%2").arg(code, detail);
}

