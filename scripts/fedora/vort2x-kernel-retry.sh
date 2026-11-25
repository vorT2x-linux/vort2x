#!/usr/bin/bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Builds the VorT2x kernel drivers again for kernels whose source download
# failed when they were installed. Offline updates (GNOME Software,
# Discover) install kernels with no network, so the hooks could not fetch
# the kernel source then. Run by vort2x-kernel-retry.timer after boot.
set -u

pending=/var/lib/vort2x-installer/pending-kernel
# Tells the kernel source hook not to mark the kernel again, so a build that
# fails for another reason is tried once, not at every boot
export VORT2X_KERNEL_RETRY=1

for marker in "$pending"/*; do
	[[ -e $marker ]] || continue
	kver=${marker##*/}
	rm -f "$marker"
	[[ -d /usr/lib/modules/$kver ]] || continue
	printf 'rebuilding the VorT2x kernel drivers for %s\n' "$kver"
	for hook in /etc/kernel/install.d/4[4-7]-vort2x-*.install; do
		[[ -x $hook ]] && "$hook" add "$kver"
	done
	dracut --force --kver "$kver" || printf 'dracut for %s failed\n' "$kver"
	if [[ $kver == "$(uname -r)" ]]; then
		printf 'restart the Mac to use the rebuilt drivers for %s\n' "$kver"
	fi
done
