# clash-kde

A KDE Plasma 6 front end for a per-user [mihomo](https://github.com/MetaCubeX/mihomo)
session. The project provides a System Settings module (KCM) and a panel
widget; both control a small per-user service that owns one mihomo process,
its profiles, rules, and node selections. The mihomo binary and its geodata
come from the system and are not bundled here.

Each signed-in account has its own configuration and its own core. One account
does not read or edit another account's profiles.

## Features

- Start, stop, and restart the current account's core. The last choice is
  restored at login; a core that was stopped stays stopped.
- Import subscriptions or local profiles, switch between them, update
  subscriptions, and inspect proxy providers.
- Select routes, test delay, sort and filter nodes, edit route details, and keep
  a separate list of own rules. Rules can also be edited as a CSV spreadsheet.
- Apply the desktop proxy only while the core is running and TUN is off. The
  KDE and GNOME proxy settings are backed up and restored.
- Per-account TUN capture, protected by polkit. It does not change firewall
  rules; the status page reports when a firewall may interfere.
- Show live connections, a traffic chart, the exit region, and a paired
  speed test (through the proxy and directly, 1–100 MB per run).
- Control the core, system proxy, profile, mode, and route from the panel
  widget without opening System Settings.

## Requirements

- KDE Plasma 6, Qt 6.6 or newer, and KF6 6.0 or newer
- Rust 1.85 or newer (the service uses edition 2024)
- Linux with systemd, a session bus, the system bus, and polkit
- `mihomo` on `PATH`; TUN additionally requires a root-owned
  `/usr/bin/mihomo`
- Geodata: an MMDB (`country.mmdb` or `geoip.metadb`) or the v2ray files
  `geoip.dat` and `geosite.dat`. The service copies either set into its runtime
  directory and enables `geodata-mode` when only the v2ray files are present.

See [`docs/COMPATIBILITY.md`](docs/COMPATIBILITY.md) for the supported
distribution matrix and the standards checklist.

## Installation

Install `mihomo` separately. The project does not ship it.

### Arch Linux

From a checkout of this repository:

```sh
cd distro/arch
makepkg -fsi
```

### Debian 13

From the repository root:

```sh
sudo apt install cargo rustc cmake extra-cmake-modules pkgconf \
    qt6-base-dev qt6-declarative-dev qt6-tools-dev libkf6kcmutils-dev \
    libkf6i18n-dev libkf6coreaddons-dev libkf6config-dev \
    kirigami-addons-dev libplasma-dev libicu-dev gettext debhelper
dpkg-buildpackage -us -uc -b
sudo apt install ../clash-kde_*.deb
```

Debian does not provide `mihomo` in its archive; install it from upstream or
your preferred source before starting the core.

### Ubuntu 25.10 and newer

Use the Debian instructions. On Ubuntu 25.04, install Rust 1.85 or newer through
rustup first.

### Fedora 42 and newer

```sh
git archive --format=tar.gz --prefix=clash-kde-1.0.3/ \
    -o ~/rpmbuild/SOURCES/clash-kde-1.0.3.tar.gz HEAD
sudo dnf builddep distro/rpm/clash-kde.spec
rpmbuild -bb distro/rpm/clash-kde.spec
```

Fedora does not provide `mihomo` in its repositories; install it separately.

### openSUSE Tumbleweed

Leap 15.x is not supported because it does not provide Plasma 6 and KF6.

```sh
sudo zypper install cargo rust1.99 cmake extra-cmake-modules gcc \
    kf6-kcmutils-devel kirigami-addons6-devel kf6-kconfig-devel \
    kf6-kcoreaddons-devel kf6-ki18n-devel kf6-kquickcharts-devel \
    libicu-devel libplasma6-devel qt6-base-devel qt6-declarative-devel \
    qt6-tools-devel systemd-rpm-macros rpm-build mihomo v2ray-core

# The openSUSE mihomo package also installs a system service on 7890/9090.
sudo systemctl disable --now mihomo

git archive --format=tar.gz --prefix=clash-kde-1.0.3/ \
    -o ~/rpmbuild/SOURCES/clash-kde-1.0.3.tar.gz HEAD
rpmbuild -bb distro/rpm/clash-kde.spec
sudo zypper install --allow-unsigned-rpm \
    ~/rpmbuild/RPMS/x86_64/clash-kde-1.0.3-1.x86_64.rpm
```

`v2ray-core` is optional but recommended: it provides
`/usr/share/v2ray/geoip.dat` and `geosite.dat`, which lets mihomo start
without downloading geodata on first use.

## Getting started

A complete first-run walkthrough is in
[`docs/GETTING-STARTED.md`](docs/GETTING-STARTED.md). The short version:

1. Install `mihomo` and, if your distribution does not package geodata,
   `v2ray-core` or a `country.mmdb`.
2. Install `clash-kde` with the steps above and sign in again.
3. Open **System Settings → Network → Mihomo**.
4. On the **Profiles** page, add a subscription or import a local YAML file.
5. Select the profile, then start the core on the **Status** page and press
   **Apply**. The KCM applies changes with **Apply**; the panel widget applies
   them immediately.
6. Optional: enable **System proxy** on the Status page. It takes effect only
   while the core is running and TUN is off.
7. Optional: add the **Mihomo** widget from the panel.

TUN capture asks for authentication through polkit. It is limited to the
current account unless an administrator enables whole-computer capture from the
**Other** page.

## Configuration

| Path | Purpose |
| --- | --- |
| `~/.config/clash-kde/config.toml` | Preferences, mode, ports, active profile, node selections |
| `~/.config/clash-kde/override.yaml` | Local overrides merged over the subscription |
| `~/.config/clash-kde/rules.yaml` | Own rules |
| `~/.config/clash-kde/notes.json` | Route and rule notes |
| `~/.local/share/clash-kde/profiles/` | Imported and downloaded profiles |
| `~/.local/share/clash-kde/runtime/` | Generated config, cache, and mihomo log |
| `~/.config/kioslaverc` | KDE proxy settings, only while the service owns them |
| `/var/lib/clash-kde/` | Root-owned TUN and LAN policy state |

The service forces the listen address, controller port, secret, mode,
allow-lan policy, and TUN block when it generates the runtime configuration.

Advanced TUN routing can be tuned with these top-level keys in `config.toml`;
the KCM keeps them when it saves other settings:

| Key | Default | Meaning |
| --- | --- | --- |
| `tun_route_table` | `2022` | Policy-routing table used by mihomo's `auto-route` |
| `tun_rule_index` | `9000` | Base `ip rule` priority; avoid a range another VPN already owns |
| `tun_exclude_private` | `true` | Keep Tailscale, private LAN, and link-local traffic out of TUN |
| `tun_health_secs` | `5` | Recheck `ip rule` and the default route every 1–60 seconds |

Restart `clash-kde.service` after editing these keys. The Status page shows a
warning if the route later disappears, and `clash-kde --diagnose` prints the
rule, table, firewall, and Tailscale state.
Local edits are kept in the working profile. The Nodes page can restore an
individual route or profile rule from the original subscription.

## Diagnostics and troubleshooting

The Status page has a **Diagnostics** button that shows the live state, paths,
mihomo version, geodata mode, port usage, proxy ownership, the recent mihomo
log, and the user service journal. From a terminal:

```sh
clash-kde --diagnose
journalctl --user -u clash-kde.service -n 200 --no-pager
tail -n 200 ~/.local/share/clash-kde/runtime/mihomo.log
```

Common causes:

- **Core does not start**: `mihomo` is missing, the configured binary is not
  executable, or a port is already in use. The diagnostics report names the
  port and prints the system `mihomo.service` state on distributions that ship
  one. Stop the other service or choose different ports.
- **First start appears to hang**: no local geodata was found and mihomo is
  downloading it. Install `v2ray-core` or place a `country.mmdb` in
  `~/.config/mihomo/`. The diagnostics report shows the selected geodata mode.
- **Desktop proxy still points at a dead port**: run
  `clash-kde --restore-proxy`, or use **Turn off system proxy** in the
  diagnostics dialog. The user service also runs this command as
  `ExecStopPost`, so a crash or normal stop restores the proxy.
- **TUN is refused**: verify that `/usr/bin/mihomo` is root-owned and not
  group- or world-writable, and that polkit is running. TUN is intentionally
  restricted to the system binary.
- **TUN is on but applications still use the physical interface**: run
  `clash-kde --diagnose` and read the `tun` section. TUN waits for its
  `ip rule` and configured table route before reporting success (table 2022 by
  default); if `route ready` is `no`, a firewall or Tailscale is rewriting the
  policy routing. The generated
  config excludes `100.64.0.0/10`, `fd7a:115c:a1e0::/48`, and private LAN
  ranges so Tailscale and local devices keep their own routes. With UFW or
  firewalld active, allow the TUN interface (or temporarily disable the
  firewall while testing).
- **The Status page switch is labelled Traffic capture while TUN is on**:
  that switch now controls TUN, not the desktop proxy. Turning it off disables
  TUN; use the Users page or turn the switch back on to re-enable it.
- **mihomo uses a different TUN device name**: the health probe expects
  `Meta` by default. Set `tun_device` in `config.toml` to the actual interface
  name; the startup error and `--diagnose` both print the expected name.
- **openSUSE**: a locally installed AppArmor profile can block the per-user
  runtime directory. Check `sudo aa-status` and
  `journalctl -k | grep -i apparmor`.
- **KCM changes do not start the core**: the KCM uses a draft. Toggle the
  switch and press **Apply**; the status page shows a reminder while the draft
  differs from the running state.

## Building from source

The Rust service and the Qt front end are built independently:

```sh
cargo build --release --locked
cmake -S kcm -B build -DCMAKE_BUILD_TYPE=Release
cmake --build build
```

Run the Rust tests with:

```sh
cargo test --locked
```

The KCM and applet require the development packages listed in
`distro/arch/PKGBUILD`, `debian/control`, or `distro/rpm/clash-kde.spec`.
Packaging scripts install the binary, KCM plugin, panel widget, D-Bus
activation files, polkit actions, systemd units, and translations.

## Compatibility and standards

The distribution matrix, file-system assumptions, and standards status are
documented in [`docs/COMPATIBILITY.md`](docs/COMPATIBILITY.md). Highlights:

- FHS paths, XDG configuration/data directories, systemd user and system
  units, D-Bus activation, and polkit actions are used.
- Source files carry SPDX identifiers; the MSRV is declared in `Cargo.toml`.
- AppStream metadata and a CI workflow are not yet present.
- Error strings from the Rust service and some C++ paths are currently Chinese
  and are not part of the gettext catalogs.

## License

GPL-3.0-or-later. See [LICENSE](LICENSE).
