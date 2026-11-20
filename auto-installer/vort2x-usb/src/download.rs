// SPDX-License-Identifier: GPL-3.0-or-later

//! Downloading and verifying the Fedora ISO, the way `download-fedora-iso.sh`
//! does: up to ten HTTPS mirrors from Fedora's mirror list, then the canonical
//! and archive URLs, resuming a `.part` file, and only keeping data whose size
//! and SHA-256 match the catalogue. `curl` does the transfers; it is on every
//! Mac and practically every Linux system.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

use anyhow::{Context, Result, bail};

use crate::kit::Edition;
use crate::report::Report;
use crate::system::{CallingUser, give_to, sha256_file};

const MIRROR_LIMIT: usize = 10;
/// curl exit codes: a server that can't resume, and local write errors.
const CURL_CANNOT_RESUME: i32 = 33;
const CURL_WRITE_ERRORS: [i32; 2] = [23, 27];

/// True when the file has the catalogue's size and SHA-256.
pub fn verify(path: &Path, edition: &Edition, report: &dyn Report) -> Result<bool> {
    let size = match std::fs::metadata(path) {
        Ok(metadata) => metadata.len(),
        Err(_) => return Ok(false),
    };
    if size != edition.size {
        return Ok(false);
    }
    Ok(sha256_file(path, report)? == edition.sha256)
}

/// Makes sure a verified ISO is at `destination`, downloading it if needed.
/// `cancel` stops a running transfer (the partial file is kept for resuming).
pub fn ensure_iso(
    destination: &Path,
    edition: &Edition,
    user: &CallingUser,
    cancel: &AtomicBool,
    report: &dyn Report,
) -> Result<()> {
    if destination.exists() {
        report.stage("Checking the Fedora ISO you already have");
        if verify(destination, edition, report)? {
            report.note(&format!("Using verified ISO: {}", destination.display()));
            return Ok(());
        }
        bail!(
            "{} has the wrong size or checksum; remove it and try again",
            destination.display()
        );
    }

    report.stage("Finding Fedora mirrors");
    let candidates = candidates(edition, mirror_list(edition));
    let partial = PathBuf::from(format!("{}.part", destination.display()));
    report.stage(&format!("Downloading {}", edition.file_name));
    for (number, url) in candidates.iter().enumerate() {
        report.note(&format!("Source {}: {}", number + 1, host(url)));
        // An over-long partial makes every resume request fail with HTTP 416.
        if std::fs::metadata(&partial).is_ok_and(|m| m.len() >= edition.size) {
            std::fs::remove_file(&partial)?;
        }
        match fetch(url, &partial, edition.size, user, cancel, report)? {
            Fetch::Done => {}
            Fetch::Failed(why) => {
                report.note(&format!("Source failed ({why}), trying the next one"));
                continue;
            }
        }
        report.stage("Verifying the download");
        if verify(&partial, edition, report)? {
            std::fs::rename(&partial, destination)?;
            report.note(&format!("Verified Fedora ISO: {}", destination.display()));
            return Ok(());
        }
        report.note("The data failed the size or SHA-256 check; discarding it");
        std::fs::remove_file(&partial)?;
        report.stage(&format!("Downloading {}", edition.file_name));
    }
    if partial.exists() {
        bail!(
            "the Fedora download failed from all {} sources; the partial file is kept for next time: {}",
            candidates.len(),
            partial.display()
        );
    }
    bail!(
        "the Fedora download failed from all {} sources",
        candidates.len()
    );
}

/// HTTPS mirrors for this ISO from Fedora's mirror list; empty if the list
/// can't be had (the official URLs are tried anyway).
fn mirror_list(edition: &Edition) -> Vec<String> {
    let url = format!(
        "https://mirrors.fedoraproject.org/mirrorlist?path=pub/fedora/linux/releases/{}",
        edition.release_path()
    );
    let result = Command::new("curl")
        .args(["--fail", "--location", "--silent", "--show-error"])
        .args(["--retry", "2", "--retry-delay", "2"])
        .args(["--connect-timeout", "10", "--max-time", "30", &url])
        .output();
    match result {
        Ok(out) if out.status.success() => String::from_utf8_lossy(&out.stdout)
            .lines()
            .map(str::trim)
            .filter(|line| line.starts_with("https://"))
            .map(str::to_owned)
            .collect(),
        _ => Vec::new(),
    }
}

fn candidates(edition: &Edition, mirrors: Vec<String>) -> Vec<String> {
    let archive = format!(
        "https://archives.fedoraproject.org/pub/archive/fedora/linux/releases/{}",
        edition.release_path()
    );
    let mut list: Vec<String> = Vec::new();
    for url in mirrors
        .into_iter()
        .take(MIRROR_LIMIT)
        .chain([edition.url.clone(), archive])
    {
        if !list.contains(&url) {
            list.push(url);
        }
    }
    list
}

