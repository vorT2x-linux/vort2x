#!/bin/sh

source_image=/vort2x-anaconda-updates.img
target_directory=/run/vort2x

if [ ! -f "$source_image" ]; then
    warn "VorT2x: Anaconda updates image is missing"
    exit 1
fi

mkdir -p "$target_directory"
cp "$source_image" "$target_directory/updates.img"

firmware_source=/vort2x-wifi-firmware
helper_source=/usr/lib/vort2x
runtime_units=/run/systemd/system

if [ -d "$firmware_source" ]; then
    mkdir -p "$target_directory/apple-firmware"
    cp -r "$firmware_source"/. "$target_directory/apple-firmware/"

    # /run survives the switch root, so the live session can install this
    # firmware for itself. Nothing placed here reaches the installed system.
    for helper in install-wifi-firmware.sh install-bt-firmware.sh \
        vort2x-live-wifi vort2x-live-bluetooth vort2x-live-diagnostics \
        vort2x-rescue; do
        if [ -f "$helper_source/$helper" ]; then
            cp "$helper_source/$helper" "$target_directory/$helper"
            chmod 0755 "$target_directory/$helper"
        else
            warn "VorT2x: live firmware helper is missing: $helper"
        fi
    done

    # /run/vort2x is in no PATH, and sudo only honours secure_path anyway.
    if [ -d /sysroot/usr/bin ]; then
        ln -sf "$target_directory/vort2x-rescue" \
            /sysroot/usr/bin/vort2x-rescue || \
            warn "VorT2x: could not link the rescue tool into the live PATH"
    fi

    mkdir -p "$runtime_units/multi-user.target.wants"
    for unit_name in vort2x-live-wifi.service vort2x-live-bluetooth.service; do
        if [ -f "$helper_source/$unit_name" ]; then
            cp "$helper_source/$unit_name" "$runtime_units/$unit_name"
            chmod 0644 "$runtime_units/$unit_name"
            ln -sf "../$unit_name" \
                "$runtime_units/multi-user.target.wants/$unit_name"
        else
            warn "VorT2x: live service unit is missing: $unit_name"
        fi
    done
fi
