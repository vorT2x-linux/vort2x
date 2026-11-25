// SPDX-License-Identifier: GPL-3.0-or-later
//! The dashboard's own look, on top of the styles its pages install.

use gtk4::{gdk, CssProvider};

const COMMON: &str = include_str!("style.css");
const DARK: &str = include_str!("style-dark.css");
const LIGHT: &str = include_str!("style-light.css");

/// Installs the stylesheet and swaps its colours with the system theme.
pub fn install() {
    let Some(display) = gdk::Display::default() else {
        return;
    };
    let provider = CssProvider::new();
    let manager = adw::StyleManager::default();
    load(&provider, manager.is_dark());
    // Above the pages' providers (application priority), so the window and
    // pane colours here win.
    gtk4::style_context_add_provider_for_display(
        &display,
        &provider,
        gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION + 1,
    );
    manager.connect_dark_notify(move |manager| load(&provider, manager.is_dark()));
}

fn load(provider: &CssProvider, dark: bool) {
    provider.load_from_string(&format!("{COMMON}\n{}", if dark { DARK } else { LIGHT }));
}
