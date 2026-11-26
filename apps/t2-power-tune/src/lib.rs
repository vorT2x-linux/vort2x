// SPDX-License-Identifier: GPL-3.0-or-later
//! T2 Power Tune: PCIe ASPM, LTR, runtime PM, wake sources and other power
//! tunables, with live package C-state residency. The root work is done by
//! `t2-power-tune-helper` (scan, apply, persist) and `t2-power-tune-status`
//! (C-state counters), both started through pkexec; this is only the page.

use std::cell::{Cell, RefCell};
use std::path::Path;
use std::rc::Rc;

use adw::prelude::*;
use gtk4::{gio, glib};
use serde::{Deserialize, Deserializer, Serialize};

const APP_ID: &str = "org.t2powertune.gtk";
const APP_VERSION: &str = "0.02";
const HELPER: &str = "/usr/local/libexec/t2-power-tune-helper";
const STATUS: &str = "/usr/local/libexec/t2-power-tune-status";
const SERVICE: &str = "/etc/systemd/system/vort2x-power-tune.service";
const WORDMARK_DARK: &str = "/usr/local/share/vort2x/vort2x-wordmark.png";
const WORDMARK_LIGHT: &str = "/usr/local/share/vort2x/vort2x-wordmark-light.png";
const CSTATES: [&str; 7] = ["C2", "C3", "C6", "C7", "C8", "C9", "C10"];

const ASPM_HELP: (&str, &str) = ("PCIe ASPM link states",
    "L0s and L1 reduce the power used by an idle PCIe link. The states are selected independently and are only written after Apply.\n\nThe numerically highest or most complete combination is not always the best one. Hardware and drivers may behave better with L1 alone. On some platforms L0s can even prevent deeper package C-states. Test each link and keep only combinations that remain stable across suspend and resume before writing to systemd.");
const LTR_HELP: (&str, &str) = ("Latency Tolerance Reporting",
    "LTR tells the platform how long a device can tolerate delayed service. A strict requirement can keep the package in a shallower C-state.\n\nIgnore does not rewrite the device's Snoop or No-snoop latency. It only tells the PMC not to use that source as a constraint. This can improve residency, but may cause timeouts, audio dropouts or unstable I/O. Apply one source at a time and verify the device afterwards.");
const RUNTIME_HELP: (&str, &str) = ("Runtime power management",
    "Runtime PM lets the kernel suspend an idle device on runtime. A checked device is changed from on to auto after Apply.\n\nSome drivers cannot resume every device reliably. Symptoms include failed probe, missing Wi-Fi or Bluetooth, and PCIe errors after suspend. In particular, leave BCM4377 runtime PM disabled unless suspend and resume have been tested successfully.");
const WAKEUP_HELP: (&str, &str) = ("Wake sources",
    "Enabled USB wake sources and Wake-on-LAN can prevent the platform from reaching deeper package C-states. A checked source is changed from enabled to disabled after Apply.\n\nDisabling a source also prevents that device from waking the computer. Test suspend and resume before making the selection persistent.");
const OTHER_HELP: (&str, &str) = ("Other power tunables",
    "These controls reduce periodic wakeups and are independent of PCIe ASPM, device runtime PM and wake-source settings.\n\nChanges can affect diagnostics and writeback timing. Apply them individually when troubleshooting, and verify suspend, resume and normal operation before making them persistent.");
const ACTION_HELP: (&str, &str) = ("Actions",
    "Apply selected changes writes the currently chosen values for this boot and verifies them. Nothing is written merely by changing a checkbox.\n\nCreate or Update systemd service stores the same selection in vort2x-power-tune.service so it is restored at boot. Unselected managed values are removed from the service.\n\nRescan reads the current hardware state again. It does not apply settings, but hardware values such as LTR can change dynamically while devices are active.");

