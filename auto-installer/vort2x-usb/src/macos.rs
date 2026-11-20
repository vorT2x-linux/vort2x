// SPDX-License-Identifier: GPL-3.0-or-later

//! Checks that only make sense when vort2x-usb runs in macOS on the T2 Mac
//! that will boot the stick. On any other computer they are skipped.

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Check {
    Good(String),
    Warning(String),
}

/// Secure Boot must be off, or the Mac won't start the stick.
pub fn secure_boot(nvram_output: Option<&str>) -> Check {
    let disabled = nvram_output
        .and_then(|out| out.split_whitespace().last())
        .is_some_and(|value| value == "%00");
    if disabled {
        Check::Good("Secure Boot is disabled.".into())
    } else {
        Check::Warning(
            "Secure Boot is not disabled, or could not be checked. Before booting the stick: \
             restart holding Command-R, open Utilities > Startup Security Utility, set Secure Boot \
             to \"No Security\" and allow booting from external media."
                .into(),
        )
    }
}

/// Touch ID on Linux uses the fingers enrolled in macOS.
pub fn touch_id(bioutil_output: Option<&str>) -> Check {
    let Some(output) = bioutil_output else {
        return Check::Warning(
            "Could not ask macOS about Touch ID; fingerprint login on Linux works if a finger is enrolled."
                .into(),
        );
    };
    match enrolled_fingers(output) {
        Some(0) => Check::Warning(
            "No Touch ID finger is enrolled for this macOS user. Enroll one in System Settings > \
             Touch ID first if you want fingerprint login on Linux."
                .into(),
        ),
        Some(count) => Check::Good(format!(
            "Touch ID: {count} finger(s) enrolled, usable for Linux login after a one-time fprintd-enroll."
        )),
        None => Check::Warning("Could not read the Touch ID enrolment count.".into()),
    }
}

/// The number before "template" or "biometric template" in `bioutil -c`.
fn enrolled_fingers(output: &str) -> Option<u32> {
    let words: Vec<&str> = output.split_whitespace().collect();
    words.iter().enumerate().find_map(|(index, word)| {
        let next = words.get(index + 1)?;
        let follows = next.starts_with("template")
            || (*next == "biometric"
                && words
                    .get(index + 2)
                    .is_some_and(|w| w.starts_with("template")));
        if follows { word.parse().ok() } else { None }
    })
}

#[cfg(target_os = "macos")]
pub mod probe {
    //! Asking macOS. Each returns `None` when the tool fails.

    use std::process::Command;

    use crate::system::{CallingUser, is_root};

    fn run(command: &mut Command) -> Option<String> {
        let out = command.output().ok()?;
        out.status
            .success()
            .then(|| String::from_utf8_lossy(&out.stdout).into_owned())
    }

    /// True on a Mac with the Apple T2 chip.
    pub fn is_t2_mac() -> bool {
        run(Command::new("/usr/sbin/system_profiler").arg("SPiBridgeDataType"))
            .is_some_and(|report| report.contains("Apple T2"))
    }

    pub fn secure_boot_policy() -> Option<String> {
        run(Command::new("/usr/sbin/nvram")
            .arg("94b73556-2197-4702-82a8-3e1337dafbfb:AppleSecureBootPolicy"))
    }

    /// Run as the calling user: Touch ID enrolment is per user.
    pub fn bioutil(user: &CallingUser) -> Option<String> {
        if is_root() {
            run(Command::new("sudo").args(["-u", &user.name, "bioutil", "-c"]))
        } else {
            run(Command::new("bioutil").arg("-c"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secure_boot_needs_the_off_policy() {
        let key = "94b73556-2197-4702-82a8-3e1337dafbfb:AppleSecureBootPolicy";
        assert!(matches!(
            secure_boot(Some(&format!("{key}\t%00\n"))),
            Check::Good(_)
        ));
        assert!(matches!(
            secure_boot(Some(&format!("{key}\t%02\n"))),
            Check::Warning(_)
        ));
        assert!(matches!(secure_boot(None), Check::Warning(_)));
    }

    #[test]
    fn counts_touch_id_fingers() {
        assert_eq!(enrolled_fingers("User 501: 2 biometric templates"), Some(2));
        assert_eq!(
            enrolled_fingers("Operation performed successfully.\n3 templates"),
            Some(3)
        );
        assert_eq!(enrolled_fingers("0 biometric template(s)"), Some(0));
        assert_eq!(enrolled_fingers("nothing here"), None);
        assert!(matches!(touch_id(Some("0 templates")), Check::Warning(_)));
        assert!(matches!(touch_id(Some("1 template")), Check::Good(_)));
    }
}
