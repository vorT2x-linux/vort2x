// SPDX-License-Identifier: GPL-3.0-or-later
//! VorT2x Dashboard: one window with a sidebar page per installed VorT2x part.

mod machine;
mod modules;
mod style;

use std::{path::Path, rc::Rc};

use adw::prelude::*;
use gtk4::{
    gdk_pixbuf::Pixbuf, gio, glib, Align, Box as GtkBox, DrawingArea, Image, Label, ListBox,
    ListBoxRow, MenuButton, Orientation, PolicyType, ScrolledWindow, SelectionMode, Stack,
};
use modules::Module;

const APP_ID: &str = "org.vort2x.Dashboard";
const WORDMARK_DARK: &str = "/usr/local/share/vort2x/vort2x-wordmark.png";
const WORDMARK_LIGHT: &str = "/usr/local/share/vort2x/vort2x-wordmark-light.png";

fn main() -> glib::ExitCode {
    gio::resources_register_include!("vort2x-dashboard.gresource")
        .expect("the icons are built into the binary");
    let app = adw::Application::builder().application_id(APP_ID).build();
    app.connect_activate(build_ui);
    let about = gio::ActionEntry::builder("about")
        .activate(|app: &adw::Application, _, _| show_about(app))
        .build();
    app.add_action_entries([about]);
    app.run()
}

