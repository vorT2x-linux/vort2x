// SPDX-License-Identifier: GPL-3.0-or-later

//! External drives through `lsblk`. A drive is offered when it is a whole
//! disk that is removable, hot-pluggable or on USB, and no system file
//! system lives on it.

use std::process::Command;

use anyhow::{Context, Result};
use serde_json::Value;

use super::Drive;
use crate::system::output;

/// Mount points that mean "this disk runs the computer".
const SYSTEM_MOUNTS: [&str; 7] = ["/", "/boot", "/boot/efi", "/efi", "/home", "/usr", "[SWAP]"];

#[cfg_attr(test, allow(dead_code))]
pub fn list() -> Result<Vec<Drive>> {
    drives_from_lsblk(&lsblk(None)?)
}

#[cfg_attr(test, allow(dead_code))]
pub fn unmount(drive: &Drive) -> Result<()> {
    let listing = lsblk(Some(&drive.path))?;
    let value: Value = serde_json::from_slice(&listing).context("unreadable lsblk output")?;
    let mut mounts = Vec::new();
    for device in value["blockdevices"].as_array().into_iter().flatten() {
        collect_mounts(device, &mut mounts);
    }
    for mount in mounts {
        output(Command::new("umount").arg(&mount))
            .with_context(|| format!("could not unmount {mount} on {}", drive.path))?;
    }
    Ok(())
}

fn lsblk(device: Option<&str>) -> Result<Vec<u8>> {
    let mut command = Command::new("lsblk");
    command.args([
        "-J",
        "-b",
        "-o",
        "PATH,TYPE,SIZE,RM,HOTPLUG,TRAN,MODEL,MOUNTPOINTS",
    ]);
    if let Some(device) = device {
        command.arg(device);
    }
    output(&mut command)
}

fn drives_from_lsblk(json: &[u8]) -> Result<Vec<Drive>> {
    let value: Value = serde_json::from_slice(json).context("unreadable lsblk output")?;
    let mut drives = Vec::new();
    for device in value["blockdevices"].as_array().into_iter().flatten() {
        let external = flag(&device["rm"])
            || flag(&device["hotplug"])
            || device["tran"].as_str() == Some("usb");
        let Some(path) = device["path"].as_str() else {
            continue;
        };
        let size = number(&device["size"]);
        if device["type"].as_str() != Some("disk") || !external || size == 0 {
            continue;
        }
        let mut mounts = Vec::new();
        collect_mounts(device, &mut mounts);
        if mounts
            .iter()
            .any(|mount| SYSTEM_MOUNTS.contains(&mount.as_str()))
        {
            continue;
        }
        let model = device["model"]
            .as_str()
            .map(str::trim)
            .filter(|model| !model.is_empty())
            .unwrap_or("unknown external disk");
        drives.push(Drive {
            path: path.to_owned(),
            raw_path: path.to_owned(),
            model: model.to_owned(),
            size,
        });
    }
    Ok(drives)
}

/// Mount points of a device and everything on it (partitions, LVM, crypt).
fn collect_mounts(device: &Value, mounts: &mut Vec<String>) {
    // util-linux 2.37 renamed `mountpoint` to the list `mountpoints`.
    let listed = device["mountpoints"].as_array().into_iter().flatten();
    for mount in listed.chain(std::iter::once(&device["mountpoint"])) {
        if let Some(mount) = mount.as_str().filter(|mount| !mount.is_empty()) {
            mounts.push(mount.to_owned());
        }
    }
    for child in device["children"].as_array().into_iter().flatten() {
        collect_mounts(child, mounts);
    }
}

/// lsblk prints flags as `true`/`false`, older versions as `"1"`/`"0"`.
fn flag(value: &Value) -> bool {
    value
        .as_bool()
        .unwrap_or_else(|| value.as_str() == Some("1"))
}

/// Sizes are numbers with `-b`, strings in older versions.
fn number(value: &Value) -> u64 {
    value
        .as_u64()
        .or_else(|| value.as_str().and_then(|s| s.parse().ok()))
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    const LSBLK: &str = r#"{"blockdevices": [
        {"path":"/dev/nvme0n1","type":"disk","size":512110190592,"rm":false,"hotplug":false,"tran":"nvme","model":"APPLE SSD","mountpoints":[null],
         "children":[{"path":"/dev/nvme0n1p3","type":"part","size":1,"rm":false,"hotplug":false,"tran":null,"model":null,"mountpoints":["/","/home"]}]},
        {"path":"/dev/sda","type":"disk","size":32017047552,"rm":true,"hotplug":true,"tran":"usb","model":"Ultra   ","mountpoints":[null],
         "children":[{"path":"/dev/sda1","type":"part","size":1,"rm":true,"hotplug":true,"tran":null,"model":null,"mountpoints":["/run/media/kirk/STICK"]}]},
        {"path":"/dev/sdb","type":"disk","size":"64000000000","rm":"0","hotplug":"1","tran":"usb","model":null,"mountpoint":null},
        {"path":"/dev/sdc","type":"disk","size":64000000000,"rm":false,"hotplug":true,"tran":"usb","model":"Boot stick","mountpoints":[null],
         "children":[{"path":"/dev/sdc1","type":"part","size":1,"rm":false,"hotplug":true,"tran":null,"model":null,"mountpoints":["/boot/efi"]}]},
        {"path":"/dev/sdd","type":"disk","size":0,"rm":true,"hotplug":true,"tran":"usb","model":"Card reader","mountpoints":[null]},
        {"path":"/dev/loop0","type":"loop","size":4096,"rm":false,"hotplug":false,"tran":null,"model":null,"mountpoints":[null]},
        {"path":"/dev/sr0","type":"rom","size":4096,"rm":true,"hotplug":true,"tran":"usb","model":"DVD","mountpoints":[null]}
    ]}"#;

    #[test]
    fn offers_only_external_drives_without_system_mounts() {
        let drives = drives_from_lsblk(LSBLK.as_bytes()).unwrap();
        let paths: Vec<_> = drives.iter().map(|d| d.path.as_str()).collect();
        // Not the internal SSD, not the stick holding /boot/efi, not the empty
        // card reader, not loop or optical devices.
        assert_eq!(paths, ["/dev/sda", "/dev/sdb"]);
        assert_eq!(drives[0].model, "Ultra");
        assert_eq!(drives[0].raw_path, "/dev/sda");
        assert_eq!(drives[1].model, "unknown external disk");
        assert_eq!(drives[1].size, 64_000_000_000);
    }

    #[test]
    fn finds_mounts_on_partitions() {
        let value: Value = serde_json::from_str(LSBLK).unwrap();
        let mut mounts = Vec::new();
        collect_mounts(&value["blockdevices"][1], &mut mounts);
        assert_eq!(mounts, ["/run/media/kirk/STICK"]);
    }
}
