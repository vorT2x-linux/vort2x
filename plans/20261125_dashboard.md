# VorT2x Dashboard: one app with a page per installed part

## Goal

VorT2x installed about a dozen separate apps, each with its own window and
menu icon. Replace them with one app, **VorT2x Dashboard**
(`org.vort2x.Dashboard`), with a sidebar page per part. A page shows only
when the installer installed that part, so an iMac Pro gets no Force Click
or TrueTone page and a MacBook gets them. The dashboard is the only VorT2x
entry in the app menu. Affects every T2 Mac.

## Plan

Toolkit: GTK4/libadwaita, which the apps already use (Kirk, 2026-11-25).
Background services, helpers and what runs as root do not change; only
the windows move.

1. Every app with a window becomes a library with `pub fn page()` plus a
   small binary whose window is built around that page: t2-airshare,
   t2-debug, t2-dgpu-control, t2-fan-control, t2-force-click,
   t2-hybrid-gpu-control, t2-power-explorer, t2-smc-control,
   t2-truetone, t2-zone-power, vort2x-update. Timers that poll run only
   while their page is on screen.
2. t2-power-tune: its Python window is rewritten in Rust as a page; the
   root helpers stay. Installed like the other Rust apps (`Makefile`);
   `pciutils` moves to `install-dependencies.sh`.
3. `apps/vort2x-dashboard`: `AdwNavigationSplitView`, sidebar on the
   window background, the page in a floating rounded card; pages built
   on first open; icons built into the binary; About credits the original
   authors (GPL-3.0 7(b)); a card with the Mac model and system.
4. Installer: the dashboard is built and installed last
   (`install-apps.sh`), rebuilt when it or any page's app changes
   (`update-map`). The old apps' desktop entries get `NoDisplay=true`, so
   only the dashboard is in the menu; their binaries stay (services, the
   AirShare "Send" file-manager entry and the page checks use them).

## Acceptance criteria

- [x] Static checks pass (`vort2x-tools/check.sh` in Fedora 45: `STATIC_CHECK_OK`),
  including the Cargo lockfile alignment check
- [x] In a clean Fedora 45 container with one shared target folder (as the
  installer builds), `cargo build --release --locked` passes with no
  warnings from changed code, and `cargo test --locked` passes, for all 13
  changed crates
- [x] `make install` of the dashboard installs its binary, desktop file and
  icon; `make install` of t2-power-tune installs its binary, both helpers,
  policy, desktop file and icon (container, temp prefix)
- [x] Page list follows the installed apps: none, only Updates, Fans and
  Updates; an app without a page adds nothing; module IDs unique (unit
  tests in `src/modules.rs`)
- [x] Power Tune reads a scan in the helper's format into the right
  sections and describes applied changes as before (unit tests)
- [x] The dashboard starts on this iMac Pro with no GTK warnings (run from
  the build)

## Decisions and failed attempts

- 2026-11-25: Kirk: rewrite t2-power-tune in Rust so it is a page too.
  Only the window was rewritten (`src/lib.rs`, gtk4/libadwaita, serde
  for the helper's JSON); `t2-power-tune-helper` and
  `t2-power-tune-status` (root, through pkexec) are unchanged, so what
  runs as root and the service files stay the same. Helper calls use
  `gio::Subprocess` (no threads). Dropped from the old window: drawing the
  scan's `constraints` list (the helper always sends it empty) and the
  2-second hover popovers on the C-state cells (plain tooltips now).
  Installed like the other Rust apps (`install_rust_app`, new `Makefile`
  doing what `install.sh` did); `install.sh` and `t2-power-tune.py`
  removed. `pciutils`, which only `install.sh` installed, moved to
  `install-dependencies.sh`.

- 2026-11-25: Kirk: "port the rest of the apps" on this branch, so step 4
  is now part of this change. Ported as pages: t2-airshare, t2-debug,
  t2-dgpu-control, t2-hybrid-gpu-control, t2-force-click,
  t2-power-explorer, t2-smc-control, t2-truetone, t2-zone-power. Apps
  that already had a library (airshare, debug, truetone) got a `gui`
  module behind their `gui` feature; the binaries keep their paths.
  Notes per app: SMC's "only poll while the window is on screen" now
  checks the page; hybrid GPU (pkexec every 3 s), TrueTone (systemctl
  every second) and Zone Power poll only while shown; Force Click's
  shortcut dialog and AirShare's send window find their window/app from
  the page instead of being handed one; T2 Debug's T2 connection closes
  when its page is destroyed (was: window close). No page: t2-drop,
  t2-kbd-backlight, t2-touchbar, t2-sidecar (services without a window).
  Not ported: t2-power-tune (Python, can't link into the Rust dashboard).
  The dashboard's stylesheet makes the pages' own window/headerbar/root
  backgrounds transparent inside its card. Pre-existing, left alone:
  t2-force-click has an unused `signal-hook` dependency (cargo warns).

- 2026-11-25: Kirk's screenshots: both pages look right, but the Fans
  sidebar icon was missing (COSMIC's icon theme has no
  `weather-windy-symbolic`) and the About window showed a broken icon (app
  icon not installed when run from the build folder). Fix: the app icon
  and two own symbolic icons (fan, update) are built into the binary as a
  GResource (`build.rs`, `resources.gresource.xml`, `icons/`), as
  t2-fan-control does; GtkApplication finds them on any theme. Checks
  re-run: static OK, clean container build and tests OK.

- 2026-11-25: First tried showing a page from the installer's
  `app:<name>` marker. Dropped: `vort2x-update-helper`'s
  `record_current_state` writes a marker for every `app:` line of the
  update map that lacks one, installed or not, so after an update an iMac
  would show pages for MacBook-only apps. A page now checks a file its app
  installs, like the update map's `if=`.
- 2026-11-25: The Fans page reads sensors only while it is on screen
  (`is_mapped()` in its 900 ms timer); its history graph has a gap for
  the time another page was shown. The standalone window is always mapped,
  so it behaves as before.
- 2026-11-25: `style::install()` moved from vort2x-update's startup into
  `page()`, so the dashboard gets its styles too.
- 2026-11-25: GTK4/libadwaita, not libcosmic (Kirk): the apps already use
  it, so pages move nearly unchanged and the build on the Mac stays small.
  Name "VorT2x Dashboard", ID `org.vort2x.Dashboard`, first pages Fans and
  Updates (Kirk).
- 2026-11-25: A page is shown when its app is installed, not by probing
  hardware again: the installer already decides per Mac what to
  install, so the decision stays in one place.
- 2026-11-25: The apps become library crates the dashboard links in, rather
  than separate plugin binaries: one binary, no plugin ABI, and the old app
  binaries keep working from the same code until they are removed.

## Tested

Filled in by a person once it's tested on hardware:

- YYYY-MM-DD, name, Mac model, Fedora version: what was checked. Ready to
  merge.