fn build_ui(app: &adw::Application) {
    // A second launch brings the open window forward.
    if let Some(window) = app.active_window() {
        window.present();
        return;
    }
    let window = adw::ApplicationWindow::builder()
        .application(app)
        .title("VorT2x Dashboard")
        .default_width(1180)
        .default_height(780)
        .build();
    window.add_css_class("vort2x-dashboard");
    style::install();

    let modules: Rc<[&'static Module]> = modules::installed(Path::new("/")).into();
    if modules.is_empty() {
        window.set_content(Some(&empty_state()));
    } else {
        window.set_content(Some(&split_view(&window, modules)));
    }
    window.present();
}

fn split_view(window: &adw::ApplicationWindow, modules: Rc<[&'static Module]>) -> adw::NavigationSplitView {
    let list = ListBox::new();
    list.add_css_class("vort2x-nav");
    list.set_selection_mode(SelectionMode::Single);
    for module in modules.iter() {
        list.append(&sidebar_row(module));
    }

    let sidebar_header = adw::HeaderBar::new();
    // The wordmark sits centred between the tile and the menu button.
    sidebar_header.set_title_widget(Some(&brand()));
    sidebar_header.pack_start(&app_tile());
    let menu = gio::Menu::new();
    menu.append(Some("About VorT2x Dashboard"), Some("app.about"));
    sidebar_header.pack_end(
        &MenuButton::builder()
            .icon_name("open-menu-symbolic")
            .menu_model(&menu)
            .tooltip_text("Main menu")
            .build(),
    );
    let sidebar = adw::ToolbarView::new();
    sidebar.add_top_bar(&sidebar_header);
    sidebar.set_content(Some(
        &ScrolledWindow::builder()
            .child(&list)
            .hscrollbar_policy(PolicyType::Never)
            .vexpand(true)
            .build(),
    ));
    sidebar.add_bottom_bar(&machine_card());

    // Each page brings its own header bar. The stack sits in a rounded card
    // that floats on the window background.
    let stack = Stack::new();
    stack.set_transition_type(gtk4::StackTransitionType::Crossfade);
    let card = GtkBox::new(Orientation::Vertical, 0);
    card.add_css_class("vort2x-content");
    card.set_overflow(gtk4::Overflow::Hidden);
    stack.set_vexpand(true);
    card.append(&stack);
    let content = adw::NavigationPage::new(&card, modules[0].title);
    let split = adw::NavigationSplitView::builder()
        .sidebar(&adw::NavigationPage::new(&sidebar, "VorT2x"))
        .content(&content)
        .min_sidebar_width(230.0)
        .build();

    list.connect_row_selected(glib::clone!(
        #[weak]
        split,
        move |_, row| {
            let Some(module) = row.and_then(|row| modules.get(row.index() as usize)) else {
                return;
            };
            show_page(&stack, module);
            content.set_title(module.title);
            split.set_show_content(true);
        }
    ));
    list.select_row(list.row_at_index(0).as_ref());

    // Narrow windows show one pane at a time.
    let narrow = adw::Breakpoint::new(adw::BreakpointCondition::new_length(
        adw::BreakpointConditionLengthType::MaxWidth,
        640.0,
        adw::LengthUnit::Sp,
    ));
    narrow.add_setter(&split, "collapsed", Some(&true.to_value()));
    window.add_breakpoint(narrow);
    split
}

fn sidebar_row(module: &Module) -> ListBoxRow {
    let row_box = GtkBox::new(Orientation::Horizontal, 14);
    row_box.append(&Image::from_icon_name(module.icon));
    let label = Label::new(Some(module.title));
    label.set_halign(Align::Start);
    row_box.append(&label);
    ListBoxRow::builder().child(&row_box).build()
}

/// Builds a page the first time it is opened, so starting the dashboard
/// neither polls the fans nor contacts GitHub until those pages are shown.
fn show_page(stack: &Stack, module: &Module) {
    if stack.child_by_name(module.id).is_none() {
        let page = (module.build)();
        let header = adw::HeaderBar::new();
        header.set_title_widget(Some(&page.title));
        for widget in &page.header_end {
            header.pack_end(widget);
        }
        let view = adw::ToolbarView::new();
        view.add_top_bar(&header);
        view.set_content(Some(&page.content));
        stack.add_named(&view, Some(module.id));
    }
    stack.set_visible_child_name(module.id);
}

/// The app icon in a rounded tile, top left.
fn app_tile() -> GtkBox {
    let icon = Image::from_icon_name(APP_ID);
    icon.set_pixel_size(26);
    let tile = GtkBox::new(Orientation::Horizontal, 0);
    tile.add_css_class("vort2x-tile");
    tile.set_margin_start(6);
    tile.append(&icon);
    tile
}

/// The Mac and system, at the bottom of the sidebar.
fn machine_card() -> GtkBox {
    let card = GtkBox::new(Orientation::Vertical, 2);
    card.add_css_class("vort2x-machine");
    for (text, class) in [(machine::model(), "title"), (machine::system(), "subtitle")] {
        let label = Label::new(Some(&text));
        label.set_xalign(0.0);
        label.set_ellipsize(gtk4::pango::EllipsizeMode::End);
        label.add_css_class(class);
        card.append(&label);
    }
    card
}

fn empty_state() -> adw::ToolbarView {
    let status = adw::StatusPage::builder()
        .icon_name(APP_ID)
        .title("Nothing to show yet")
        .description("None of the VorT2x parts with a dashboard page is installed. Run the VorT2x installer, then open the dashboard again.")
        .build();
    let view = adw::ToolbarView::new();
    view.add_top_bar(&adw::HeaderBar::new());
    view.set_content(Some(&status));
    view
}

/// The VorT2x wordmark in the tone of the current theme: white type on dark,
/// dark type on light. Falls back to plain text if the artwork is missing.
fn brand() -> gtk4::Widget {
    let Ok(dark) = Pixbuf::from_file(WORDMARK_DARK) else {
        return Label::new(Some("VorT2x")).upcast();
    };
    let light = Pixbuf::from_file(WORDMARK_LIGHT).unwrap_or_else(|_| dark.clone());
    let brand = DrawingArea::new();
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
    brand.upcast()
}

fn show_about(app: &adw::Application) {
    let about = adw::AboutWindow::builder()
        .application_name("VorT2x Dashboard")
        .application_icon(APP_ID)
        .version(env!("CARGO_PKG_VERSION"))
        .developer_name("VorT2x contributors")
        .copyright("© 2026 VorT2x contributors")
        .license_type(gtk4::License::Gpl30)
        .website("https://vort2x.org")
        .issue_url("https://github.com/vorT2x-linux/vort2x/issues")
        .build();
    if let Some(window) = app.active_window() {
        about.set_transient_for(Some(&window));
    }
    // GPL-3.0 section 7(b): the original authors' attribution is kept in
    // the legal notices of every work that contains their code.
    about.add_legal_section(
        "Original authors",
        Some("© 2026 André Eikmeyer\n© 2026 4f1sh3r"),
        gtk4::License::Gpl30,
        None,
    );
    about.present();
}