fn host(url: &str) -> &str {
    url.trim_start_matches("https://")
        .split('/')
        .next()
        .unwrap_or(url)
}

enum Fetch {
    Done,
    Failed(String),
}

/// One source, resuming `partial`. Restarts once from zero when the server
/// can't resume, so a good mirror isn't dropped for that.
fn fetch(
    url: &str,
    partial: &Path,
    total: u64,
    user: &CallingUser,
    cancel: &AtomicBool,
    report: &dyn Report,
) -> Result<Fetch> {
    let mut code = curl(url, partial, true, total, user, cancel, report)?;
    if code == Some(CURL_CANNOT_RESUME) && partial.exists() {
        report.note("This source can't resume; starting it again from the beginning");
        std::fs::remove_file(partial)?;
        code = curl(url, partial, false, total, user, cancel, report)?;
    }
    match code {
        Some(0) => Ok(Fetch::Done),
        Some(code) if CURL_WRITE_ERRORS.contains(&code) => bail!(
            "could not write {} (curl {code}); check free space and permissions",
            partial.display()
        ),
        Some(code) => Ok(Fetch::Failed(format!("curl {code}"))),
        None => Ok(Fetch::Failed("curl was stopped".into())),
    }
}

#[allow(clippy::too_many_arguments)]
fn curl(
    url: &str,
    partial: &Path,
    resume: bool,
    total: u64,
    user: &CallingUser,
    cancel: &AtomicBool,
    report: &dyn Report,
) -> Result<Option<i32>> {
    let mut command = Command::new("curl");
    command
        .args(["--fail", "--location", "--silent", "--show-error"])
        .args([
            "--retry",
            "2",
            "--retry-delay",
            "2",
            "--connect-timeout",
            "15",
        ])
        .args(["--speed-limit", "1024", "--speed-time", "30"]);
    if resume {
        command.args(["--continue-at", "-"]);
    }
    command
        .arg("--output")
        .arg(partial)
        .arg(url)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let mut child = command.spawn().context("could not start curl")?;
    let mut owned = false;
    loop {
        if let Some(status) = child.try_wait()? {
            if partial.exists() {
                give_to(partial, user)?;
            }
            return Ok(status.code());
        }
        if cancel.load(Ordering::Relaxed) {
            let _ = child.kill();
            let _ = child.wait();
            bail!("cancelled");
        }
        if let Ok(metadata) = std::fs::metadata(partial) {
            if !owned {
                give_to(partial, user)?;
                owned = true;
            }
            report.progress(metadata.len(), total);
        }
        thread::sleep(Duration::from_millis(250));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn edition() -> Edition {
        Edition {
            id: "cosmic".into(),
            name: "Fedora COSMIC Spin".into(),
            file_name: "a.iso".into(),
            url: "https://download.fedoraproject.org/pub/fedora/linux/releases/45/Spins/x86_64/iso/a.iso"
                .into(),
            size: 3,
            sha256: "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad".into(),
        }
    }

    #[test]
    fn mirrors_come_first_then_official_urls_once() {
        let mirrors = (0..12)
            .map(|n| format!("https://m{n}.example/a.iso"))
            .collect();
        let list = candidates(&edition(), mirrors);
        assert_eq!(list.len(), MIRROR_LIMIT + 2);
        assert_eq!(list[0], "https://m0.example/a.iso");
        assert_eq!(list[MIRROR_LIMIT], edition().url);
        assert!(
            list[MIRROR_LIMIT + 1].starts_with("https://archives.fedoraproject.org/pub/archive/")
        );

        let duplicate = candidates(&edition(), vec![edition().url]);
        assert_eq!(duplicate.len(), 2);
    }

    #[test]
    fn verification_checks_size_and_checksum() {
        let dir = std::env::temp_dir().join(format!("vort2x-usb-verify-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("a.iso");
        let quiet = crate::report::Plain::new();
        std::fs::write(&path, "abc").unwrap();
        assert!(verify(&path, &edition(), &quiet).unwrap());
        std::fs::write(&path, "abd").unwrap();
        assert!(!verify(&path, &edition(), &quiet).unwrap());
        std::fs::write(&path, "abcd").unwrap();
        assert!(!verify(&path, &edition(), &quiet).unwrap());
        assert!(!verify(&dir.join("missing"), &edition(), &quiet).unwrap());
        std::fs::remove_dir_all(dir).unwrap();
    }
}
