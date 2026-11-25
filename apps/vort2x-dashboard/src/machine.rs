// SPDX-License-Identifier: GPL-3.0-or-later
//! The Mac and system the dashboard runs on, for the sidebar footer.

const PRODUCT_NAME: &str = "/sys/devices/virtual/dmi/id/product_name";
const OS_RELEASE: &str = "/etc/os-release";

/// Marketing names of the T2 Macs, by the model ID in the firmware.
const MODELS: &[(&str, &str)] = &[
    ("MacBookPro15,1", "MacBook Pro 15-inch (2018–19)"),
    ("MacBookPro15,2", "MacBook Pro 13-inch (2018–19)"),
    ("MacBookPro15,3", "MacBook Pro 15-inch (2018–19)"),
    ("MacBookPro15,4", "MacBook Pro 13-inch (2019)"),
    ("MacBookPro16,1", "MacBook Pro 16-inch (2019)"),
    ("MacBookPro16,2", "MacBook Pro 13-inch (2020)"),
    ("MacBookPro16,3", "MacBook Pro 13-inch (2020)"),
    ("MacBookPro16,4", "MacBook Pro 16-inch (2019)"),
    ("MacBookAir8,1", "MacBook Air (2018)"),
    ("MacBookAir8,2", "MacBook Air (2019)"),
    ("MacBookAir9,1", "MacBook Air (2020)"),
    ("Macmini8,1", "Mac mini (2018)"),
    ("iMac20,1", "iMac 27-inch (2020)"),
    ("iMac20,2", "iMac 27-inch (2020)"),
    ("iMacPro1,1", "iMac Pro (2017)"),
    ("MacPro7,1", "Mac Pro (2019)"),
];

/// "iMac Pro (2017)", or the raw model ID for a model not in the list.
pub fn model() -> String {
    let id = std::fs::read_to_string(PRODUCT_NAME).unwrap_or_default();
    model_name(id.trim())
}

fn model_name(id: &str) -> String {
    match MODELS.iter().find(|(model, _)| *model == id) {
        Some((_, name)) => (*name).to_owned(),
        None if id.is_empty() => "Unknown Mac".to_owned(),
        None => id.to_owned(),
    }
}

/// "Fedora 45 · COSMIC" from os-release.
pub fn system() -> String {
    let text = std::fs::read_to_string(OS_RELEASE).unwrap_or_default();
    system_name(&text)
}

fn system_name(os_release: &str) -> String {
    let field = |key: &str| {
        os_release
            .lines()
            .find_map(|line| line.strip_prefix(key)?.strip_prefix('='))
            .map(|value| value.trim_matches('"').to_owned())
    };
    let name = field("NAME").unwrap_or_else(|| "Linux".to_owned());
    let name = name.strip_suffix(" Linux").unwrap_or(&name).to_owned();
    let mut text = match field("VERSION_ID") {
        Some(version) => format!("{name} {version}"),
        None => name,
    };
    if let Some(variant) = field("VARIANT") {
        text.push_str(" · ");
        text.push_str(&variant);
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_known_models_and_keeps_unknown_ids() {
        assert_eq!(model_name("iMacPro1,1"), "iMac Pro (2017)");
        assert_eq!(model_name("MacBookPro99,1"), "MacBookPro99,1");
        assert_eq!(model_name(""), "Unknown Mac");
    }

    #[test]
    fn reads_the_system_from_os_release() {
        let text = "NAME=\"Fedora Linux\"\nVERSION_ID=45\nVARIANT=\"COSMIC\"\n";
        assert_eq!(system_name(text), "Fedora 45 · COSMIC");
        assert_eq!(system_name("NAME=Fedora Linux\nVERSION_ID=45\n"), "Fedora 45");
    }
}
