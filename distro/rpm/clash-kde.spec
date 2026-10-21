Name:           clash-kde
Version:        1.0.2
Release:        1%{?dist}
Summary:        KDE settings page and panel widget for a per-user mihomo proxy
License:        GPL-3.0-or-later
URL:            https://github.com/KangxiDictionary/clash-kde
Source0:        %{name}-%{version}.tar.gz

BuildRequires:  cargo
BuildRequires:  cmake
BuildRequires:  extra-cmake-modules
BuildRequires:  gcc
BuildRequires:  kf6-kcmutils-devel
BuildRequires:  kf6-kirigami-addons-devel
BuildRequires:  kf6-kconfig-devel
BuildRequires:  kf6-kcoreaddons-devel
BuildRequires:  kf6-ki18n-devel
BuildRequires:  libicu-devel
BuildRequires:  libplasma-devel
BuildRequires:  qt6-qtbase-devel
BuildRequires:  qt6-qtdeclarative-devel
BuildRequires:  qt6-qttools-devel
BuildRequires:  rust
BuildRequires:  systemd-rpm-macros

Requires:       dbus
Requires:       kf6-kcmutils
Requires:       kf6-kirigami
Requires:       kf6-kirigami-addons
Requires:       kf6-kquickcharts
Requires:       mihomo
Requires:       plasma-workspace
Requires:       polkit

%description
Each signed-in account gets its own mihomo process, profiles, and route
choices. System Settings applies those choices with the Apply button. The
panel widget changes them immediately. The mihomo program itself comes from
the system package and is not bundled here.

%prep
%autosetup

%build
cargo build --locked --release
cmake -S kcm -B redhat-build \
    -DCMAKE_BUILD_TYPE=Release \
    -DCMAKE_INSTALL_PREFIX=%{_prefix} \
    -DCMAKE_INSTALL_LIBDIR=%{_libdir}
cmake --build redhat-build

%install
DESTDIR=%{buildroot} cmake --install redhat-build
install -Dm755 target/release/clash-kde %{buildroot}%{_bindir}/clash-kde
install -Dm644 distro/arch/clash-kde.service %{buildroot}%{_userunitdir}/clash-kde.service
install -Dm644 distro/arch/org.kde.clashkde.service %{buildroot}%{_datadir}/dbus-1/services/org.kde.clashkde.service
install -Dm644 distro/arch/90-clash-kde.preset %{buildroot}%{_userpresetdir}/90-clash-kde.preset
install -Dm644 distro/arch/clash-kde-system.service %{buildroot}%{_unitdir}/clash-kde-system.service
install -Dm644 distro/arch/org.kde.clashkde.Admin.service %{buildroot}%{_datadir}/dbus-1/system-services/org.kde.clashkde.Admin.service
install -Dm644 distro/arch/org.kde.clashkde.Admin.conf %{buildroot}%{_datadir}/dbus-1/system.d/org.kde.clashkde.Admin.conf
install -Dm644 distro/arch/org.kde.clashkde.policy %{buildroot}%{_datadir}/polkit-1/actions/org.kde.clashkde.policy

%post
%systemd_post clash-kde-system.service
%systemd_user_post clash-kde.service

%preun
if [ $1 -eq 0 ] && [ -x %{_bindir}/clash-kde ]; then
    systemctl stop clash-kde-system.service >/dev/null 2>&1 || :
    for dir in /run/user/*; do
        uid=${dir##*/}
        printf '%s' "$uid" | grep -Eq '^[0-9]+$' || continue
        [ -S "$dir/bus" ] || continue
        user=$(id -nu "$uid" 2>/dev/null) || continue
        XDG_RUNTIME_DIR=$dir \
        DBUS_SESSION_BUS_ADDRESS=unix:path=$dir/bus \
            runuser -u "$user" -- systemctl --user stop clash-kde.service >/dev/null 2>&1 || :
        XDG_RUNTIME_DIR=$dir \
        DBUS_SESSION_BUS_ADDRESS=unix:path=$dir/bus \
            runuser -u "$user" -- %{_bindir}/clash-kde --restore-proxy >/dev/null 2>&1 || :
    done
fi
%systemd_preun clash-kde-system.service
%systemd_user_preun clash-kde.service

%postun
%systemd_postun_with_restart clash-kde-system.service
%systemd_user_postun clash-kde.service

%files
%license LICENSE
%doc README.md
%{_bindir}/clash-kde
%{_userunitdir}/clash-kde.service
%{_unitdir}/clash-kde-system.service
%{_userpresetdir}/90-clash-kde.preset
%{_datadir}/dbus-1/services/org.kde.clashkde.service
%{_datadir}/dbus-1/system-services/org.kde.clashkde.Admin.service
%{_datadir}/dbus-1/system.d/org.kde.clashkde.Admin.conf
%{_datadir}/polkit-1/actions/org.kde.clashkde.policy
%{_libdir}/qt6/plugins/plasma/kcms/systemsettings/kcm_clashkde.so
%{_libdir}/qt6/plugins/plasma/applets/org.kde.plasma.clashkde.so
%{_datadir}/applications/kcm_clashkde.desktop
%{_datadir}/locale/*/LC_MESSAGES/kcm_clashkde.mo
%{_datadir}/locale/*/LC_MESSAGES/plasma_applet_org.kde.plasma.clashkde.mo

%changelog
* Wed Oct 21 2026 KangxiDictionary <108598928+KangxiDictionary@users.noreply.github.com> - 1.0.2-1
- The Status page can hide its note about the included panel widget.
- Default discards unsaved route, profile, and selection edits.
- The widget route menu no longer collapses while delays are updating.
- A route can be edited beside the list, and the lists sort and search in place.
- This account can open its mixed port to the LAN, and copying an address is a right-click.
- Numbers and dates follow the desktop format. Compact mode applies with the rest of the draft.

* Sun Oct 18 2026 KangxiDictionary <108598928+KangxiDictionary@users.noreply.github.com> - 1.0.1-1
- Closing the system proxy also turns off the GNOME proxy.
- The exit check follows the live path, and the address can be blurred.

* Fri Oct 16 2026 KangxiDictionary <108598928+KangxiDictionary@users.noreply.github.com> - 1.0.0-1
- First stable release. Replaces 0.1.0.

* Fri Oct 16 2026 KangxiDictionary <108598928+KangxiDictionary@users.noreply.github.com> - 0.1.0-1
- First release.
