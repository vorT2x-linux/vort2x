// SPDX-License-Identifier: GPL-3.0-or-later

//! Apple's Wi-Fi (and BCM4377 Bluetooth) firmware for the stick. On a T2 Mac
//! running macOS it is taken from the files macOS loaded this boot; otherwise
//! from a folder the user gives. VorT2x never ships these files itself.
//!
//! On the stick they go into `vort2x-wifi-initramfs.img`, a gzip'd cpio
//! archive holding `vort2x-wifi-firmware/` with a `SHA256SUMS` file, which
//! the live system and the installer read.

use std::io::Write;
use std::path::Path;

use anyhow::{Context, Result, ensure};
use flate2::{Compression, GzBuilder};
use sha2::{Digest, Sha256};

use crate::cpio::{self, Entry};
use crate::system::hex;

#[derive(Debug)]
pub struct Firmware {
    trx: (String, Vec<u8>),
    clm: (String, Vec<u8>),
    txcap: (String, Vec<u8>),
    nvram: (String, Vec<u8>),
    /// BCM4377 Bluetooth files, sorted, `.bin` before `.ptb`.
    bluetooth: Vec<(String, Vec<u8>)>,
}

impl Firmware {
    /// Picks the Wi-Fi files out of `files` (name, contents) and checks that
    /// they belong together, as the bash script did.
    pub fn from_files(
        files: Vec<(String, Vec<u8>)>,
        bluetooth: Vec<(String, Vec<u8>)>,
    ) -> Result<Self> {
        let take = |label: &str, matches: &dyn Fn(&str) -> bool| -> Result<(String, Vec<u8>)> {
            let found: Vec<_> = files.iter().filter(|(name, _)| matches(name)).collect();
            ensure!(
                found.len() == 1,
                "expected exactly one {label} file, found {}",
                found.len()
            );
            let (name, data) = found[0].clone();
            ensure!(!data.is_empty(), "{label} file is empty: {name}");
            Ok((name, data))
        };
        let trx = take(".trx firmware", &|n| n.ends_with(".trx"))?;
        let clm = take(".clmb regulatory blob", &|n| n.ends_with(".clmb"))?;
        let txcap = take(".txcb TxCap blob", &|n| n.ends_with(".txcb"))?;
        let nvram = take("P-*.txt NVRAM", &|n| {
            n.starts_with("P-") && n.ends_with(".txt")
        })?;

        let clm_board = clm.0.trim_end_matches(".clmb");
        ensure!(
            clm_board == txcap.0.trim_end_matches(".txcb"),
            ".clmb and .txcb board names differ"
        );
        let nvram_board = nvram_board(&nvram.0)
            .with_context(|| format!("unexpected NVRAM file name: {}", nvram.0))?;
        ensure!(
            nvram_board == clm_board,
            "NVRAM board name does not match .clmb/.txcb board name"
        );

        let mut bluetooth = bluetooth;
        bluetooth.sort_by(|a, b| (a.0.ends_with(".ptb"), &a.0).cmp(&(b.0.ends_with(".ptb"), &b.0)));
        Ok(Self {
            trx,
            clm,
            txcap,
            nvram,
            bluetooth,
        })
    }

    /// Wi-Fi files from a folder, plus BCM4377 Bluetooth files next to them
    /// or in its `bluetooth/` subfolder.
    pub fn from_dir(dir: &Path) -> Result<Self> {
        ensure!(dir.is_dir(), "firmware folder not found: {}", dir.display());
        let wifi = read_matching(dir, |n| {
            n.ends_with(".trx")
                || n.ends_with(".clmb")
                || n.ends_with(".txcb")
                || (n.starts_with("P-") && n.ends_with(".txt"))
        })?;
        let mut bluetooth = read_matching(dir, is_bcm4377_file)?;
        if dir.join("bluetooth").is_dir() {
            bluetooth.extend(read_matching(&dir.join("bluetooth"), is_bcm4377_file)?);
        }
        bluetooth.sort();
        bluetooth.dedup_by(|a, b| a.0 == b.0);
        Self::from_files(wifi, bluetooth)
    }

    /// The names shown to the user, Wi-Fi first.
    pub fn names(&self) -> Vec<String> {
        let mut names: Vec<String> = [&self.trx, &self.clm, &self.txcap, &self.nvram]
            .iter()
            .map(|(name, _)| name.clone())
            .collect();
        names.extend(
            self.bluetooth
                .iter()
                .map(|(name, _)| format!("bluetooth/{name}")),
        );
        names
    }

