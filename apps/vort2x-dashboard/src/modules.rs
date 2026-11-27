// SPDX-License-Identifier: GPL-3.0-or-later
//! The dashboard's pages and which of them this Mac gets.
//!
//! The installer decides per Mac what to install, so a page shows exactly
//! when a file its app installs is there, and hardware checks stay in one
//! place. The installer's `app:<name>` markers are no good for this:
//! vort2x-update writes one for every app it knows, installed or not.

use std::path::Path;

use adw::prelude::*;
use gtk4::{PolicyType, ScrolledWindow};

/// What a page puts in the dashboard.
pub struct Page {
    /// The header bar's title widget.
    pub title: gtk4::Widget,
    /// Packed at the end of the header bar in this order, the first one
    /// outermost.
    pub header_end: Vec<gtk4::Widget>,
    pub content: gtk4::Widget,
}

impl Page {
    fn new(title: impl IsA<gtk4::Widget>, content: impl IsA<gtk4::Widget>) -> Self {
        Self {
            title: title.upcast(),
            header_end: Vec::new(),
            content: content.upcast(),
        }
    }

    fn with_end(mut self, widget: impl IsA<gtk4::Widget>) -> Self {
        self.header_end.push(widget.upcast());
        self
    }
}

pub struct Module {
    /// The installer's name for the app.
    pub id: &'static str,
    /// Absolute path of a file the app installs.
    pub installed_if: &'static str,
    pub title: &'static str,
    /// Built into the binary (`icons/`), so it shows on every icon theme.
    pub icon: &'static str,
    pub build: fn() -> Page,
}

/// In sidebar order. The two GPU apps share a title: a Mac gets one of them.
pub const MODULES: &[Module] = &[
    Module {
        id: "check-my-mac",
        // Part of the dashboard itself, so every Mac gets it.
        installed_if: "/usr/local/bin/vort2x-dashboard",
        title: "Check my Mac",
        icon: "vort2x-check-symbolic",
        build: check_mac,
    },
    Module {
        id: "t2-fan-control",
        installed_if: "/usr/local/bin/t2-fancontrol-gtk",
        title: "Fans",
        icon: "vort2x-fan-symbolic",
        build: fans,
    },
    Module {
        id: "t2-zone-power",
        installed_if: "/usr/local/bin/t2-zonepower-gtk",
        title: "Zone Power",
        icon: "vort2x-zone-power-symbolic",
        build: zone_power,
    },
    Module {
        id: "t2-power-explorer",
        installed_if: "/usr/local/bin/t2-power-explorer",
        title: "Power Explorer",
        icon: "vort2x-power-explorer-symbolic",
        build: power_explorer,
    },
    Module {
        id: "t2-power-tune",
        installed_if: "/usr/local/bin/t2-power-tune",
        title: "Power Tune",
        icon: "vort2x-power-tune-symbolic",
        build: power_tune,
    },
    Module {
        id: "t2-dgpu-control",
        installed_if: "/usr/local/bin/t2-dgpu-control",
        title: "Graphics",
        icon: "vort2x-graphics-symbolic",
        build: dgpu,
    },
    Module {
        id: "t2-hybrid-gpu-control",
        installed_if: "/usr/local/bin/t2-hybrid-gpu-control",
        title: "Graphics",
        icon: "vort2x-graphics-symbolic",
        build: hybrid_gpu,
    },
    Module {
        id: "t2-smc-control",
        installed_if: "/usr/local/bin/t2-smc-control",
        title: "SMC",
        icon: "vort2x-smc-symbolic",
        build: smc,
    },
    Module {
        id: "t2-force-click",
        installed_if: "/usr/local/bin/t2-force-click",
        title: "Force Click",
        icon: "vort2x-force-click-symbolic",
        build: force_click,
    },
    Module {
        id: "t2-truetone",
        installed_if: "/usr/local/bin/t2-truetone",
        title: "TrueTone",
        icon: "vort2x-truetone-symbolic",
        build: truetone,
    },
    Module {
        id: "t2-airshare",
        installed_if: "/usr/local/bin/t2-airshare",
        title: "AirShare",
        icon: "vort2x-airshare-symbolic",
        build: airshare,
    },
    Module {
        id: "t2-debug",
        installed_if: "/usr/local/bin/t2-debug",
        title: "Debug",
        icon: "vort2x-debug-symbolic",
        build: debug,
    },
    Module {
        id: "vort2x-update",
        installed_if: "/usr/local/bin/vort2x-update",
        title: "Updates",
        icon: "vort2x-update-symbolic",
        build: updates,
    },
];

