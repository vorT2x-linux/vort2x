# Thunderbolt Display stays dark after sleep (iMac Pro)

## Goal

On the iMac Pro (iMacPro1,1, two Alpine Ridge JHL6540 controllers) sleep
and wake work, but the Apple Thunderbolt Display stays dark after wake
until a reboot. Make it come back after every wake. Affects the iMac Pro;
other Macs only if they also boot with `pcie_ports=compat`.

## What the logs show (boots of 2026-11-23 and 2026-11-24, all 3 sleeps)

- Suspend, noirq phase: both NHIs (`08:00.0`, `4e:00.0`) and both TB xHCIs
  (`09:00.0`, `4f:00.0`) are already "inaccessible" when the PCI core tries
  D0 -> D3hot. The controllers lose power during suspend (expected in S3).
- Resume, noirq phase: the bridges under the root ports `05:02.0`
  (`\_SB.PC02.BR2C`) and `05:03.0` (`BR2D`) are restored at once and are
  still inaccessible; `tb_resume_noirq` fails on every access
  (`tb_cfg_read/write: -108`, 22 warnings). Nothing re-enumerates the
  controllers afterwards, so the display never comes back.
- The root ports have no driver: `pcie_ports=compat` (iMacPro1,1 only,
  `install-kernel-args.sh`, "hangs in the SMP bring up on resume") keeps
  pcieport off them. With pcieport the PCI core would wait for the link
  after resume and pciehp would re-enumerate.
- The stock Fedora `thunderbolt.ko` is loaded; VorT2x's thunderbolt patch
  is for Titan Ridge/Ice Lake only.
- Side effect: cosmic-comp's `surface-DP-5` thread panicked on resume
  (`src/shell/mod.rs:1391`, unwrap on None) when the output vanished.

## Plan

1. Test A (current setup): after a wake with the display dark, remove the
   two TB hierarchies and rescan the PCI bus by hand. Shows whether the
   controllers have power again and whether re-enumeration is enough.
2. Test B (one boot, no permanent change): boot without
   `pcie_ports=compat` (edit the line in GRUB with `e`), sleep and wake.
   Shows whether the SMP hang still happens with `t2smp` and whether
   pcieport/pciehp bring the display back on their own.
3. Pick the fix from the results:
   - B resumes fine and the display returns: drop `pcie_ports=compat` for
     the iMac Pro in `install-kernel-args.sh` (and remove it on update).
   - B hangs, A works: keep compat, re-enumerate the TB controllers after
     resume (sleep hook or small kernel-side wait), whichever is simpler.
   - Neither works: look at powering the controller back up (ACPI).

## Acceptance criteria

- [ ] `bash scripts/tests/static-check.sh` passes (podman, Fedora 45)
- [ ] (filled in once the fix is chosen)

## Hardware tests (write results here before the next step)

### Test A: re-enumerate by hand

1. Display connected, logged in. `systemctl suspend`, wait 30 s, wake.
2. Display dark? Note it. Then run:

   ```bash
   sudo sh -c 'echo 1 > /sys/bus/pci/devices/0000:06:00.0/remove; \
     echo 1 > /sys/bus/pci/devices/0000:4c:00.0/remove; sleep 2; \
     echo 1 > /sys/bus/pci/rescan'
   ```

3. Wait 10 s. Note: does the display come back? Then tell Claude the time;
   Claude reads the journal.

Result (2026-11-24, Kirk, suspend 11:26:14, wake 11:27:51, rescan 11:28:09):
display stayed dark. Removal worked (`thunderbolt 0-3: device
disconnected`, buses 07 and 4d released) but the rescan found nothing.
Both root ports report link width 0: the controllers have no power after
wake, so re-enumerating alone cannot fix it. The ports have no ACPI power
resources (`_PR0`/`_PR3`); the power switch must be in Apple's methods
(SSDT `TbtPEG23`?). Next: dump the ACPI tables and read them.

### ACPI findings (tables dumped 2026-11-24, SSDT `TbtPEG23` = SSDT10)

- TB power is two GPIOs driven by `\_SB.PC02.BR2C.UGIO`: `0x0103000F`
  (controller) and `0x01030010` (its xHCI); BR2D has its own copy. `UGIO`
  turns a line on with `SGDI` (then waits 500 ms) and off with
  `SGOV(x,0)`+`SGDO`.
