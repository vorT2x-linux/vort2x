// SPDX-License-Identifier: GPL-3.0-or-later

//! What vort2x-usb changes in the Fedora image: only its EFI System
//! partition. It gets a VorT2x `grub.cfg` (Fedora's is kept next to it) and
//! the `EFI/BOOT/vort2x/` files. The partition is edited in memory, so
//! nothing has to be mounted on either OS.

use std::io::{Cursor, Read, Seek, SeekFrom, Write};

use anyhow::{Context, Result, bail, ensure};

const SECTOR: u64 = 512;
const PVD_OFFSET: u64 = 16 * 2048;
const ISO_LABEL_PLACEHOLDER: &str = "@ISO_VOLUME_LABEL@";
/// The EFI System partition type `C12A7328-F81F-11D2-BA4B-00A0C93EC93B` as
/// stored on disk (the first three fields are little-endian).
const ESP_GUID: [u8; 16] = [
    0x28, 0x73, 0x2a, 0xc1, 0x1f, 0xf8, 0xd2, 0x11, 0xba, 0x4b, 0x00, 0xa0, 0xc9, 0x3e, 0xc9, 0x3b,
];
const MBR_ESP_TYPE: u8 = 0xef;

/// The ISO9660 volume label, e.g. `Fedora-CSMC-Live-45`. GRUB finds the live
/// system by it. Spaces are dropped, as the bash script did.
pub fn volume_label(image: &mut (impl Read + Seek)) -> Result<String> {
    let mut sector = [0u8; 2048];
    image.seek(SeekFrom::Start(PVD_OFFSET))?;
    image
        .read_exact(&mut sector)
        .context("the image is too short for an ISO9660 volume descriptor")?;
    ensure!(
        sector[0] == 1 && &sector[1..6] == b"CD001",
        "the Fedora ISO has no ISO9660 primary volume descriptor"
    );
    let label: String = sector[40..72]
        .iter()
        .filter(|byte| **byte != b' ')
        .map(|byte| *byte as char)
        .collect();
    ensure!(
        !label.is_empty()
            && label
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'.' | b'-')),
        "unexpected Fedora ISO volume label: {label:?}"
    );
    Ok(label)
}

/// Byte offset and length of the EFI System partition, from the GPT, or from
/// the MBR when there is no GPT.
pub fn esp_range(image: &mut (impl Read + Seek), image_size: u64) -> Result<(u64, u64)> {
    let mut head = vec![0u8; 2 * SECTOR as usize];
    image.seek(SeekFrom::Start(0))?;
    image
        .read_exact(&mut head)
        .context("the image has no partition table")?;
    let (start, length) = if &head[512..520] == b"EFI PART" {
        gpt_esp(image, &head[512..])?
    } else {
        mbr_esp(&head[..512])?
    };
    ensure!(
        length > 0
            && start
                .checked_add(length)
                .is_some_and(|end| end <= image_size),
        "the EFI partition lies outside the image"
    );
    Ok((start, length))
}

fn gpt_esp(image: &mut (impl Read + Seek), header: &[u8]) -> Result<(u64, u64)> {
    let entries_lba = u64::from_le_bytes(header[72..80].try_into()?);
    let count = u32::from_le_bytes(header[80..84].try_into()?) as u64;
    let entry_size = u32::from_le_bytes(header[84..88].try_into()?) as u64;
    ensure!(
        (128..=4096).contains(&entry_size) && count <= 1024,
        "unexpected GPT layout"
    );
    let mut entries = vec![0u8; (count * entry_size) as usize];
    image.seek(SeekFrom::Start(entries_lba * SECTOR))?;
    image.read_exact(&mut entries)?;
    for entry in entries.chunks(entry_size as usize) {
        if entry[..16] == ESP_GUID {
            let first = u64::from_le_bytes(entry[32..40].try_into()?);
            let last = u64::from_le_bytes(entry[40..48].try_into()?);
            ensure!(last >= first, "unexpected GPT partition range");
            return Ok((first * SECTOR, (last - first + 1) * SECTOR));
        }
    }
    bail!("the Fedora ISO has no EFI System partition")
}

