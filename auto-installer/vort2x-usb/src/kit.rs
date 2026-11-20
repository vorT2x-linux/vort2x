// SPDX-License-Identifier: GPL-3.0-or-later

//! The installer kit: which Fedora release, which editions with their
//! checksums, the GRUB template and the input-driver initramfs. The release
//! build (`scripts/fedora/build-installer.sh`) puts these in the kit zip.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail, ensure};

const RELEASES_URL: &str = "https://download.fedoraproject.org/pub/fedora/linux/releases/";

#[derive(Debug)]
pub struct Target {
    pub id: String,
    pub fedora_release: String,
    pub default_edition: String,
}

#[derive(Clone, Debug)]
pub struct Edition {
    pub id: String,
    pub name: String,
    pub file_name: String,
    pub url: String,
    pub size: u64,
    pub sha256: String,
}

impl Edition {
    /// Path below Fedora's `releases/`, e.g. `45/Spins/x86_64/iso/….iso`.
    pub fn release_path(&self) -> &str {
        // Checked when the catalogue was loaded.
        &self.url[RELEASES_URL.len()..]
    }
}

#[derive(Debug)]
pub struct Kit {
    pub target: Target,
    pub editions: Vec<Edition>,
    pub grub_template: String,
    pub input_initramfs: PathBuf,
}

impl Kit {
    pub fn load(dir: &Path) -> Result<Self> {
        let read = |name: &str| -> Result<String> {
            let path = dir.join(name);
            let text = std::fs::read_to_string(&path)
                .with_context(|| format!("installer kit file is missing: {}", path.display()))?;
            ensure!(
                !text.is_empty(),
                "installer kit file is empty: {}",
                path.display()
            );
            Ok(text)
        };
        let target = parse_target(&read("installer-target.conf")?)?;
        let editions = parse_catalog(&read("installer-editions.tsv")?, &target.fedora_release)?;
        ensure!(
            editions.iter().any(|e| e.id == target.default_edition),
            "default edition is not in the catalogue: {}",
            target.default_edition
        );
        let grub_template = read("grub.cfg.in")?;
        let input_initramfs = dir.join("vort2x-input-initramfs.img");
        let size = std::fs::metadata(&input_initramfs)
            .map(|m| m.len())
            .with_context(|| {
                format!(
                    "installer kit file is missing: {}",
                    input_initramfs.display()
                )
            })?;
        ensure!(
            size > 0,
            "installer kit file is empty: {}",
            input_initramfs.display()
        );
        Ok(Self {
            target,
            editions,
            grub_template,
            input_initramfs,
        })
    }

    pub fn edition(&self, id: &str) -> Option<&Edition> {
        self.editions.iter().find(|e| e.id == id)
    }

    pub fn default_index(&self) -> usize {
        self.editions
            .iter()
            .position(|e| e.id == self.target.default_edition)
            .unwrap_or(0)
    }
}

/// `installer-target.conf` is a shell file of `KEY="value"` lines.
pub fn parse_target(text: &str) -> Result<Target> {
    let value = |key: &str| -> Option<String> {
        text.lines().find_map(|line| {
            let rest = line.trim().strip_prefix(key)?.strip_prefix('=')?;
            Some(rest.trim_matches('"').to_owned())
        })
    };
    match (
        value("TARGET_ID"),
        value("FEDORA_RELEASE"),
        value("DEFAULT_EDITION"),
    ) {
        (Some(id), Some(fedora_release), Some(default_edition))
            if !id.is_empty() && !fedora_release.is_empty() && !default_edition.is_empty() =>
        {
            Ok(Target {
                id,
                fedora_release,
                default_edition,
            })
        }
        _ => bail!("installer-target.conf is incomplete"),
    }
}

/// One edition per line: id, name, variant, subvariant, ISO file name, URL,
/// size and SHA-256, separated by tabs.
pub fn parse_catalog(text: &str, fedora_release: &str) -> Result<Vec<Edition>> {
    let mut editions = Vec::new();
    let mut seen = HashSet::new();
    for line in text.lines().filter(|line| !line.trim().is_empty()) {
        let fields: Vec<&str> = line.split('\t').collect();
        ensure!(
            fields.len() == 8,
            "edition entry needs 8 tab-separated fields: {line}"
        );
        let [id, name, variant, subvariant, file_name, url, size, sha256] = fields[..] else {
            unreachable!()
        };
        ensure!(valid_id(id), "invalid edition ID: {id}");
        ensure!(seen.insert(id), "duplicate edition ID: {id}");
        ensure!(
            !name.is_empty() && !variant.is_empty() && !subvariant.is_empty(),
            "incomplete edition entry: {id}"
        );
        let path = fedora_release_path(url)
            .with_context(|| format!("invalid Fedora download for edition: {id}"))?;
        ensure!(
            file_name.ends_with(".iso")
                && path.rsplit('/').next() == Some(file_name)
                && path.starts_with(&format!("{fedora_release}/")),
            "invalid Fedora download for edition: {id}"
        );
        let size: u64 = size
            .parse()
            .ok()
            .filter(|size| *size > 0)
            .with_context(|| format!("invalid size for edition: {id}"))?;
        ensure!(
            sha256.len() == 64
                && sha256
                    .bytes()
                    .all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')),
            "invalid checksum for edition: {id}"
        );
        editions.push(Edition {
            id: id.to_owned(),
            name: name.to_owned(),
            file_name: file_name.to_owned(),
            url: url.to_owned(),
            size,
            sha256: sha256.to_owned(),
        });
    }
    ensure!(
        !editions.is_empty(),
        "the installer edition catalogue is empty"
    );
    Ok(editions)
}

