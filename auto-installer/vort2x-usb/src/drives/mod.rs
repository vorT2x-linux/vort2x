// SPDX-License-Identifier: GPL-3.0-or-later

//! The USB drives the stick can be written to. Only whole external drives
//! are ever offered; internal and system disks are left out on purpose.

use anyhow::{Result, bail};

#[cfg(any(target_os = "linux", test))]
mod linux;
#[cfg(any(target_os = "macos", test))]
mod macos;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Drive {
    /// Device path, e.g. `/dev/disk4` or `/dev/sdb`.
    pub path: String,
    /// The device written to: `/dev/rdisk4` on macOS (unbuffered and much
    /// faster), the same path on Linux.
    pub raw_path: String,
    pub model: String,
    pub size: u64,
}

impl Drive {
    /// The short name used in the confirmation, e.g. `disk4` or `sdb`.
    pub fn short_name(&self) -> &str {
        self.path.rsplit('/').next().unwrap_or(&self.path)
    }

    /// What the user types to confirm erasing this drive.
    pub fn confirm_phrase(&self) -> String {
        format!("ERASE {}", self.short_name())
    }
}

pub fn list() -> Result<Vec<Drive>> {
    #[cfg(target_os = "macos")]
    return macos::list();
    #[cfg(target_os = "linux")]
    return linux::list();
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    bail!("vort2x-usb runs on macOS and Linux only");
}

/// The drive given with `--disk`, only if it is one `list` would offer.
pub fn find(path: &str) -> Result<Drive> {
    match list()?.into_iter().find(|drive| drive.path == path) {
        Some(drive) => Ok(drive),
        None => {
            bail!("{path} is not an external whole drive; refusing to erase it (see --list-drives)")
        }
    }
}

/// Unmounts every file system on the drive, so nothing else writes to it.
pub fn unmount(drive: &Drive) -> Result<()> {
    #[cfg(target_os = "macos")]
    return macos::unmount(drive);
    #[cfg(target_os = "linux")]
    return linux::unmount(drive);
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        let _ = drive;
        bail!("vort2x-usb runs on macOS and Linux only");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn confirmation_names_the_drive() {
        let drive = Drive {
            path: "/dev/disk4".into(),
            raw_path: "/dev/rdisk4".into(),
            model: "SanDisk".into(),
            size: 1,
        };
        assert_eq!(drive.short_name(), "disk4");
        assert_eq!(drive.confirm_phrase(), "ERASE disk4");
    }
}
