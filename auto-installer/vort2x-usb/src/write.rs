// SPDX-License-Identifier: GPL-3.0-or-later

//! Writes the Fedora ISO, with the edited EFI partition in place of its own,
//! in one pass, then reads it all back and compares checksums.

use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;

use anyhow::{Context, Result, ensure};
use sha2::{Digest, Sha256};

use crate::report::Report;

/// 4 MiB blocks: fast on USB sticks, and every block (also the last, as an
/// ISO is a whole number of 2048-byte sectors) is a multiple of 512 bytes,
/// which raw devices require.
const BLOCK: usize = 4 << 20;

pub struct Image<'a> {
    pub iso: &'a Path,
    pub size: u64,
    pub esp_offset: u64,
    pub esp: &'a [u8],
}

/// Where the stick image goes: a drive's raw device or a plain file.
pub fn open_target(path: &Path, is_file: bool) -> Result<File> {
    let mut options = OpenOptions::new();
    options.write(true).read(true);
    if is_file {
        options.create(true).truncate(true);
    }
    options
        .open(path)
        .with_context(|| format!("could not open {} for writing", path.display()))
}

/// Writes the image and returns the SHA-256 of what was written.
pub fn write(image: &Image, target: &mut File, report: &dyn Report) -> Result<[u8; 32]> {
    ensure!(
        image.size.is_multiple_of(512),
        "the ISO size is not a whole number of sectors"
    );
    let esp_end = image.esp_offset + image.esp.len() as u64;
    ensure!(
        esp_end <= image.size,
        "the EFI partition lies outside the ISO"
    );

    let mut iso =
        File::open(image.iso).with_context(|| format!("could not open {}", image.iso.display()))?;
    let mut buffer = vec![0u8; BLOCK];
    let mut hasher = Sha256::new();
    let mut offset = 0u64;
    target.seek(SeekFrom::Start(0))?;
    while offset < image.size {
        let length = BLOCK.min((image.size - offset) as usize);
        let block = &mut buffer[..length];
        iso.read_exact(block)
            .with_context(|| format!("the ISO ended early at byte {offset}"))?;
        overlay(block, offset, image.esp_offset, image.esp);
        target
            .write_all(block)
            .with_context(|| format!("writing failed at byte {offset}"))?;
        hasher.update(&*block);
        offset += length as u64;
        report.progress(offset, image.size);
    }
    target.sync_all().context("could not flush the drive")?;
    Ok(hasher.finalize().into())
}

/// Replaces the part of `block` (at `offset` in the image) that the EFI
/// partition covers with the edited partition's bytes.
fn overlay(block: &mut [u8], offset: u64, esp_offset: u64, esp: &[u8]) {
    let block_end = offset + block.len() as u64;
    let esp_end = esp_offset + esp.len() as u64;
    let start = offset.max(esp_offset);
    let end = block_end.min(esp_end);
    if start >= end {
        return;
    }
    let length = (end - start) as usize;
    let in_block = (start - offset) as usize;
    let in_esp = (start - esp_offset) as usize;
    block[in_block..in_block + length].copy_from_slice(&esp[in_esp..in_esp + length]);
}

/// Reads the first `size` bytes back and checks they hash to `expected`.
pub fn verify(
    target: &mut File,
    size: u64,
    expected: &[u8; 32],
    report: &dyn Report,
) -> Result<()> {
    drop_cache(target);
    target.seek(SeekFrom::Start(0))?;
    let mut buffer = vec![0u8; BLOCK];
    let mut hasher = Sha256::new();
    let mut offset = 0u64;
    while offset < size {
        let length = BLOCK.min((size - offset) as usize);
        target
            .read_exact(&mut buffer[..length])
            .with_context(|| format!("reading back failed at byte {offset}"))?;
        hasher.update(&buffer[..length]);
        offset += length as u64;
        report.progress(offset, size);
    }
    ensure!(
        hasher.finalize().as_slice() == expected,
        "the stick reads back differently from what was written; try another USB drive"
    );
    Ok(())
}

/// On Linux the block device has a page cache; empty it so the check reads
/// the stick, not memory. macOS raw devices (`/dev/rdiskN`) have no cache.
fn drop_cache(target: &File) {
    #[cfg(target_os = "linux")]
    {
        use std::os::fd::AsRawFd;
        // BLKFLSBUF = _IO(0x12, 97). Fails harmlessly on regular files.
        const BLKFLSBUF: libc::c_ulong = 0x1261;
        // SAFETY: the descriptor is open for the whole call; the ioctl takes
        // no argument.
        unsafe {
            libc::ioctl(target.as_raw_fd(), BLKFLSBUF as _, 0);
        }
    }
    #[cfg(not(target_os = "linux"))]
    let _ = target;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overlay_replaces_only_the_partition() {
        // Partition straddles two blocks: bytes 6..14 of the image.
        let esp: Vec<u8> = (0..8).collect();
        let mut first = [0xffu8; 10];
        overlay(&mut first, 0, 6, &esp);
        assert_eq!(first, [0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0, 1, 2, 3]);
        let mut second = [0xffu8; 10];
        overlay(&mut second, 10, 6, &esp);
        assert_eq!(second, [4, 5, 6, 7, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff]);
        let mut outside = [0xffu8; 4];
        overlay(&mut outside, 20, 6, &esp);
        assert_eq!(outside, [0xff; 4]);
    }

    #[test]
    fn writes_and_verifies_a_file() {
        let dir = std::env::temp_dir().join(format!("vort2x-usb-write-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let iso_path = dir.join("a.iso");
        // Bigger than one block, so the overlay crosses a block boundary.
        let iso: Vec<u8> = (0..(BLOCK + 8192)).map(|i| (i % 251) as u8).collect();
        std::fs::write(&iso_path, &iso).unwrap();
        let esp = vec![9u8; 4096];
        let esp_offset = BLOCK as u64 - 1024;
        let image = Image {
            iso: &iso_path,
            size: iso.len() as u64,
            esp_offset,
            esp: &esp,
        };
        let quiet = crate::report::Plain::new();
        let target_path = dir.join("stick.img");
        let mut target = open_target(&target_path, true).unwrap();
        let hash = write(&image, &mut target, &quiet).unwrap();
        verify(&mut target, image.size, &hash, &quiet).unwrap();

        let written = std::fs::read(&target_path).unwrap();
        let start = esp_offset as usize;
        assert_eq!(&written[..start], &iso[..start]);
        assert_eq!(&written[start..start + 4096], &esp[..]);
        assert_eq!(&written[start + 4096..], &iso[start + 4096..]);

        // A changed byte on the "stick" must be caught.
        std::fs::write(
            &target_path,
            [&written[..10], &[0u8], &written[11..]].concat(),
        )
        .unwrap();
        let mut target = OpenOptions::new().read(true).open(&target_path).unwrap();
        assert!(verify(&mut target, image.size, &hash, &quiet).is_err());
        std::fs::remove_dir_all(dir).unwrap();
    }
}
