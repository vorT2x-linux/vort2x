# Fans to full speed when the GPU gets busy

## Goal

`t2-fancontrol` follows temperatures: it reads them every 2 s and ramps
1% per 100 ms until a sensor passes the curve's end (95 C). The iMac Pro's
Vega 56/64 heats from 52 to 101 C in about 4 s when a game starts
(CS2, 2026-11-23 test 6), so the fans only reach full speed (2500 rpm)
after the GPU is already at 100 C. During play the GPU sits a little
cooler and the fans drop back.

The fix: when the AMD dGPU is busy, run the fans at full speed straight
away, before its temperature climbs, and hold them there for a while
after the load ends. Affects Macs with an AMD dGPU (`amdgpu` with
`gpu_busy_percent`) that are T2 iMacs: the iMac Pro (`iMacPro1,1`) and
the 2020 27" iMacs (`iMac20,1`, `iMac20,2`), only while fan control is on
"automatic" in the app. MacBook Pros with an AMD GPU are left out. Part of the CS2 sleep hunt (`plans/20261122_vega_power_limit.md`
on branch `vega-power-limit`).

## Plan

1. `sysfs.rs`: `GpuLoad` reads `gpu_busy_percent` from the amdgpu hwmon's
   device, skipping the read while the GPU is runtime-suspended (it would
   wake it). `discover_gpu_load()` finds it, on the T2 iMacs only
   (DMI `product_name`).
2. `control.rs`: `check_gpu_load()` every 250 ms. Two reads in a row at
   or above 80% start the boost: fans to 100% at once (no 1% ramp), held
   for 30 s after the last busy read. The boost counts as "forced" in the
   normal 2 s tick, so the curve can't pull it down; afterwards the normal
   ramp-down (1% per 500 ms) takes over.
3. `main.rs`: the daemon wakes every 250 ms when a GPU was found and
   automatic control is on; looks for the GPU again on each full sensor
   refresh (every 10 s) until found, since amdgpu can load after the daemon.

Thresholds are constants for now, picked from the test logs: in CS2 the
GPU is at median 100% busy (min 57%) under load; desktop spikes reached
27-49% and a single 99% read at game start.

## Acceptance criteria

- [x] `cargo test --release` in `apps/t2-fan-control` (Fedora 45 podman): 17 passed, incl. `gpu_boost_needs_busy_reads_in_a_row` and `gpu_boost_only_on_t2_imacs`
- [x] `cargo build --release`: no warnings
- [x] `vort2x-tools/check.sh`: `STATIC_CHECK_OK`

## Decisions and failed attempts

- 2026-11-23: Kirk: "limit the fan boost to the iMac Pro, and the other
  two imacs". On a MacBook Pro full fans in every game would only be
  noise, and the boost was only tested on the iMac Pro. The model check
  is in `discover_gpu_load()`, so other Macs never read the GPU load
  (they re-read the one-line DMI file every 10 s, which is negligible).

- 2026-11-23 14:32-14:48: Second run (test 8, a full CS2 match, log
  `~/vega-fast-t8.csv`): no sleep in ~13 min of load; fan target 2500 on
  every busy row. With test 7 this makes two passes; together with the
  GPU clock hold on `vega-power-limit` it is the fix for the CS2 sleep.

- 2026-11-23 13:35-13:40: First hardware run (test 7 in the vega plan,
  log `~/vega-fast-t7.csv`): no sleep in ~3.5 min of CS2 load. Fan target
  went to 2500 rpm 0.5 s after the GPU got busy; edge temp rose ~3x
  slower than in test 6, where the Mac slept. Repeat pending (test 8).

- 2026-11-23: Trigger on `gpu_busy_percent`, not GPU power: busy% is the
  same scale on every AMD GPU, while power differs per model (Vega idles
  at 8-18 W, games at ~115 W). Two reads in a row (0.25-0.5 s) so a
  single desktop spike doesn't spin the fans up for 30 s.
- 2026-11-23: No UI or config setting yet; the snapshot's target % already
  shows 100 while boosting. Add a switch later if the boost proves useful.
- 2026-11-23: Built after test 6 on `vega-power-limit`: the SMC still put
  the Mac to sleep 14 s into CS2 load with the clock held at states 0-4.
  Kirk: "we want fan speed up as quickly as possible even preemptively",
  and the fans slow down mid-game. In the test 5 repeat log the fans were
  already at 2500 rpm for most of the load and the GPU still sat at
  ~99 C, so this mainly wins the first 4-5 s of each load spike; the 4 Hz
  logger (`~/vega-fastlog.py`) is meant to show whether that matters.

## Tested

Filled in by a person once it's tested on hardware:

- YYYY-MM-DD, name, Mac model, Fedora version: what was checked. Ready to
  merge.
