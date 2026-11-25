# Fixes from the 2026-11-25 code review

## Goal

Work through the findings of the 2026-11-25 read-only code review of
`main` (v0.3.0) on one branch, one commit per item (Kirk's call: one
branch for the whole review instead of one per item). The items cover the
fan and power daemons, the installer and updater scripts, the network
receivers (t2-airshare, t2-drop), the updater's trust model, kernel
modules (vhci, t2touchbar, t2smc, t2gmux, t2bdrm, t2smp, sidecar, audio)
and idle wakeups. Which Macs each item affects is noted per item below.

## Plan

One commit per review item, in the review's suggested order first
(A1, A2, B1+B2, C1, C2, D1, C3, P1, A3), then the rest by section. Items
that are only leads or hardware tests (H, G) are noted, not coded.

## Acceptance criteria

- [ ] Static checks print `STATIC_CHECK_OK` (podman, Fedora 45)
- [ ] `cargo test` passes, without new warnings, for every changed crate
- [ ] Every changed kernel module builds against 7.3.4-300.fc45 with `W=1`
      and no new warnings
- [ ] `bash -n` and shellcheck pass for every changed shell script

## Items

Status per item: done (commit), skipped (why).

- **A1** fan daemon left the fans in manual mode on exit. SIGTERM/SIGINT
  now end the loop, and every exit path (signal, error) releases the fans
  to the SMC. New `--release` mode, run as `ExecStopPost=` so SIGKILL and
  panics are covered too. t2smc releases manual mode on shutdown and
  module remove. All Macs with t2-fan-control.
- **A2** no temperature reading set the fans to minimum. With automatic
  control on, a tick with no reading now hands the fans to the SMC; the
  next good reading takes them back. Five failed ticks in a row (about
  10 s, no reading or a failed fan write) end the daemon with an error
  so systemd restarts it and the hwmon paths are found again.
- **B1 + B2** failed steps were marked applied. `install.sh` now counts
  the hard errors before each step and treats a step that added one as
  failed, so it is not marked applied and VorT2x Update offers it again.
  `install-acpi-fixes.sh` catches the autofix pipeline's status, so a
  failure is the intended warning instead of ending the installer. All
  Macs.
- **C1** t2-airshare wrote Quick Share files before anyone accepted. The
  introduction now only notes the files; file chunks before acceptance end
  the connection; after acceptance data goes to `name.part` (created with
  `create_new`) and is renamed when complete. A `Drop` on the session
  removes partial files on decline, cancel or error. A second
  introduction, more than 1000 files, negative sizes and more bytes than
  the free space are refused. Macs with t2-airshare.
- **C2** t2-drop (root) allocated whatever `Content-Length` said, up to
  2 GiB, for any request. Bodies are now capped at 1 MiB except /Upload
  after an accepted ask; the buffer grows with the data that arrives, not
  with the header; request and header lines are capped at 8 KiB and 100
  headers; chunk sizes use `checked_add`; gunzip output is capped; at
  most 8 connections at once. The unit gets `MemoryMax=50%`,
  `NoNewPrivileges=yes` and `ProtectSystem=strict` (the daemon writes only
  to /proc/sys and talks netlink/D-Bus; needs the hardware test to
  confirm). Macs with t2-drop.
- **D1** vhci: a control status for a URB already marked cancelled
  completed and freed it while it was still on the cancel list (use after
  free, double giveback). The status check now leaves cancelled URBs to
  the cancel worker, and `bce_vhci_urb_complete()` unlinks `cancel_list`
  before freeing. All T2 Macs (internal USB).
