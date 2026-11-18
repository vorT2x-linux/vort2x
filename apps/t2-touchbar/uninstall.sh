#!/usr/bin/env bash
#
# Removes vort2x-touchbar and hands the Touch Bar back to Apple's native
# firmware row. Per-user configuration and learned state, the
# adwaita-sans-fonts package and previously disabled third-party Touch Bar
# daemons (tiny-dfr, react-drm) are left as they are.
set -Eeuo pipefail

APP_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)
source "$APP_DIR/../../scripts/fedora/lib.sh"

require_root
require_repo_root
require_fedora
require_command make systemctl udevadm

BIN=/usr/local/bin/vort2x-touchbar
UDEV_RULE=/usr/local/lib/udev/rules.d/99-zz-vort2x-touchbar.rules

target_user="${SUDO_USER:-}"
target_uid=
if [[ -n "$target_user" && "$target_user" != root ]]; then
	target_uid=$(id -u "$target_user")
fi

user_systemctl() {
	local runtime="/run/user/$target_uid"
	[[ -n "$target_uid" && -S "$runtime/bus" ]] || return 1
	sudo -H -u "$target_user" env \
		XDG_RUNTIME_DIR="$runtime" \
		DBUS_SESSION_BUS_ADDRESS="unix:path=$runtime/bus" \
		systemctl --user "$@"
}

stop_services() {
	user_systemctl stop vort2x-touchbar.service || true
	systemctl --global disable vort2x-touchbar.service 2>/dev/null || true
	# Sessions of other users run their own instance of the global unit.
	pkill -x vort2x-touchbar || true
	systemctl disable --now vort2x-touchbar-attach.service 2>/dev/null || true
}

restore_firmware_row() {
	# Drop the rule first: it would otherwise switch the device straight back
	# to the DRM configuration as soon as the firmware configuration appears.
	rm -f "$UDEV_RULE"
	udevadm control --reload
	if [[ -x "$BIN" ]]; then
		"$BIN" --detach
	else
		warn "$BIN is missing. Reboot to restore the native Touch Bar row"
	fi
}

remove_files() {
	make -C "$APP_DIR" uninstall
	rm -f /etc/vort2x/touchbar.toml
	rmdir --ignore-fail-on-non-empty /etc/vort2x 2>/dev/null || true
	systemctl daemon-reload
	user_systemctl daemon-reload || true
}

restore_device_access() {
	udevadm trigger --subsystem-match=drm --action=add
	udevadm trigger --subsystem-match=input --action=add
	udevadm trigger --subsystem-match=backlight --action=add
	udevadm trigger --subsystem-match=hid --action=add
	udevadm trigger --subsystem-match=misc --action=add
	udevadm settle
	if getent group vort2x-touchbar >/dev/null; then
		groupdel vort2x-touchbar
	fi
}

run_step "stop vort2x-touchbar services" stop_services
run_step "restore the native Touch Bar row" restore_firmware_row
run_step "remove vort2x-touchbar files" remove_files
run_step "restore Touch Bar device access" restore_device_access

info "vort2x-touchbar removed. The native Touch Bar row is active"
if [[ -n "$target_user" ]]; then
	info "personal settings and learned state were kept in ~$target_user/.config/vort2x-touchbar and ~$target_user/.local/state/vort2x-touchbar"
fi
