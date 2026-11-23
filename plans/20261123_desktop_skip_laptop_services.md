# Skip laptop-only services on desktop Macs

## Goal

The installer installs three MacBook-only parts on every T2 Mac. On the
desktop models (iMac, iMac Pro, Mac mini, Mac Pro) they can't work and
keep failing:

- `t2-smc-gaugingd` (battery gauge calibration): no battery, so it exits
  with an error and systemd restarts it every 10 s (138 restarts in 28 min
  on the iMac Pro, 2026-11-23).
- `vort2x-t2-touchid` (Touch ID for fprintd): no sensor; logs "cannot
  reach the sensor" every second. The installer also turns on fingerprint
  login in PAM.
- `t2-force-click` (built-in Force Touch trackpad): looks for the
  trackpad every 5 s and never finds it.

Affects the desktop T2 Macs only; MacBooks install exactly as before.

## Plan

1. `scripts/fedora/lib.sh`: new `is_macbook` (MacBookPro15/16,
   MacBookAir8/9). `has_keyboard_backlight` reuses it (same list).
2. `apps/t2-smc-control/Makefile`: `GAUGING ?= 1`; with `GAUGING=0`,
   `make install` installs the app but not the gauging service, and
   disables and removes it if an earlier install added it.
3. `scripts/fedora/install-apps.sh`: `install_rust_app` passes extra
   arguments to `make install`. SMC Control is installed everywhere, with
   `GAUGING=0` on desktops. Force click is installed on MacBooks only and
   removed on other Macs (same pattern as TrueTone).
4. `scripts/fedora/install-touchid.sh`: on a desktop, skip; if an earlier
   run installed Touch ID, remove it: `vort2x-lifecycle.py t2-touchid
   remove` (stops the service, turns off the fingerprint PAM feature if we
   turned it on and nothing changed since, removes the SELinux module),
   then delete the binary, unit, fprintd drop-in and D-Bus policy.
   `/etc/vort2x/t2-touchid.conf` is kept (a user config file).

## Acceptance criteria

- [x] `bash -n` passes for the three changed scripts.
- [x] `is_macbook` / `has_keyboard_backlight` give "desktop" for
      iMacPro1,1, iMac20,1, Macmini8,1, MacPro7,1 and with no DMI, and
      "macbook" for MacBookPro15,4, MacBookPro16,1, MacBookAir8,2,
      MacBookAir9,1 (function run against a fake product name file).
- [x] `make -n install GAUGING=0` in `apps/t2-smc-control` doesn't
      install or enable the gauging unit, and disables/removes it;
      `make -n install` still installs and enables it.
- [x] Static checks print `STATIC_CHECK_OK` (Fedora 45 container, on a
      clone with the change applied: the worktree's `.git` file doesn't
      resolve inside the container).

Hardware tests for the Tested section (a person):

- iMac Pro (or other desktop) with the three services installed:
  `sudo scripts/fedora/install-apps.sh t2-smc-control t2-force-click` and
  `sudo scripts/fedora/install-touchid.sh` remove the gauging service,
  force click and Touch ID; SMC Control still opens; login and `sudo`
  still work. `with-fingerprint` stays in `authselect current` when
  Fedora set it (see Decisions); it goes only if VorT2x turned it on.
- A MacBook: a normal install still installs and runs all three.

## Decisions and failed attempts

Newest first.

- 2026-11-23: Kirk's test on the iMac Pro: services gone, but
  `authselect current` still lists `with-fingerprint`. Expected: Fedora's
  installer set it (the authselect files date from 13:26 on 2026-11-20,
  before the first boot at 13:27:47 and before VorT2x ran), and
  `enable_pam` never claims a feature that was already on, so removal
  leaves it. Harmless without a reader (`pam_fprintd` finds no device and
  password login carries on). Test wording changed to match.

- 2026-11-23: One helper `is_macbook` for all three, because on T2 Macs
  battery, Touch ID and the built-in trackpad come together (every T2
  MacBook has all three, no desktop has any). The Magic Keyboard with
  Touch ID only works with Apple silicon, and `t2-force-click` only drives
  the built-in trackpad (`t2_precision_trackpad`), so external devices
  don't change this.
- 2026-11-23: SMC Control stays on desktops: it also shows fans,
  temperatures and SMC power, which are useful on the iMac Pro. Only its
  battery service is left out.
- 2026-11-23: Not changed: the gauging daemon still exits with an error
  when there's no battery. With the service gone on desktops it no longer
  matters; a quiet exit could be a later cleanup.
- 2026-11-23: Kirk: "this is an imac, it has no battery, why are we even
  loading this? also there is no touch id keyboard etc". Agreed to do it
  after test 8 of the Vega sleep hunt so that test ran unchanged.

## Tested

Filled in by a person once it's tested on hardware:

- YYYY-MM-DD, name, Mac model, Fedora version: what was checked. Ready to
  merge.
