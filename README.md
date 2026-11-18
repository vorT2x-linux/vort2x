<p align="center">
  <img src="assets/vorT2x-logo.png" alt="VorT2x" width="720">
</p>

# VorT2x

VorT2x (pronounced "vortex") brings T2 Mac support to stock Fedora using DKMS
modules. You keep getting kernel updates straight from Fedora, and the T2
modules, apps and fixes come from VorT2x.

[Website](https://vort2x.org) |
[Install](#install) |
[Contributing](#contributing) |
[License](#license)

> **Status:** VorT2x has just been forked from KAIT2EN and renamed. It has no
> release yet and hasn't been tested under its new name. Expect rough edges.

## Install

VorT2x has no release installer yet. On Fedora, from a clone of this
repository:

```bash
git clone https://github.com/vorT2x-linux/vort2x.git
cd vort2x
sudo ./scripts/fedora/install.sh
```

## Contributing

Contributions are welcome. Please open an issue or a pull request, and keep
each change focused on one thing. Fixes that move T2 support closer to clean
upstream Linux support are especially welcome.

## Origin

VorT2x is a fork of [KAIT2EN](https://github.com/kaiT2en/KaiT2en-Fedora),
taken on 2026-11-17. It is an independent project and is not affiliated with
or endorsed by KAIT2EN. [NOTICE](NOTICE) and [AUTHORS](AUTHORS) list the
original authors, whose credit the licence requires to be kept.

## License

The kernel modules are GPL-2.0. Everything else, apps, protocols, audio DSP
profiles, scripts, howto documents and project text, is GPL-3.0-or-later.
Vendored third-party code keeps its upstream license. The full breakdown is in
[LICENSING.md](LICENSING.md).

What the project stands for is in [VALUES.md](VALUES.md).