    pub fn has_bluetooth_ptb(&self) -> bool {
        self.bluetooth
            .iter()
            .any(|(name, _)| name.ends_with(".ptb"))
    }

    /// `shasum -a 256` output for every file, as the live system checks it.
    pub fn sha256sums(&self) -> String {
        let wifi = [&self.trx, &self.clm, &self.txcap, &self.nvram];
        let mut sums = String::new();
        for (name, data) in wifi.into_iter().map(|(n, d)| (n.clone(), d)).chain(
            self.bluetooth
                .iter()
                .map(|(name, data)| (format!("bluetooth/{name}"), data)),
        ) {
            sums.push_str(&format!("{}  {name}\n", hex(&Sha256::digest(data))));
        }
        sums
    }

    /// The gzip'd cpio archive that becomes `vort2x-wifi-initramfs.img`.
    /// Entries are in the order `find . | LC_ALL=C sort` gives.
    pub fn initramfs(&self) -> Result<Vec<u8>> {
        let sums = self.sha256sums();
        let mut files: Vec<(String, &[u8])> = [&self.trx, &self.clm, &self.txcap, &self.nvram]
            .iter()
            .map(|(name, data)| (format!("./vort2x-wifi-firmware/{name}"), data.as_slice()))
            .collect();
        files.push(("./vort2x-wifi-firmware/SHA256SUMS".into(), sums.as_bytes()));
        for (name, data) in &self.bluetooth {
            files.push((format!("./vort2x-wifi-firmware/bluetooth/{name}"), data));
        }
        let mut entries: Vec<(String, Option<&[u8]>)> =
            vec![(".".into(), None), ("./vort2x-wifi-firmware".into(), None)];
        if !self.bluetooth.is_empty() {
            entries.push(("./vort2x-wifi-firmware/bluetooth".into(), None));
        }
        entries.extend(files.into_iter().map(|(name, data)| (name, Some(data))));
        entries.sort_by(|a, b| a.0.as_bytes().cmp(b.0.as_bytes()));
        let entries: Vec<Entry> = entries
            .iter()
            .map(|(name, data)| match data {
                Some(data) => Entry::File(name, data),
                None => Entry::Directory(name),
            })
            .collect();

        // Like `gzip -n -9`: no file name, zero time.
        let mut encoder = GzBuilder::new()
            .mtime(0)
            .write(Vec::new(), Compression::best());
        encoder.write_all(&cpio::archive(&entries))?;
        Ok(encoder.finish()?)
    }
}

/// Board name from `P-<board>_M-<module>_V-<vendor>__m-<version>.txt`.
fn nvram_board(name: &str) -> Option<&str> {
    let rest = name.strip_prefix("P-")?.strip_suffix(".txt")?;
    let (board, rest) = rest.split_once("_M-")?;
    let (module, rest) = rest.split_once("_V-")?;
    let (vendor, version) = rest.split_once("__m-")?;
    let plain = |s: &str| !s.is_empty() && !s.contains('_');
    (plain(board) && plain(module) && plain(vendor) && !version.is_empty()).then_some(board)
}

fn is_bcm4377_file(name: &str) -> bool {
    name.starts_with("BCM4377") && (name.ends_with(".bin") || name.ends_with(".ptb"))
}

/// Regular files in `dir` whose names match. Only the bytes are read, so
/// macOS file flags and metadata never come along.
fn read_matching(dir: &Path, matches: impl Fn(&str) -> bool) -> Result<Vec<(String, Vec<u8>)>> {
    let mut files = Vec::new();
    for entry in
        std::fs::read_dir(dir).with_context(|| format!("could not read {}", dir.display()))?
    {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if matches(&name) && entry.file_type()?.is_file() {
            let data =
                std::fs::read(entry.path()).with_context(|| format!("could not read {name}"))?;
            files.push((name, data));
        }
    }
    files.sort();
    Ok(files)
}

#[cfg(target_os = "macos")]
pub mod macos {
    //! The firmware macOS loaded on this boot, as `ioreg` reports it.

    use std::path::Path;
    use std::process::Command;

    use anyhow::{Result, bail};

    use super::{Firmware, is_bcm4377_file, read_matching};
    use crate::system::output;