fn cstate_help(state: &str) -> &'static str {
    match state {
        "C2" => "PC2 is a package state. To enter it, every individual CPU core must already be in Core C6 or deeper; Core C6 and Package C6 are different states. Graphics must be in RC6. Timers, device traffic or LTR constraints can keep the package at PC2.",
        "C3" => "PC3 requires every individual CPU core to be in Core C6 or deeper, graphics in RC6 and device LTR that permits PC3. Memory can enter self-refresh, its clock can stop, and the LLC may be flushed. This does not require prior Package C6 residency.",
        "C6" => "PC6 builds on the PC3 platform conditions. BCLK can stop and voltage regulators can reduce voltage. Frequent core wakeups, near timers or restrictive device latency requirements prevent useful residency.",
        "C7" => "A platform-specific intermediate deep state. It requires the platform conditions for PC6 plus sufficiently long, uninterrupted core and device idle. Intel does not publish one universal entry delay for all systems.",
        "C8" => "Requires all cores to request C8 or deeper, graphics in RC6 and device LTR that permits PC8. The LLC is flushed and its voltage can be removed.",
        "C9" => "A platform-specific step between PC8 and PC10. It needs deeper core and platform idle; display configuration, PSR, device latency and wake activity can limit entry.",
        _ => "Requires PC8 conditions, deep idle on every core, graphics in RC6, suitable device LTR and low wake activity. With the internal display active, the panel must normally be in PSR; otherwise the display must be powered off. Regulators reach their deep power state and the crystal clock can stop.",
    }
}

/// One configurable value from `helper scan`.
#[derive(Deserialize)]
struct Item {
    kind: String,
    id: String,
    label: String,
    #[serde(deserialize_with = "text")]
    current: String,
    #[serde(deserialize_with = "text")]
    target: String,
    #[serde(default)]
    selected: bool,
    /// ASPM only: the link states the device supports, bit 0 L0s, bit 1 L1.
    #[serde(default)]
    supported: u8,
    #[serde(default)]
    group: Option<String>,
}

/// The helper writes values as strings, older saved selections may hold numbers.
fn text<'de, D: Deserializer<'de>>(deserializer: D) -> Result<String, D::Error> {
    Ok(match serde_json::Value::deserialize(deserializer)? {
        serde_json::Value::String(text) => text,
        other => other.to_string(),
    })
}

#[derive(Deserialize)]
struct Scan {
    items: Vec<Item>,
}

/// One entry of the selection sent to `helper apply|persist`.
#[derive(Serialize)]
struct Selection<'a> {
    kind: &'a str,
    id: &'a str,
    original: &'a str,
    target: String,
    enabled: bool,
}

/// One value `helper apply` changed.
#[derive(Deserialize)]
struct Change {
    kind: String,
    id: String,
    #[serde(default)]
    location: String,
    #[serde(deserialize_with = "text")]
    value: String,
}

#[derive(Clone, Copy, PartialEq)]
enum Section {
    Aspm,
    Ltr,
    Runtime,
    Wakeup,
    Other,
}

impl Section {
    fn of(item: &Item) -> Self {
        match (item.kind.as_str(), item.group.as_deref()) {
            ("aspm", _) => Self::Aspm,
            ("ltr", _) => Self::Ltr,
            (_, Some("runtime")) => Self::Runtime,
            (_, Some("wakeup")) => Self::Wakeup,
            _ => Self::Other,
        }
    }

    fn empty_text(self) -> &'static str {
        match self {
            Self::Aspm => "No additional ASPM states found.",
            Self::Ltr => "No active LTR requirements found.",
            Self::Runtime => "Runtime PM is already allowed for every discovered device.",
            Self::Wakeup => "No active wake sources found.",
            Self::Other => "No additional inactive tunables found.",
        }
    }
}

enum Control {
    Aspm { l0s: gtk4::CheckButton, l1: gtk4::CheckButton },
    Check(gtk4::CheckButton),
}

struct Ui {
    sections: Vec<(Section, gtk4::Box)>,
    rows: RefCell<Vec<(Item, Control)>>,
    cstates: Vec<(gtk4::Box, gtk4::Label)>,
    apply: gtk4::Button,
    service: gtk4::Button,
    rescan: gtk4::Button,
    notice: gtk4::Label,
    service_existed: Cell<bool>,
}

