#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Checks a stick image made with `vort2x-usb --target-file` against the Fedora
# ISO, the installer kit and the firmware it was given. Needs a real ISO, so
# it is run by hand (see README.md), on Linux with mtools, cpio and sfdisk.
#
# Usage: check-image.sh ISO KIT_DIR FIRMWARE_DIR IMAGE

set -Eeuo pipefail

(($# == 4)) || {
	printf 'Usage: %s ISO KIT_DIR FIRMWARE_DIR IMAGE\n' "${0##*/}" >&2
	exit 2
}
iso=$1 kit=$2 firmware=$3 image=$4
work=$(mktemp -d "${TMPDIR:-/tmp}/vort2x-usb-check.XXXXXX")
trap 'rm -rf "$work"' EXIT
export MTOOLS_SKIP_CHECK=1

fail() {
	printf 'FAIL: %s\n' "$*" >&2
	exit 1
}

# Size: the image is exactly the ISO, nothing appended.
iso_size=$(stat -c %s "$iso")
[[ $(stat -c %s "$image") == "$iso_size" ]] || fail 'image size differs from the ISO'

# The EFI System partition, from the ISO's own partition table.
read -r esp_start esp_sectors < <(
	sfdisk --json "$iso" |
		python3 -c '
import json, sys
table = json.load(sys.stdin)["partitiontable"]
esp = [p for p in table["partitions"]
       if p.get("type", "").upper() in ("C12A7328-F81F-11D2-BA4B-00A0C93EC93B", "EF")]
assert len(esp) == 1, "expected one EFI System partition"
print(esp[0]["start"], esp[0]["size"])
'
)
offset=$((esp_start * 512))
end=$(((esp_start + esp_sectors) * 512))

# Outside the EFI partition the image is byte-identical to the ISO.
cmp -s <(head -c "$offset" "$iso") <(head -c "$offset" "$image") ||
	fail 'image differs from the ISO before the EFI partition'
cmp -s -i "$end" "$iso" "$image" || fail 'image differs from the ISO after the EFI partition'

esp_file() {
	mcopy -n -i "$1@@$offset" "::$2" "$3"
}

# grub.cfg is the kit's template with the ISO's volume label.
label=$(dd if="$iso" bs=1 skip=32808 count=32 status=none | tr -d ' ')
sed "s|@ISO_VOLUME_LABEL@|$label|g" "$kit/grub.cfg.in" >"$work/grub.expected"
esp_file "$image" EFI/BOOT/grub.cfg "$work/grub.cfg"
cmp -s "$work/grub.expected" "$work/grub.cfg" || fail 'grub.cfg is not the rendered kit template'

# Fedora's own grub.cfg is kept.
esp_file "$iso" EFI/BOOT/grub.cfg "$work/fedora.cfg"
esp_file "$image" EFI/BOOT/grub.cfg.fedora-original "$work/original.cfg"
cmp -s "$work/fedora.cfg" "$work/original.cfg" || fail 'grub.cfg.fedora-original is not the ISO grub.cfg'

esp_file "$image" EFI/BOOT/vort2x/vort2x-input-initramfs.img "$work/input.img"
cmp -s "$kit/vort2x-input-initramfs.img" "$work/input.img" || fail 'input initramfs differs from the kit'

# The firmware initramfs holds exactly the given files, with valid checksums.
esp_file "$image" EFI/BOOT/vort2x/vort2x-wifi-initramfs.img "$work/wifi.img"
esp_file "$image" EFI/BOOT/vort2x/wifi-firmware.sha256 "$work/wifi.sha256"
mkdir "$work/unpacked"
(cd "$work/unpacked" && gzip -dc "$work/wifi.img" | cpio -idm --quiet)
fw="$work/unpacked/vort2x-wifi-firmware"
(cd "$fw" && sha256sum --quiet -c SHA256SUMS) || fail 'SHA256SUMS in the firmware initramfs does not match'
cmp -s "$fw/SHA256SUMS" "$work/wifi.sha256" || fail 'wifi-firmware.sha256 differs from SHA256SUMS'
for file in "$firmware"/*; do
	[[ -f "$file" ]] || continue
	cmp -s "$file" "$fw/${file##*/}" || fail "firmware file differs: ${file##*/}"
done

printf 'IMAGE_OK %s\n' "$(sha256sum "$image" | cut -d' ' -f1)"
