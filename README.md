# clash-kde

A KDE Plasma 6 System Settings page and panel widget for a per-user [mihomo](https://github.com/MetaCubeX/mihomo) process. The core comes from the system `mihomo` package. This project does not ship one.

Each signed-in account has its own core, profiles, and node choices. One account does not edit another's configuration.

## What it does

- Starts and stops that account's core. The last choice is restored at login. A core that was left stopped stays stopped.
- Applies the system proxy only while the core is running and TUN is off. Turning the proxy off in **System Settings → Network → Proxy Server** turns this switch off as well, so a later start does not write that setting back.
- TUN captures only this account's traffic. Turning it on asks for permission. It does not change firewall rules; if a firewall is enabled, the status page says so.
- Imports a subscription or a profile file, selects a route, and tests delay. There is no rule-set editor and no raw YAML page for import.
- The panel widget changes the core, the system proxy, the profile, and the route immediately. In System Settings, the switches, the profile, the route, and the advanced fields apply when you press **Apply**. Testing delay and checking the exit region happen immediately. **Reset** discards the unsaved draft. **Default** restores the first-install preferences and leaves profile files in place.

## Requirements

- KDE Plasma 6 and Qt 6
- `mihomo` on `PATH`
- The runtime libraries listed in [`distro/arch/PKGBUILD`](distro/arch/PKGBUILD)

## Install

`mihomo` has to be installed separately. This package does not ship it.

On Arch Linux, from a checkout of this repository:

```sh
cd distro/arch
makepkg -fsi
```

On Debian 13 or Ubuntu 25.04 and newer, from the repository root:

```sh
sudo apt install cargo rustc cmake extra-cmake-modules pkgconf qt6-base-dev qt6-declarative-dev qt6-tools-dev libkf6kcmutils-dev libkf6i18n-dev libkf6coreaddons-dev libkf6config-dev libplasma-dev libicu-dev gettext debhelper
dpkg-buildpackage -us -uc -b
sudo apt install ../clash-kde_*.deb
```

On Fedora 41 and newer:

```sh
git archive --format=tar.gz --prefix=clash-kde-1.0.2/ -o ~/rpmbuild/SOURCES/clash-kde-1.0.2.tar.gz HEAD
sudo dnf builddep distro/rpm/clash-kde.spec
rpmbuild -bb distro/rpm/clash-kde.spec
```

Use the same command after pulling newer code. The package enables a per-user service. Open **System Settings → Network → Mihomo**, and add the Mihomo widget from the panel.

## License

GPL-3.0-or-later. See [LICENSE](LICENSE).
