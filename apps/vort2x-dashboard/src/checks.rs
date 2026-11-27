// SPDX-License-Identifier: GPL-3.0-or-later
//! The "Check my Mac" self-test: which T2 parts work on this Mac.
//!
//! Everything is read without root from `/sys`, `/proc`, systemd and the
//! kernel log of this boot. Gathering (`Readings::read`) is kept apart from
//! judging (`evaluate`), so the rules are tested on sample readings.

use std::{fs, path::Path, process::Command};

use crate::machine;

/// The checkout vort2x-install keeps; its tag is the installed release.
const CHECKOUT: &str = "/usr/local/src/VorT2x-Fedora";
const ISSUE_FORM: &str = "https://github.com/vorT2x-linux/vort2x/issues/new?template=mac-report.yml";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Works,
    Problem,
    /// This model doesn't have the part.
    Absent,
    /// Nothing to judge yet, e.g. the Mac hasn't slept this boot.
    Untried,
}

impl Status {
    pub fn word(self) -> &'static str {
        match self {
            Self::Works => "works",
            Self::Problem => "problem",
            Self::Absent => "not on this Mac",
            Self::Untried => "not tried yet",
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct Check {
    pub name: &'static str,
    pub status: Status,
    pub detail: String,
}

/// What a model has, from its firmware model ID.
struct Parts {
    laptop: bool,
    touch_bar: bool,
    camera: bool,
    five_k: bool,
}

impl Parts {
    fn of(model_id: &str) -> Self {
        let laptop = model_id.starts_with("MacBook");
        Self {
            laptop,
            touch_bar: model_id.starts_with("MacBookPro"),
            // MacBooks and iMacs have a camera and light sensor; the Mac
            // mini and Mac Pro have no screen.
            camera: laptop || model_id.starts_with("iMac"),
            five_k: matches!(model_id, "iMacPro1,1" | "iMac20,1" | "iMac20,2"),
        }
    }
}

/// The raw facts the checks judge.
#[derive(Default)]
pub struct Readings {
    pub model_id: String,
    /// First column of `/proc/modules`.
    pub modules: Vec<String>,
    /// `/proc/bus/input/devices`.
    pub input_devices: String,
    /// `/proc/asound/cards`.
    pub sound_cards: String,
    pub camera_names: Vec<String>,
    pub iio_names: Vec<String>,
    pub wireless: bool,
    pub bluetooth: bool,
    /// Drivers bound to the DRM cards, e.g. "amdgpu".
    pub gpu_drivers: Vec<String>,
    /// Modes of connected built-in (eDP) displays, e.g. "5120x2880".
    pub panel_modes: Vec<String>,
    pub touch_id_active: bool,
    /// `PM:` lines of this boot's kernel log, `None` if it can't be read.
    pub sleep_log: Option<String>,
}

impl Readings {
    pub fn read() -> Self {
        Self {
            model_id: machine::model_id(),
            modules: read("/proc/modules")
                .lines()
                .filter_map(|line| line.split_whitespace().next())
                .map(str::to_owned)
                .collect(),
            input_devices: read("/proc/bus/input/devices"),
            sound_cards: read("/proc/asound/cards"),
            camera_names: entries("/sys/class/video4linux")
                .map(|dir| read(dir.join("name")))
                .collect(),
            iio_names: entries("/sys/bus/iio/devices")
                .map(|dir| read(dir.join("name")))
                .collect(),
            wireless: entries("/sys/class/net").any(|dir| dir.join("wireless").exists()),
            bluetooth: entries("/sys/class/bluetooth").next().is_some(),
            gpu_drivers: drm_cards()
                .filter_map(|dir| fs::read_link(dir.join("device/driver")).ok())
                .filter_map(|link| Some(link.file_name()?.to_string_lossy().into_owned()))
                .collect(),
            panel_modes: panel_modes(),
            touch_id_active: service_active("vort2x-t2-touchid.service"),
            sleep_log: sleep_log(),
        }
    }
}

fn read(path: impl AsRef<Path>) -> String {
    fs::read_to_string(path).map(|text| text.trim().to_owned()).unwrap_or_default()
}

/// The entries of a directory, none if it is missing.
fn entries(dir: &str) -> impl Iterator<Item = std::path::PathBuf> {
    fs::read_dir(dir).into_iter().flatten().flatten().map(|entry| entry.path())
}

/// `card0`, `card1`...; not the connectors such as `card1-DP-1`.
fn drm_cards() -> impl Iterator<Item = std::path::PathBuf> {
    entries("/sys/class/drm").filter(|dir| {
        dir.file_name()
            .and_then(|name| name.to_str()?.strip_prefix("card"))
            .is_some_and(|rest| !rest.is_empty() && rest.bytes().all(|b| b.is_ascii_digit()))
    })
}

fn panel_modes() -> Vec<String> {
    entries("/sys/class/drm")
        .filter(|dir| dir.to_string_lossy().contains("-eDP-"))
        .filter(|dir| read(dir.join("status")) == "connected")
        .flat_map(|dir| read(dir.join("modes")).lines().map(str::to_owned).collect::<Vec<_>>())
        .collect()
}

fn service_active(unit: &str) -> bool {
    Command::new("systemctl")
        .args(["is-active", "--quiet", unit])
        .status()
        .is_ok_and(|status| status.success())
}

fn sleep_log() -> Option<String> {
    let output = Command::new("journalctl")
        .args(["--dmesg", "--boot=0", "--output=cat", "--no-pager", "--quiet", "--grep=^PM: "])
        .output()
        .ok()?;
    // journalctl exits 1 when --grep matches nothing; that just means no
    // sleep yet. Anything on stderr means the log couldn't be read.
    if !output.stderr.is_empty() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Judges the readings, in the order the page shows them.
pub fn evaluate(readings: &Readings) -> Vec<Check> {
    let parts = Parts::of(&readings.model_id);
    let loaded = |name: &str| readings.modules.iter().any(|module| module == name);
    let input = |name: &str| {
        readings
            .input_devices
            .lines()
            .filter_map(|line| line.strip_prefix("N: Name="))
            .any(|value| value.contains(name))
    };

    let mut checks = vec![found(
        "T2 bridge",
        loaded("apple_t2_bridge") && loaded("apple_t2_vhci_hcd"),
        "driver loaded",
        "apple_t2_bridge or apple_t2_vhci_hcd is not loaded",
    )];
    checks.push(only_if(parts.laptop, "Keyboard and trackpad", || {
        found(
            "Keyboard and trackpad",
            input("Apple Internal Keyboard / Trackpad"),
            "found",
            "the internal keyboard is not an input device",
        )
    }));
    checks.push(only_if(parts.touch_bar, "Touch Bar", || {
        found("Touch Bar", input("Touch Bar Display"), "found", "no Touch Bar input device")
    }));
    checks.push(only_if(parts.camera, "Camera", || {
        found(
            "Camera",
            readings.camera_names.iter().any(|name| name.contains("FaceTime")),
            "FaceTime camera found",
            "no FaceTime camera",
        )
    }));
    checks.push(only_if(parts.camera, "Light sensor", || {
        found(
            "Light sensor",
            readings.iio_names.iter().any(|name| name == "als"),
            "found",
            "no ambient light sensor",
        )
    }));
    checks.push(found(
        "Speakers and microphone",
        readings.sound_cards.contains("Apple T2 Audio"),
        "T2 audio card found",
        "no T2 audio card",
    ));
    checks.push(found("Wi-Fi", readings.wireless, "found", "no wireless network device"));
    checks.push(found("Bluetooth", readings.bluetooth, "found", "no Bluetooth adapter"));
    checks.push(found("SMC (fans and sensors)", loaded("t2smc"), "driver loaded", "t2smc is not loaded"));
    checks.push(only_if(parts.laptop, "Touch ID", || {
        found("Touch ID", readings.touch_id_active, "service running", "vort2x-t2-touchid is not running")
    }));
    checks.push(graphics(&readings.gpu_drivers));
    checks.push(only_if(parts.five_k, "5K display", || {
        found(
            "5K display",
            readings.panel_modes.iter().any(|mode| mode == "5120x2880"),
            "one 5120x2880 screen",
            "the built-in panel doesn't offer 5120x2880",
        )
    }));
    checks.push(sleep(readings.sleep_log.as_deref()));
    checks
}

fn found(name: &'static str, ok: bool, works: &str, problem: &str) -> Check {
    let (status, detail) = if ok { (Status::Works, works) } else { (Status::Problem, problem) };
    Check { name, status, detail: detail.to_owned() }
}

fn only_if(has: bool, name: &'static str, check: impl FnOnce() -> Check) -> Check {
    if has {
        check()
    } else {
        Check { name, status: Status::Absent, detail: String::new() }
    }
}

fn graphics(drivers: &[String]) -> Check {
    let mut known: Vec<&str> = drivers
        .iter()
        .map(String::as_str)
        .filter(|driver| matches!(*driver, "amdgpu" | "i915"))
        .collect();
    known.sort_unstable();
    known.dedup();
    if known.is_empty() {
        return found("Graphics", false, "", "no amdgpu or i915 graphics driver");
    }
    found("Graphics", true, &known.join(" and "), "")
}

fn sleep(log: Option<&str>) -> Check {
    let Some(log) = log else {
        return Check {
            name: "Sleep",
            status: Status::Untried,
            detail: "couldn't read this boot's kernel log".to_owned(),
        };
    };
    let wakes = log.lines().filter(|line| line.starts_with("PM: suspend exit")).count();
    let failures = log
        .lines()
        .filter(|line| {
            let line = line.to_ascii_lowercase();
            line.contains("failed") || line.contains("abort")
        })
        .count();
    let times = |n: usize| if n == 1 { "once".to_owned() } else { format!("{n} times") };
    match (wakes, failures) {
        (0, 0) => Check { name: "Sleep", status: Status::Untried, detail: "hasn't slept since boot".to_owned() },
        (_, 0) => found("Sleep", true, &format!("slept and woke {} this boot", times(wakes)), ""),
        _ => found(
            "Sleep",
            false,
            "",
            &format!("woke {} this boot, {failures} errors in the kernel log", times(wakes)),
        ),
    }
}

/// The system lines at the top of a report.
pub struct About {
    pub model_id: String,
    pub system: String,
    pub kernel: String,
    pub release: String,
    pub desktop: String,
}

impl About {
    pub fn read() -> Self {
        Self {
            model_id: machine::model_id(),
            system: machine::system(),
            kernel: read("/proc/sys/kernel/osrelease"),
            release: installed_release(),
            desktop: std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default(),
        }
    }
}

/// "v0.3.1", or e.g. "v0.3.1-4-gabc1234" for a branch after it.
fn installed_release() -> String {
    let safe = format!("safe.directory={CHECKOUT}");
    Command::new("git")
        .args(["-c", &safe, "-C", CHECKOUT, "describe", "--tags", "--match", "v*"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned())
        .unwrap_or_else(|| "unknown".to_owned())
}

/// The report as Markdown, for the issue form and the clipboard. Holds no
/// serial number, host or user name.
pub fn report(about: &About, checks: &[Check]) -> String {
    let mut text = format!(
        "Model: {} ({})\nSystem: {}\nKernel: {}\nVorT2x: {}\nDesktop: {}\n\n",
        about.model_id,
        machine::model_name(&about.model_id),
        about.system,
        about.kernel,
        about.release,
        if about.desktop.is_empty() { "unknown" } else { &about.desktop },
    );
    for check in checks {
        text.push_str(&format!("- {}: {}", check.name, check.status.word()));
        if !check.detail.is_empty() {
            text.push_str(&format!(" ({})", check.detail));
        }
        text.push('\n');
    }
    text
}

/// The new-issue link with the model and the report filled in.
pub fn issue_url(about: &About, report: &str) -> String {
    let title = format!("Mac report: {}, {}", about.model_id, about.release);
    let escape = |text: &str| gtk4::glib::Uri::escape_string(text, None, false);
    format!(
        "{ISSUE_FORM}&title={}&model={}&checks={}",
        escape(&title),
        escape(&about.model_id),
        escape(report)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// GitHub refuses longer new-issue links; ours stay far below.
    const MAX_URL_LEN: usize = 8000;

    /// What Kirk's iMac Pro reads (2026-11-27, Fedora 45).
    fn imac_pro() -> Readings {
        Readings {
            model_id: "iMacPro1,1".to_owned(),
            modules: ["apple_t2_bridge", "apple_t2_vhci_hcd", "t2smc", "amdgpu"].map(String::from).to_vec(),
            input_devices: "I: Bus=0003\nN: Name=\"Apple T2 Audio Codec Output\"\nN: Name=\"Logitech MX Keys\"\n".to_owned(),
            sound_cards: " 0 [t2-imacpro1_1  ]: AppleT2x4 - Apple T2 Audio\n                      Apple T2 Audio".to_owned(),
            camera_names: vec!["apple-ave-enc".to_owned(), "FaceTime HD Camera (Built-in):".to_owned()],
            iio_names: vec!["als".to_owned(), "als-dev0".to_owned()],
            wireless: true,
            bluetooth: true,
            gpu_drivers: vec!["amdgpu".to_owned()],
            panel_modes: vec!["5120x2880".to_owned(), "3840x2160".to_owned()],
            touch_id_active: false,
            sleep_log: Some(String::new()),
        }
    }

    fn status(checks: &[Check], name: &str) -> Status {
        checks.iter().find(|check| check.name == name).unwrap().status
    }

    #[test]
    fn imac_pro_works_and_skips_laptop_parts() {
        let checks = evaluate(&imac_pro());
        for name in ["T2 bridge", "Camera", "Light sensor", "Speakers and microphone", "Wi-Fi",
            "Bluetooth", "SMC (fans and sensors)", "Graphics", "5K display"] {
            assert_eq!(status(&checks, name), Status::Works, "{name}");
        }
        for name in ["Keyboard and trackpad", "Touch Bar", "Touch ID"] {
            assert_eq!(status(&checks, name), Status::Absent, "{name}");
        }
        assert_eq!(status(&checks, "Sleep"), Status::Untried);
    }

    #[test]
    fn missing_parts_on_a_macbook_pro_are_problems() {
        let readings = Readings { model_id: "MacBookPro16,2".to_owned(), ..Readings::default() };
        let checks = evaluate(&readings);
        for name in ["T2 bridge", "Keyboard and trackpad", "Touch Bar", "Camera", "Touch ID", "Graphics"] {
            assert_eq!(status(&checks, name), Status::Problem, "{name}");
        }
        assert_eq!(status(&checks, "5K display"), Status::Absent);
        assert_eq!(status(&checks, "Sleep"), Status::Untried);
    }

    #[test]
    fn finds_the_internal_keyboard_and_touch_bar() {
        let readings = Readings {
            model_id: "MacBookPro16,2".to_owned(),
            input_devices: "N: Name=\"Apple Inc. Apple Internal Keyboard / Trackpad\"\n\
                N: Name=\"Apple Inc. Touch Bar Display\"\n"
                .to_owned(),
            ..Readings::default()
        };
        let checks = evaluate(&readings);
        assert_eq!(status(&checks, "Keyboard and trackpad"), Status::Works);
        assert_eq!(status(&checks, "Touch Bar"), Status::Works);
    }

    #[test]
    fn mac_mini_has_no_camera_or_keyboard() {
        let checks = evaluate(&Readings { model_id: "Macmini8,1".to_owned(), ..Readings::default() });
        for name in ["Keyboard and trackpad", "Touch Bar", "Camera", "Light sensor", "Touch ID", "5K display"] {
            assert_eq!(status(&checks, name), Status::Absent, "{name}");
        }
    }

    #[test]
    fn macbook_air_has_touch_id_but_no_touch_bar() {
        let parts = Parts::of("MacBookAir9,1");
        assert!(parts.laptop && parts.camera && !parts.touch_bar && !parts.five_k);
    }

    #[test]
    fn sleep_counts_wakes_and_errors() {
        let ok = "PM: suspend entry (deep)\nPM: suspend exit\nPM: suspend entry (deep)\nPM: suspend exit\n";
        assert_eq!(sleep(Some(ok)), found("Sleep", true, "slept and woke 2 times this boot", ""));
        let one = "PM: suspend entry (s2idle)\nPM: suspend exit\n";
        assert_eq!(sleep(Some(one)).detail, "slept and woke once this boot");
        let bad = "PM: suspend entry (deep)\nPM: Some devices failed to suspend, or early wake event detected\nPM: suspend exit\n";
        assert_eq!(sleep(Some(bad)).status, Status::Problem);
        assert_eq!(sleep(None).status, Status::Untried);
    }

    #[test]
    fn graphics_names_both_gpus_once() {
        let drivers = ["i915", "amdgpu", "amdgpu", "simpledrm"].map(String::from);
        assert_eq!(graphics(&drivers).detail, "amdgpu and i915");
        assert_eq!(graphics(&["simpledrm".to_owned()]).status, Status::Problem);
    }

    fn about() -> About {
        About {
            model_id: "iMacPro1,1".to_owned(),
            system: "Fedora 45 · Workstation".to_owned(),
            kernel: "7.3.4-300.fc45.x86_64".to_owned(),
            release: "v0.3.1".to_owned(),
            desktop: "GNOME".to_owned(),
        }
    }

    #[test]
    fn report_lists_the_system_and_every_check() {
        let checks = evaluate(&imac_pro());
        let text = report(&about(), &checks);
        assert!(text.starts_with("Model: iMacPro1,1 (iMac Pro (2017))\nSystem: Fedora 45"));
        assert!(text.contains("- 5K display: works (one 5120x2880 screen)\n"));
        assert!(text.contains("- Touch Bar: not on this Mac\n"));
        assert_eq!(text.lines().filter(|line| line.starts_with("- ")).count(), checks.len());
    }

    #[test]
    fn issue_url_is_escaped_and_short_enough() {
        let about = about();
        let url = issue_url(&about, &report(&about, &evaluate(&imac_pro())));
        assert!(url.starts_with("https://github.com/vorT2x-linux/vort2x/issues/new?template=mac-report.yml&title="));
        assert!(url.contains("&model=iMacPro1%2C1&checks=Model%3A%20"));
        assert!(!url[ISSUE_FORM.len()..].contains(['\n', ' ', '#', '(']));
        assert!(url.len() < MAX_URL_LEN);
    }
}
