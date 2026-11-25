# VorT2x Dashboard

<p align="center">
<img src="icons/hicolor/scalable/apps/org.vort2x.Dashboard.svg" alt="VorT2x Dashboard logo" width="96">
</p>

One window for the VorT2x apps: a sidebar with a page per part, and the
page on the right. A page shows only when its app is installed, so each Mac
sees just the parts the installer set up for it.

| Page | From | Shown when this file exists |
|---|---|---|
| Fans | `apps/t2-fan-control` | `/usr/local/bin/t2-fancontrol-gtk` |
| Zone Power | `apps/t2-zone-power` | `/usr/local/bin/t2-zonepower-gtk` |
| Power Explorer | `apps/t2-power-explorer` | `/usr/local/bin/t2-power-explorer` |
| Power Tune | `apps/t2-power-tune` | `/usr/local/bin/t2-power-tune` |
| Graphics | `apps/t2-dgpu-control` | `/usr/local/bin/t2-dgpu-control` |
| Graphics | `apps/t2-hybrid-gpu-control` | `/usr/local/bin/t2-hybrid-gpu-control` |
| SMC | `apps/t2-smc-control` | `/usr/local/bin/t2-smc-control` |
| Force Click | `apps/t2-force-click` | `/usr/local/bin/t2-force-click` |
| TrueTone | `apps/t2-truetone` | `/usr/local/bin/t2-truetone` |
| AirShare | `apps/t2-airshare` | `/usr/local/bin/t2-airshare` |
| Debug | `apps/t2-debug` | `/usr/local/bin/t2-debug` |
| Updates | `apps/vort2x-update` | `/usr/local/bin/vort2x-update` |

Pages that read sensors or run helpers on a timer (Fans, Zone Power,
SMC, Graphics, TrueTone) do so only while they are on screen.

The background services do not change; only the windows move here. Each
app is a library with a `page()` function, used both by its own window and
by the dashboard. A page is built the first time it is opened.

## Adding a page

1. In the app, move the window code into the library (`src/lib.rs`, or a
   `gui` module behind the app's `gui` feature), split it into a
   `pub fn page() -> Page` and a window built around that page, and keep a
   small binary that calls `run()`. Timers that poll should skip their work
   while the page is not mapped.
2. Add the app as a path dependency in `Cargo.toml`, a `Module` in
   `src/modules.rs` and a symbolic icon in `icons/` and
   `resources.gresource.xml`. Keep `Cargo.lock` aligned with the other
   apps (`scripts/tests/cargo-lock-alignment.sh`).
3. Add the app's folder to the `app:vort2x-dashboard` line of
   `scripts/fedora/update-map`.

## Build and install

```bash
make build
sudo make install
```
