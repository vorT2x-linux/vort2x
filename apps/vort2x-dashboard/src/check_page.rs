// SPDX-License-Identifier: GPL-3.0-or-later
//! The "Check my Mac" page: the self-test results and the report buttons.

use std::{cell::RefCell, rc::Rc};

use adw::prelude::*;
use gtk4::{gio, glib, Align, Box as GtkBox, Button, Image, Label, ListBox, Orientation, SelectionMode};

use crate::checks::{self, About, Check, Readings, Status};

/// The last results, ready to send or copy.
#[derive(Default)]
struct Results {
    report: String,
    url: String,
}

struct Widgets {
    about: ListBox,
    parts: ListBox,
    send: Button,
    copy: Button,
    refresh: Button,
    message: Label,
    results: RefCell<Results>,
}

/// The page content and its "Check again" header button.
pub fn build() -> (GtkBox, Button) {
    let root = GtkBox::new(Orientation::Vertical, 18);
    root.set_margin_top(24);
    root.set_margin_bottom(24);
    root.set_margin_start(18);
    root.set_margin_end(18);

    root.append(&paragraph(
        "VorT2x looks for each T2 part on this Mac. Nothing leaves the Mac unless you send the report.",
    ));
    let about = boxed_list();
    root.append(&heading("This Mac"));
    root.append(&about);
    let parts = boxed_list();
    root.append(&heading("Parts"));
    root.append(&parts);

    root.append(&heading("Send a report"));
    root.append(&paragraph(
        "Send report opens a new issue on GitHub with the lines above filled in. Add what only you \
         can tell, such as whether sound plays, then submit it. The report holds nothing else: no \
         serial number, host name or user name. Reports show which Macs VorT2x works on.",
    ));
    let send = Button::with_label("Send report");
    send.add_css_class("suggested-action");
    send.add_css_class("pill");
    let copy = Button::with_label("Copy report");
    copy.add_css_class("pill");
    let buttons = GtkBox::new(Orientation::Horizontal, 12);
    buttons.set_halign(Align::Start);
    buttons.append(&send);
    buttons.append(&copy);
    root.append(&buttons);
    let message = paragraph("");
    message.set_visible(false);
    root.append(&message);

    let refresh = Button::builder()
        .icon_name("view-refresh-symbolic")
        .tooltip_text("Check again")
        .build();

    let widgets = Rc::new(Widgets {
        about,
        parts,
        send: send.clone(),
        copy: copy.clone(),
        refresh: refresh.clone(),
        message,
        results: RefCell::default(),
    });
    send.connect_clicked(glib::clone!(
        #[strong]
        widgets,
        move |button| open_issue(button, &widgets)
    ));
    copy.connect_clicked(glib::clone!(
        #[strong]
        widgets,
        move |button| copy_report(button, &widgets)
    ));
    refresh.connect_clicked(glib::clone!(
        #[strong]
        widgets,
        move |_| run(&widgets)
    ));
    run(&widgets);
    (root, refresh)
}

/// Reads off the main thread: `journalctl` and `git` can take a moment.
fn run(widgets: &Rc<Widgets>) {
    for button in [&widgets.send, &widgets.copy, &widgets.refresh] {
        button.set_sensitive(false);
    }
    widgets.message.set_visible(false);
    let widgets = widgets.clone();
    glib::spawn_future_local(async move {
        let Ok((about, checks)) =
            gio::spawn_blocking(|| (About::read(), checks::evaluate(&Readings::read()))).await
        else {
            show_message(&widgets, "The checks stopped unexpectedly. Try Check again.");
            widgets.refresh.set_sensitive(true);
            return;
        };
        show(&widgets, &about, &checks);
    });
}

fn show(widgets: &Widgets, about: &About, results: &[Check]) {
    widgets.about.remove_all();
    let desktop = if about.desktop.is_empty() { "unknown" } else { &about.desktop };
    for (title, value) in [
        ("Model", format!("{} ({})", crate::machine::model_name(&about.model_id), about.model_id)),
        ("System", about.system.clone()),
        ("Kernel", about.kernel.clone()),
        ("VorT2x", about.release.clone()),
        ("Desktop", desktop.to_owned()),
    ] {
        let row = adw::ActionRow::builder().title(title).build();
        let label = Label::new(Some(&value));
        label.add_css_class("dim-label");
        label.set_selectable(true);
        row.add_suffix(&label);
        widgets.about.append(&row);
    }

    widgets.parts.remove_all();
    for check in results {
        widgets.parts.append(&check_row(check));
    }

    let report = checks::report(about, results);
    let url = checks::issue_url(about, &report);
    *widgets.results.borrow_mut() = Results { report, url };
    for button in [&widgets.send, &widgets.copy, &widgets.refresh] {
        button.set_sensitive(true);
    }
}

fn check_row(check: &Check) -> adw::ActionRow {
    let row = adw::ActionRow::builder().title(check.name).build();
    if !check.detail.is_empty() {
        row.set_subtitle(&check.detail);
    }
    let (icon, class) = match check.status {
        Status::Works => ("emblem-ok-symbolic", "success"),
        Status::Problem => ("dialog-warning-symbolic", "error"),
        Status::Absent => ("action-unavailable-symbolic", "dim-label"),
        Status::Untried => ("content-loading-symbolic", "dim-label"),
    };
    let image = Image::from_icon_name(icon);
    image.add_css_class(class);
    row.add_prefix(&image);
    let word = Label::new(Some(check.status.word()));
    word.add_css_class(class);
    row.add_suffix(&word);
    row
}

fn open_issue(button: &Button, widgets: &Rc<Widgets>) {
    let url = widgets.results.borrow().url.clone();
    let window = button.root().and_downcast::<gtk4::Window>();
    let widgets = widgets.clone();
    gtk4::UriLauncher::new(&url).launch(window.as_ref(), None::<&gio::Cancellable>, move |result| {
        if let Err(error) = result {
            show_message(
                &widgets,
                &format!("Couldn't open the browser ({error}). Use Copy report and paste it into a new issue instead."),
            );
        }
    });
}

fn copy_report(button: &Button, widgets: &Widgets) {
    button.clipboard().set_text(&widgets.results.borrow().report);
    button.set_label("Copied");
    glib::timeout_add_seconds_local_once(
        2,
        glib::clone!(
            #[weak]
            button,
            move || button.set_label("Copy report")
        ),
    );
}

fn show_message(widgets: &Widgets, text: &str) {
    widgets.message.set_label(text);
    widgets.message.set_visible(true);
}

fn boxed_list() -> ListBox {
    let list = ListBox::new();
    list.add_css_class("boxed-list");
    list.set_selection_mode(SelectionMode::None);
    list
}

fn heading(text: &str) -> Label {
    let label = Label::new(Some(text));
    label.add_css_class("heading");
    label.set_halign(Align::Start);
    label
}

fn paragraph(text: &str) -> Label {
    let label = Label::new(Some(text));
    label.set_wrap(true);
    label.set_xalign(0.0);
    label
}
