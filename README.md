# clash-kde

A KDE Plasma 6 System Settings page and panel widget for a per-user [mihomo](https://github.com/MetaCubeX/mihomo) process. The core comes from the system `mihomo` package. This project does not ship one.

Each signed-in account has its own core, profiles, and node choices. One account does not edit another's configuration.

## What it does

- Starts and stops that account's core. The last choice is restored at login. A core that was left stopped stays stopped.
- Applies the system proxy only while the core is running and TUN is off. Turning the proxy off in **System Settings → Network → Proxy Server** turns this switch off as well, so a later start does not write that setting back.
- TUN captures only this account's traffic. Turning it on asks for permission. It does not change firewall rules; if a firewall is enabled, the status page says so.
- Imports a subscription or a profile file, selects a route, and tests delay. There is no rule-set editor and no raw YAML page for import.
- The panel widget changes the core, the system proxy, the profile, and the route immediately. In System Settings, those switches and the advanced fields apply when you press **Apply**. **Reset** discards the unsaved draft. **Default** restores the first-install preferences and leaves subscription files in place.

## Requirements

- KDE Plasma 6 and Qt 6
- `mihomo` on `PATH`
- The runtime libraries listed in [`distro/arch/PKGBUILD`](distro/arch/PKGBUILD)

## Install

On Arch Linux, from a checkout of this repository:

```sh
cd distro/arch
makepkg -fsi
```

Use the same command after pulling newer code. The package enables a per-user service. Open **System Settings → Network → Mihomo**, and add the Mihomo widget from the panel.

## License

GPL-3.0-or-later. See [LICENSE](LICENSE).
