%global _build_id_links none
%global debug_package %{nil}

Name:           kmod-vort2x-input
Version:        0.1
Release:        2%{?dist}
Summary:        Temporary T2 Mac input drivers for the Fedora installer kernel
License:        GPL-2.0-only OR GPL-2.0-or-later
URL:            https://github.com/vorT2x-linux/vort2x
Source0:        vort2x-input-%{version}.tar.gz

BuildRequires:  elfutils-libelf-devel
BuildRequires:  gcc
BuildRequires:  make
Requires:       kernel-core-uname-r = %{kernel_release}
Provides:       kernel-modules = %{kernel_release}
AutoReqProv:    no

%description
Input drivers used by Anaconda and the first Fedora boot on T2 Macs. The full
VorT2x installer replaces this package with DKMS modules for an updated kernel.

%prep
%setup -q -n vort2x-input-%{version}

%build
make -C modules/apple-t2-bridge KVERSION=%{kernel_release}
make -C modules/t2touchbar KDIR=/usr/src/kernels/%{kernel_release} KVER=%{kernel_release}
make -C modules/hid_t2magicmouse KDIR=/usr/src/kernels/%{kernel_release} KVER=%{kernel_release}
make -C modules/t2_precision_trackpad KDIR=/usr/src/kernels/%{kernel_release} KVER=%{kernel_release}
make -C modules/t2_trackpad_actuator KDIR=/usr/src/kernels/%{kernel_release} KVER=%{kernel_release}

%install
install -d -m 0755 %{buildroot}/usr/lib/modules/%{kernel_release}/updates/vort2x
install -m 0644 modules/apple-t2-bridge/drivers/platform/x86/apple/apple-t2-bridge.ko \
    %{buildroot}/usr/lib/modules/%{kernel_release}/updates/vort2x/
install -m 0644 modules/apple-t2-bridge/drivers/usb/host/apple-t2-vhci-hcd.ko \
    %{buildroot}/usr/lib/modules/%{kernel_release}/updates/vort2x/
install -m 0644 modules/t2touchbar/t2hid.ko \
    %{buildroot}/usr/lib/modules/%{kernel_release}/updates/vort2x/
install -m 0644 modules/hid_t2magicmouse/hid_t2magicmouse.ko \
    %{buildroot}/usr/lib/modules/%{kernel_release}/updates/vort2x/

install -m 0644 modules/t2_precision_trackpad/t2_precision_trackpad.ko \
    %{buildroot}/usr/lib/modules/%{kernel_release}/updates/vort2x/
install -m 0644 modules/t2_trackpad_actuator/t2_trackpad_actuator.ko \
    %{buildroot}/usr/lib/modules/%{kernel_release}/updates/vort2x/

%post
/usr/sbin/depmod -a %{kernel_release} >/dev/null 2>&1 || :

%postun
/usr/sbin/depmod -a %{kernel_release} >/dev/null 2>&1 || :

%files
%license LICENSE
/usr/lib/modules/%{kernel_release}/updates/vort2x/*.ko

%changelog
* Tue Nov 03 2026 VorT2x maintainers <maintainers@vort2x.invalid> - 0.1-2
- Include the precision trackpad and haptic actuator drivers

* Tue Jul 14 2026 VorT2x maintainers <maintainers@vort2x.invalid> - 0.1-1
- Initial Fedora installer transition package