/// Pages made for a narrow window stay a centred column that scrolls,
/// instead of stretching across the pane.
fn column(content: &impl IsA<gtk4::Widget>) -> ScrolledWindow {
    let clamp = adw::Clamp::builder().maximum_size(760).child(content).build();
    ScrolledWindow::builder()
        .child(&clamp)
        .hscrollbar_policy(PolicyType::Automatic)
        .vexpand(true)
        .build()
}

/// Pages made for a wide window scroll sideways when the card is narrower
/// than they need, instead of pushing the header off the card. They scroll
/// up and down themselves.
fn wide(content: &impl IsA<gtk4::Widget>) -> ScrolledWindow {
    ScrolledWindow::builder()
        .child(content)
        .hscrollbar_policy(PolicyType::Automatic)
        .vscrollbar_policy(PolicyType::Never)
        .vexpand(true)
        .build()
}

fn check_mac() -> Page {
    let (content, refresh) = crate::check_page::build();
    Page::new(adw::WindowTitle::new("Check my Mac", ""), column(&content)).with_end(refresh)
}

fn fans() -> Page {
    let page = t2_fancontrol_gtk::page();
    Page::new(page.title, column(&page.content)).with_end(page.header_end)
}

fn zone_power() -> Page {
    let page = t2_zonepower_gtk::page();
    Page::new(page.title, column(&page.content))
}

fn power_explorer() -> Page {
    let page = t2_power_explorer::page();
    Page::new(page.title, wide(&page.content)).with_end(page.header_end)
}

fn power_tune() -> Page {
    let page = t2_power_tune::page();
    Page::new(page.title, wide(&page.content))
}

fn dgpu() -> Page {
    let page = t2_dgpu_control::page();
    Page::new(page.title, wide(&page.content))
}

fn hybrid_gpu() -> Page {
    let page = t2_hybrid_gpu_control::page();
    Page::new(page.title, wide(&page.content))
}

fn smc() -> Page {
    let page = t2_smc_control::page();
    Page {
        title: page.title.upcast(),
        header_end: page.header_end,
        content: wide(&page.content).upcast(),
    }
}

fn force_click() -> Page {
    let page = t2_force_click::page();
    Page::new(page.title, column(&page.content))
}

fn truetone() -> Page {
    let page = t2_truetone::gui::page();
    Page::new(page.title, wide(&page.content))
}

fn airshare() -> Page {
    let page = t2_airshare::gui::page();
    Page::new(page.title, wide(&page.content))
}

fn debug() -> Page {
    let page = t2_debug::gui::page();
    Page::new(page.title, wide(&page.content)).with_end(page.header_end)
}

fn updates() -> Page {
    let page = vort2x_update::page();
    Page::new(page.title, wide(&page.content)).with_end(page.header_end)
}

/// The pages whose app is installed under `root` ("/" outside tests).
pub fn installed(root: &Path) -> Vec<&'static Module> {
    MODULES
        .iter()
        .filter(|module| root.join(module.installed_if.trim_start_matches('/')).exists())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(root: &Path) -> Vec<&'static str> {
        installed(root).iter().map(|module| module.id).collect()
    }

    #[test]
    fn pages_follow_the_installed_apps() {
        let root = std::env::temp_dir().join(format!("vort2x-dashboard-test-{}", std::process::id()));
        let bin = root.join("usr/local/bin");
        std::fs::create_dir_all(&bin).unwrap();

        assert!(ids(&root).is_empty());
        std::fs::write(bin.join("vort2x-update"), "").unwrap();
        assert_eq!(ids(&root), ["vort2x-update"]);
        std::fs::write(bin.join("t2-fancontrol-gtk"), "").unwrap();
        std::fs::write(bin.join("t2drop"), "").unwrap();
        assert_eq!(ids(&root), ["t2-fan-control", "vort2x-update"]);
        std::fs::write(bin.join("vort2x-dashboard"), "").unwrap();
        assert_eq!(ids(&root), ["check-my-mac", "t2-fan-control", "vort2x-update"]);

        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn nothing_installed_means_no_pages() {
        assert!(ids(Path::new("/nonexistent/vort2x-root")).is_empty());
    }

    #[test]
    fn module_ids_are_unique() {
        let mut ids: Vec<_> = MODULES.iter().map(|module| module.id).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), MODULES.len());
    }
}
