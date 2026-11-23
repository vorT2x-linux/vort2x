// SPDX-License-Identifier: GPL-3.0-or-later

//! Small helpers around processes, users and files that every step needs.

use std::ffi::{CStr, CString};
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};
use sha2::{Digest, Sha256};

use crate::report::Report;

/// The person who started vort2x-usb, also when it now runs under sudo.
/// Downloads go to their folder and belong to them, not to root.
#[derive(Clone, Debug)]
pub struct CallingUser {
    pub name: String,
    pub uid: u32,
    pub gid: u32,
    pub home: PathBuf,
}

pub fn is_root() -> bool {
    // SAFETY: geteuid has no preconditions.
    unsafe { libc::geteuid() == 0 }
}

pub fn calling_user() -> Result<CallingUser> {
    if is_root()
        && let Ok(name) = std::env::var("SUDO_USER")
        && name != "root"
    {
        return user_by_name(&name);
    }
    // SAFETY: getuid has no preconditions.
    let uid = unsafe { libc::getuid() };
    user_by_uid(uid)
}

fn user_by_name(name: &str) -> Result<CallingUser> {
    let c_name = CString::new(name).context("user name contains a NUL byte")?;
    // SAFETY: c_name is a valid C string; the result is copied before any
    // other call can reuse the static buffer.
    let entry = unsafe { libc::getpwnam(c_name.as_ptr()) };
    copy_passwd(entry).with_context(|| format!("unknown user {name}"))
}

fn user_by_uid(uid: u32) -> Result<CallingUser> {
    // SAFETY: as above, the entry is copied right away.
    let entry = unsafe { libc::getpwuid(uid) };
    copy_passwd(entry).with_context(|| format!("unknown user id {uid}"))
}

fn copy_passwd(entry: *mut libc::passwd) -> Option<CallingUser> {
    if entry.is_null() {
        return None;
    }
    // SAFETY: entry is non-null and points to getpw*'s static record.
    let entry = unsafe { &*entry };
    // SAFETY: pw_name and pw_dir are NUL-terminated strings in that record.
    let (name, home) = unsafe { (CStr::from_ptr(entry.pw_name), CStr::from_ptr(entry.pw_dir)) };
    Some(CallingUser {
        name: name.to_string_lossy().into_owned(),
        uid: entry.pw_uid,
        gid: entry.pw_gid,
        home: PathBuf::from(home.to_string_lossy().into_owned()),
    })
}

/// Gives a file made while running as root back to the calling user.
pub fn give_to(path: &Path, user: &CallingUser) -> Result<()> {
    if !is_root() {
        return Ok(());
    }
    std::os::unix::fs::chown(path, Some(user.uid), Some(user.gid))
        .with_context(|| format!("could not give {} to {}", path.display(), user.name))
}

/// Runs a command and returns its standard output; fails with its error
/// output when it exits non-zero.
pub fn output(command: &mut Command) -> Result<Vec<u8>> {
    let program = format!("{:?}", command.get_program());
    let result = command
        .output()
        .with_context(|| format!("could not run {program}"))?;
    if !result.status.success() {
        bail!(
            "{program} failed ({}): {}",
            result.status,
            String::from_utf8_lossy(&result.stderr).trim()
        );
    }
    Ok(result.stdout)
}

/// SHA-256 of a file as lowercase hex, reporting progress against its size.
pub fn sha256_file(path: &Path, report: &dyn Report) -> Result<String> {
    let mut file =
        File::open(path).with_context(|| format!("could not open {}", path.display()))?;
    let total = file.metadata()?.len();
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 4 << 20];
    let mut done = 0u64;
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
        done += read as u64;
        report.progress(done, total);
    }
    Ok(hex(&hasher.finalize()))
}

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_is_lowercase_and_padded() {
        assert_eq!(hex(&[0x00, 0x0f, 0xab]), "000fab");
    }

    #[test]
    fn calling_user_has_a_home() {
        let user = calling_user().unwrap();
        assert!(!user.name.is_empty());
        assert!(user.home.is_absolute());
    }
}