- **C3** (partly) vort2x-update-helper picked the highest *local* tag of
  the user-owned checkout. It now lists release tags with `git ls-remote`
  on the constant GitHub URL and checks out the commit GitHub names for
  the tag, so a tag planted in the checkout is never used; the fetch
  prunes local `v*` tags GitHub doesn't have. The `optional` actions move
  the checkout to the release before running anything instead of using
  the working tree as found. **Not done, needs Kirk's decision:** the
  checkout stays user-owned, so a process running as the user can still
  change files between checkout and the root run, or set git filters in
  `.git/config`; and apps are built as the user into the user's cargo
  cache and then installed by root. The full fix (root-owned source tree,
  builds outside the user's reach) changes vort2x-install, the multicall
  tool and the app build, which is bigger than one review item. Helper
  tests not added yet (the helper needs root and pkexec; making it
  testable is part of that redesign).
- **P1** the SideCar receive thread waited in uninterruptible sleep
  (state D), adding 1.0 to the load average all the time. It now waits
  with `TASK_IDLE`. Repeated receive errors are logged rate-limited and,
  after three in a row, retried every 100 ms instead of in a tight loop.
  All T2 Macs with t2-sidecar.
- **A3** the Vega clock hold only rewrote the sclk mask after seeing the
  GPU in states 5-7, so a mask dropped by a GPU reset while idling was
  noticed only after the jump to the top clocks. The mask is now also
  rewritten after a system sleep (a wall-clock gap of 2 s or more between
  ticks; the monotonic clock stops in suspend), when the GPU comes back
  from runtime suspend, and every 5 s while it is active. The log line
  appears only when the clock was actually above the limit. iMac Pro.
  logind `PrepareForSleep` was not used: the wall-clock gap catches the
  same resume without a D-Bus dependency.
- **A4** the fan daemon's socket was mode 0666 and read with no timeout
  or length cap inside the control loop. The daemon now reads and writes
  with a 200 ms timeout and at most 4096 bytes per request; the GUI side
  has a 3 s timeout, so a stuck daemon no longer freezes the dashboard
  for good. `SET_*` requests are taken only from root or a user whose
  logind state is `active` (checked with `SO_PEERCRED` and `loginctl`);
  anyone may still read the state. The config is saved through a temp
  file and rename. New direct dependency `libc` (already in the lock
  file through signal-hook). All Macs with t2-fan-control.
- **A5** t2-smc gauging subtracted saved timestamps from the current time
  as plain `u64`; with the clock behind them (stale SMC RTC before NTP)
  that wrapped and lifted the charge limit. A tick with the clock behind
  any saved time now changes nothing, and the age sums use
  `saturating_sub`. New test. Laptops.
- **B3** kernel patches were applied with `--fuzz=3`, which ignores all
  context, so an add-only hunk could land in the wrong function. Patches
  now apply with fuzz 2; only if that fails is fuzz 3 tried, and then the
  hunks that needed it are printed and recorded as a warning. Checked in
  a scratch tree: an add-only hunk with no matching context is applied
  at fuzz 3 and reported. Not made a hard failure in the kernel hook:
  the 2026-11-20 history notes an i915 hunk that needs fuzz 3 on 7.3.5,
  and failing would leave Intel MacBooks on stock i915 (Kirk's call;
  the patches themselves were not changed).
- **B4** kernel updates installed offline (GNOME Software / Discover,
  `system-update.target`) could not download the kernel source, so the
  new kernel booted with stock amdgpu, brcmfmac, i915 and thunderbolt
  and nothing retried. The kernel source hook now leaves a marker in
  `/var/lib/vort2x-installer/pending-kernel/<kver>` when the source is
  unavailable. New `vort2x-kernel-retry.sh` runs from
  `vort2x-kernel-retry.timer` two minutes after boot (a timer, so an
  amdgpu rebuild never holds up the desktop; the service has
  `ConditionDirectoryNotEmpty=` and `After=network-online.target`): it
  re-runs the 44-47 hooks for each marked kernel, runs `dracut --force`,
  removes the marker and logs that a restart is needed when it was the
  running kernel. A retry that fails again is not re-marked, so it is
  tried once, not at every boot. All Macs.
- **B5** the macOS USB script's Touch ID count ended the script under
  `pipefail` when grep found nothing (likely on Macs without Touch ID).
  The assignment now falls back to empty, so the "could not read" message
  is reached. Same idea as the change KAIT2EN made for this check. Macs
  without Touch ID, macOS side.
- **B6** `vort2x-rescue` copied the live resolv.conf onto the target's,
  which with the live `/run` bound in is the same file, so it was emptied
  (no DNS until exit). The copy and its restore are now skipped when the
  target's resolv.conf is a symlink or the same file as the live one (a
  symlink already leads to the live resolver). Live USB rescue.
- **B7** three small installer fixes. `prepare-fedora-installer.sh
  --reuse-media` no longer overwrites `grub.cfg.fedora-original` with
  VorT2x's own grub.cfg. `vort2x-rescue` reports "carries all T2 input
  drivers" per kernel again (the missing flag was not reset per kernel).
  The gpu, i915, thunderbolt, wireless and ACPI scripts replaced lib.sh's
  EXIT trap, so their own runs (kernel hooks) never printed the summary
  or removed the `/tmp/vort2x-install-errors.*` ledger; they now use
  `cleanup_then_exit`, and a run without errors removes its empty
  ledger. Checked with a scratch script: work dir and ledger removed,
  exit status kept.

## Decisions and failed attempts

Newest first.

- 2026-11-25: A1 also releases manual fan mode in t2smc's shutdown and
  remove paths, so a reboot or module reload hands the fans back even when
  the daemon never got to.

## Tested

Filled in by a person once it's tested on hardware:

- YYYY-MM-DD, name, Mac model, Fedora version: what was checked. Ready to
  merge.
