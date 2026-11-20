// SPDX-License-Identifier: GPL-3.0-or-later

//! vort2x-usb makes the VorT2x Fedora USB stick for T2 Macs, on macOS and
//! Linux. It is a port of `scripts/macos/prepare-fedora-installer.sh` and
//! `download-fedora-iso.sh` by André Eikmeyer and 4f1sh3r (KAIT2EN).

mod cpio;
mod download;
mod drives;
mod firmware;
mod image;
mod job;
mod kit;
#[cfg(any(target_os = "macos", test))]
mod macos;
mod report;
mod system;
mod ui;
mod write;

use std::io::{BufRead, Write as _};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::AtomicBool;

use anyhow::{Context, Result, bail, ensure};
use clap::Parser;

use job::{FirmwareSource, IsoChoice, Target};
use kit::Kit;
use report::gigabytes;

pub const CREDIT: &str = "Based on the KAIT2EN installer scripts by André Eikmeyer and 4f1sh3r.";

/// Makes the VorT2x Fedora USB stick for T2 Macs.
#[derive(Parser, Debug)]
#[command(version, after_help = CREDIT)]
pub struct Args {
    /// The unpacked VorT2x installer kit (default: the folder this program is in)
    #[arg(long, value_name = "DIR")]
    kit: Option<PathBuf>,
    /// Fedora edition, e.g. cosmic (see --list-editions)
    #[arg(long, value_name = "ID")]
    edition: Option<String>,
    /// Use a Fedora ISO you already downloaded
    #[arg(long, value_name = "FILE")]
    iso: Option<PathBuf>,
    /// External drive to erase, e.g. /dev/disk4 or /dev/sdb (see --list-drives)
    #[arg(long, value_name = "DEVICE", conflicts_with = "target_file")]
    disk: Option<String>,
    /// Folder for the Fedora download (default: your Downloads folder)
    #[arg(long, value_name = "DIR")]
    download_dir: Option<PathBuf>,
    /// Apple Wi-Fi/Bluetooth firmware from this folder instead of this Mac
    #[arg(long, value_name = "DIR")]
    firmware_dir: Option<PathBuf>,
    /// Write the stick image to this file instead of a drive (for checking)
    #[arg(long, value_name = "FILE")]
    target_file: Option<PathBuf>,
    /// Print progress lines instead of the full-screen interface
    #[arg(long)]
    plain: bool,
    /// List the Fedora editions and exit
    #[arg(long)]
    list_editions: bool,
    /// List the external drives that can be used and exit
    #[arg(long)]
    list_drives: bool,
}

fn main() {
    if let Err(error) = run(Args::parse()) {
        eprintln!("Error: {error:#}");
        std::process::exit(1);
    }
}

fn run(args: Args) -> Result<()> {
    if args.list_drives {
        return list_drives();
    }
    let kit_dir = kit_dir(args.kit.as_deref())?;
    let kit = Kit::load(&kit_dir)?;
    if args.list_editions {
        for edition in &kit.editions {
            println!("{:<12} {}", edition.id, edition.name);
        }
        return Ok(());
    }
    // Said before asking for a password that would be of no use.
    let this_mac_is_t2 = this_mac_is_t2();
    let firmware = match &args.firmware_dir {
        Some(dir) => FirmwareSource::Dir(dir.clone()),
        None if this_mac_is_t2 => FirmwareSource::ThisMac,
        None => bail!(
            "vort2x-usb needs the Apple Wi-Fi firmware. Run it in macOS on the T2 Mac that will use \
             the stick, or give a folder with the firmware with --firmware-dir."
        ),
    };
    if args.target_file.is_none() && !system::is_root() {
        become_root();
    }
    let setup = Setup::new(&args, kit, firmware, this_mac_is_t2)?;
    if args.plain {
        run_plain(&args, &setup)
    } else {
        ui::run(&setup, &args)
    }
}

fn list_drives() -> Result<()> {
    let drives = drives::list()?;
    if drives.is_empty() {
        println!("No external drive found. Connect a USB drive and try again.");
    }
    for drive in drives {
        println!(
            "{:<14} {:>9}  {}",
            drive.path,
            gigabytes(drive.size),
            drive.model
        );
    }
    Ok(())
}

fn kit_dir(given: Option<&Path>) -> Result<PathBuf> {
    if let Some(dir) = given {
        return Ok(dir.to_owned());
    }
    let exe = std::env::current_exe().context("could not find this program's folder")?;
    let dir = exe
        .parent()
        .context("could not find this program's folder")?;
    ensure!(
        dir.join("installer-target.conf").is_file(),
        "no installer kit next to this program; give its folder with --kit"
    );
    Ok(dir.to_owned())
}

