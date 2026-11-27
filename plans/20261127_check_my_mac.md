# Check my Mac: self-test page and report form

## Goal

Users can see in one place which T2 parts work on their Mac, and send
that to the project as a report. The reports will later feed a public
per-model "what works" table on vort2x.org (separate PR, website repo).
Affects every T2 Mac: a new page in the VorT2x Dashboard and a new GitHub
issue form.

## Plan

1. `apps/vort2x-dashboard/src/checks.rs`: the automatic checks, read-only,
   no root, from `/sys`, `/proc`, `systemctl is-active` and the kernel log
   of the current boot. Each check knows which models have the part, so a
   missing part on a Mac that has it shows as a problem, and a part the
   Mac doesn't have shows as "not on this Mac".
   - T2 bridge (`apple_t2_bridge`, `apple_t2_vhci_hcd` loaded): all Macs
   - Keyboard and trackpad (input device "Apple Internal Keyboard /
     Trackpad"): MacBooks
   - Touch Bar (input device "Apple Inc. Touch Bar Display"): MacBook Pros
   - Camera (video4linux name "FaceTime"): MacBooks and iMacs
   - Light sensor (iio device "als"): MacBooks and iMacs
   - Speakers and microphone (ALSA card "Apple T2 Audio"): all Macs
   - Wi-Fi (a wireless network device), Bluetooth (`hci*`): all Macs
   - SMC (`t2smc` loaded): all Macs
   - Touch ID (`vort2x-t2-touchid.service` active): MacBooks
   - Graphics (a DRM card bound to amdgpu or i915): all Macs
   - 5K display as one 5120x2880 screen (eDP mode): iMacPro1,1, iMac20,x
   - Sleep this boot (kernel log: count of `PM: suspend exit`, any
     `PM: ... failed`): all Macs, "not tried yet" when it hasn't slept
   Plus the report header: model ID, Fedora, kernel, VorT2x release
   (`git describe` of the system checkout), desktop.
2. `apps/vort2x-dashboard/src/check_page.rs`: the page. A list of the
   results with an icon each, a "Check again" button, what the report
   contains, and two buttons: "Send report" (opens the pre-filled GitHub
   issue form in the browser with `gtk::UriLauncher`) and "Copy report"
   (clipboard, for people without a GitHub account). Checks run off the
   main thread so the window never stalls on `journalctl`.
3. `modules.rs`: a "Check my Mac" module, shown whenever the dashboard is
   installed, first in the sidebar. New symbolic icon.
4. `.github/ISSUE_TEMPLATE/mac-report.yml`: issue form with the model and
   automatic results pre-filled (URL query parameters), dropdowns for what
   only a person can say (sound, microphone, camera picture, sleep/wake,
   keyboard, Touch Bar, Touch ID, Wi-Fi, Bluetooth, display), and notes.
   Label `mac-report`.
5. Dashboard README: the page table.

## Acceptance criteria

- [x] Static checks print `STATIC_CHECK_OK` (podman, Fedora 45)
- [x] `cargo check` and `cargo test` pass for `apps/vort2x-dashboard`
      (podman, Fedora 45)
- [x] Unit tests: model rules (which parts each model has), parsing of
      `/proc/bus/input/devices`, ALSA cards, kernel log sleep lines, the
      report text and the issue URL (escaped, under GitHub's URL length)
- [x] The checks run against this iMac Pro's real `/sys` give the
      expected answers (run the check code from a test binary or the
      built app and compare with the readings in Decisions below)
- [x] The issue form YAML parses and every field id used in the URL
      exists in it
- [x] `Cargo.lock` alignment script passes
- [x] No serial number, hostname, user name or MAC address in the report

## Decisions and failed attempts

Newest first.

- 2026-11-27: Checks done. Static checks `STATIC_CHECK_OK`; `cargo check`
  with no warnings and `cargo test` 14/14 in podman Fedora 45; lock
  alignment OK; issue form parsed with PyYAML, its ids cover `model` and
  `checks` from the URL. A throwaway test ran the real checks on the iMac
  Pro: every part "works", laptop parts "not on this Mac", sleep "not
  tried yet", VorT2x `v0.3.0` (the installed checkout really is at
  `fd1a7a0`), link 1047 characters. The report has only the model ID,
  os-release name, kernel, release tag, desktop and the check lines.
- 2026-11-27: No YAML anchors in the issue form (unsure GitHub's form
  parser accepts them); the five answers are written out per dropdown.
  The report textarea has no `render`, so the list shows as Markdown.
- 2026-11-27: The GitHub label `mac-report` has to exist for the form's
  label to stick; Kirk creates it (or asks).

- 2026-11-27: Readings on Kirk's iMac Pro (iMacPro1,1, Fedora 45, kernel
  7.3.4) used as the reference: modules `apple_t2_bridge`,
  `apple_t2_vhci_hcd`, `t2smc` loaded; ALSA card "Apple T2 Audio"; v4l2
  "FaceTime HD Camera (Built-in)"; iio "als"; `wlp1s0` wireless, `hci0`;
  `card1-eDP-1` connected with 5120x2880. The laptop names (keyboard,
  Touch Bar) come from the repo's own udev/driver strings and are not
  yet checked on a MacBook.
- 2026-11-27: Kirk chose: page name "Check my Mac"; reports as a public
  GitHub issue form plus a "Copy report" fallback; the website table
  later in its own PR.
- 2026-11-27: The page lives in the dashboard crate itself, not a new app
  crate: it has no window of its own and no service.
- 2026-11-27: Dropdowns in GitHub issue forms can't be pre-filled from
  the URL, so the app pre-fills only the model and the automatic
  results; the person answers the rest on GitHub.

## Tested

Filled in by a person once it's tested on hardware:

- YYYY-MM-DD, name, Mac model, Fedora version: what was checked. Ready to
  merge.
