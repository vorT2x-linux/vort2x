# Let t2-zone-power cap the iMac Pro's Vega GPU

## Goal

On the iMac Pro (iMacPro1,1, Radeon Pro Vega 56/64, PCI ID `1002:6860`; Kirk's is a 16 GB Vega 64)
starting a game (CS2, fullscreen 5120x2880) makes the firmware press the
ACPI Sleep Button, and once the Mac powered off outright (SMC shutdown
cause 5). Once the game runs it plays fine.

Likely cause: `apps/t2-zone-power` applies the SMC's power-limit (PLimit)
steps the way macOS does, but `floor_ratio()` in `src/actuators.rs` only
knows Navi 1x and Polaris. Vega 10 is not in it, so the daemon logs "no
supported dGPU found yet, dGPU step is not applied": the GPU is never
capped and runs at its full 210 W when the SMC asks for less. At a sudden
CPU+GPU load jump the SMC's last resorts are sleep, then power-off.

Affects the iMac Pro only (the only T2 Mac with a Vega 10). Other Macs:
no change.

## Plan

1. **Confirm the cause first** (Kirk, on the iMac Pro): cap the GPU to
   150 W by hand (resets on reboot), then start a CS2 match.
   No sleep = cause confirmed. Still sleeps = try 120 W; still sleeps =
   the theory is wrong, stop and record it here.
**Changed 2026-11-23 after tests 1-5:** the power cap doesn't matter, the
clock limit does. Steps 2-3 below are replaced by:

2'. `VegaClockLimit` in `apps/t2-zone-power/src/actuators.rs`: on Vega 10
   (`1002:6860..687f`) the daemon sets `manual` and sclk states 0-4 and
   checks every 250 ms tick (re-applies after a GPU reset, which keeps
   `manual` but drops the mask). Sets `auto` again when the daemon stops.
   `floor_ratio()` unchanged (Vega not added: the power step isn't needed).

Original steps (dropped):

2. If confirmed: add Vega 10 (`0x6860..=0x687f`) to `floor_ratio()` in
   `apps/t2-zone-power/src/actuators.rs`. The floor macOS's AMD driver
   uses for Vega is not known; start at 0.4 like Navi (210 W → 84 W
   floor) and say so in the comment.
3. Check that the daemon then finds the dGPU and only caps when the SMC
   asks for a step (full 210 W otherwise).

## Current test (update BEFORE each run; survives power cuts)

Rule: before Kirk runs a test, write here the exact settings, commands,
log file name and what each outcome would mean. After the run, move the
result to "Decisions and failed attempts" and write the next test here.
`~/vega-log.sh` now records the GPU settings itself
(`<log>.settings.txt`, plus `gpu_sclk_state`/`gpu_perf_level` columns),
so a log never depends on memory again.

**Test 8 (repeat test 7 once, to be sure): DONE 14:32-14:48, survived; result under "Decisions". No test pending.** Written
2026-11-23 13:45. Nothing changes from test 7: same binaries, same CS2
settings, new log name. Check both daemons are active and CS2 is at
2560x1440 first, then:

```
~/vega-fastlog.py ~/vega-fast-t8.csv
```

Play a full match this time (test 7 had ~3.5 min of load). Outcomes: no
sleep → the fan boost is the fix (with states 0-4); raise its PR.
Sleep → the 4 Hz log shows the last second before it; test 7 was luck.

Checked 14:28 (same boot as test 7, up 1:16, no dnf update, kernel
7.3.4): both daemons active, fan binary `102685ea`, GPU `manual` state
0, `cs2_video.txt` 2560x1440 / `fullscreen` 0 / `nowindowborder` 1 /
`mat_vsync` 0, CS2 not running, no `~/vega-fast-t8*` yet. Kirk starting.

**Test 7 (fans at full speed when the GPU gets busy, plus 4 Hz log):
DONE 13:35-13:40, survived; result under "Decisions".** Written 2026-11-23 ~13:15. One change from test 6: the fan
daemon. Branch `fan-gpu-boost` (worktree `~/Documents/code/vort2x-fan-boost`,
plan `plans/20261123_fan_gpu_boost.md`, uncommitted): fans go to 100% at
once after 2 reads in a row of `gpu_busy_percent` >= 80 (250 ms apart),
held 30 s after the last busy read. Binary `~/t2-fancontrol-gtk.fan-boost`
(sha256 `102685ea...0bc1`). Unchanged from test 6: zone-power daemon
holds states 0-4 (test 6 binary still installed), CS2 2560x1440 window,
vsync 0, fan config `custom_curve=0:10,70:25,95:65`. New logger
`~/vega-fastlog.py` (4 samples/s, ms timestamps, power zones, GPU/CPU
temps, fans and fan targets, amdgpu sensors; settings incl. the fan
binary's sha256 in `<log>.settings.txt`). Steps:

```
sudo cp /usr/local/bin/t2-fancontrol-gtk /usr/local/bin/t2-fancontrol-gtk.bak-20261123
sudo systemctl stop t2-fancontrol
sudo cp ~/t2-fancontrol-gtk.fan-boost /usr/local/bin/t2-fancontrol-gtk
sudo systemctl start t2-fancontrol
systemctl is-active t2-fancontrol t2-zonepowerd      # expect active twice
~/vega-fastlog.py ~/vega-fast-t7.csv
```

~13:20: Kirk ran every step except the logger (the fan-boost binary is
installed), then rebooted because the Thunderbolt display died. After the
reboot: check both daemons are active, then start the logger.

Checked 13:31 (booted 13:12): both daemons active; installed fan binary
sha256 `102685ea...0bc1` (the fan-boost build); zone-power held states
0-4, level `manual`, GPU at state 0; `cs2_video.txt` 2560x1440,
`fullscreen` 0, `nowindowborder` 1, `mat_vsync` 0; CS2 not running;
`~/vega-fast-t7.csv` doesn't exist yet. Next: Kirk starts the logger, then CS2.

Check first: `fan1_target` jumps to 2500 within ~0.5 s of the game
loading (and the fans follow ~3 s later). Outcomes: no sleep → early fans
help (repeat once to be sure); sleep again → fans aren't the answer, the
4 Hz log shows what spiked in the last second before it. Undo:
`sudo systemctl stop t2-fancontrol; sudo cp
/usr/local/bin/t2-fancontrol-gtk.bak-20261123 /usr/local/bin/t2-fancontrol-gtk;
sudo systemctl start t2-fancontrol`.

**Test 5: run 12:14-12:19, invalid (CS2 was at 5K); result under
"Decisions". Before repeating it: reboot (the GPU was reset at
12:17:35), put `cs2_video.txt` back to 2560x1440 with CS2 closed, and
check it in the file before starting.** Done 12:2x: `cs2_video.txt`
set to 2560x1440 and `mat_vsync` 0 (as in the runs that survived;
backup `cs2_video.txt.5k-backup-20261123`). `~/vega-log.sh` now copies
CS2's video settings into `<log>.settings.txt` and logs a `cs2_res`
column every line (backup `~/vega-log.sh.bak-20261123`).

**Test 6 (the fix itself): DONE 12:54-12:57, the Mac still slept; result under "Decisions". Next test not chosen yet.** Build from branch
`vega-power-limit` (uncommitted, 2026-11-23 ~12:50), release binary at
`apps/t2-zone-power/target/release/t2-zonepower-gtk`. Steps:

```
sudo cp /usr/local/bin/t2-zonepower-gtk /usr/local/bin/t2-zonepower-gtk.bak-20261123
sudo systemctl stop t2-zonepowerd
sudo cp ~/Documents/code/vort2x/apps/t2-zone-power/target/release/t2-zonepower-gtk /usr/local/bin/
sudo systemctl start t2-zonepowerd
journalctl -b -u t2-zonepowerd --no-pager | tail -5   # expect "Vega 10 graphics clock held at sclk states 0-4"
cat /sys/class/drm/card1/device/power_dpm_force_performance_level   # expect manual
~/vega-log.sh ~/vega-log-t6-daemon.csv
```

Rebooted 12:52 (up 0 min). Checked 12:53: GPU `auto`, all 8 states,
daemon active (old binary); `cs2_video.txt` 2560x1440, `fullscreen` 0,
`nowindowborder` 1, but `mat_vsync` was 1 (CS2 saved it at 12:43, during
test 5 repeat): set back to 0 to match the runs that survived.

12:54:03: new binary installed (old one at
`/usr/local/bin/t2-zonepower-gtk.bak-20261123`), daemon logged "Vega 10
graphics clock held at sclk states 0-4", level `manual`. Kirk confirmed.
Next: logger `~/vega-log-t6-daemon.csv`, then CS2.

Nothing set by hand this time: the daemon must do it. Check CS2 is at
2560x1440 in `cs2_video.txt` first. Then: CS2, find match, play a game.
Optional: sleep and wake the Mac, check the journal line appears again
only if the mask was lost, and `gpu_sclk_state` never passes 4.
Outcomes: no sleep/power-off and state never > 4 → fix works; state > 4
in the log → the daemon missed a reset; dies with state <= 4 → as test 5
at 5K, 0-4 isn't always enough.
Undo: `sudo systemctl stop t2-zonepowerd; sudo cp
/usr/local/bin/t2-zonepower-gtk.bak-20261123 /usr/local/bin/t2-zonepower-gtk;
sudo systemctl start t2-zonepowerd`, then reboot (stopping sets `auto`).

**Test 5 repeat: DONE 12:26-12:31, survived; result under
"Decisions".**

Rebooted 12:23; checked 12:25: `cs2_video.txt` 2560x1440, `fullscreen`
0, `nowindowborder` 1, `mat_vsync` 0; GPU `auto`, all 8 states (limit
not yet set). Ready to run test 5 as written, log
`~/vega-log-t5b-s4-presets.csv`. Watch: applying a preset may set 5K
again (it did at 12:07:05); the `cs2_res` column will show it.

Test 5 as planned: does states 0-4 survive the preset-change trigger?
Same as the runs that survived (0-4, no power cap), but deliberately
repeat the trigger:

```
echo manual | sudo tee /sys/class/drm/card1/device/power_dpm_force_performance_level
echo "0 1 2 3 4" | sudo tee /sys/class/drm/card1/device/pp_dpm_sclk
~/vega-log.sh ~/vega-log-t5-s4-presets.csv
```

Start CS2, wait ~1 min in the menu, then in the graphics menu switch
presets medium → low → medium → low ... about 10 times, ~15 s apart.
Outcomes:
- survives all switches: the load-step theory holds and 0-4 is safe
  against it; the fix is a boot-time clock limit at 0-4.
- dies: 0-4 is not safe either; the limit must be lower (0-3), or a
  different approach (e.g. a ramp limit) is needed.

**Test 4: DONE 2026-11-23 12:05-12:07, power-off; result under
"Decisions".**

Test 4 as run: repeat sclk states 0-5, no power cap, no FPS cap.
Purpose: confirm whether the power-off comes when the SMC's `TG0E`
passes ~100.4 C (one data point so far, and the 0-5 setting of that run
is unverified).

```
echo manual | sudo tee /sys/class/drm/card1/device/power_dpm_force_performance_level
echo "0 1 2 3 4 5" | sudo tee /sys/class/drm/card1/device/pp_dpm_sclk
~/vega-log.sh ~/vega-log-t4-s5.csv
```

Then start CS2 and stay in the menu (it loads the GPU 100%). Outcomes:
- dies, and `TG0E` passed ~100 C in the last seconds: temperature
  threshold confirmed. The fix becomes keeping `TG0E` below ~95 C.
- dies with `TG0E` under 100 C: the threshold idea is wrong.
- survives 10 min: 0-5 is borderline; the earlier death needs another
  explanation.

## Acceptance criteria

- [x] `cargo check` and `cargo test` pass for `apps/t2-zone-power` (Fedora
      45 container).
- [x] Unit tests for reading the current state from `pp_dpm_sclk` and
      building the 0-4 mask (`cargo test`: 4 passed). (Replaces the
      Vega power-step test, dropped with step 2.)
- [x] No new clippy warnings (2 old ones in `gui.rs`/`sensors.rs`).
- [x] Static checks print `STATIC_CHECK_OK`.

Hardware tests for the Tested section (a person, on the iMac Pro):

- the daemon logs "Vega 10 graphics clock held at sclk states 0-4" and
  the GPU is in `manual` with states 0-4 after boot;
- after sleep/wake the GPU never runs above state 4;
- with the fan boost from branch `fan-gpu-boost` installed too (test 6:
  the clock hold alone is not enough), starting a CS2 match no longer
  puts the iMac to sleep or powers it off;
- game performance at the desktop and in a long match is no worse.

## Decisions and failed attempts

- 2026-11-23 12:54-12:57, test 6 (the daemon fix, log
  `~/vega-log-t6-daemon.csv`): the daemon worked as written. It held
  states 0-4 at 12:54:03, set them again after the wake (12:55:49), and
  `gpu_sclk_state` never went above 4 (69 rows at 0, 15 at 4). **The Mac
  still slept**: `Suspend key pressed short` at 12:55:28, 14 s into
  full load (state 4, 115 W PPT, edge 52 -> 101 C in 4 s, fans 1350 ->
  2480 rpm, PZ0F 209 W, PSTR 268 W). After the wake, CS2 kept running and
  was playable. So holding states 0-4 doesn't stop the sleep. The sleeps
  at 12:15:30, 12:15:51 and 12:17:26 (test 5) looked the same: edge
  100-102 C, PZ0F 189-205 W, PSTR 250-262 W, states 2-3. But every
  surviving run reached the same peaks (edge 102-104 C, PZ0F 205-211 W,
  PSTR 279-369 W) for minutes, so no logged value marks the trigger at
  2 s sampling. The 12:36:30 sleep came 3 min after the t5b log stopped
  (last row: idle, 59 C). Journal: 10 `Suspend key` events since
  2026-11-20.

Newest first.

- 2026-11-23 14:32-14:48, **test 8 (repeat of test 7, full match, log
  `~/vega-fast-t8.csv`, settings verified): no sleep, no power-off**.
  Kirk: "played a full match, no sleep this time either". Full load
  14:33:16-14:48:14 (3150 of 3701 rows at busy >= 80, ~13 min).
  `fan1_target` was 2500 on every busy row. sclk state never above 4 and
  the mask was not lost this time (no extra "clock held" line). Peaks:
  `TG0E` 100.6 C (median 98.5 C under load, 25 rows >= 100 C), junction
  104 C, PSTR 284 W, PZ0F 216.5 W, PGTR 159 W, GPU 122 W: the same peaks
  as the runs that slept, so the difference is the fans reaching full
  speed in ~2 s instead of following the curve. **Conclusion: the fix is
  the clock hold (states 0-4) together with the fan boost (branch
  `fan-gpu-boost`)**; test 6 showed the clock hold alone is not enough.

- 2026-11-23 13:35-13:40, **test 7 (fan boost, log
  `~/vega-fast-t7.csv`, settings verified in `.settings.txt`): no sleep,
  no power-off**, Kirk: "all seemed perfectly fine". Full load
  13:35:53-13:39:30 (~3.5 min, 805 of 1101 rows at busy >= 80). The boost
  worked as built: busy 96% at 13:35:53.031, `fan1_target` 2500 at
  .531 (0.5 s), fans at 2621 rpm by 13:35:55.3. Edge (`TG0E`) rose more
  slowly than in test 6 (54.8 C to 97.5 C in 12 s, vs 52 to 101 C in 4 s)
  and peaked at 100.4 C at 13:36:14, then settled at 89-95 C. Peaks:
  junction 103 C, PSTR 290 W, PZ0F 212 W, PGTR 161 W. One row
  (13:35:53.031) showed sclk state 6 at 499 MHz: the mask was lost as the
  load started, and the daemon held 0-4 again the same second
  ("Vega 10 graphics clock held" in the journal); cause unknown (no
  amdgpu reset logged). One pass only: repeat once (test 8).

- 2026-11-23: **Fix built** (step 2'). Kirk OK'd it, sad to lose the top
  ~11% of the Vega 64's clock (1201 vs 1350 MHz). Clock limit put in
  `t2-zone-power` rather than a udev rule because it must be re-applied
  after a GPU reset (test 5: `manual` stays, mask lost). Checks: cargo
  check/test, clippy (no new warnings), STATIC_CHECK_OK.

- 2026-11-23: **Test 5 (repeat) passed: states 0-4 survive the
  preset-change trigger.** Settings verified
  (`~/vega-log-t5b-s4-presets.settings.txt`: `manual`, states 0-4, cap
  210 W, CS2 2560x1440). Log `~/vega-log-t5b-s4-presets.csv`
  12:26:20-12:31:07, full GPU load ~12:27:00-12:30:15. Kirk switched
  presets ~10 times; each switch set CS2 back to 5K and he changed it
  back by hand (`cs2_res` shows 15 samples at 5120x2880, plus
  1920x1080 and 2048x1152 near the end). No sleep or suspend in the
  journal, no power-off. Highest sclk state 4 (the limit held), max
  system `PSTR` 283 W, GPU `PGTR` 160 W, `TG0E` 100.4 C, junction
  104 C; fans at max. So 0-4 is the highest tested-safe limit (0-5 died
  twice). Caveat: ~3.5 min of load, short 5K stretches only.

- 2026-11-23: **Test 5 didn't test the presets: CS2 was at 5K, and the
  SMC put the Mac to sleep 3 times.** Settings verified
  (`~/vega-log-t5-s4-presets.settings.txt`: `manual`, states 0-4, cap
  210 W). Log `~/vega-log-t5-s4-presets.csv` 12:14:42-12:19:34, no
  power-off (same boot). CS2's `cs2_video.txt` was last written at
  **12:07:05**, i.e. applying the medium preset in test 4 set the
  resolution to **5120x2880** (and `mat_vsync` 1). So the last 15 s of
  test 4 and all of test 5 rendered at 5K. In test 5 Kirk could not
  reach the settings to change it back.
  Sleeps (`Suspend key pressed short` = the SMC's sleep button) at
  12:15:30, 12:15:51, 12:17:27, each **8-11 s after the GPU went to
  full load**. At each: GPU 1.12-1.16 GHz (states 2-4, the limit
  held), GPU `PGTR` 140-165 W (vs ~125 W at 1440p), system `PSTR`
  250-280 W, edge 100-102 C, but `TG0E` only 96-97 C (rising ~4 C/s
  from ~52 C). A fourth 26 s stretch (12:17:37-12:18:03) reached
  `TG0E` 100.2 C with no sleep, then Kirk alt-tabbed out. On the third
  resume (12:17:35) amdgpu did a **GPU MODE1 reset** (`psp gfx command
  UNLOAD_TA failed`); after it the log shows sclk state 7, so **the 0-4
  limit was probably lost** from then on. `t2-zonepowerd` logs nothing
  about SMC steps, so we can't see whether the SMC asked for a dGPU
  step before pressing sleep.
  Takeaways: at 5K even states 0-4 aren't safe (sleep within ~10 s);
  sleep and power-off both look like the SMC's answer to the GPU's
  load. A test only counts if the game's resolution is in the
  settings file.

- 2026-11-23: **Test 4 result: power-off, but not at 100 C.** States
  0-5, no power cap, `manual` (verified this time in
  `~/vega-log-t4-s5.settings.txt`, started 12:05:05). Log
  `~/vega-log-t4-s5.csv` ends 12:07:21; the journal stops 12:07:16; SMC
  shutdown cause **-108** (same as the 10:39 run; the 11:51 run gave 5).
  About 2 min 15 s of full GPU load. `TG0E` passed 100 C at 12:05:55
  (max 100.2 C) and dropped below again **without** a power-off; at the
  power-off it read only 99.06 C. So **the "TG0E passes ~100 C" idea is
  wrong** (or at least not enough on its own). What did happen just
  before: at 12:07:06 the GPU suddenly dropped to idle (17% busy) while
  the CPU jumped to 106-125 W, then at 12:07:11 both went full together:
  system `PSTR` **369 W** (highest in any log; ~250 W otherwise, ~280 W
  max in the surviving runs), GPU `PGTR` 168 W, CPU 125 W. Power-off
  about 10 s later. That looks like a load change (match search / map
  load?), which fits "it always dies when I click find match". Also
  seen: with states 0-5 allowed the GPU still ran mostly 1.02-1.14 GHz
  (states 1-4), so the driver was already holding it back for heat.
  Side note: the logger samples every ~1.4 s, not 1 s, and short spikes
  between samples are invisible. `PZ3T` ramps up after ~6 min of load in
  the long surviving runs; dying runs never got that far, so it doesn't
  tell them apart.
  What Kirk did (told afterwards): turned on `showfps` (~65 FPS in the
  menu), opened the graphics menu and tried the detail presets; the
  12:07:06 dip was applying **medium**, and it powered off while
  changing **medium → low**. Applying a preset reloads shaders and
  textures: GPU drops to idle, CPU works, then the GPU jumps straight
  back to full. "Find match" (map load) is the same pattern. So the
  trigger looks like **a sudden idle → full GPU load step**, not heat
  or average power. The 65 FPS fits the GPU being held at ~1.1 GHz at
  100 C.

- 2026-11-23: **Power-off in the CS2 menu with sclk states 0-5**
  (what the previous entry asked for and the log name `~/vega-log-s5.csv`
  says; the commands were lost with the shell history in the power cut,
  so **not verified**), no
  power cap, about 45 s after launch, while Kirk was clicking around the
  menus. SMC shutdown cause 5 again. The menu alone is a full GPU load:
  at 11:50:56 the GPU went from idle to 100% busy, 132 W and 1.18 GHz,
  and its edge temperature from 55 to 96 C in one second (fans still at
  1400 rpm; they reached max about 3 s later). It then held 95-103 C
  until the log stops at 11:51:25. The SMC's own GPU reading `TG0E`
  climbed steadily and was above 100 C for the last 8 s (max 100.47 C).
  In the two runs that survived (states 0-4) `TG0E` spent 8-11 min at
  98 C or more but **never passed 99.9 C**. New lead: the SMC powers
  off when `TG0E` passes about 100 C; states 0-5 run just hot enough to
  cross it. Peak SMC power was also higher (system `PSTR` 303 W vs
  ~280 W, GPU `PGTR` 179 W vs ~157 W). Note: the journal stops at
  11:50:57, 28 s before the logger's last line, so the journal loses
  the last half-minute before a power cut; "nothing in the journal"
  proves little.
  CS2's video config: borderless window (`fullscreen 0`,
  `nowindowborder 1`) at 2560x1440, vsync off, desktop at 5120x2880
  scale 2 (COSMIC), so the game most likely renders at 1440p through
  XWayland and is scaled up, not at 5K. With vsync off and no FPS cap
  the GPU runs at 100% at any resolution, which is why the heat appears
  even in the menu.

- 2026-11-23: **Clock limit alone is enough.** After a reboot, sclk
  states 0-4 (max 1201 MHz) with `power1_cap` left at 210 W: Kirk played
  two games, no power-off. Log `~/vega-log-noCap.csv` 11:31-11:46: GPU
  max 120 W, average 76 W (only 13 samples over 100 W), PCPT max 115 W,
  but edge and junction peaked at 103 C (amdgpu crit 100 C), mem 83 C.
  So the power cap isn't needed; the fix is a clock limit. Kirk finds
  the performance lower than he'd expect. Next: try states 0-5
  (1269 MHz), then 0-6 (1294 MHz), to find the highest safe state.

- 2026-11-23: **Clock limit works.** With power cap 150 W + sclk states
  0-4 (max 1201 MHz) Kirk clicked "find match" and played a couple of
  games without a power-off (it had died at that click every time
  before, including today with the 150 W cap alone). Log `~/vega-log.csv`
  10:53-11:13: GPU max 127 W, average 75 W, junction peak 102 C, PCPT
  max 97 W. So the trigger is the top clock states, not average power.
  Still open: does the clock limit alone (no power cap) do it, and how
  high can it go (states 0-5 = 1269 MHz, 0-6 = 1294 MHz)? Then build
  the fix to apply it at boot.

- 2026-11-23: Correction to the entry below (Kirk): with the 150 W cap
  on 2026-11-22 (boot from 09:34) CS2 played about 30 min without
  trouble, and the power-off always comes at the same point in the game.
  So "the theory is wrong" was too quick. New angle: Vega 10 is known
  for very short power spikes when clocks jump; `power1_cap` limits
  average power, and a 2 s log can't see spikes. Next test: hold the
  GPU below its top clock states (`pp_dpm_sclk`, 852-1350 MHz, 8
  states) and play to that point. Asked Kirk what the point is.
  Answer: it **always** dies on clicking the button to search for a match
  (today: ~65 s after launch, GPU at only 70 W). Testing now with power
  cap 150 W + sclk states 0-4 (max 1201 MHz), logger running, no other
  change. If it still dies, test with the sound muted next (the click
  and search start play UI sounds through the T2 speaker path).

- 2026-11-23: Step 1, second run: hard power-off with `power1_cap` at
  150 W, about 65 s after CS2 started (10:38:44 → off just after
  10:39:47). SMC shutdown cause **-108**, in neither Apple's list nor the
  community lists in `t2smc.c` (the 2026-11-20 power-off logged 5). The
  journal shows nothing at the time. In the last minute
  (`~/cs2-power2.log`, every 2 s) the GPU drew only 50-86 W (never
  near the cap), edge/junction 80-84 C, memory 68-69 C, CPU 60-75 C,
  SMC CPU power (PCPT/PCPR) 50-90 W, fans at max from launch (t2-fancontrol
  targets 45 C). So the cap never engaged and total power was modest:
  **the power-budget theory doesn't explain this power-off.** Something
  not logged tripped (VRM, PSU or another SMC sensor), or the cause isn't
  thermal or power at all. Next: log all 208 SMC/amdgpu sensors every
  second (`~/vega-log.sh`, flushed to disk each line) during the next
  attempt; and as controls, run CS2 the same way on macOS, and a
  different heavy GPU load at 5K on Linux.

- 2026-11-22: Step 1, first run: with `power1_cap` at 150 W CS2 launched
  without sleeping (no `KEY_SLEEP`/suspend in the journal). GPU draw
  peaked at 151 W, mostly 80-120 W for about 40 s, then low. But the GPU
  edge temperature went from 53 to 90 C in 2 s and peaked at 102 C (amdgpu
  crit 100 C) even under the cap; fans at max (2500 rpm) afterwards.
  Heat may matter as much as power. Not conclusive yet: short load,
  unclear whether a match was started. Next: a full match of 10+ min,
  logging GPU power/edge/junction/mem temps, fans and SMC power to
  `~/cs2-power2.log`.

- 2026-11-22: Branch started from `main` after v0.2.2. Daemon on the
  iMac Pro (7.3.4-300.fc45 with the 5K series): "no supported dGPU found
  yet"; amdgpu `power1_cap` = `power1_cap_default` = `power1_cap_max` =
  210 W, `power1_cap_min` 0. Step 1 (150 W test) not done yet.
- 2026-11-21: Why limit the GPU at all when stock Linux doesn't: the SMC
  enforces the Mac's power budget whatever the OS; stock Linux just
  doesn't answer it. With Vega added the daemon caps only when the SMC
  asks. If the 150 W test doesn't stop the sleep, no limit gets added.
- 2026-11-20: CS2 sleep found: `KEY_SLEEP` from the ACPI Sleep Button
  (`PNP0C0E`), `systemd-logind: Suspend key pressed short`; the earlier
  power-off left SMC shutdown cause 5 and nothing in the journal. SMC
  power zones at idle: `PZ0E` 400 W, `PZ1E` 180 W, `PZ3E` 270 W, `PZ4E`
  490 W, `PZ5E` 640 W.

## Tested

Filled in by a person once it's tested on hardware:

- YYYY-MM-DD, name, Mac model, Fedora version: what was checked. Ready to
  merge.
