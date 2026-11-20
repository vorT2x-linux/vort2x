# Wi-Fi firmware under the bare board name (iMac Pro, iMac 2019)

## Goal

On iMacPro1,1, iMac19,1 and iMac19,2 the ACPI tables give the Wi-Fi board
name (`module-instance`) but no antenna SKU. Fedora's brcmfmac then skips
Apple's detailed firmware selection and asks only for the bare board name,
for example `brcmfmac4364b2-pcie.apple,ekans.{bin,txt,clm_blob,txcap_blob}`
on Kirk's iMac Pro, before the generic names. Only the generic `.bin`
failure is logged.

`install-wifi-firmware.sh` installs the right four files, but under names
this driver never asks for (`apple,ekans-HRPN-u-7.5.{bin,txt}`,
`apple,ekans-X3.{clm_blob,txcap_blob}`). Expected result, from reading the
brcmfmac source of 7.2.11 (the ISO kernel), not tested on hardware: no
Wi-Fi in the live USB and in installed Fedora until phase 2; after phase 2
(patch 0025) the CLM and TxCap blobs are still never loaded.

Fix: also make the four files available under the bare board name. Every
brcmfmac tries that name last (`bt[5]` with patch 0025, `board_types[0]`
without it), so other T2 Macs are unaffected.

## Plan

1. `auto-installer/runtime/install-wifi-firmware.sh`: after installing the
   four files, add relative symlinks
   `<prefix>.apple,<board>.{bin,txt,clm_blob,txcap_blob}` to them, where
   `<board>` is the `.trx` base name. Skip a link whose name is the file
   itself. Update the comment on naming and the final listing.
2. `scripts/tests/wifi-firmware.sh`: check the bare-name links in every
   case, and add an iMac Pro case (stock kernel: only the chip prefix and
   the generic `.bin` failure in the log; ekans files from Kirk's iMac Pro).
3. `scripts/tests/live-wifi.sh`: adjust its file checks if they break.
4. Run the static checks in Docker.

The Anaconda add-on counts `brcmfmac*.apple,*` entries with
`os.listdir` and needs at least 4; links only raise that count, so it
stays as it is.

## Acceptance criteria

- [x] `bash scripts/tests/wifi-firmware.sh` passes, including the new
      iMac Pro case: all four bare names exist, are relative symlinks and
      resolve to the installed files with the right contents.
- [x] Existing cases still install the same four regular files under the
      same names as before (only links added).
- [x] When the requested name is already the bare board name, no link
      points to itself.
- [x] `bash scripts/tests/live-wifi.sh` passes.
- [x] `bash scripts/tests/static-check.sh` passes (Fedora 44 container,
      as in AGENTS.md: prints `STATIC_CHECK_OK`).

## Decisions and failed attempts

Newest first.

- 2026-11-20: Checks run in Docker (fedora:44): `wifi-firmware.sh` and
  `live-wifi.sh` pass, `check.sh` prints `STATIC_CHECK_OK`. The new test
  run against `main`'s helper fails (`bare board link wrong`), so it
  catches the bug. First try compared link contents with `cmp`, which the
  minimal Fedora image lacks; the `readlink` check plus "resolves to a
  file" proves the same without a new dependency.
- 2026-11-20: Symlinks rather than copies: the targets are the same files,
  nothing to keep in sync, and the firmware loader follows links (Fedora's
  linux-firmware uses them a lot). Only the four files this Mac uses get a
  bare name; carrying all firmware for every board is a separate change
  (IDEAS.md).
- 2026-11-20: Checked on macOS 27's `/usr/share/firmware/wifi/C-4364__s-B2`
  that ekans' `.clmb`, `.txcb` and `V-u 7.5` NVRAM are identical for every
  antenna variant, so using the `-X3` files under the bare name is right.
- 2026-11-20: Kirk's iMac Pro `ioreg`: `C-4364__s-B2/ekans.trx`,
  `ekans-X3.txcb`, `ekans-X3.clmb`, `P-ekans-X3_M-HRPN_V-u__m-7.5.txt`.

## Tested

Filled in by a person once it's tested on hardware:

- YYYY-MM-DD, name, Mac model, Fedora version: what was checked. Ready to
  merge.