/// The power tune page, shown in the standalone window and in the VorT2x Dashboard.
pub struct Page {
    pub title: adw::WindowTitle,
    pub content: gtk4::Widget,
}

/// Runs the standalone T2 Power Tune window.
pub fn run() -> glib::ExitCode {
    let app = adw::Application::builder().application_id(APP_ID).build();
    app.connect_activate(build_ui);
    app.run()
}

fn build_ui(app: &adw::Application) {
    let page = page();
    let header = adw::HeaderBar::new();
    header.set_title_widget(Some(&page.title));
    if let Some(brand) = vort2x_brand() {
        brand.set_margin_start(10);
        brand.set_margin_end(10);
        header.pack_start(&brand);
    }
    let root = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    root.add_css_class("app-background");
    root.append(&header);
    root.append(&page.content);
    let window = adw::ApplicationWindow::builder()
        .application(app)
        .title("T2 Power Tune")
        .default_width(950)
        .default_height(780)
        .content(&root)
        .build();
    window.present();
}

/// Builds the page, scans the hardware and starts the C-state readout.
pub fn page() -> Page {
    install_css();
    let root = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    root.add_css_class("app-background");
    root.set_vexpand(true);

    let metrics = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    metrics.add_css_class("sticky-bar");
    metrics.set_margin_bottom(8);
    metrics.append(&label("Package C-states", &["heading"]));
    let mut cstates = Vec::new();
    for state in CSTATES {
        let cell = gtk4::Box::new(gtk4::Orientation::Horizontal, 2);
        cell.set_size_request(86, 30);
        cell.set_valign(gtk4::Align::Center);
        cell.add_css_class("cstate-cell");
        cell.set_tooltip_text(Some(cstate_help(state)));
        cell.append(&gtk4::Label::new(Some(state)));
        let value = gtk4::Label::new(Some("…"));
        value.set_xalign(1.0);
        value.set_width_chars(7);
        value.set_max_width_chars(7);
        cell.append(&value);
        metrics.append(&cell);
        cstates.push((cell, value));
    }
    metrics.append(&label("Last 1 s · live", &["dim-label"]));
    root.append(&metrics);

    let body = gtk4::Box::new(gtk4::Orientation::Vertical, 14);
    body.set_margin_top(18);
    body.set_margin_bottom(18);
    body.set_margin_start(18);
    body.set_margin_end(18);
    let sections = [
        (Section::Aspm, "PCIe ASPM candidates", ASPM_HELP),
        (Section::Ltr, "LTR requirements", LTR_HELP),
        (Section::Runtime, "Runtime power management", RUNTIME_HELP),
        (Section::Wakeup, "Wake sources", WAKEUP_HELP),
        (Section::Other, "Other power tunables", OTHER_HELP),
    ]
    .into_iter()
    .map(|(section, title, help)| (section, section_box(&body, title, help)))
    .collect();
    root.append(
        &gtk4::ScrolledWindow::builder()
            .child(&body)
            .vexpand(true)
            .hscrollbar_policy(gtk4::PolicyType::Never)
            .build(),
    );

    let footer = gtk4::Box::new(gtk4::Orientation::Vertical, 6);
    footer.add_css_class("sticky-bar");
    let actions = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    let apply = gtk4::Button::with_label("Apply selected changes");
    apply.add_css_class("suggested-action");
    apply.set_tooltip_text(Some("Write and verify the selected values for this boot"));
    let service = gtk4::Button::new();
    service.set_tooltip_text(Some("Make the selected values persistent at boot"));
    let rescan = gtk4::Button::with_label("Rescan");
    rescan.set_tooltip_text(Some("Read the current hardware state again"));
    actions.append(&apply);
    actions.append(&service);
    actions.append(&rescan);
    actions.append(&help_button("Explain the actions", ACTION_HELP));
    footer.append(&actions);
    let notice = label("", &["dim-label"]);
    notice.set_wrap(true);
    footer.append(&notice);
    root.append(&footer);

    let status = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    status.set_margin_start(18);
    status.set_margin_end(18);
    status.set_margin_bottom(8);
    let spacer = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
    spacer.set_hexpand(true);
    status.append(&spacer);
    status.append(&label(&format!("v{APP_VERSION}"), &["dim-label"]));
    root.append(&status);

    let ui = Rc::new(Ui {
        sections,
        rows: RefCell::new(Vec::new()),
        cstates,
        apply,
        service,
        rescan,
        notice,
        service_existed: Cell::new(false),
    });
    update_service_button(&ui);
    ui.apply.connect_clicked(glib::clone!(
        #[weak]
        ui,
        move |_| start_action(&ui, "apply")
    ));
    ui.service.connect_clicked(glib::clone!(
        #[weak]
        ui,
        move |_| start_action(&ui, "persist")
    ));
    ui.rescan.connect_clicked(glib::clone!(
        #[weak]
        ui,
        move |_| scan(&ui)
    ));

    scan(&ui);
    start_cstates(&ui);
    Page {
        title: adw::WindowTitle::new("Power Tune", "PCIe ASPM and power tunables"),
        content: root.upcast(),
    }
}

