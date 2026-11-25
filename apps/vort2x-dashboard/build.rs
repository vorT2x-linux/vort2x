// SPDX-License-Identifier: GPL-3.0-or-later
use std::{env, path::PathBuf, process::Command};

fn main() {
    println!("cargo:rerun-if-changed=resources.gresource.xml");
    println!("cargo:rerun-if-changed=icons");

    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR is not set"));
    let status = Command::new("glib-compile-resources")
        .args(["resources.gresource.xml", "--sourcedir=.", "--target"])
        .arg(out_dir.join("vort2x-dashboard.gresource"))
        .status()
        .expect("failed to run glib-compile-resources");
    assert!(status.success(), "glib-compile-resources failed: {status}");
}