    const WIFI_ROOT: &str = "/usr/share/firmware/wifi";
    const BLUETOOTH_ROOT: &str = "/usr/share/firmware/bluetooth";

    pub fn collect() -> Result<Firmware> {
        let ioreg = output(Command::new("/usr/sbin/ioreg").args(["-l", "-w0"]))?;
        let paths = super::requested_files(&String::from_utf8_lossy(&ioreg));
        if paths.is_empty() {
            bail!(
                "macOS did not report the Wi-Fi firmware it uses. Turn Wi-Fi on, restart macOS and try again, or use --firmware-dir"
            );
        }
        let mut wifi = Vec::new();
        for path in paths {
            let full = Path::new(WIFI_ROOT).join(&path);
            if let (Ok(data), Some(name)) = (std::fs::read(&full), full.file_name()) {
                wifi.push((name.to_string_lossy().into_owned(), data));
            }
        }
        let bluetooth = match bluetooth_chipset()? {
            Some(chipset) if chipset.starts_with("BCM4377") => {
                read_matching(Path::new(BLUETOOTH_ROOT), |n| {
                    is_bcm4377_file(n) && n.starts_with(&chipset)
                })?
            }
            _ => Vec::new(),
        };
        Firmware::from_files(wifi, bluetooth)
    }

    /// e.g. `BCM4377B3` from `system_profiler SPBluetoothDataType`.
    fn bluetooth_chipset() -> Result<Option<String>> {
        let report = output(Command::new("/usr/sbin/system_profiler").arg("SPBluetoothDataType"))?;
        Ok(super::chipset(&String::from_utf8_lossy(&report)))
    }
}

/// Paths below `/usr/share/firmware/wifi` from `ioreg -l` `RequestedFiles`.
#[cfg(any(target_os = "macos", test))]
pub fn requested_files(ioreg: &str) -> Vec<String> {
    let mut paths: Vec<String> = ioreg
        .lines()
        .filter(|line| line.contains("\"RequestedFiles\""))
        .flat_map(|line| line.split('"').skip(1).step_by(2))
        .filter(|value| {
            value.split_once('/').is_some_and(|(dir, name)| {
                !dir.is_empty()
                    && dir != ".."
                    && !name.contains('/')
                    && (name.ends_with(".trx")
                        || name.ends_with(".clmb")
                        || name.ends_with(".txcb")
                        || (name.starts_with("P-") && name.ends_with(".txt")))
            })
        })
        .map(str::to_owned)
        .collect();
    paths.sort();
    paths.dedup();
    paths
}

/// The Bluetooth chipset, e.g. `BCM4377B3` from `Chipset: BCM_4377B3`.
#[cfg(any(target_os = "macos", test))]
pub fn chipset(report: &str) -> Option<String> {
    let line = report.lines().find(|line| line.contains("Chipset:"))?;
    let start = line.find("BCM")?;
    let name: String = line[start..]
        .chars()
        .take_while(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || *c == '_')
        .filter(|c| *c != '_')
        .collect();
    (name.len() > 3).then_some(name)
}

