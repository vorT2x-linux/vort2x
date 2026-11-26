#!/usr/bin/env bash

set -Eeuo pipefail

repo_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd -P)
cd "$repo_root"

shell_files=(
	auto-installer/build-in-container.sh
	auto-installer/build-input-kmod.sh
	auto-installer/initramfs/20-vort2x-input.sh.in
	auto-installer/initramfs/90-vort2x-updates.sh
	auto-installer/macos-release-bootstrap.sh.in
	auto-installer/runtime/install-bt-firmware.sh
	auto-installer/runtime/install-wifi-firmware.sh
	auto-installer/runtime/vort2x-install
	auto-installer/runtime/vort2x-launch-terminal
	auto-installer/runtime/vort2x-live-bluetooth
	auto-installer/runtime/vort2x-live-diagnostics
	auto-installer/runtime/vort2x-rescue
	auto-installer/runtime/vort2x-live-wifi
	auto-installer/runtime/vort2x-prepare
	apps/t2-touchbar/install.sh
	apps/t2-touchbar/uninstall.sh
	scripts/fedora/build-installer.sh
	scripts/fedora/install-gdm-branding.sh
	scripts/fedora/install-dkms-modules.sh
	scripts/fedora/install-kernel-args.sh
	scripts/fedora/install-kernel-source.sh
	scripts/fedora/install-gpu-runtime-pm.sh
	scripts/fedora/install-i915.sh
	scripts/fedora/install-thunderbolt.sh
	scripts/fedora/install-wireless.sh
	scripts/fedora/install-rtc-sync.sh
	scripts/fedora/vort2x-kernel-retry.sh
	scripts/fedora/lib.sh
	scripts/macos/download-fedora-iso.sh
	scripts/macos/prepare-fedora-installer.sh
	scripts/tests/edition-catalog.sh
	scripts/tests/iso-download.sh
	scripts/tests/installer-input.sh
	scripts/tests/install-launcher.sh
	scripts/tests/static-check.sh
	scripts/tests/prepare-install.sh
	scripts/tests/release-bootstrap.sh
	scripts/tests/run-step-continuation.sh
	scripts/tests/step-status.sh
	scripts/tests/bt-firmware.sh
	scripts/tests/live-bluetooth.sh
	scripts/tests/rescue.sh
	scripts/tests/live-wifi.sh
	scripts/tests/transition-rpm.sh
	scripts/tests/terminal-launcher.sh
	scripts/tests/wifi-firmware.sh
)
for file in "${shell_files[@]}"; do
	bash -n "$file"
done

if command -v shellcheck >/dev/null 2>&1; then
	shellcheck --severity=warning -x "${shell_files[@]}"
fi

while IFS= read -r file; do
	python3 -c \
		'import ast, pathlib, sys; ast.parse(pathlib.Path(sys.argv[1]).read_text())' \
		"$file"
done < <(git ls-files 'auto-installer/anaconda-addon/*.py' \
	'auto-installer/anaconda-addon/**/*.py')

! grep -rInE --exclude-dir=target 'OEMDRV|rhdd3|inst\.dd=|inst\.ks=|vort2x\.wifi_required' \
	auto-installer/grub.cfg.in \
	auto-installer/initramfs \
	auto-installer/anaconda-addon \
	scripts/macos
! grep -rInE --exclude-dir=target 'brcmfmac(4364|4377).*alias|generic.*brcmfmac|brcmfmac[^ ]*-pcie\.txt' \
	auto-installer
# A VorT2x entry without the input initramfs loses the keyboard it rescues.
awk '
$1 == "linux" && index($0, "${vort2x_common}") { entries++ }
$1 == "initrd" && $2 == "${vort2x_initrd}" { overlays++ }
END { exit !(entries >= 5 && entries == overlays) }
' auto-installer/grub.cfg.in
grep -Fq 'plymouth.enable=0' auto-installer/grub.cfg.in
grep -Fq 'nomodeset' auto-installer/grub.cfg.in
if grep -Eq '^set vort2x_blacklist=.*apple_gmux' auto-installer/grub.cfg.in; then
	exit 1