fn label(text: &str, classes: &[&str]) -> gtk4::Label {
    let label = gtk4::Label::new(Some(text));
    label.set_xalign(0.0);
    for class in classes {
        label.add_css_class(class);
    }
    label
}

/// A titled panel in `parent`; returns the box its rows go into.
fn section_box(parent: &gtk4::Box, title: &str, help: (&'static str, &'static str)) -> gtk4::Box {
    let panel = gtk4::Box::new(gtk4::Orientation::Vertical, 8);
    panel.add_css_class("unified-box");
    let heading_row = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
    let heading = label(title, &["title-4"]);
    heading.set_hexpand(true);
    heading_row.append(&heading);
    heading_row.append(&help_button(&format!("Explain {title}"), help));
    let content = gtk4::Box::new(gtk4::Orientation::Vertical, 5);
    panel.append(&heading_row);
    panel.append(&content);
    parent.append(&panel);
    content
}

fn help_button(tooltip: &str, (title, text): (&'static str, &'static str)) -> gtk4::Button {
    let button = gtk4::Button::from_icon_name("help-about-symbolic");
    button.set_has_frame(false);
    button.set_valign(gtk4::Align::Center);
    button.set_tooltip_text(Some(tooltip));
    button.connect_clicked(move |button| {
        let text = label(text, &[]);
        text.set_yalign(0.0);
        text.set_wrap(true);
        text.set_margin_top(20);
        text.set_margin_bottom(20);
        text.set_margin_start(20);
        text.set_margin_end(20);
        show_dialog(button, title, &text, 560, 360);
    });
    button
}

/// A modal window over whichever window holds `anchor`: its own or the dashboard.
fn show_dialog(anchor: &impl IsA<gtk4::Widget>, title: &str, child: &impl IsA<gtk4::Widget>, width: i32, height: i32) -> adw::Window {
    let header = adw::HeaderBar::new();
    header.set_title_widget(Some(&adw::WindowTitle::new(title, "")));
    let content = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    content.append(&header);
    content.append(child);
    let dialog = adw::Window::builder()
        .modal(true)
        .title(title)
        .default_width(width)
        .default_height(height)
        .content(&content)
        .build();
    dialog.set_transient_for(anchor.root().and_downcast::<gtk4::Window>().as_ref());
    dialog.present();
    dialog
}

fn set_busy(ui: &Ui, busy: bool, message: &str) {
    for button in [&ui.apply, &ui.service, &ui.rescan] {
        button.set_sensitive(!busy);
    }
    if !message.is_empty() {
        ui.notice.set_text(message);
    }
}

fn update_service_button(ui: &Ui) {
    let action = if Path::new(SERVICE).is_file() { "Update" } else { "Create" };
    ui.service.set_label(&format!("{action} systemd service"));
}

