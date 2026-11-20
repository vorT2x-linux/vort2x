// SPDX-License-Identifier: GPL-3.0-or-later

//! The two halves of making a stick. `prepare` gets everything ready without
//! touching the drive (so a failed download leaves it as it was); `burn`
//! erases and writes it, after the user confirmed.

use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

use anyhow::{Context, Result, bail};

use crate::drives::{self, Drive};
use crate::firmware::{self, Firmware};
use crate::image::{self, BootFiles};
use crate::kit::{Edition, Kit};
use crate::report::Report;
use crate::system::{CallingUser, sha256_file};
use crate::{download, write};

#[derive(Clone, Debug)]
pub enum FirmwareSource {
    /// The files macOS uses on this T2 Mac.
    ThisMac,
    Dir(PathBuf),
}

#[derive(Clone, Debug)]
pub enum Target {
    Drive(Drive),
    /// A stick image in a file, for checking the result without a drive.
    File(PathBuf),
}

#[derive(Clone, Debug)]
pub enum IsoChoice {
    /// Download (or reuse) this edition in the download folder.
    Edition(usize),
    /// A Fedora ISO the user already has; its checksum says which edition.
    File(PathBuf),
}

pub struct Prepared {
    pub edition: Edition,
    pub iso: PathBuf,
    pub iso_size: u64,
    pub esp_offset: u64,
    pub esp: Vec<u8>,
    pub firmware_names: Vec<String>,
    pub warnings: Vec<String>,
}

pub fn prepare(
    kit: &Kit,
    iso: &IsoChoice,
    download_dir: &Path,
    firmware_source: &FirmwareSource,
    user: &CallingUser,
    cancel: &AtomicBool,
    report: &dyn Report,
) -> Result<Prepared> {
    let (edition, iso_path) = match iso {
        IsoChoice::Edition(index) => {
            let edition = kit.editions[*index].clone();
            let path = download_dir.join(&edition.file_name);
            download::ensure_iso(&path, &edition, user, cancel, report)?;
            (edition, path)
        }
        IsoChoice::File(path) => (identify(kit, path, report)?, path.clone()),
    };

    report.stage("Collecting the Apple Wi-Fi firmware");
    let firmware = match firmware_source {
        FirmwareSource::Dir(dir) => Firmware::from_dir(dir)?,
        #[cfg(target_os = "macos")]
        FirmwareSource::ThisMac => firmware::macos::collect()?,
        #[cfg(not(target_os = "macos"))]
        FirmwareSource::ThisMac => {
            bail!("firmware can only be read from macOS; use --firmware-dir")
        }
    };
    let firmware_names = firmware.names();
    for name in &firmware_names {
        report.note(name);
    }
    let warnings: Vec<String> = firmware::bluetooth_warning(&firmware)
        .into_iter()
        .map(str::to_owned)
        .collect();

    report.stage("Preparing the boot files");
    let mut file =
        File::open(&iso_path).with_context(|| format!("could not open {}", iso_path.display()))?;
    let iso_size = file.metadata()?.len();
    let label = image::volume_label(&mut file)?;
    let (esp_offset, esp_length) = image::esp_range(&mut file, iso_size)?;
    let mut esp = vec![0u8; esp_length as usize];
    {
        use std::io::{Read, Seek, SeekFrom};
        file.seek(SeekFrom::Start(esp_offset))?;
        file.read_exact(&mut esp)?;
    }
    let grub = image::render_grub(&kit.grub_template, &label)?;
    let input = std::fs::read(&kit.input_initramfs)?;
    let wifi = firmware.initramfs()?;
    let sums = firmware.sha256sums();
    let esp = image::edit_esp(
        esp,
        &BootFiles {
            grub_cfg: grub.as_bytes(),
            input_initramfs: &input,
            wifi_initramfs: &wifi,
            wifi_sha256: sums.as_bytes(),
        },
    )?;
    report.note(&format!("Fedora volume label: {label}"));

    Ok(Prepared {
        edition,
        iso: iso_path,
        iso_size,
        esp_offset,
        esp,
        firmware_names,
        warnings,
    })
}

/// Which edition an ISO is, by its SHA-256; refuses anything else.
fn identify(kit: &Kit, path: &Path, report: &dyn Report) -> Result<Edition> {
    let size = std::fs::metadata(path)
        .with_context(|| format!("ISO file not found: {}", path.display()))?
        .len();
    if !kit.editions.iter().any(|e| e.size == size) {
        bail!("the ISO is not one of the supported, unmodified Fedora images");
    }
    report.stage("Checking your Fedora ISO");
    let sha = sha256_file(path, report)?;
    kit.editions
        .iter()
        .find(|e| e.sha256 == sha)
        .cloned()
        .context("the ISO is not one of the supported, unmodified Fedora images")
}

/// Erases the target and writes the stick, then reads it back.
pub fn burn(prepared: &Prepared, target: &Target, report: &dyn Report) -> Result<()> {
    let (path, is_file) = match target {
        Target::Drive(drive) => {
            report.stage("Unmounting the USB drive");
            fits(drive, prepared.iso_size)?;
            drives::unmount(drive)?;
            (PathBuf::from(&drive.raw_path), false)
        }
        Target::File(path) => (path.clone(), true),
    };
    let mut file = write::open_target(&path, is_file)?;
    report.stage("Writing Fedora and VorT2x to the stick");
    let image = write::Image {
        iso: &prepared.iso,
        size: prepared.iso_size,
        esp_offset: prepared.esp_offset,
        esp: &prepared.esp,
    };
    let hash = write::write(&image, &mut file, report)?;
    report.stage("Reading the stick back to check it");
    write::verify(&mut file, prepared.iso_size, &hash, report)?;
    drop(file);
    if let Target::Drive(drive) = target {
        // macOS mounts the new partitions right away; leave the stick ready
        // to unplug. Failing here changes nothing on the stick.
        let _ = drives::unmount(drive);
    }
    Ok(())
}

pub fn fits(drive: &Drive, iso_size: u64) -> Result<()> {
    if drive.size < iso_size {
        bail!("{} is smaller than the Fedora ISO", drive.path);
    }
    Ok(())
}
