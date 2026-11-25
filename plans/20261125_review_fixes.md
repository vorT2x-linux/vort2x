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

## Decisions and failed attempts

Newest first.

- 2026-11-25: A1 also releases manual fan mode in t2smc's shutdown and
  remove paths, so a reboot or module reload hands the fans back even when
  the daemon never got to.

## Tested

Filled in by a person once it's tested on hardware:

- YYYY-MM-DD, name, Mac model, Fedora version: what was checked. Ready to
  merge.