/// A Bluetooth blob without its PTB file can't start the controller. Said
/// now rather than on the Mac that boots the stick; only a warning, as before.
pub fn bluetooth_warning(firmware: &Firmware) -> Option<&'static str> {
    (!firmware.bluetooth.is_empty() && !firmware.has_bluetooth_ptb())
        .then_some("No Bluetooth PTB file was found; Bluetooth may stay off")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kirks_imac_pro() -> Vec<(String, Vec<u8>)> {
        [
            "ekans.trx",
            "ekans-X3.clmb",
            "ekans-X3.txcb",
            "P-ekans-X3_M-HRPN_V-u__m-7.5.txt",
        ]
        .iter()
        .map(|name| (name.to_string(), name.as_bytes().to_vec()))
        .collect()
    }

    #[test]
    fn accepts_a_matching_set() {
        let firmware = Firmware::from_files(kirks_imac_pro(), Vec::new()).unwrap();
        assert_eq!(firmware.names()[3], "P-ekans-X3_M-HRPN_V-u__m-7.5.txt");
        let sums = firmware.sha256sums();
        assert_eq!(sums.lines().count(), 4);
        assert!(sums.starts_with(&format!(
            "{}  ekans.trx\n",
            hex(&Sha256::digest(b"ekans.trx"))
        )));
    }

    #[test]
    fn refuses_missing_duplicate_empty_or_mismatched_files() {
        let mut missing = kirks_imac_pro();
        missing.remove(0);
        let mut duplicate = kirks_imac_pro();
        duplicate.push(("other.trx".into(), b"x".to_vec()));
        let mut empty = kirks_imac_pro();
        empty[1].1.clear();
        let mut other_board = kirks_imac_pro();
        other_board[3].0 = "P-maui-X3_M-HRPN_V-u__m-7.5.txt".into();
        let mut blobs_differ = kirks_imac_pro();
        blobs_differ[2].0 = "ekans-X2.txcb".into();
        let mut bad_nvram = kirks_imac_pro();
        bad_nvram[3].0 = "P-ekans-X3.txt".into();
        for files in [
            missing,
            duplicate,
            empty,
            other_board,
            blobs_differ,
            bad_nvram,
        ] {
            assert!(Firmware::from_files(files, Vec::new()).is_err());
        }
    }

    #[test]
    fn bluetooth_files_are_sorted_and_checked() {
        let bluetooth = vec![
            ("BCM4377B3_b.ptb".into(), b"p".to_vec()),
            ("BCM4377B3_a.bin".into(), b"b".to_vec()),
        ];
        let firmware = Firmware::from_files(kirks_imac_pro(), bluetooth).unwrap();
        assert_eq!(
            &firmware.names()[4..],
            ["bluetooth/BCM4377B3_a.bin", "bluetooth/BCM4377B3_b.ptb"]
        );
        assert_eq!(bluetooth_warning(&firmware), None);

        let only_bin = vec![("BCM4377B3_a.bin".into(), b"b".to_vec())];
        let firmware = Firmware::from_files(kirks_imac_pro(), only_bin).unwrap();
        assert!(bluetooth_warning(&firmware).is_some());
    }

    #[test]
    fn reads_ioreg_requested_files() {
        let ioreg = r#"    | |   |         |   "RequestedFiles" = ({"Firmware"="C-4364__s-B2/ekans.trx","TxCap"="C-4364__s-B2/ekans-X3.txcb","Regulatory"="C-4364__s-B2/ekans-X3.clmb","NVRAM"="C-4364__s-B2/P-ekans-X3_M-HRPN_V-u__m-7.5.txt"})
    | |   |         |   "Other" = "x/y.trx""#;
        assert_eq!(
            requested_files(ioreg),
            [
                "C-4364__s-B2/P-ekans-X3_M-HRPN_V-u__m-7.5.txt",
                "C-4364__s-B2/ekans-X3.clmb",
                "C-4364__s-B2/ekans-X3.txcb",
                "C-4364__s-B2/ekans.trx",
            ]
        );
        assert!(
            requested_files(r#""RequestedFiles" = ({"Firmware"="../etc/a.trx","X"="a/b/c.trx"})"#)
                .is_empty()
        );
    }

    #[test]
    fn reads_the_bluetooth_chipset() {
        assert_eq!(
            chipset("Bluetooth:\n  Chipset: BCM_4377B3\n").as_deref(),
            Some("BCM4377B3")
        );
        assert_eq!(
            chipset("  Chipset: BCM_4364B3\n").as_deref(),
            Some("BCM4364B3")
        );
        assert_eq!(chipset("  Address: 00:00\n"), None);
    }

    #[test]
    fn initramfs_is_deterministic_gzip() {
        let firmware = Firmware::from_files(kirks_imac_pro(), Vec::new()).unwrap();
        let image = firmware.initramfs().unwrap();
        assert_eq!(&image[..2], &[0x1f, 0x8b]);
        assert_eq!(image, firmware.initramfs().unwrap());
    }

    #[test]
    fn reads_a_firmware_folder() {
        let dir = std::env::temp_dir().join(format!("vort2x-usb-firmware-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("bluetooth")).unwrap();
        for (name, data) in kirks_imac_pro() {
            std::fs::write(dir.join(name), data).unwrap();
        }
        std::fs::write(dir.join("notes.txt"), "not firmware").unwrap();
        std::fs::write(dir.join("bluetooth/BCM4377B3_a.bin"), "b").unwrap();
        std::fs::write(dir.join("bluetooth/BCM4377B3_a.ptb"), "p").unwrap();
        let firmware = Firmware::from_dir(&dir).unwrap();
        assert_eq!(firmware.names().len(), 6);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