fi
! grep -InE 'INPUT_COMPAT_PATCH|compat_patch|auto-installer/patches' \
	auto-installer/runtime/vort2x-prepare
grep -Fq '"$transition_source" "$target_kernel" "$work/rpm"' \
	auto-installer/runtime/vort2x-prepare
# The transition modules only live in the initramfs, so they must be forced in.
grep -Fq 'dracut --force --force-drivers' \
	auto-installer/runtime/vort2x-prepare
! grep -Fq -- '--add-drivers' auto-installer/runtime/vort2x-prepare

# DKMS drops every kernel's build before rebuilding any of them, so a kernel
# without headers has to be refused before anything is removed.
grep -Fq 'require_kernel_headers' scripts/fedora/install-dkms-modules.sh
grep -Fq 'require_kernel_headers()' scripts/fedora/lib.sh
grep -Fq 'apps/t2-touchbar/install.sh' scripts/fedora/install-apps.sh
# The Touch Bar daemon is optional and asked for before any step runs.
grep -Fq 'VORT2X_INSTALL_TOUCHBAR' scripts/fedora/install-apps.sh
grep -Fq 'ask_touchbar' scripts/fedora/install.sh
# Only models with a Touch Bar are asked, and the first install must not
# switch the panel before the session can open it.
grep -Fq 'has_touch_bar' scripts/fedora/install.sh
grep -Fq 'has_touch_bar' scripts/fedora/install-apps.sh
# VorT2x Eddy is optional, offered only with GNOME and installed as
# the user who runs sudo.
grep -Fq 'ask_eddy' scripts/fedora/install.sh
grep -Fq 'VORT2X_INSTALL_EDDY' scripts/fedora/install.sh
grep -Fq 'as_eddy_user bash "$SCRIPT_DIR/install-eddy.sh"' scripts/fedora/install.sh
grep -Fq 'device_access_ready' apps/t2-touchbar/install.sh
grep -Fq '"$BIN" --detach' apps/t2-touchbar/uninstall.sh
grep -Fq 'build_cargo_project "$APP_DIR"' apps/t2-touchbar/install.sh
grep -Fq 'env CARGO_TARGET_DIR="$CARGO_TARGET_DIR" make -C "$path" build' scripts/fedora/lib.sh
grep -Fq 'device_group_is_live' apps/t2-touchbar/install.sh
grep -Fq 'vort2x-touchbar --attach' \
	apps/t2-touchbar/integration/systemd/system/vort2x-touchbar-attach.service
# A wrong origin URL must be repaired; dying leaves the user with no way out.
grep -Fq 'repair_origin_url' auto-installer/runtime/vort2x-install
grep -Fq 'repair_origin_url' auto-installer/runtime/vort2x-prepare
! grep -Fq 'unexpected origin URL' \
	auto-installer/runtime/vort2x-install \
	auto-installer/runtime/vort2x-prepare
grep -Fq '/etc/modprobe.d/vort2x-silent-blacklist.conf' \
	scripts/fedora/install-kernel-args.sh
grep -Fq 'ln -sfn vort2x-multicall /usr/local/bin/edit-grub' scripts/fedora/install.sh
grep -Fq 'ln -sfn vort2x-multicall /usr/local/bin/update-grub' scripts/fedora/install.sh
grep -Fq 'ln -sfn vort2x-multicall /usr/local/bin/boot-macos' scripts/fedora/install.sh
grep -Fq 'ln -sfn vort2x-multicall /usr/local/bin/update-modules' scripts/fedora/install.sh
grep -Fq 'ln -sfn vort2x-multicall /usr/local/bin/update-apps' scripts/fedora/install.sh
grep -Fq 'exec sudo nano /etc/default/grub' scripts/fedora/vort2x-multicall
grep -Fq 'exec sudo grub2-mkconfig -o /boot/grub2/grub.cfg' \
	scripts/fedora/vort2x-multicall