- Linux answers `_OSI("Darwin")` (DMI, Apple hardware), so `OSDW` is true.
  Under Darwin `_WAK` does not repower TB (only the non-Darwin path calls
  `ICMB`); macOS repowers through `_PS0`: `UPSB._PS0` -> `PCED` (sets
  `GPCI`, calls `UGIO`, waits for link training and the upstream port to
  answer) and `NHI0._PS0` -> `PCED` (sets `GNHI`, `UGIO`, waits for the NHI).
- Linux only runs `_PS0` when it powers a device up from a low state. With
  `pcie_ports=compat` the TB bridges have no driver, stay in D0 through
  suspend, and on resume `acpi_device_set_power(D0)` is a no-op: `_PS0`
  never runs and the power stays off. Without compat, pcieport binds them,
  they go to D3 at suspend and resume calls `_PS0`, so Test B should bring
  the display back, unless the old SMP hang returns.

### Test B: one boot without pcie_ports=compat

1. Reboot. In the GRUB menu press `e` on the Fedora entry, delete
   `pcie_ports=compat` from the `linux` line, press Ctrl-X to boot.
   (Only this boot; the next boot has it again.)
2. Check the display works, then `systemctl suspend`, wait 30 s, wake.
3. Note: does the Mac wake at all (or hang/black on both screens)? Does
   the Thunderbolt Display come back? If it hangs, hold the power button
   to turn it off; the next boot is back to normal.

Result: (pending)

### Test C: run the firmware's TB power-on by hand after wake

Throwaway module `tbps0.ko` (not in the repo; built in Claude's scratchpad,
`tbps0/tbps0.c`). On load it evaluates `\OSDW`, then
`\_SB.PC02.BR2C.UPSB._PS0`, `...BR2C.UPSB.DSB0.NHI0._PS0` and the same two
for BR2D, logs each result, and rescans PCI bus 05. ACPI read: `UPSB._PS0`
-> `PCED` sets `GPCI`, `UGIO` switches both GPIOs on, then waits up to
10 s for link training and for the upstream port to answer; `NHI0._PS0`
waits for the NHI. Normal boot with `pcie_ports=compat`.

1. Display connected, logged in. `systemctl suspend`, wait 30 s, wake.
2. Remove the dead TB hierarchies (as in Test A), then load the module:

   ```bash
   sudo sh -c 'echo 1 > /sys/bus/pci/devices/0000:06:00.0/remove; \
     echo 1 > /sys/bus/pci/devices/0000:4c:00.0/remove; sleep 2; \
     insmod <scratchpad>/tbps0/tbps0.ko'
   ```

3. Wait 15 s. Note: does the display come back? Tell Claude the time.

Result (2026-11-24, Kirk, wake 11:47:05, insmod 11:47:27): display stayed
dark. All four `_PS0` ran (`OSDW` = 1, so the Darwin path), but each took
exactly 1 s: the 1 s timeout waiting for the upstream port, with no
retry. That only happens when `UGIO` did not switch power on (`PRSR`
stayed 0), i.e. `GGDV` already reported the pad as "on". `GGDV` returns 1
for every pad state except output mode (`TEMP` = 2), so a pad reset during
S3 reads as on while the controller is off. Root ports still link width 0.

### Test D: power-cycle the TB GPIOs by hand, then `_PS0`

Same throwaway module, rewritten. For BR2C (pads `0x0103000F`,
`0x01030010`) and BR2D (`0x0103000D`, `0x0103000E`): log
`GGDV`/`GGIV`/`GGOV` per pad, drive the pads low (`SGOV`+`SGDO`, like
`PCDA`), wait 100 ms, release them (`SGDI`, like `UGIO`), wait 500 ms, log
again plus root port link width, then the four `_PS0` with timings, link
width again, rescan bus 05.

1. Still after Test C's wake (TB devices already gone):
   `sudo sh -c 'rmmod tbps0; insmod <scratchpad>/tbps0/tbps0.ko'`
2. Wait 15 s. Note: does the display come back?

Result (2026-11-24, Kirk, insmod 11:50:51): display stayed dark. All four
pads read `GGDV=1 in=1 out=0` (released, line high = "on") before and
after the toggle. Root ports link width 0 throughout; each `_PS0` again
took ~1010 ms (timeout). So either the pads are on and power is not the
problem, or the pads are no longer in GPIO mode after S3 and the toggle
did nothing. `GGDV` cannot tell the two apart.