/// Runs the helper through pkexec with `stdin`, then calls `done` with its
/// success, stdout and stderr, without blocking the main loop.
fn run_helper(action: &str, stdin: Option<String>, done: impl FnOnce(bool, String, String) + 'static) {
    let flags = gio::SubprocessFlags::STDIN_PIPE | gio::SubprocessFlags::STDOUT_PIPE | gio::SubprocessFlags::STDERR_PIPE;
    let argv = ["pkexec", "--disable-internal-agent", HELPER, action].map(std::ffi::OsStr::new);
    let process = match gio::Subprocess::newv(&argv, flags) {
        Ok(process) => process,
        Err(error) => return done(false, String::new(), error.to_string()),
    };
    process.clone().communicate_utf8_async(stdin, gio::Cancellable::NONE, move |result| match result {
        Ok((stdout, stderr)) => done(
            process.is_successful(),
            stdout.map(String::from).unwrap_or_default(),
            stderr.map(String::from).unwrap_or_default(),
        ),
        Err(error) => done(false, String::new(), error.to_string()),
    });
}

fn scan(ui: &Rc<Ui>) {
    set_busy(ui, true, "Scanning hardware…");
    let ui = ui.clone();
    run_helper("scan", None, move |ok, stdout, stderr| {
        set_busy(&ui, false, "");
        if !ok {
            ui.notice.set_text(fallback(&stderr, "Scan failed or authentication was cancelled."));
            return;
        }
        match serde_json::from_str::<Scan>(&stdout) {
            Ok(scan) => show_scan(&ui, scan.items),
            Err(error) => ui.notice.set_text(&format!("Invalid scan result: {error}")),
        }
    });
}

fn fallback<'a>(text: &'a str, otherwise: &'a str) -> &'a str {
    match text.trim() {
        "" => otherwise,
        text => text,
    }
}

fn show_scan(ui: &Ui, mut items: Vec<Item>) {
    items.sort_by_cached_key(|item| (Section::of(item) as u8, item.label.to_lowercase()));
    for (_, section) in &ui.sections {
        while let Some(child) = section.first_child() {
            section.remove(&child);
        }
    }
    let count = items.len();
    let mut rows = Vec::with_capacity(count);
    for item in items {
        let row = gtk4::Box::new(gtk4::Orientation::Horizontal, 10);
        let title = label(&item.label, &[]);
        title.set_wrap(true);
        title.set_hexpand(true);
        title.set_valign(gtk4::Align::Center);
        let control = if item.kind == "aspm" {
            let source = if item.selected { &item.target } else { &item.current };
            let desired: u8 = source.parse().unwrap_or(0);
            let state = |name: &str, bit: u8| {
                let check = gtk4::CheckButton::with_label(name);
                check.set_active(desired & bit != 0);
                check.set_sensitive(item.supported & bit != 0);
                check
            };
            let (l0s, l1) = (state("L0s", 1), state("L1", 2));
            let choices = gtk4::Box::new(gtk4::Orientation::Horizontal, 12);
            choices.append(&l0s);
            choices.append(&l1);
            row.append(&title);
            row.append(&choices);
            Control::Aspm { l0s, l1 }
        } else {
            let check = gtk4::CheckButton::new();
            check.set_active(item.selected);
            check.set_valign(gtk4::Align::Center);
            row.append(&check);
            row.append(&title);
            Control::Check(check)
        };
        if let Some((_, section)) = ui.sections.iter().find(|(section, _)| *section == Section::of(&item)) {
            section.append(&row);
        }
        rows.push((item, control));
    }
    for (section, container) in &ui.sections {
        if container.first_child().is_none() {
            container.append(&label(section.empty_text(), &[]));
        }
    }
    *ui.rows.borrow_mut() = rows;
    ui.notice.set_text(&format!("{count} configurable item(s) found."));
}

/// A row's target value and whether it is selected. An ASPM row counts as
/// selected when its states differ from the current ones.
fn row_choice(item: &Item, control: &Control) -> (String, bool) {
    match control {
        Control::Aspm { l0s, l1 } => {
            let target = u8::from(l0s.is_active()) | u8::from(l1.is_active()) << 1;
            (target.to_string(), item.current.parse::<u8>().ok() != Some(target))
        }
        Control::Check(check) => (item.target.clone(), check.is_active()),
    }
}