fn mbr_esp(mbr: &[u8]) -> Result<(u64, u64)> {
    ensure!(
        mbr[510..512] == [0x55, 0xaa],
        "the Fedora ISO has no partition table"
    );
    for entry in mbr[446..510].chunks(16) {
        if entry[4] == MBR_ESP_TYPE {
            let first = u32::from_le_bytes(entry[8..12].try_into()?) as u64;
            let count = u32::from_le_bytes(entry[12..16].try_into()?) as u64;
            return Ok((first * SECTOR, count * SECTOR));
        }
    }
    bail!("the Fedora ISO has no EFI System partition")
}

/// `grub.cfg.in` with the ISO's volume label filled in.
pub fn render_grub(template: &str, label: &str) -> Result<String> {
    ensure!(
        template.contains(ISO_LABEL_PLACEHOLDER),
        "the GRUB template has no {ISO_LABEL_PLACEHOLDER}"
    );
    Ok(template.replace(ISO_LABEL_PLACEHOLDER, label))
}

/// The files VorT2x adds to the EFI partition.
pub struct BootFiles<'a> {
    pub grub_cfg: &'a [u8],
    pub input_initramfs: &'a [u8],
    pub wifi_initramfs: &'a [u8],
    pub wifi_sha256: &'a [u8],
}

pub const FEDORA_GRUB: &str = "EFI/BOOT/grub.cfg";
pub const FEDORA_GRUB_ORIGINAL: &str = "EFI/BOOT/grub.cfg.fedora-original";
pub const VORT2X_DIR: &str = "EFI/BOOT/vort2x";

/// Adds the VorT2x files to an EFI partition image and returns it. Fedora's
/// `grub.cfg` is kept as `grub.cfg.fedora-original`; an existing copy is never
/// overwritten, so running this twice keeps Fedora's file.
pub fn edit_esp(esp: Vec<u8>, files: &BootFiles) -> Result<Vec<u8>> {
    let mut disk = Cursor::new(esp);
    {
        let fs = fatfs::FileSystem::new(&mut disk, fatfs::FsOptions::new())
            .context("the EFI partition is not a FAT file system")?;
        let root = fs.root_dir();
        let fedora_grub = read(&root, FEDORA_GRUB)
            .context("the original Fedora GRUB configuration is missing")?;
        if read(&root, FEDORA_GRUB_ORIGINAL).is_err() {
            write(&root, FEDORA_GRUB_ORIGINAL, &fedora_grub)?;
        }
        root.create_dir(VORT2X_DIR)?;
        let wanted = [
            (FEDORA_GRUB.to_owned(), files.grub_cfg),
            (
                format!("{VORT2X_DIR}/vort2x-input-initramfs.img"),
                files.input_initramfs,
            ),
            (
                format!("{VORT2X_DIR}/vort2x-wifi-initramfs.img"),
                files.wifi_initramfs,
            ),
            (
                format!("{VORT2X_DIR}/wifi-firmware.sha256"),
                files.wifi_sha256,
            ),
        ];
        for (path, data) in &wanted {
            write(&root, path, data).with_context(|| {
                format!("could not write {path}; the EFI partition may not have enough free space")
            })?;
        }
        for (path, data) in &wanted {
            ensure!(read(&root, path)? == *data, "{path} reads back differently");
        }
        drop(root);
        fs.unmount().context("could not finish the EFI partition")?;
    }
    Ok(disk.into_inner())
}

fn read<T: fatfs::ReadWriteSeek>(root: &fatfs::Dir<T>, path: &str) -> Result<Vec<u8>> {
    let mut data = Vec::new();
    root.open_file(path)?.read_to_end(&mut data)?;
    Ok(data)
}