### Test E: raw pad registers and a link bounce

Module rewritten again (param `act`). It always logs each pad's raw DW0
(`\_SB.GPC0`: pad mode, RX/TX disable, RX/TX state, reset config) and the
root ports' LNKCTL/LNKSTA/SLTSTA. With `act=1` it then sets and clears
Link Disable on 05:02.0/05:03.0, retrains, logs again and rescans.

1. Now (still after the S3 wake):
   `sudo sh -c 'rmmod tbps0; insmod <scratchpad>/tbps0/tbps0.ko act=1'`
   Note whether the display comes back.
2. Later, after a fresh reboot with the display working, before any
   sleep: `sudo insmod <scratchpad>/tbps0/tbps0.ko` (read-only) to get
   the good-state values to compare.

Result (2026-11-24, Kirk, insmod 11:52:58): display stayed dark. All four
pads `DW0=0x04000102`: GPIO mode, reset config RSMRST (kept through S3),
TX enabled driving 0, RX disabled. So `SGDI` = "enable output" and the
power enable is active low: the pads are **on**, exactly as the firmware
wants. Root ports: LNKCTL 0x0040 (Link Disable clear), LNKSTA 0x1001
(no training, DLLLA 0, width 0), SLTSTA 0x0040 (presence detect set).
Link bounce + retrain changed nothing. Power is not the problem.

Re-reading the suspend log (11:47:05): both NHIs and both TB xHCIs are
already gone in the **suspend** noirq phase, before `Preparing to enter
system sleep state S3`. The thunderbolt driver's `suspend_noirq` runs
just before the PCI core's D3hot attempt, so the controller drops off
when the driver puts it to sleep, not because S3 cuts power. A sleeping
Alpine Ridge with no ICM and no `_PS0` power cycle never wakes up.

### Test F: keep the thunderbolt driver from putting it to sleep

Remove the TB hierarchies before suspend, rescan after wake. If this
works, the fix is a small sleep hook (`vort2x-suspend.sh`).

1. Reboot (normal, compat stays). Check the display works.
2. Control, no sleep: run the remove command, wait 5 s, then
   `sudo sh -c 'echo 1 > /sys/bus/pci/rescan'`. Note: display back?
   Remove command:
   `sudo sh -c 'echo 1 > /sys/bus/pci/devices/0000:06:00.0/remove; echo 1 > /sys/bus/pci/devices/0000:4c:00.0/remove'`
3. Remove command again, then `systemctl suspend`, wait 30 s, wake, then
   the rescan command. Note: display back?

Result step 2 (2026-11-24, Kirk, boot 11:57, remove 11:58:52, rescan
12:01:38): works. Removing the bridges does not blank the display (the DP
tunnel keeps running in hardware; root ports stay at x4 8 GT/s). The
rescan found 06:00.0 and 4c:00.0 again, the display flicked off and on
within ~1 s and works, boltctl shows it authorized. Only harmless I/O
window "failed to assign" messages.

Result step 3 (2026-11-24, Kirk, remove 12:03:06, suspend 12:03:25,
wake 12:03:57): display went dark during sleep and did not come back
after the rescan. With the thunderbolt driver gone before suspend, both
root ports are still at link width 0 after wake (05:02.0 and 05:03.0
`width=0 speed=2.5 GT/s`) and the rescan finds nothing. So the driver's
suspend is **not** the cause: S3 itself leaves the controllers off the
link, and Test E's reading (driver puts it to sleep) is wrong.

### Test G: long power cycle of the TB pads after wake

Module rebuilt in `~/tb-debug/tbps0/` (outside /tmp so a reboot keeps it;
not in the repo). Params: `act=0` logs each pad's DW0 (`\_SB.GPC0`) and
the root ports' LNKSTA/width only; `act=1` then drives the four pads low
the firmware's "off" way (`SGOV(pad,0)` + `SGDO`), keeps them off for
`off_ms` (default 3000), releases them (`SGDI`, firmware "on"), waits
500 ms, runs the four `_PS0` with timings, logs, rescans bus 05. Logs at
start / off / off-end / on+500ms / after-ps0.