/// The selection as the helper expects it.
fn selection(ui: &Ui) -> String {
    let rows = ui.rows.borrow();
    let selection: Vec<Selection> = rows
        .iter()
        .map(|(item, control)| {
            let (target, enabled) = row_choice(item, control);
            Selection { kind: &item.kind, id: &item.id, original: &item.current, target, enabled }
        })
        .collect();
    serde_json::to_string(&selection).unwrap_or_else(|_| "[]".to_owned())
}

fn start_action(ui: &Rc<Ui>, action: &'static str) {
    let existed = Path::new(SERVICE).is_file();
    ui.service_existed.set(existed);
    let message = match (action, existed) {
        ("apply", _) => "Applying changes…",
        (_, true) => "Updating systemd service…",
        (_, false) => "Creating systemd service…",
    };
    set_busy(ui, true, message);
    let payload = selection(ui);
    let ui = ui.clone();
    run_helper(action, Some(payload), move |ok, stdout, stderr| {
        set_busy(&ui, false, "");
        if !ok {
            ui.notice.set_text(fallback(&stderr, "Operation failed or authentication was cancelled."));
        } else if action == "apply" {
            ui.notice.set_text("Selection applied and verified.");
            let changes: Vec<Change> = serde_json::from_str(&stdout).unwrap_or_default();
            show_apply_result(&ui, &changes);
            if !changes.is_empty() {
                // Some values settle a moment after they are written.
                let ui = ui.clone();
                glib::timeout_add_seconds_local_once(2, move || scan(&ui));
            }
        } else {
            let any_enabled = ui.rows.borrow().iter().any(|(item, control)| row_choice(item, control).1);
            ui.notice.set_text(&if any_enabled {
                let verb = if ui.service_existed.get() { "updated" } else { "created" };
                format!("vort2x-power-tune.service was {verb} and enabled.")
            } else {
                "The empty service configuration was removed.".to_owned()
            });
            update_service_button(&ui);
        }
    });
}

fn describe_change(change: &Change) -> String {
    match change.kind.as_str() {
        "aspm" => {
            let state = match change.value.as_str() {
                "0" => "Disabled",
                "1" => "L0s",
                "2" => "L1",
                "3" => "L0s + L1",
                other => other,
            };
            format!("PCI {}\n  ASPM = {state}", change.id)
        }
        "ltr" => {
            let state = if change.value == "1" { "ignored" } else { "restored" };
            format!("{}\n  {} = {state}", change.id, change.location)
        }
        _ => {
            let device = |bus: &str| {
                let marker = format!("/{bus}/devices/");
                change.id.split_once(&marker).map(|(_, rest)| rest.split('/').next().unwrap_or(rest).to_owned())
            };
            let address = match (device("usb"), device("pci")) {
                (Some(usb), _) => format!("USB {usb}"),
                (None, Some(pci)) => format!("PCI {pci}"),
                _ => "System tunable".to_owned(),
            };
            format!("{address}\n  {} = {}", change.location, change.value)
        }
    }
}

fn show_apply_result(ui: &Ui, changes: &[Change]) {
    let body = if changes.is_empty() {
        "No values needed changing.".to_owned()
    } else {
        changes.iter().map(describe_change).collect::<Vec<_>>().join("\n\n")
    };
    let view = gtk4::TextView::builder()
        .editable(false)
        .cursor_visible(false)
        .monospace(true)
        .wrap_mode(gtk4::WrapMode::WordChar)
        .build();
    view.buffer().set_text(&body);
    let scroll = gtk4::ScrolledWindow::builder().child(&view).vexpand(true).build();
    scroll.set_margin_top(12);
    scroll.set_margin_start(18);
    scroll.set_margin_end(18);
    let ok = gtk4::Button::with_label("OK");
    ok.add_css_class("suggested-action");
    ok.set_halign(gtk4::Align::End);
    ok.set_margin_top(12);
    ok.set_margin_bottom(12);
    ok.set_margin_end(18);
    let content = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    content.append(&scroll);
    content.append(&ok);
    let dialog = show_dialog(&ui.notice, "Applied and verified", &content, 620, 440);
    ok.connect_clicked(move |_| dialog.close());
}

