# Getting started

This guide takes you from a fresh installation to a running per-user mihomo
core. It assumes KDE Plasma 6 on a Linux system with systemd, D-Bus, and
polkit.

## 1. Before you start

You need:

- a working Plasma 6 desktop;
- the `mihomo` binary already installed (the project does not ship it);
- either an MMDB file, or `v2ray-core` for its `geoip.dat` and
  `geosite.dat`;
- a subscription URL or a local mihomo YAML profile.

On distributions whose `mihomo` package also installs a system service using
ports 7890/9090, stop that service before starting the per-user core:

```sh
systemctl is-active mihomo.service
sudo systemctl disable --now mihomo
```

## 2. Install the package

Use the section for your distribution in the [README](../README.md#installation).
After installation, sign out and back in, or start the service manually:

```sh
systemctl --user daemon-reload
systemctl --user start clash-kde.service
systemctl --user status clash-kde.service
```

Open **System Settings → Network → Mihomo**. If the page is missing, restart
System Settings so it reloads the plugin.

## 3. Add a profile

Open the **Profiles** page.

- **Add subscription**: enter a name (optional) and a complete
  `http://` or `https://` URL. The dialog refuses incomplete URLs.
- **Import**: choose a local `.yaml` or `.yml` file.

Select the profile in the list. The profile name appears on the **Status**
page.

## 4. Start the core

On the **Status** page:

1. Toggle the core switch on.
2. Press **Apply** in the System Settings window.

The KCM uses a draft. A reminder appears while the draft differs from the
running core. The panel widget is different: its core switch takes effect
immediately.

The status page shows one of:

- **Running**: the controller answers on `127.0.0.1:9090`.
- **Waiting to start**: mihomo is starting, or downloading geodata.
- **Stopped** with an error: open **Diagnostics**.

The mixed HTTP/SOCKS endpoint is shown as `http://127.0.0.1:7890` (or the port
you configured on the **Users** page).

## 5. Enable the desktop proxy (optional)

On the Status page, enable **System proxy** and press **Apply**.

The proxy is written to KDE's `kioslaverc` only while:

- the core is running, and
- TUN is off.

The previous KDE and GNOME proxy settings are backed up. Normal stop, service
crash, and package removal restore them. If the desktop is ever left pointing
at a dead local port, run:

```sh
clash-kde --restore-proxy
```

or use **Turn off system proxy** in the Diagnostics dialog.

## 6. Add the panel widget (optional)

Right-click the panel, choose **Add Widgets**, and add **Mihomo**. The widget
can start or stop the core, toggle the system proxy, change the mode and
profile, select a route, and copy a terminal proxy command. Its changes apply
immediately.

## 7. TUN capture (optional)

TUN captures traffic from this account. Enabling it asks for authentication
through polkit. The root-owned `/usr/bin/mihomo` is required; a user-owned
binary is refused.

TUN does not modify firewall rules. If a firewall is active, the Status page
warns that it may drop TUN traffic.

Whole-computer capture is on the **Other** page and requires administrator
authentication.

## 8. Verify the installation

Run:

```sh
clash-kde --diagnose
```

A healthy report shows:

- the profile and generated config parse;
- `mihomo` resolves to an executable and prints its version;
- geodata is either `country.mmdb/geoip.metadb` or
  `geoip.dat/geosite.dat (geodata-mode: true)`;
- ports are free before start, or held by the running core after start;
- `system mihomo.service: inactive`;
- the proxy state is `off` or `ours`.

If the core does not start, the same report includes the last mihomo log lines
and the last service journal entries. The Status page **Diagnostics** button
shows the same information and can copy it for a bug report.

## Everyday use

- **Stop**: Status page switch off, then **Apply**. The proxy is withdrawn.
- **Restart**: **Restart** on the Status page.
- **Change node**: Nodes page, select a route, then **Apply**.
- **Update subscription**: Profiles page, **Update**.
- **Own rules**: Nodes → Rules; add rules individually or edit them as a CSV
  spreadsheet.
- **Check exit region**: Status page, **Check region**.
- **Speed test**: Status page; downloads 1–100 MB through the proxy and
  directly. The size is set on the **Other** page and is bounded to 1–100 MB.

## Uninstall

Remove the package with your distribution's package manager. For a manual
cleanup:

```sh
systemctl --user disable --now clash-kde.service
clash-kde --restore-proxy
```

User data stays in `~/.config/clash-kde` and `~/.local/share/clash-kde` until
you remove it. Root-owned TUN and LAN policy state lives in
`/var/lib/clash-kde`.

## Getting help

Collect:

```sh
clash-kde --diagnose
journalctl --user -u clash-kde.service -n 200 --no-pager
```

and, if the core started and then failed:

```sh
tail -n 200 ~/.local/share/clash-kde/runtime/mihomo.log
```

Remove subscription URLs and node credentials before posting logs publicly.
The diagnostics report does not print the controller secret or the LAN
password, and connection hosts are stripped of tokens.
