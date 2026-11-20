# vort2x-usb

Makes the VorT2x Fedora USB stick for T2 Macs, on **macOS or Linux**, in a
full-screen terminal interface: pick the Fedora desktop and the USB drive,
and it downloads and checks Fedora, adds the VorT2x boot files and the
Apple Wi-Fi firmware, writes the stick and reads it back to check it.

It replaces `scripts/macos/prepare-fedora-installer.sh` and makes the same
stick. Until it has been tested on hardware and added to the release, the
release still uses that script.

## Use

From an unpacked VorT2x installer kit (the release zip), on the T2 Mac in
macOS:

```bash
./vort2x-usb --kit /path/to/kit
```

It asks for your password (it needs administrator rights to write the
drive) and then guides you through it. Only external drives are offered,
and nothing is erased until you type `ERASE <drive>`, e.g. `ERASE disk4`.

**Firmware.** On a T2 Mac in macOS it takes the Wi-Fi firmware macOS uses.
On Linux, or any other computer, give a folder with the firmware files
(`*.trx`, `*.clmb`, `*.txcb`, `P-*.txt`, and for BCM4377 Macs the Bluetooth
`BCM4377*.bin`/`.ptb`) with `--firmware-dir`.

Other options (`--help` lists them all):

| Option | What |
|---|---|
| `--edition ID` | Skip the menu, e.g. `cosmic` (`--list-editions`) |
| `--iso FILE` | Use a Fedora ISO you already have; its checksum must match |
| `--disk DEVICE` | Skip the drive menu, e.g. `/dev/disk4` or `/dev/sdb` (`--list-drives`) |
| `--download-dir DIR` | Where the ISO is downloaded (default `~/Downloads`) |
| `--plain` | Progress lines instead of the full-screen interface |
| `--target-file FILE` | Write the stick image to a file instead of a drive |

## How it works

The Fedora ISO's EFI System partition is read into memory, the VorT2x files
are added there (`EFI/BOOT/grub.cfg`, Fedora's kept as
`grub.cfg.fedora-original`, and `EFI/BOOT/vort2x/`), and the ISO is written
in one pass with that partition replaced. Nothing is mounted. The written
stick is read back and its checksum compared.

Downloads use the system `curl`, from Fedora's mirror list with the same
mirror order, resume and checks as the old script.

## Development

```bash
cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test
```

End to end with a real ISO, on Linux (needs `mtools`, `cpio`, `sfdisk`):

```bash
vort2x-usb --kit KIT --iso Fedora-….iso --firmware-dir FW \
    --target-file stick.img --plain
tests/check-image.sh Fedora-….iso KIT FW stick.img   # prints IMAGE_OK
```

## Licence

GPL-3.0-or-later. A port of the KAIT2EN installer scripts by André Eikmeyer
and 4f1sh3r; see [LICENSE](LICENSE).