/// Starts the status helper, which prints the package C-state residency of
/// each second as JSON and exits with this process.
fn start_cstates(ui: &Rc<Ui>) {
    let pid = std::process::id().to_string();
    let argv = ["pkexec", "--disable-internal-agent", STATUS, &pid].map(std::ffi::OsStr::new);
    let flags = gio::SubprocessFlags::STDOUT_PIPE | gio::SubprocessFlags::STDERR_SILENCE;
    let Ok(process) = gio::Subprocess::newv(&argv, flags) else {
        return show_cstates(ui, None);
    };
    let Some(stdout) = process.stdout_pipe() else {
        return show_cstates(ui, None);
    };
    read_cstates(ui.clone(), gio::DataInputStream::new(&stdout));
}

fn read_cstates(ui: Rc<Ui>, stream: gio::DataInputStream) {
    stream.clone().read_line_utf8_async(glib::Priority::DEFAULT, gio::Cancellable::NONE, move |line| {
        let Ok(Some(line)) = line else {
            // The helper ended: no counters, or authentication refused.
            return show_cstates(&ui, None);
        };
        if let Ok(data) = serde_json::from_str::<serde_json::Value>(&line) {
            show_cstates(&ui, data.get("package"));
        }
        read_cstates(ui, stream);
    });
}

/// `None` marks the readout as unavailable.
fn show_cstates(ui: &Ui, package: Option<&serde_json::Value>) {
    for (state, (cell, value)) in CSTATES.iter().zip(&ui.cstates) {
        let Some(package) = package else {
            value.set_text("Unavailable");
            cell.set_css_classes(&["cstate-cell"]);
            continue;
        };
        match package.get(state).and_then(serde_json::Value::as_f64) {
            None => {
                value.set_text("—");
                cell.set_css_classes(&["cstate-cell"]);
            }
            Some(percent) if percent > 0.0 => {
                value.set_text(&if percent < 0.01 { "<0.01%".to_owned() } else { format!("{percent:.2}%") });
                cell.set_css_classes(&["cstate-cell", "cstate-positive"]);
            }
            Some(percent) => {
                value.set_text(&format!("{percent:.2}%"));
                cell.set_css_classes(&["cstate-cell", "cstate-zero"]);
            }
        }
    }
}