Note on Test E: by the Intel PCH DW0 layout (bit 0 TX state, 1 RX state,
8 TX disable, 9 RX disable), `0x04000102` is **TX disabled, RX enabled,
line reads high**, not "TX enabled driving 0". `SGDI` probably means "set
direction input": the firmware's "on" is releasing the pad so a pull-up
holds the enable high, "off" is driving it low. Conclusion (pads on)
unchanged; Test D's 100 ms "off" may have been too short.

1. Fresh reboot, display working, **before any sleep**. Dump the ACPI
   tables and log the good state:
   ```bash
   sudo sh -c 'cd ~kirk/tb-debug/acpi && acpidump -b && chown kirk *.dat; \
     insmod ~kirk/tb-debug/tbps0/tbps0.ko; rmmod tbps0'
   ```
   Tell Claude; Claude checks the method names/arguments in the tables
   and compares the good-state DW0/link with Test E.
2. `systemctl suspend`, wait 30 s, wake (display dark). Then:
   ```bash
   sudo sh -c 'echo 1 > /sys/bus/pci/devices/0000:06:00.0/remove; \
     echo 1 > /sys/bus/pci/devices/0000:4c:00.0/remove; sleep 2; \
     insmod ~kirk/tb-debug/tbps0/tbps0.ko act=1'
   ```
   (Takes ~4 s plus up to ~4 s of `_PS0`.) Wait 15 s. Note: display back?

Result step 1 (2026-11-24, Kirk, boot 12:09:44, insmod 12:11:00, before
any sleep): good state = both controllers' pads `DW0 0x4000102` (rx 1),
**the same as after the S3 wake in Test E**, root ports `LNKSTA 0xb043`
width 4 (DLLLA set). So the pads look identical whether TB works or not;
the difference is only the link. ACPI dump in `~/tb-debug/acpi/` (23
tables). Checked in the DSDT/SSDT10: `GPC0(pad)` returns DW0; `SGOV(pad,
v)` sets bit 0 (TX state); `SGDO` writes 2 to bits 8-9 (TX enabled, RX
disabled = output); `SGDI` writes 1 (TX disabled, RX enabled = input).
The firmware's off (`PCDA`) is `SGOV(pad,0)`+`SGDO(pad)` for
`0x0103000F`/`0x01030010` (BR2C) and `0x0103000D`/`0x0103000E` (BR2D),
then `UGIO` turns them back on with `SGDI`. The module matches all of
this; step 2 can run as written.

Result step 2 (2026-11-24, Kirk, suspend 12:12:28, wake 12:13:19, insmod
12:13:40): display stayed dark. The pads did switch: `[off]` DW0
`0x4000200` (driven low, line reads 0) for 3 s, then back to `0x4000102`.
Root ports `LNKSTA 0x1001` width 0 at every step; each `_PS0` again ~1010
ms (timeout). A real 3 s power cycle of the TB enables does not bring
the link back either. Power is ruled out for good.

### ACPI: the firmware's own S3 path for TB is the non-Darwin one

`OSDW` = `OSYS == 0x2710` (Darwin). Linux sets `_OSI("Darwin")` on Apple
hardware via DMI (boot log: "DMI detected to setup _OSI(Darwin)"), so the
firmware runs its macOS paths. Under **non-Darwin** it does the S3 work
itself: `BR2C/BR2D._INI` saves the root port and TB bridge config and sets
`TBG2`/`TBG3`; `_WAK` then calls `BR2C.ICMB()`/`BR2D.ICMB()`, which
restores that config and starts the controller's firmware connection
manager (ICM, `ICMS`). Under Darwin `_WAK` does nothing for TB; macOS's
own TB driver handles it, which Linux's native-mode driver does not.
Linux's thunderbolt driver supports ICM on Alpine Ridge.

### Test H: one boot with `acpi_osi=!Darwin`

Makes Linux stop claiming to be macOS for one boot, so the firmware runs
the Windows paths (ICM mode, `_WAK` restore). Side effects possible on
other Darwin-only firmware paths (EC wake enables, power button notify),
so watch for anything odd. Nothing permanent.

1. Reboot. In GRUB press `e` on the Fedora entry, add `acpi_osi=!Darwin`
   at the end of the `linux` line, Ctrl-X to boot.
2. Log in. Note: does the Thunderbolt Display work (it may ask to be
   authorised; `boltctl` shows it)? Then tell Claude, who checks the boot
   log (`_OSI(Darwin)` gone, thunderbolt in ICM mode).