/// Writing a drive needs administrator rights. Like the bash script, start
/// again through sudo before anything is shown.
fn become_root() {
    println!("Administrator access is required to write the USB drive.");
    let exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("vort2x-usb"));
    let error = Command::new("sudo")
        .arg(exe)
        .args(std::env::args_os().skip(1))
        .exec();
    eprintln!("Error: could not start sudo: {error}");
    std::process::exit(1);
}

/// Everything decided before the first screen.
pub struct Setup {
    pub kit: Kit,
    pub user: system::CallingUser,
    pub download_dir: PathBuf,
    pub firmware: FirmwareSource,
    /// Secure Boot and Touch ID, only on a T2 Mac in macOS.
    pub mac_checks: Vec<(bool, String)>,
    /// Chosen with --iso or --edition, so the menu is skipped.
    pub iso: Option<IsoChoice>,
}

impl Setup {
    fn new(args: &Args, kit: Kit, firmware: FirmwareSource, this_mac_is_t2: bool) -> Result<Self> {
        let user = system::calling_user()?;
        let download_dir = match &args.download_dir {
            Some(dir) => dir.clone(),
            // Not every system has (an English) Downloads folder.
            None => Some(user.home.join("Downloads"))
                .filter(|dir| dir.is_dir())
                .unwrap_or_else(|| user.home.clone()),
        };
        ensure!(
            download_dir.is_dir(),
            "download folder not found: {}",
            download_dir.display()
        );

        let iso = match (&args.iso, &args.edition) {
            (Some(path), _) => Some(IsoChoice::File(path.clone())),
            (None, Some(id)) => {
                let index = kit
                    .editions
                    .iter()
                    .position(|e| &e.id == id)
                    .with_context(|| format!("unsupported Fedora edition: {id}"))?;
                Some(IsoChoice::Edition(index))
            }
            (None, None) => None,
        };

        let mac_checks = if this_mac_is_t2 {
            mac_checks(&user)
        } else {
            Vec::new()
        };
        Ok(Self {
            kit,
            user,
            download_dir,
            firmware,
            mac_checks,
            iso,
        })
    }
}

#[cfg(target_os = "macos")]
fn this_mac_is_t2() -> bool {
    macos::probe::is_t2_mac()
}

#[cfg(not(target_os = "macos"))]
fn this_mac_is_t2() -> bool {
    false
}

#[cfg(target_os = "macos")]
fn mac_checks(user: &system::CallingUser) -> Vec<(bool, String)> {
    use macos::{Check, probe};
    [
        macos::secure_boot(probe::secure_boot_policy().as_deref()),
        macos::touch_id(probe::bioutil(user).as_deref()),
    ]
    .into_iter()
    .map(|check| match check {
        Check::Good(text) => (true, text),
        Check::Warning(text) => (false, text),
    })
    .collect()
}

#[cfg(not(target_os = "macos"))]
fn mac_checks(_user: &system::CallingUser) -> Vec<(bool, String)> {
    Vec::new()
}

/// The same steps with plain output, for scripts and checks.
fn run_plain(args: &Args, setup: &Setup) -> Result<()> {
    let report = report::Plain::new();
    for (good, text) in &setup.mac_checks {
        println!("{}{text}", if *good { "Good: " } else { "Warning: " });
    }
    let Some(iso) = &setup.iso else {
        bail!("--plain needs --edition or --iso");
    };
    let target = match (&args.target_file, &args.disk) {
        (Some(file), _) => Target::File(file.clone()),
        (None, Some(disk)) => Target::Drive(drives::find(disk)?),
        (None, None) => bail!("--plain needs --disk or --target-file"),
    };
    let cancel = AtomicBool::new(false);
    let prepared = job::prepare(
        &setup.kit,
        iso,
        &setup.download_dir,
        &setup.firmware,
        &setup.user,
        &cancel,
        &report,
    )?;
    for warning in &prepared.warnings {
        println!("Warning: {warning}");
    }
    if let Target::Drive(drive) = &target {
        job::fits(drive, prepared.iso_size)?;
        confirm_plain(drive)?;
    }
    job::burn(&prepared, &target, &report)?;
    println!("\nThe VorT2x Fedora USB stick is ready.");
    for line in ui::NEXT_STEPS {
        println!("  - {line}");
    }
    Ok(())
}

fn confirm_plain(drive: &drives::Drive) -> Result<()> {
    let phrase = drive.confirm_phrase();
    let stdin = std::io::stdin();
    loop {
        print!(
            "All data on {} ({}, {}) will be erased. Type '{phrase}' to continue: ",
            drive.path,
            drive.model,
            gigabytes(drive.size)
        );
        std::io::stdout().flush()?;
        let mut line = String::new();
        if stdin.lock().read_line(&mut line)? == 0 {
            bail!("cancelled");
        }
        if line.trim_end_matches(['\r', '\n']) == phrase {
            return Ok(());
        }
        println!("That did not match '{phrase}'; try again (Ctrl-C to cancel).");
    }
}
