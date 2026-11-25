# Wake the T2 USB root hub when bridgeOS reports a device

## Goal

On some boots the FaceTime camera (and the other T2 USB devices: ambient
light sensor, headset, SideCar AVE, and the Touch Bar on laptops) never
appears. The virtual USB bus from `apple-t2-vhci` (bus 7 on the iMac Pro)
comes up empty, usbcore autosuspends the empty root hub within a few
milliseconds, and bridgeOS reports its devices about 200 ms later. The
port-change event only sets a bit in `port_change_pending`; a suspended
root hub is not polled, so the devices are never enumerated until something
wakes the hub. Whether the camera works depends on which side wins the race.

Affects every T2 Mac using `apple-t2-bridge`.

## Plan

1. `modules/apple-t2-bridge/drivers/usb/host/apple-t2-vhci.c`: on a
   `BCE_VHCI_EV_PORT_STATUS_CHANGE` event, wake the root hub with
   `usb_hcd_resume_root_hub()` if it is suspended, otherwise poll it at
   once with `usb_hcd_poll_rh_status()` (new helper
   `bce_vhci_notify_port_change()`). This is what other host controller
   drivers do on a port change.
2. System sleep is unaffected: the driver's PM `.prepare` callback pauses
   the event queues before any suspend callback runs, so no port-change
   event can arrive during system suspend.

## Acceptance criteria

- [x] Static checks pass (`vort2x-tools/check.sh` in Fedora 45: `STATIC_CHECK_OK`)
- [x] `apple-t2-bridge` builds against 7.3.4-300.fc45 with `W=1`, no
  warnings from the changed file

## Decisions and failed attempts

- 2026-11-25: Found on Kirk's iMac Pro. In this boot's log the VHCI bus
  logged `bus_suspend entry` 5 ms after `device initialized` and no
  `usb 7-*` device ever appeared; in the four earlier boots the devices
  appeared 200-300 ms after init with no suspend in between.
  `/sys/bus/usb/devices/usb7/power/runtime_status` was `suspended`.
  Writing `on` to `usb7/power/control` (forcing the hub awake) enumerated
  the camera, light sensor, headset and SideCar AVE at once, which
  confirms the cause.
- 2026-11-25: Chose the wake-on-event fix over disabling root hub
  autosuspend (`pm_runtime_forbid`): it fixes the missing wakeup itself,
  also covers devices that appear later, and keeps runtime PM.

## Tested

Filled in by a person once it's tested on hardware:

- YYYY-MM-DD, name, Mac model, Fedora version: what was checked. Ready to
  merge.