3. `systemctl suspend`, wait 30 s, wake. Note: does the Mac wake, does
   the display come back (give it 15 s)? Tell Claude the time.

Result: step 2 (boot 12:18): boots fine; sound came up ~10 s after login.
GRUB showed "can't find command `eload_video`" (a stray `e` typed into
the GRUB editor, harmless: only skips loading GRUB's video modules). Log:
`_OSI(Darwin) query ignored via cmdline`. The firmware now brings the
display up itself (DP-4 connected, its USB hub/HID appear at 12:18:55,
Light Ridge bridges 2b/2c present), but the Linux `thunderbolt` driver
fails on both NHIs (08:00.0, 4e:00.0): `failed to allocate host router`
-ETIMEDOUT, probe error -110, so `boltctl` shows the display
"disconnected". With Darwin (boot 12:09) the driver bound fine. Step 3
(suspend) pending.

### Test I: one sleep with s2idle instead of deep (S3)

Light sleep ("suspend to idle"): the firmware never enters S3, so it
should not cut power to the TB controllers and the links may stay up.
Costs more power while asleep; unknown whether the iMac Pro wakes from it.
Nothing permanent: the setting resets at the next boot (the boot line has
`mem_sleep_default=deep`).

1. Normal boot (no `acpi_osi=!Darwin`). Check the display works.
2. `echo s2idle | sudo tee /sys/power/mem_sleep` (should print `s2idle`).
3. `systemctl suspend`, wait 30 s, wake (key or mouse). Note: does the Mac
   wake, does the display come back (give it 15 s)? Tell Claude the time.
4. If the Mac does not wake: hold the power button to switch off, boot
   normally (deep sleep is back) and tell Claude roughly when it was.

Result (2026-11-24, Kirk, boot 12:28, suspend 12:30:02, hard reset
~12:30:49): no picture after wake, hard reset needed. The log shows
`PM: suspend entry (s2idle)` and `PM: suspend exit` at 12:30:19, so the
kernel did wake; the screen stayed off because amdgpu failed to resume
(`PSP resume failed`, `amdgpu_device_ip_resume failed (-62)`, after
`UNLOAD_TA failed` on suspend), and cosmic-comp could not restart. s2idle
is unusable on the iMac Pro as it is.

The useful part: **the TB controllers dropped off in s2idle too**, at the
same point as with S3: both NHIs and TB xHCIs "inaccessible" in the first
noirq step after `PM: suspend devices took 0.761 seconds`, then
`tb_resume_noirq` -108 and `Controller not ready at resume -19`. No
firmware S3 ran, so the controllers are turned off during the normal
device-suspend phase by something on the Linux side or the T2/SMC it
talks to (candidates: the T2 bridge/vhci telling the T2 the Mac is going
to sleep, t2smc, the thunderbolt driver's own suspend). This matches the
first reading of Test E and contradicts the end of Test F, unless Test F's
cause is the T2 rather than the thunderbolt driver.

### Test J: find which device's suspend turns TB off

Runs only the device-suspend phase, then resumes without sleeping
(`pm_test=devices`; no firmware, no S3), with each device's suspend
callback logged with its time, so the journal shows what ran just before
the TB controllers went away. The screen may not come back (amdgpu, as in
Test I); if so, hold the power button. Logs survive the reset.

1. Normal boot (deep sleep, compat). Check the display works.
2. ```bash
   sudo sh -c 'echo 1 > /sys/power/pm_print_times;      echo 1 > /sys/power/pm_debug_messages;      echo devices > /sys/power/pm_test; echo mem > /sys/power/state'
   ```
   The Mac goes through suspend and comes back by itself after ~5 s.
3. Note: does the picture come back, does the TB display? Tell Claude the
   time. If the Mac is still usable, undo with
   `sudo sh -c 'echo none > /sys/power/pm_test'`.

Result (2026-11-24, Kirk, boot 12:31, test 12:35:22, hard reset ~12:36):
both screens went dark at once and stayed dark; hard reset. The log shows
why: amdgpu failed again (`UNLOAD_TA failed` on suspend, `PSP resume
failed` -62 on resume, then a GPU reset that also failed). So amdgpu
cannot survive any suspend that is not a real S3 power cycle; every
`pm_test` or s2idle run will leave the screens dark. The kernel itself
resumed fine (`PM: suspend exit` 12:35:31).

The useful part: **the TB controllers survived the device-suspend phase.**
Every TB device (NHIs 08:00.0/4e:00.0, xHCIs 09/4f, bridges 06/07/4c/4d,
root ports 05:02.0/05:03.0) ran `pci_pm_suspend` and `pci_pm_resume`
normally; no "inaccessible", no `tb_cfg` -108, no "Controller not ready".
So the device-suspend callbacks (thunderbolt's `.suspend`, t2 bridge/vhci,
t2smc) are not what turns TB off. In Test I it dropped in the noirq step,
so the culprit runs in the **late or noirq** phase.

### Test K (proposed, not run yet): `pm_test=platform`

Same as Test J but one level deeper: runs the late and noirq phases and
the ACPI prepare step (`_PTS`), then resumes without sleeping. With
`pm_print_times` the log shows every late/noirq callback in order, so the
one just before the TB devices turn "inaccessible" is the culprit. The
screens will go dark again (amdgpu, as in Test J): hard reset is expected.

```bash
sudo sh -c 'echo 1 > /sys/power/pm_print_times; echo 1 > /sys/power/pm_debug_messages; echo platform > /sys/power/pm_test; echo mem > /sys/power/state'
```

Result: (pending)

## Decisions and failed attempts

- 2026-11-24: Test J: TB survives the device-suspend phase (no drop-off
  with `pm_test=devices`); the drop is in late/noirq. Screens went dark
  because amdgpu cannot resume without a real S3 power cycle; any further
  `pm_test` run needs a hard reset. Next: Test K, `pm_test=platform`.

- 2026-11-24: Test I (s2idle) failed: amdgpu PSP fails to resume, no
  picture, hard reset. But TB dropped off in s2idle exactly as in S3, so
  S3/firmware power is not the cause; something in Linux's device-suspend
  phase (or the T2 reacting to it) turns the controllers off. Test J
  pins it down with `pm_test=devices` + `pm_print_times`.

- 2026-11-24: Test H failed: with `acpi_osi=!Darwin` the firmware tunnels
  the display at boot (ICM mode, Linux TB driver then times out), but
  after S3 the TB links are down exactly as with Darwin. OS mode is ruled
  out alongside power (G), rescan (A, F) and `_PS0`/GPIO/link bounce
  (C-E). Back to the normal boot (no `!Darwin`).

- 2026-11-24: Test G failed: a 3 s power cycle of the TB pads after S3 (pads
  verified driven low) plus `_PS0` leaves the link down. Power is ruled
  out. Next lead: the firmware only handles TB across S3 when the OS is
  not Darwin (`_WAK` -> `ICMB`); Test H boots once with `acpi_osi=!Darwin`.

- 2026-11-24: Test F failed. Step 2 (no sleep) passed: remove + rescan
  brings the display back in ~1 s, so a rescan works whenever the link is
  up. Step 3: with the driver removed before suspend the link is still
  down after S3. A sleep hook that only removes/rescans is ruled out; the
  controllers have to be brought back onto the link after S3 (power
  cycle / reset), which Tests C-E have not managed yet.

- 2026-11-24: Test B failed: without `pcie_ports=compat` the iMac Pro does
  not wake from S3 at all. Compat stays; go with the fallback (run the TB
  `_PS0` methods after resume, then rescan). First a throwaway test module
  to prove it by hand before writing the real fix.

- 2026-11-24: ACPI read: TB repower only happens via `UPSB/NHI0._PS0`,
  which Linux skips while `pcie_ports=compat` leaves the bridges driverless.
  Test B is the deciding test. If compat must stay, the fallback is a small
  module that runs `\_SB.PC02.BR2x.UPSB._PS0` (and `NHI0._PS0`) on resume
  and rescans the root port.
- 2026-11-24: Test A failed: the TB controllers stay unpowered after S3
  (root port link down), so a rescan-only hook is ruled out.
- 2026-11-24: Diagnosed from the logs above. Old lead (`pcie_ports=compat`)
  fits: nothing waits for or re-enumerates the TB link after S3.

## Tested

Filled in by a person once it's tested on hardware:

- YYYY-MM-DD, name, Mac model, Fedora version: what was checked. Ready to
  merge.