grep -Fq "printf 'install %s /bin/true" scripts/fedora/install-kernel-args.sh
grep -Fq '"etc", "xdg", "autostart"' \
	auto-installer/anaconda-addon/com_vort2x_input/service/installation.py
! grep -InE 'find_regular_user|home\.lstrip|os\.chown' \
	auto-installer/anaconda-addon/com_vort2x_input/service/installation.py
grep -Fq 'VORT2X_AUTOSTART_FILE:-/etc/xdg/autostart/vort2x-install.desktop' \
	auto-installer/runtime/vort2x-prepare
! grep -InE '\$HOME/\.config/autostart' \
	auto-installer/runtime/vort2x-install
if grep -rInE --exclude-dir=target 'vort2x-first-boot|VORT2X_FIRST_BOOT' \
		auto-installer; then
	exit 1
fi
# The live Wi-Fi helpers must ride along in the input initramfs and must stay
# inside /run, which never reaches the installed system.
grep -Fq 'usr/lib/vort2x/vort2x-live-wifi' auto-installer/build-in-container.sh
grep -Fq 'usr/lib/vort2x/vort2x-live-wifi.service' \
	auto-installer/build-in-container.sh
grep -Fq 'usr/lib/vort2x/install-wifi-firmware.sh' \
	auto-installer/build-in-container.sh
grep -Fq 'usr/lib/vort2x/vort2x-live-diagnostics' \
	auto-installer/build-in-container.sh
grep -Fq 'runtime_units=/run/systemd/system' \
	auto-installer/initramfs/90-vort2x-updates.sh
grep -Fq 'ExecStart=/run/vort2x/vort2x-live-wifi' \
	auto-installer/runtime/vort2x-live-wifi.service
! grep -rInE --exclude-dir=target 'vort2x-live-wifi' \
	auto-installer/anaconda-addon

grep -Fq 'usr/lib/vort2x/vort2x-rescue' auto-installer/build-in-container.sh
grep -Fq 'vort2x-rescue' auto-installer/initramfs/90-vort2x-updates.sh
grep -Fq '/sysroot/usr/bin/vort2x-rescue' \
	auto-installer/initramfs/90-vort2x-updates.sh
grep -Fq 'apfs' auto-installer/runtime/vort2x-rescue
grep -Fq 'set-default-index' auto-installer/runtime/vort2x-rescue
! grep -Fq -- '--set-default ' auto-installer/runtime/vort2x-rescue
! grep -rInE --exclude-dir=target 'vort2x-rescue' \
	auto-installer/anaconda-addon

# Bluetooth firmware is loaded from disk by BCM4377 alone. Every entry point has
# to check for that PCI function, and the UART .hcd path must stay out of here.
grep -Fq '0x5fa0' auto-installer/runtime/install-bt-firmware.sh
grep -Fq '0x5fa0' auto-installer/runtime/vort2x-live-bluetooth
grep -Fq '0x5fa0' \
	auto-installer/anaconda-addon/com_vort2x_input/service/installation.py
grep -Fq 'BCM4377' scripts/macos/prepare-fedora-installer.sh
! grep -rInE --exclude-dir=target '\.hcd' auto-installer scripts/macos
grep -Fq 'usr/lib/vort2x/install-bt-firmware.sh' \
	auto-installer/build-in-container.sh
grep -Fq 'usr/lib/vort2x/vort2x-live-bluetooth' \
	auto-installer/build-in-container.sh
grep -Fq 'usr/lib/vort2x/vort2x-live-bluetooth.service' \
	auto-installer/build-in-container.sh
grep -Fq 'ExecStart=/run/vort2x/vort2x-live-bluetooth' \
	auto-installer/runtime/vort2x-live-bluetooth.service
! grep -rInE --exclude-dir=target 'vort2x-live-bluetooth' \
	auto-installer/anaconda-addon