fn palette_css(dark: bool) -> String {
    let (window, fg, panel, line, rim, hairline) = if dark {
        ("#151517", "#e8e8e8", "#0d0d0f", "rgba(255,255,255,0.13)", "rgba(255,255,255,0.24)", "rgba(255,255,255,0.06)")
    } else {
        ("#ffffff", "#242424", "#e2e2e8", "rgba(0,0,0,0.16)", "rgba(255,255,255,0.70)", "rgba(0,0,0,0.08)")
    };
    format!(
        "window {{ color: alpha({fg}, 0.72); }}
         button, button label, entry, spinbutton, dropdown {{ color: alpha({fg}, 0.72); }}
         window.csd {{ border-top: 1px solid {rim}; }}
         window.csd.maximized, window.csd.tiled, window.csd.tiled-top, window.csd.tiled-left,
         window.csd.tiled-right, window.csd.tiled-bottom, window.csd.fullscreen {{ border-top-width: 0; }}
         .app-background {{ background: {window}; color: alpha({fg}, 0.72); }}
         .app-background .title-4, .app-background .title, .app-background .heading,
         windowtitle .title {{ color: {fg}; font-weight: 600; }}
         .app-background .dim-label, windowtitle .subtitle {{ color: alpha({fg}, 0.72); }}
         .app-background headerbar {{ background: {window}; color: alpha({fg}, 0.72); }}
         .unified-box {{ background: {panel}; color: alpha({fg}, 0.72); border-radius: 12px;
                         padding: 14px; box-shadow: inset 0 0 0 1px {hairline}; }}
         .sticky-bar {{ padding: 9px 18px; }}
         .cstate-cell {{ border: 1px solid {line}; border-radius: 7px; padding: 3px 6px;
                         font-family: 'JetBrains Mono'; font-feature-settings: 'tnum'; }}
         .app-background .cstate-zero {{ color: #808080; border-color: #808080; }}
         .app-background .cstate-positive {{ color: #3b8132; border-color: #3b8132; }}"
    )
}

fn install_css() {
    let Some(display) = gtk4::gdk::Display::default() else {
        return;
    };
    let manager = adw::StyleManager::default();
    let provider = gtk4::CssProvider::new();
    provider.load_from_string(&palette_css(manager.is_dark()));
    gtk4::style_context_add_provider_for_display(&display, &provider, gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION);
    manager.connect_dark_notify(move |manager| provider.load_from_string(&palette_css(manager.is_dark())));
}

/// The VorT2x wordmark in the tone of the current theme, if it is installed.
fn vort2x_brand() -> Option<gtk4::DrawingArea> {
    let dark = gtk4::gdk_pixbuf::Pixbuf::from_file(WORDMARK_DARK).ok()?;
    let light = gtk4::gdk_pixbuf::Pixbuf::from_file(WORDMARK_LIGHT).unwrap_or_else(|_| dark.clone());
    let brand = gtk4::DrawingArea::new();
    brand.set_content_width(80);
    brand.set_content_height(21);
    brand.set_draw_func(move |_, context, width, height| {
        let pixbuf = if adw::StyleManager::default().is_dark() { &dark } else { &light };
        let (width, height) = (f64::from(width), f64::from(height));
        let (image_width, image_height) = (f64::from(pixbuf.width()), f64::from(pixbuf.height()));
        let scale = f64::min(width / image_width, height / image_height);
        context.translate((width - image_width * scale) / 2.0, (height - image_height * scale) / 2.0);
        context.scale(scale, scale);
        context.set_source_pixbuf(pixbuf, 0.0, 0.0);
        let _ = context.paint();
    });
    let weak = brand.downgrade();
    adw::StyleManager::default().connect_dark_notify(move |_| {
        if let Some(brand) = weak.upgrade() {
            brand.queue_draw();
        }
    });
    Some(brand)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_a_scan_and_sorts_items_into_sections() {
        let scan: Scan = serde_json::from_str(
            r#"{"items":[
                {"kind":"aspm","id":"0000:02:00.0","label":"Wi-Fi","current":"0","target":"3","supported":3},
                {"kind":"tunable","id":"/sys/x","label":"USB","current":"enabled","target":"disabled","group":"wakeup","selected":true},
                {"kind":"ltr","id":"PMC0:x","label":"X","current":"0","target":"1","group":"ltr","selected":false},
                {"kind":"tunable","id":"/sys/y","label":"Old","current":5,"target":0}
            ],"constraints":[]}"#,
        )
        .unwrap();
        let sections: Vec<_> = scan.items.iter().map(|item| Section::of(item) as u8).collect();
        assert_eq!(sections, [0, 3, 1, 4]);
        assert!(scan.items[1].selected);
        assert_eq!(scan.items[3].current, "5");
        assert_eq!(scan.items[0].supported, 3);
    }

    #[test]
    fn describes_applied_changes() {
        let change = |kind: &str, id: &str, location: &str, value: &str| Change {
            kind: kind.into(),
            id: id.into(),
            location: location.into(),
            value: value.into(),
        };
        assert_eq!(describe_change(&change("aspm", "0000:02:00.0", "", "2")), "PCI 0000:02:00.0\n  ASPM = L1");
        assert_eq!(
            describe_change(&change("tunable", "/sys/bus/usb/devices/1-2/power/wakeup", "/x", "disabled")),
            "USB 1-2\n  /x = disabled"
        );
        assert_eq!(describe_change(&change("tunable", "/proc/sys/vm/x", "/proc/sys/vm/x", "0")), "System tunable\n  /proc/sys/vm/x = 0");
    }
}
