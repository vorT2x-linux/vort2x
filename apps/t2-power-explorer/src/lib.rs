// SPDX-License-Identifier: GPL-3.0-or-later
mod collector;
mod diagnostics;
mod model;
mod ui;

use adw::prelude::*;

pub use ui::{page, Page};

const APP_ID: &str = "org.t2powerexplorer.gtk";

/// Runs the standalone T2 Power Explorer window.
pub fn run() {
    let app = adw::Application::builder().application_id(APP_ID).build();
    app.connect_activate(ui::build);
    app.run();
}
