// SPDX-License-Identifier: GPL-3.0-or-later

//! External drives through `diskutil`, as the bash script did.

use std::process::Command;

use anyhow::{Context, Result};
use plist::Value;

use super::Drive;
use crate::system::output;

#[cfg_attr(test, allow(dead_code))]
pub fn list() -> Result<Vec<Drive>> {
    let listing =
        output(Command::new("diskutil").args(["list", "-plist", "external", "physical"]))?;
    let mut drives = Vec::new();
    for disk in whole_disks(&listing)? {
        let info =
            output(Command::new("diskutil").args(["info", "-plist", &format!("/dev/{disk}")]))?;
        if let Some(drive) = drive_from_info(&disk, &info)? {
            drives.push(drive);
        }
    }
    Ok(drives)
}

#[cfg_attr(test, allow(dead_code))]
pub fn unmount(drive: &Drive) -> Result<()> {
    output(Command::new("diskutil").args(["unmountDisk", &drive.path]))?;
    Ok(())
}

/// `WholeDisks` from `diskutil list -plist`, e.g. `["disk4"]`.
fn whole_disks(plist: &[u8]) -> Result<Vec<String>> {
    let value = Value::from_reader(std::io::Cursor::new(plist))
        .context("unreadable diskutil list output")?;
    Ok(value
        .as_dictionary()
        .and_then(|dict| dict.get("WholeDisks"))
        .and_then(Value::as_array)
        .map(|disks| {
            disks
                .iter()
                .filter_map(Value::as_string)
                .filter(|name| valid_disk_name(name))
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default())
}

/// A drive from `diskutil info -plist`, only if it is external and whole.
fn drive_from_info(disk: &str, plist: &[u8]) -> Result<Option<Drive>> {
    let value = Value::from_reader(std::io::Cursor::new(plist))
        .context("unreadable diskutil info output")?;
    let Some(info) = value.as_dictionary() else {
        return Ok(None);
    };
    let flag = |key: &str| info.get(key).and_then(Value::as_boolean);
    if flag("Internal") != Some(false) || flag("WholeDisk") != Some(true) {
        return Ok(None);
    }
    let Some(size) = info.get("TotalSize").and_then(Value::as_unsigned_integer) else {
        return Ok(None);
    };
    let model = info
        .get("MediaName")
        .and_then(Value::as_string)
        .filter(|name| !name.trim().is_empty())
        .unwrap_or("unknown external disk")
        .trim()
        .to_owned();
    Ok(Some(Drive {
        path: format!("/dev/{disk}"),
        raw_path: format!("/dev/r{disk}"),
        model,
        size,
    }))
}

fn valid_disk_name(name: &str) -> bool {
    name.strip_prefix("disk")
        .is_some_and(|number| !number.is_empty() && number.bytes().all(|b| b.is_ascii_digit()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plist(body: &str) -> Vec<u8> {
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>{body}</dict></plist>"#
        )
        .into_bytes()
    }

    #[test]
    fn lists_whole_disks_only() {
        let listing = plist(
            "<key>WholeDisks</key><array><string>disk4</string><string>disk4s1</string><string>../x</string></array>",
        );
        assert_eq!(whole_disks(&listing).unwrap(), vec!["disk4"]);
        assert!(whole_disks(&plist("")).unwrap().is_empty());
    }

    #[test]
    fn external_whole_disk_is_offered() {
        let info = plist(
            "<key>Internal</key><false/><key>WholeDisk</key><true/>\
             <key>MediaName</key><string>SanDisk Ultra </string>\
             <key>TotalSize</key><integer>32017047552</integer>",
        );
        let drive = drive_from_info("disk4", &info).unwrap().unwrap();
        assert_eq!(drive.path, "/dev/disk4");
        assert_eq!(drive.raw_path, "/dev/rdisk4");
        assert_eq!(drive.model, "SanDisk Ultra");
        assert_eq!(drive.size, 32_017_047_552);
    }

    #[test]
    fn internal_or_partial_disks_are_refused() {
        let internal = plist(
            "<key>Internal</key><true/><key>WholeDisk</key><true/><key>TotalSize</key><integer>1</integer>",
        );
        let partition = plist(
            "<key>Internal</key><false/><key>WholeDisk</key><false/><key>TotalSize</key><integer>1</integer>",
        );
        let unknown = plist("<key>WholeDisk</key><true/><key>TotalSize</key><integer>1</integer>");
        for info in [internal, partition, unknown] {
            assert_eq!(drive_from_info("disk0", &info).unwrap(), None);
        }
    }
}