fn write<T: fatfs::ReadWriteSeek>(root: &fatfs::Dir<T>, path: &str, data: &[u8]) -> Result<()> {
    let mut file = root.create_file(path)?;
    file.truncate()?;
    file.write_all(data)?;
    file.flush()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read_esp_file(esp: &[u8], path: &str) -> Result<Vec<u8>> {
        let mut disk = Cursor::new(esp.to_vec());
        let fs = fatfs::FileSystem::new(&mut disk, fatfs::FsOptions::new())?;
        read(&fs.root_dir(), path)
    }

    /// A 16 MiB FAT16 partition with Fedora's grub.cfg in it.
    fn fedora_esp() -> Vec<u8> {
        let mut disk = Cursor::new(vec![0u8; 16 << 20]);
        fatfs::format_volume(
            &mut disk,
            fatfs::FormatVolumeOptions::new().fat_type(fatfs::FatType::Fat16),
        )
        .unwrap();
        {
            let fs = fatfs::FileSystem::new(&mut disk, fatfs::FsOptions::new()).unwrap();
            let root = fs.root_dir();
            root.create_dir("EFI").unwrap();
            root.create_dir("EFI/BOOT").unwrap();
            write(&root, FEDORA_GRUB, b"fedora grub").unwrap();
            write(&root, "EFI/BOOT/BOOTX64.EFI", b"shim").unwrap();
            drop(root);
            fs.unmount().unwrap();
        }
        disk.into_inner()
    }

    fn files<'a>(grub: &'a [u8]) -> BootFiles<'a> {
        BootFiles {
            grub_cfg: grub,
            input_initramfs: b"input",
            wifi_initramfs: &[7u8; 300_000],
            wifi_sha256: b"sums",
        }
    }

    #[test]
    fn adds_files_and_keeps_fedora_grub() {
        let esp = edit_esp(fedora_esp(), &files(b"vort2x grub")).unwrap();
        assert_eq!(read_esp_file(&esp, FEDORA_GRUB).unwrap(), b"vort2x grub");
        assert_eq!(
            read_esp_file(&esp, FEDORA_GRUB_ORIGINAL).unwrap(),
            b"fedora grub"
        );
        assert_eq!(
            read_esp_file(&esp, "EFI/BOOT/BOOTX64.EFI").unwrap(),
            b"shim"
        );
        assert_eq!(
            read_esp_file(&esp, "EFI/BOOT/vort2x/vort2x-input-initramfs.img").unwrap(),
            b"input"
        );
        assert_eq!(
            read_esp_file(&esp, "EFI/BOOT/vort2x/vort2x-wifi-initramfs.img").unwrap(),
            vec![7u8; 300_000]
        );
        assert_eq!(
            read_esp_file(&esp, "EFI/BOOT/vort2x/wifi-firmware.sha256").unwrap(),
            b"sums"
        );
    }

    #[test]
    fn running_twice_keeps_the_original() {
        let once = edit_esp(fedora_esp(), &files(b"first")).unwrap();
        let twice = edit_esp(once, &files(b"second, longer than the first")).unwrap();
        assert_eq!(
            read_esp_file(&twice, FEDORA_GRUB_ORIGINAL).unwrap(),
            b"fedora grub"
        );
        assert_eq!(
            read_esp_file(&twice, FEDORA_GRUB).unwrap(),
            b"second, longer than the first"
        );
    }

    #[test]
    fn refuses_a_partition_without_fedora_grub() {
        let mut disk = Cursor::new(vec![0u8; 16 << 20]);
        fatfs::format_volume(&mut disk, fatfs::FormatVolumeOptions::new()).unwrap();
        assert!(edit_esp(disk.into_inner(), &files(b"x")).is_err());
        assert!(edit_esp(vec![0u8; 1 << 20], &files(b"x")).is_err());
    }

    #[test]
    fn too_little_space_is_an_error() {
        let mut big = files(b"x");
        let huge = vec![1u8; 20 << 20];
        big.wifi_initramfs = &huge;
        assert!(edit_esp(fedora_esp(), &big).is_err());
    }

    fn iso_with_label(label: &[u8]) -> Cursor<Vec<u8>> {
        let mut image = vec![0u8; 40 * 2048];
        let pvd = PVD_OFFSET as usize;
        image[pvd] = 1;
        image[pvd + 1..pvd + 6].copy_from_slice(b"CD001");
        image[pvd + 40..pvd + 72].fill(b' ');
        image[pvd + 40..pvd + 40 + label.len()].copy_from_slice(label);
        Cursor::new(image)
    }

    #[test]
    fn reads_the_volume_label() {
        assert_eq!(
            volume_label(&mut iso_with_label(b"Fedora-CSMC-Live-45")).unwrap(),
            "Fedora-CSMC-Live-45"
        );
        assert!(volume_label(&mut iso_with_label(b"bad/label")).is_err());
        assert!(volume_label(&mut iso_with_label(b"")).is_err());
        assert!(volume_label(&mut Cursor::new(vec![0u8; 40 * 2048])).is_err());
    }

    #[test]
    fn finds_the_esp_in_a_gpt() {
        // The layout of Fedora-COSMIC-Live-45-1.4: ISO9660 first, then the ESP.
        let mut image = vec![0u8; 64 * 1024];
        image[510] = 0x55;
        image[511] = 0xaa;
        image[446 + 4] = 0xee;
        let header = 512;
        image[header..header + 8].copy_from_slice(b"EFI PART");
        image[header + 72..header + 80].copy_from_slice(&2u64.to_le_bytes());
        image[header + 80..header + 84].copy_from_slice(&2u32.to_le_bytes());
        image[header + 84..header + 88].copy_from_slice(&128u32.to_le_bytes());
        let first = 1024;
        image[first..first + 16].fill(0xa2); // basic data
        image[first + 32..first + 40].copy_from_slice(&64u64.to_le_bytes());
        image[first + 40..first + 48].copy_from_slice(&6_299_659u64.to_le_bytes());
        let second = first + 128;
        image[second..second + 16].copy_from_slice(&ESP_GUID);
        image[second + 32..second + 40].copy_from_slice(&6_299_660u64.to_le_bytes());
        image[second + 40..second + 48].copy_from_slice(&6_361_099u64.to_le_bytes());
        let size = 3_256_915_968;
        assert_eq!(
            esp_range(&mut Cursor::new(image.clone()), size).unwrap(),
            (6_299_660 * 512, 61_440 * 512)
        );
        assert!(esp_range(&mut Cursor::new(image), 1000).is_err());
    }

    #[test]
    fn finds_the_esp_in_an_mbr() {
        let mut image = vec![0u8; 4096];
        image[510] = 0x55;
        image[511] = 0xaa;
        let entry = 446 + 16;
        image[entry + 4] = MBR_ESP_TYPE;
        image[entry + 8..entry + 12].copy_from_slice(&100u32.to_le_bytes());
        image[entry + 12..entry + 16].copy_from_slice(&50u32.to_le_bytes());
        assert_eq!(
            esp_range(&mut Cursor::new(image), 1 << 20).unwrap(),
            (100 * 512, 50 * 512)
        );
        assert!(esp_range(&mut Cursor::new(vec![0u8; 4096]), 1 << 20).is_err());
    }

    #[test]
    fn renders_the_label_into_grub() {
        let grub = render_grub(
            "search --label @ISO_VOLUME_LABEL@\nroot=live:CDLABEL=@ISO_VOLUME_LABEL@",
            "F-45",
        )
        .unwrap();
        assert_eq!(grub, "search --label F-45\nroot=live:CDLABEL=F-45");
        assert!(render_grub("no placeholder", "F-45").is_err());
    }
}