grep -Fq 'Do not close this window!' auto-installer/runtime/vort2x-install
grep -Fq 'Ensure that you are connected to Wi-Fi before continuing.' \
	auto-installer/runtime/vort2x-install
grep -Fq 'Press any key to continue.' auto-installer/runtime/vort2x-install

# macOS Bash 3.2 treats an expanded empty array as unbound under `set -u`.
grep -Fq 'ORIGINAL_ARGC=$#' scripts/macos/prepare-fedora-installer.sh
grep -Fq 'if ((ORIGINAL_ARGC == 0)); then' scripts/macos/prepare-fedora-installer.sh
grep -Fq 'plist_value "$disk" WholeDisk' scripts/macos/prepare-fedora-installer.sh
! grep -Fq 'plist_value "$disk" Whole ' scripts/macos/prepare-fedora-installer.sh
grep -Fq 'The ISO was verified OK.' scripts/macos/prepare-fedora-installer.sh
grep -Fq 'Next steps:' scripts/macos/prepare-fedora-installer.sh
grep -Fq 'Select the orange EFI Boot entry for this USB drive.' \
	scripts/macos/prepare-fedora-installer.sh
grep -Fq 'The VorT2x installation will continue automatically in a terminal.' \
	scripts/macos/prepare-fedora-installer.sh
grep -Fq 'Good: Secure Boot has been disabled.' \
	scripts/macos/prepare-fedora-installer.sh
grep -Fq 'Set Secure Boot to No Security.' \
	scripts/macos/prepare-fedora-installer.sh
grep -Fq 'Allow booting from external or removable media.' \
	scripts/macos/prepare-fedora-installer.sh
grep -Fq 'reconnect the USB drive and retry with --reuse-media' \
	scripts/macos/prepare-fedora-installer.sh
grep -Fq 'source "$SCRIPT_DIR/download-fedora-iso.sh"' \
	scripts/macos/prepare-fedora-installer.sh
grep -Fq 'scripts/macos/download-fedora-iso.sh' \
	auto-installer/build-in-container.sh
grep -Fq 'FEDORA_METALINK=' auto-installer/targets/fedora-45.conf
grep -Fq 'FEDORA_ARCHIVE_BASEURL=' auto-installer/targets/fedora-45.conf
! grep -rIn 'FEDORA_BASEURL\|dl.fedoraproject.org/pub/fedora/linux/releases' \
	auto-installer/targets scripts/fedora auto-installer/build-in-container.sh
! grep -Fq 'Keep no second driver disk connected' scripts/macos/prepare-fedora-installer.sh
! grep -Fq 'before the intentional EFI customization' scripts/macos/prepare-fedora-installer.sh
grep -Fq 'shasum -a 256 -c' auto-installer/macos-release-bootstrap.sh.in
grep -Fq 'VORT2X_TTY:-/dev/tty' auto-installer/macos-release-bootstrap.sh.in
grep -Fq 'uses: actions/upload-artifact@v6' .github/workflows/installer.yml
grep -Fq 'uses: actions/download-artifact@v7' .github/workflows/installer.yml
! grep -InE 'uses: actions/(checkout|upload-artifact|download-artifact)@v4' \
	.github/workflows/installer.yml

bash scripts/tests/installer-input.sh

bash scripts/tests/wifi-firmware.sh
bash scripts/tests/bt-firmware.sh
bash scripts/tests/live-wifi.sh
bash scripts/tests/live-bluetooth.sh
bash scripts/tests/rescue.sh
bash scripts/tests/prepare-install.sh
bash scripts/tests/run-step-continuation.sh
bash scripts/tests/step-status.sh
bash scripts/tests/install-launcher.sh
bash scripts/tests/release-bootstrap.sh
bash scripts/tests/terminal-launcher.sh
bash scripts/tests/iso-download.sh
bash scripts/tests/edition-catalog.sh
bash scripts/tests/cargo-lock-alignment.sh
printf 'Installer static checks passed.\n'
