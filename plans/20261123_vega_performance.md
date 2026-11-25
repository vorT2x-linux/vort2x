# More gaming performance from the iMac Pro's Vega

## Goal

v0.2.3 stops the iMac Pro sleeping or powering off in games by holding the
Vega at sclk states 0-4 (1201 MHz instead of 1350 MHz, ~11% off the top
clock) and boosting the fans when the GPU gets busy. Win back performance
without bringing the sleep back. Affects the iMac Pro only.

## Plan

1. **Baseline** (this file, "Current test"): MangoHud FPS/frame-time log
   of a fixed CS2 benchmark with the v0.2.3 setup, plus `~/vega-fastlog.py`
   for temperatures, power and sclk state.
2. **Undervolt**: turn on amdgpu OverDrive (`amdgpu.ppfeaturemask` with
   bit `0x4000`; today `0xfff7bfff`, so `pp_od_clk_voltage` is missing),
   then lower the voltage of the top states step by step, one change per
   test, benchmark + logger each time.
3. **Clock limit again**: with the fan boost (and undervolt if it works),
   retest states 0-5, then higher.
4. If 2 or 3 win: make `t2-zone-power` apply it at boot, on this branch.

Checked 2026-11-23, nothing to gain: PCIe link 8 GT/s x16 (max), mclk
786 MHz (Apple's stock top state), Mesa 26.2.7, CPU governor and EPP
`performance`. GameMode installed, not used yet.

## Current test (update BEFORE each run; survives power cuts)

**Test B1 (baseline), NOT RUN YET.** Written 2026-11-23. Setup = v0.2.3
as tested: zone-power holds sclk 0-4, fan-boost build `102685ea`
installed, power cap 210 W. CS2 `cs2_video.txt` at this time:
2560x1440 windowed borderless (`fullscreen` 0, `nowindowborder` 1),
`mat_vsync` 0, `r_low_latency` 1, `msaa_samples` 2, `shaderquality` 0,
`r_texturefilteringquality` 1, shadows 1, dynamic shadows 1, textures 1,
particles 1, AO 0, parallax 0, HDR 3, `videocfg_fsr_detail` 2,
cpu/gpu/gpu_mem level 3. Keep all of these the same for every test.

MangoHud config `~/.config/MangoHud/MangoHud.conf` (logs to
`~/mangohud-logs/`, Shift_L+F2 starts/stops a log, 100 ms interval).

Steps (Kirk):

```
sudo dnf install mangohud
~/vega-fastlog.py ~/vega-fast-b1.csv          # in a terminal, leave running
```

1. Steam → CS2 → Properties → Launch options:
   `SDL_VIDEODRIVER=wayland mangohud %command%` (was
   `SDL_VIDEODRIVER=wayland %command%`).
2. Subscribe to the Workshop map **"CS2 FPS BENCHMARK"** (by angel), start
   it from Workshop Maps. Overlay must show FPS top-left.
3. Press **Shift_L+F2** when the fly-through starts, again when it ends
   (the map shows its own average at the end too: note it down).
4. Run it **3 times** (one log each), then quit CS2 and stop the logger
   with Ctrl+C.

2026-11-23 evening: Kirk installed MangoHud, subscribed to the map and
set the launch options (steps 1-2 done). Logger and runs not done yet;
continuing 2026-11-24.

2026-11-24: resumed. Checked before the run: no MangoHud logs and no
`~/vega-fast-b1.csv` yet, sclk table unchanged (0-7, 852-1350 MHz),
OverDrive still off (no `pp_od_clk_voltage`). Kirk now starts the logger
and does the 3 runs (steps 3-4).

**B1 result (2026-11-24, 1 run only, Kirk chose not to do 3):**
`~/mangohud-logs/cs2_2026-11-24_15-00-00.csv`, `~/vega-fast-b1.csv`.
Run 113 s, 15:00:00-15:01:53.

- FPS (MangoHud summary): average **126.2**, 1% low **77.4**, 0.1% low
  67.7. Frame time median 8.1 ms, p99 12.9 ms, max 14.8 ms (no stutter).
- GPU: sclk state 4 for 80% of the time, but the real clock averaged
  1118 MHz (max 1198), under state 4's 1201 MHz. Power average 80 W,
  max 108 W, far under the 210 W cap. Edge 92 C average, **101 C max**;
  junction 102 C max; memory 82 C max. Load 87%.
- Fans at **2500 rpm = their maximum** the whole run (fan boost).
- CPU: 24% load, 73 C, 3.9 GHz: not the limit.
- Whole system (PSTR) ~255 W average, 275 W max.

Heat check (Kirk asked if the GPU runs too hot for its power): at load
start (14:59:37) edge went 54 -> 87 C in 2 s and 99 C in 4 s at ~120 W;
at the end (15:01:53) it fell 94 -> 74 -> 66 C in 4 s, then slowly to
57 C. So the heatsink sits ~60-65 C and the chip runs ~35 C above it at
~110 W (PGTR): about 0.3 C/W from chip to heatsink, which looks high
(a well-mounted GPU is nearer 0.1 C/W). Possible causes: dried thermal
paste (bought 2017, ~9-10 years old, years of animation and rendering at full load), poor contact, or Apple's design. Not
proven. Also odd: edge and junction read the same (usually junction is
10-25 C hotter); may be how Apple's firmware reports it. Proposed
check: same GPU load in macOS, compare temperature at the same watts.

Reading: the GPU is held back by heat, not by power or the CPU. With
the fans already at maximum, the only way to more clock is less heat
per clock: undervolting is the right next step. One run means we
can't tell noise from a small gain: changes under ~5% need repeat runs.

**Next: U0, turn on OverDrive (no voltage change yet).** Written
2026-11-24 before the reboot. Kirk runs
`sudo grubby --update-kernel=ALL --args="amdgpu.ppfeaturemask=0xfff7ffff"`
(today's default `0xfff7bfff` plus bit `0x4000`), reboots, then
`cat /sys/class/drm/card*/device/pp_od_clk_voltage`. Expected: the file
exists and lists the 8 sclk states with their voltages; nothing else
changes. Undo: `sudo grubby --update-kernel=ALL
--remove-args="amdgpu.ppfeaturemask"`. The kernel marks itself tainted
with OverDrive on (amdgpu does this for overclocking); harmless, but
note it on any bug report.

**U0 result (2026-11-24 15:29 boot): works.** `ppfeaturemask` reads
`0xfff7ffff`, kernel logs "Overdrive is enabled", `pp_od_clk_voltage`
exists. OD_SCLK 0-7: 852/800, 956/825, 1086/850, 1145/900, 1201/950,
1269/1000, 1294/1031, 1350/1062 (MHz/mV). OD_MCLK 0-3: 167/800,
536/800, 688/850, 786/900. OD_RANGE: SCLK 852-1350 MHz, MCLK 167-786
MHz, VDDC 800-1063 mV (so no overvolting above Apple's top state).
zone-power still holds `manual` + states 0-4 (it re-applies every tick
if the GPU goes above state 4, so an OD commit can't undo the hold).
The two "BAR 0/2 failed to assign" lines were in the previous boot
too: not new.

**Next: U1, state 4 from 950 to 925 mV (-25 mV).** Written 2026-11-24
before the run. State 4 is where the GPU sits 80% of B1, so only it
changes. Kirk runs:

```
cd /sys/class/drm/card1/device
echo "s 4 1201 925" | sudo tee pp_od_clk_voltage
echo "c" | sudo tee pp_od_clk_voltage
cat pp_od_clk_voltage pp_dpm_sclk power_dpm_force_performance_level
~/vega-fastlog.py ~/vega-fast-u1.csv          # in a terminal, leave running
```

Then the same CS2 benchmark as B1 (same settings, Shift_L+F2 at start
and end; 3 runs if possible, at least 1). Expected: no crash, state 4
reads 925 mV, lower power and temperature at the same clock, so the
real clock gets nearer 1201 MHz and FPS rises a little. If the screen
freezes, CS2 crashes or the kernel logs a GPU reset/ring timeout: that
voltage is too low (note it here). Undo without reboot:
`echo "r" | sudo tee pp_od_clk_voltage; echo "c" | sudo tee
pp_od_clk_voltage`; a reboot also undoes it (nothing is saved).

**U1 result (2026-11-24, 1 run, 15:33:45-15:35:39):** applied fine
(state 4 reads 925 mV, `manual` + states 0-4 kept), no crash, no GPU
errors in the kernel log. `~/mangohud-logs/cs2_2026-11-24_15-33-45.csv`,
`~/vega-fast-u1.csv`. vs B1:

| | B1 (950 mV) | U1 (925 mV) |
|---|---|---|
| FPS avg | 126.2 | 129.8 (+2.9%) |
| 1% / 0.1% low | 77.4 / 67.7 | 70.1 / 44.3 |
| frame time median / p99 / max | 8.07 / 12.9 / 14.8 ms | 7.59 / 14.0 / 22.6 ms |
| real clock avg | 1118 MHz | 1138 MHz |
| GPU power avg / max | 79.7 / 108 W | **88.0 / 110 W** |
| edge temp avg / max | 91.9 / 101 C | **96.2 / 102 C** |
| PGTR avg | 114 W | 126 W |

Power and heat went **up**, not down. Suspected cause: on Vega 10,
amdgpu turns AVFS off when an OD voltage is changed (`vega10_update_avfs`
disables it on `DPMTABLE_OD_UPDATE_VDDC`, from memory of the driver,
not checked in the source yet). AVFS normally runs each chip below its
table voltage, so a fixed 925 mV may be higher than what AVFS used at
950 mV. Hint: at idle (state 0, table 800 mV) `in0_input` (vddgfx)
reads **925 mV**. The FPS gain is within one-run noise and the lows got
worse: U1 is not a win.

**Next: U1b, check the AVFS theory at idle (no game).** Kirk runs the
undo (`r` then `c`), then reads vddgfx at idle 5 times:
`for i in 1 2 3 4 5; do cat /sys/class/drm/card1/device/hwmon/hwmon2/in0_input; sleep 1; done`.
Expected if the theory holds: well under 925 mV (AVFS back on). If it
still reads 925, the reading means something else.

**U1b result (2026-11-24):** after the OD reset, idle vddgfx reads
**806 mV** (5 of 5), against 925 mV at idle with U1. So editing an OD
voltage makes the GPU run a fixed, higher voltage instead of its own
lower choice (AVFS): undervolting through `pp_od_clk_voltage` costs
power on this Vega. Undervolt route dropped. Editing only a clock (same
voltage) may keep AVFS, but more clock doesn't help while heat already
holds the real clock under state 4's 1201 MHz.

Reading: the Vega is heat-bound with the fans at maximum; the repaste
(B2) is the next real lever. Proposed: turn OverDrive off again
(`sudo grubby --update-kernel=ALL --remove-args="amdgpu.ppfeaturemask"`,
reboot) since it has no use now and taints the kernel; pause until the
PTM7950 arrives, then B2, then retest the clock limit (step 3).

Outcome: average FPS, 1% low and frame-time spread from the 3 logs,
GPU clock/temp/power from the logger. That's the number every later
test is compared with. Expected: no sleep (the benchmark is shorter
than test 8).

## Acceptance criteria

- [ ] (filled in once a change is chosen)

## Decisions and failed attempts

Newest first.

- 2026-11-24: PTM7950 needs heat to melt and settle (above ~45 C), so
  B2 runs only after a few hot load cycles (e.g. 2-3 gaming sessions),
  not straight after the repaste.

- 2026-11-24: no thermal pads to replace: Kirk checked an iFixit
  teardown, none visible under the iMac Pro heatsink. Repaste only
  (PTM7950 on GPU and CPU).

- 2026-11-24: undervolting dropped (U1, U1b): an OD voltage edit turns
  off the GPU's own voltage choice (idle 806 -> 925 mV), so power and
  heat went up. Next lever is the repaste (B2).

- 2026-11-24: change of order: Kirk has to order the PTM7950 first, so
  undervolting starts now (U0 next) with the old paste, compared with
  B1. After the repaste: rerun with the undervolt off (B2, vs B1 = what
  the repaste did) and on (the undervolt may need retuning then).

- 2026-11-24: Kirk will repaste the GPU and CPU (the B1 heat check
  hinted at poor chip-to-heatsink contact). Suggested: PTM7950
  phase-change pad or a thick non-conductive paste (Thermalright TFX,
  Arctic MX-6) on the bare Vega die; no liquid metal. Plan: after the
  repaste, rerun the baseline as **B2** (same settings as B1) before
  any undervolt, so the repaste's effect is measured on its own; U0
  waits until then.

- 2026-11-23: Kirk asked whether gaming performance can be improved.
  Options ranked: measure first, undervolt, retest the clock limit with
  the fan boost, GameMode. Kirk: start with MangoHud and a baseline.

## Tested

Filled in by a person once it's tested on hardware:

- YYYY-MM-DD, name, Mac model, Fedora version: what was checked. Ready to
  merge.