fn valid_id(id: &str) -> bool {
    let mut bytes = id.bytes();
    matches!(bytes.next(), Some(b'a'..=b'z' | b'0'..=b'9'))
        && bytes.all(|b| matches!(b, b'a'..=b'z' | b'0'..=b'9' | b'-'))
}

/// The part of a Fedora release URL below `releases/`, only if it is a plain
/// path: Fedora's mirror list takes it as a parameter, so no query strings,
/// escapes or `.`/`..` components.
pub fn fedora_release_path(url: &str) -> Option<&str> {
    let path = url.strip_prefix(RELEASES_URL)?;
    let mut parts = path.split('/');
    let release = parts.next()?;
    if release.is_empty() || !release.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let mut rest = 0;
    for part in parts {
        let plain = !part.is_empty()
            && part != "."
            && part != ".."
            && part
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'+' | b'-'));
        if !plain {
            return None;
        }
        rest += 1;
    }
    (rest > 0).then_some(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    const GOOD: &str = "cosmic\tFedora COSMIC Spin\tSpins\tCOSMIC\tFedora-COSMIC-Live-45-1.4.x86_64.iso\thttps://download.fedoraproject.org/pub/fedora/linux/releases/45/Spins/x86_64/iso/Fedora-COSMIC-Live-45-1.4.x86_64.iso\t3256915968\t3b3a9d4537b089b9c5f56e9691b65cf1a32398d38366e58499e45d1f50719f84";

    #[test]
    fn reads_the_real_catalogue() {
        let text = include_str!("../../targets/fedora-45-editions.tsv");
        let editions = parse_catalog(text, "45").unwrap();
        assert_eq!(editions.len(), 3);
        let cosmic = editions.iter().find(|e| e.id == "cosmic").unwrap();
        assert_eq!(cosmic.size, 3_256_915_968);
        assert!(cosmic.release_path().starts_with("45/Spins/"));
    }

    #[test]
    fn reads_the_real_target() {
        let target = parse_target(include_str!("../../targets/fedora-45.conf")).unwrap();
        assert_eq!(target.id, "fedora-45");
        assert_eq!(target.fedora_release, "45");
        assert!(!target.default_edition.is_empty());
    }

    #[test]
    fn rejects_bad_catalogue_lines() {
        let swap = |from: &str, to: &str| GOOD.replacen(from, to, 1);
        let bad = [
            swap("cosmic\t", "Cosmic\t"),
            swap("3256915968", "big"),
            swap("3b3a9d45", "3B3A9D45"),
            swap("\tSpins\t", "\t\t"),
            swap("releases/45/", "releases/44/"),
            swap("x86_64/iso/", "x86_64/../iso/"),
            swap("https://download", "http://download"),
            GOOD.replace(".iso\thttps", "\thttps"),
            format!("{GOOD}\textra"),
            format!("{GOOD}\n{GOOD}"),
            String::new(),
        ];
        for text in bad {
            assert!(parse_catalog(&text, "45").is_err(), "accepted: {text}");
        }
        assert!(parse_catalog(GOOD, "45").is_ok());
    }

    #[test]
    fn release_paths_must_be_plain() {
        let ok = format!("{RELEASES_URL}45/Spins/x86_64/iso/a.iso");
        assert_eq!(fedora_release_path(&ok), Some("45/Spins/x86_64/iso/a.iso"));
        for path in [
            "45",
            "x/a.iso",
            "45/a.iso?x=1",
            "45//a.iso",
            "45/%2e/a.iso",
            "45/./a.iso",
        ] {
            assert_eq!(
                fedora_release_path(&format!("{RELEASES_URL}{path}")),
                None,
                "{path}"
            );
        }
    }

    #[test]
    fn incomplete_target_is_refused() {
        assert!(parse_target("TARGET_ID=\"fedora-45\"\nFEDORA_RELEASE=\"45\"\n").is_err());
        let target =
            parse_target("TARGET_ID=a\nFEDORA_RELEASE=\"45\"\nDEFAULT_EDITION=\"kde\"").unwrap();
        assert_eq!(target.default_edition, "kde");
    }
}
