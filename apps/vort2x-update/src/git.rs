// SPDX-License-Identifier: GPL-3.0-or-later
//! Read only access to the system VorT2x checkout.

use std::{
    path::{Path, PathBuf},
    process::Command,
};

/// Lists the Fedora releases a VorT2x release supports, read from each tag.
pub const SUPPORTED_FILE: &str = "scripts/fedora/supported-releases";
/// The only source updates come from. The origin remote of the checkout is
/// ignored, so a changed remote cannot redirect an update.
pub const REPOSITORY_URL: &str = "https://github.com/vorT2x-linux/vort2x.git";

/// One commit between the installed state and GitHub.
pub struct Commit {
    pub short: String,
    pub date: String,
    pub subject: String,
}

pub struct Repository {
    path: PathBuf,
}

impl Repository {
    /// The checkout vort2x-install keeps, VORT2X_REPOSITORY overrides it.
    pub fn system() -> Self {
        let path = std::env::var_os("VORT2X_REPOSITORY")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("/usr/local/src/VorT2x-Fedora"));
        Self { path }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    fn git(&self, args: &[&str]) -> Result<String, String> {
        let safe = format!("safe.directory={}", self.path.display());
        let output = Command::new("git")
            .arg("-c")
            .arg(&safe)
            .arg("-C")
            .arg(&self.path)
            .args(args)
            .output()
            .map_err(|error| format!("Could not run git: {error}"))?;
        if output.status.success() {
            return Ok(String::from_utf8_lossy(&output.stdout).into_owned());
        }
        let message = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        Err(if message.is_empty() { format!("git {} failed", args.join(" ")) } else { message })
    }

    /// Fetches the release tags from GitHub as the user, who owns the
    /// checkout, no matter where the origin remote points.
    pub fn fetch(&self) -> Result<(), String> {
        self.git(&["fetch", "--quiet", "--force", REPOSITORY_URL, "+refs/tags/v*:refs/tags/v*"])
            .map(|_| ())
    }

    /// The newest release tag vX.Y.Z that supports Fedora `fedora`. The
    /// update helper and vort2x-install use the same rule.
    pub fn release_for(&self, fedora: &str) -> Option<String> {
        self.git(&["tag", "--list", "v*", "--sort=-version:refname"])
            .ok()?
            .lines()
            .filter(|tag| is_release_tag(tag))
            .find(|tag| {
                self.show(&format!("refs/tags/{tag}"), SUPPORTED_FILE)
                    .is_some_and(|text| supports(&text, fedora))
            })
            .map(str::to_owned)
    }

    /// The release tag HEAD sits on, if any.
    pub fn tag_at(&self, rev: &str) -> Option<String> {
        self.git(&["describe", "--tags", "--exact-match", "--match", "v*", rev])
            .ok()
            .map(|tag| tag.trim().to_owned())
            .filter(|tag| is_release_tag(tag))
    }

    pub fn resolve(&self, rev: &str) -> Result<String, String> {
        let spec = format!("{rev}^{{commit}}");
        Ok(self.git(&["rev-parse", "--verify", "--quiet", &spec])?.trim().to_owned())
    }

    pub fn has_commit(&self, rev: &str) -> bool {
        self.resolve(rev).is_ok()
    }

    pub fn commit_date(&self, rev: &str) -> String {
        self.git(&["show", "-s", "--format=%cs", rev])
            .map(|date| date.trim().to_owned())
            .unwrap_or_default()
    }

    pub fn changed_files(&self, base: &str, target: &str) -> Result<Vec<String>, String> {
        Ok(self
            .git(&["diff", "--name-only", "--no-renames", base, target])?
            .lines()
            .map(str::to_owned)
            .collect())
    }

    pub fn commits(&self, base: &str, target: &str) -> Result<Vec<Commit>, String> {
        let range = format!("{base}..{target}");
        Ok(self
            .git(&["log", "--format=%h%x09%cs%x09%s", &range])?
            .lines()
            .filter_map(|line| {
                let mut fields = line.splitn(3, '\t');
                Some(Commit {
                    short: fields.next()?.to_owned(),
                    date: fields.next()?.to_owned(),
                    subject: fields.next()?.to_owned(),
                })
            })
            .collect())
    }

    /// A file as it is in a commit, None when it does not exist there.
    pub fn show(&self, rev: &str, path: &str) -> Option<String> {
        self.git(&["show", &format!("{rev}:{path}")]).ok()
    }
}

/// vX.Y.Z with numbers only, so test or pre-release tags are never picked.
pub fn is_release_tag(tag: &str) -> bool {
    let Some(version) = tag.strip_prefix('v') else { return false };
    let parts: Vec<&str> = version.split('.').collect();
    parts.len() == 3 && parts.iter().all(|part| !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit()))
}

/// Whether a supported-releases file lists `fedora`; `#` starts a comment.
pub fn supports(text: &str, fedora: &str) -> bool {
    text.lines()
        .filter_map(|line| line.split('#').next())
        .any(|line| line.split_whitespace().next() == Some(fedora))
}

/// VERSION_ID from /etc/os-release.
pub fn fedora_release() -> Option<String> {
    let text = std::fs::read_to_string("/etc/os-release").ok()?;
    text.lines()
        .find_map(|line| line.strip_prefix("VERSION_ID="))
        .map(|value| value.trim().trim_matches('"').to_owned())
        .filter(|value| !value.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn release_tags() {
        assert!(is_release_tag("v0.2.0"));
        assert!(is_release_tag("v10.0.12"));
        assert!(!is_release_tag("v0.2"));
        assert!(!is_release_tag("v0.2.0-rc1"));
        assert!(!is_release_tag("0.2.0"));
        assert!(!is_release_tag("v0..1"));
    }

    #[test]
    fn supported_releases() {
        let text = "# comment\n45\n 46  # later\n";
        assert!(supports(text, "45"));
        assert!(supports(text, "46"));
        assert!(!supports(text, "44"));
        assert!(!supports(text, "4"));
    }
}
