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

## Recent wins

| Date | Release | What |
|---|---|---|
| 2026-11-25 | v0.2.4 | **Camera on every boot:** the FaceTime camera, ambient light sensor and other devices behind the T2 chip (the Touch Bar on MacBooks) no longer go missing on some boots. Tested on two iMac Pros (2017), camera present on every boot. The fix is in the T2 bridge driver used by every T2 Mac; other models not tested yet. |
| 2026-11-23 | v0.2.3 | **Gaming on the iMac Pro:** starting a game no longer puts the iMac Pro to sleep or switches it off. The fans now go to full speed the moment the GPU gets busy, and the Vega's top clock is held back slightly; full CS2 matches play through. Tested on an iMac Pro (2017). The fan boost also covers the 2020 iMac (iMac20,1/20,2), not tested yet. Desktop Macs also no longer install the laptop-only battery, Touch ID and Force Click services. |
| 2026-11-22 | v0.2.2 | **5K display on the iMac Pro:** the 5K panel is now one 5120x2880 display, with no tearing at the centre seam and the pointer working on both halves. Tested on an iMac Pro (2017). The 2020 iMac (iMac20,1/20,2) gets the same merged display but hasn't been tested yet: reports welcome. |

## Working on

| What | Status |
|---|---|
| **vort2x-usb:** a terminal app that makes the Fedora USB stick from macOS or Linux, replacing the macOS script | In review ([#4](https://github.com/vorT2x-linux/vort2x/pull/4)), not tested yet |

## Install

VorT2x is an installer. You run it once on macOS to make a Fedora USB stick,
then Fedora finishes the setup by itself after the first login.

1. On a Mac, in macOS, connect an empty USB drive and run:

   ```bash
   curl -fsSL https://github.com/vorT2x-linux/vort2x/releases/latest/download/install-vort2x-fedora.sh | bash
   ```

   Pick a Fedora edition and the USB drive. It downloads and checks Fedora,
   then asks before it erases the drive.
2. Boot the Mac from the USB stick and install Fedora as usual.
3. After the first login, connect to Wi-Fi. The VorT2x installer opens, asks
   for one reboot, then installs the rest.

Run `vort2x-install` at any time later to update to the newest VorT2x
release for your Fedora version. [auto-installer/README.md](auto-installer/README.md)
has the details, rescue tools and troubleshooting.

## Contributing

Contributions are welcome, including ones made with AI tools. Fork the
repository, work on a branch and open a pull request. Please read
[CONTRIBUTING.md](CONTRIBUTING.md) first: it covers plans, testing, AI
disclosure and coding standards.

## Origin

VorT2x started as a fork of KAIT2EN in November 2026 and is developed
independently. [NOTICE](NOTICE) and [AUTHORS](AUTHORS) credit the original
authors.

## License

The kernel modules are GPL-2.0. Everything else, apps, protocols, audio DSP
profiles, scripts, howto documents and project text, is GPL-3.0-or-later.
Vendored third-party code keeps its upstream license. The full breakdown is in
[LICENSING.md](LICENSING.md).

What the project stands for is in [VALUES.md](VALUES.md).
