# VorT2x Update

<p align="center">
<img src="org.vort2xupdate.gtk.svg" alt="VorT2x Update logo" width="96">
</p>

VorT2x Update fetches the release tags into `/usr/local/src/VorT2x-Fedora`,
picks the newest release whose `scripts/fedora/supported-releases` lists
this Fedora, reads the map from that release and compares each record with
it. A step runs again when one of its sources changed since its record.

| Change | Runs |
|---|---|
| `modules/t2smc/t2smc.c` | DKMS modules and the initramfs, then a reboot |
| `apps/t2-fan-control/src/main.rs` | only T2 Fan Control |
| `assets/eddy/...` | only VorT2x Eddy, where it is installed |
| `README.md`, `website/` | nothing |
| `scripts/fedora/lib.sh`, `install.sh` | a full installation is offered |

Parts without a record yet, for example after an update from an older
release, are compared with the local checkout. A record with a commit the
checkout does not know runs again.

The update itself runs through `pkexec vort2x-update-helper`, which only
accepts names from the map and calls

```bash
install.sh --only step,step
VORT2X_APPS=app,app install.sh --only install-apps.sh
```

Optional parts like the Touch Bar, the VorT2x login screen and VorT2x
Eddy stay as they are. They are added or removed in the Optional section.

## Adding a step or an app

Add a line to `scripts/fedora/update-map` with its sources, in installation
order. An `install-*.sh` script depends on itself automatically. Apps
installed by `install-apps.sh` are named `app:<name>` and need a matching
`wanted <name>` there.

## Install

Installed with the other apps by `scripts/fedora/install-apps.sh`.
