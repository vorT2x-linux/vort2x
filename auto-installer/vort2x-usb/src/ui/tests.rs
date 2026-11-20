// SPDX-License-Identifier: GPL-3.0-or-later

use std::path::PathBuf;

use clap::Parser;
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::*;
use crate::kit::{Kit, parse_catalog, parse_target};
use crate::system::CallingUser;

fn setup() -> Setup {
    let target = parse_target(include_str!("../../../targets/fedora-45.conf")).unwrap();
    let editions = parse_catalog(
        include_str!("../../../targets/fedora-45-editions.tsv"),
        "45",
    )
    .unwrap();
    Setup {
        kit: Kit {
            target,
            editions,
            grub_template: String::new(),
            input_initramfs: PathBuf::new(),
        },
        user: CallingUser {
            name: "kirk".into(),
            uid: 501,
            gid: 20,
            home: "/Users/kirk".into(),
        },
        download_dir: "/Users/kirk/Downloads".into(),
        firmware: crate::job::FirmwareSource::ThisMac,
        mac_checks: vec![
            (true, "Secure Boot is disabled.".into()),
            (false, "No Touch ID finger is enrolled.".into()),
        ],
        iso: None,
    }
}

fn args() -> Args {
    Args::parse_from(["vort2x-usb", "--target-file", "/tmp/stick.img"])
}

fn press(app: &mut App, code: KeyCode) {
    app.key(KeyEvent::new(code, KeyModifiers::NONE), None);
}

fn drive() -> Drive {
    Drive {
        path: "/dev/disk4".into(),
        raw_path: "/dev/rdisk4".into(),
        model: "SanDisk Ultra".into(),
        size: 32_017_047_552,
    }
}

fn prepared() -> Prepared {
    Prepared {
        edition: setup().kit.editions[0].clone(),
        iso: "/Users/kirk/Downloads/a.iso".into(),
        iso_size: 3_256_915_968,
        esp_offset: 0,
        esp: Vec::new(),
        firmware_names: vec![
            "ekans.trx".into(),
            "P-ekans-X3_M-HRPN_V-u__m-7.5.txt".into(),
        ],
        warnings: vec!["No Bluetooth PTB file was found; Bluetooth may stay off".into()],
    }
}

fn rendered(app: &mut App, width: u16, height: u16) -> String {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal.draw(|frame| view::draw(frame, app)).unwrap();
    let buffer = terminal.backend().buffer();
    buffer.content.iter().map(|cell| cell.symbol()).collect()
}

#[test]
fn every_screen_renders_at_small_and_large_sizes() {
    let setup = setup();
    let screens = [
        Screen::Welcome,
        Screen::Edition,
        Screen::Drive,
        Screen::Preparing,
        Screen::Confirm,
        Screen::Writing,
        Screen::Done,
        Screen::Failed {
            message: "the Fedora download failed".into(),
            drive_changed: false,
        },
    ];
    for (width, height) in [(80, 24), (120, 40), (40, 12)] {
        for screen in &screens {
            let mut app = App::new(&setup, &args()).unwrap();
            app.screen = screen.clone();
            app.drives = vec![drive()];
            app.drive_list.select(Some(0));
            app.target = Some(Target::Drive(drive()));
            app.prepared = Some(prepared());
            app.stage = "Downloading Fedora".into();
            app.progress = (1_000_000_000, 3_256_915_968);
            app.notes = vec!["Source 1: mirror.example".into()];
            app.hint = Some("a hint".into());
            let text = rendered(&mut app, width, height);
            assert!(!text.trim().is_empty(), "{screen:?} at {width}x{height}");
        }
    }
}

#[test]
fn screens_show_what_matters() {
    let setup = setup();
    let mut app = App::new(&setup, &args()).unwrap();
    assert!(rendered(&mut app, 100, 30).contains("Secure Boot is disabled."));

    app.screen = Screen::Edition;
    let text = rendered(&mut app, 100, 30);
    assert!(text.contains("Fedora COSMIC Spin"));
    assert!(text.contains("recommended"));

    app.screen = Screen::Confirm;
    app.target = Some(Target::Drive(drive()));
    app.prepared = Some(prepared());
    let text = rendered(&mut app, 100, 30);
    assert!(text.contains("ERASE disk4"));
    assert!(text.contains("Bluetooth may stay off"));
}

#[test]
fn edition_menu_starts_on_the_default_and_moves() {
    let setup = setup();
    let mut app = App::new(&setup, &args()).unwrap();
    press(&mut app, KeyCode::Enter);
    assert_eq!(app.screen, Screen::Edition);
    assert_eq!(app.editions.selected(), Some(setup.kit.default_index()));
    for _ in 0..10 {
        press(&mut app, KeyCode::Down);
    }
    assert_eq!(app.editions.selected(), Some(setup.kit.editions.len() - 1));
    press(&mut app, KeyCode::Esc);
    assert_eq!(app.screen, Screen::Welcome);
}

#[test]
fn only_the_exact_phrase_confirms() {
    let setup = setup();
    let mut app = App::new(&setup, &args()).unwrap();
    app.target = Some(Target::Drive(drive()));
    app.prepared = Some(prepared());
    app.screen = Screen::Confirm;
    for c in "erase disk4".chars() {
        press(&mut app, KeyCode::Char(c));
    }
    assert!(!app.confirmed());
    press(&mut app, KeyCode::Enter);
    assert_eq!(app.screen, Screen::Confirm);
    assert!(
        app.hint
            .as_deref()
            .is_some_and(|h| h.contains("ERASE disk4"))
    );

    app.typed.clear();
    for c in "ERASE disk44".chars() {
        press(&mut app, KeyCode::Char(c));
    }
    press(&mut app, KeyCode::Backspace);
    assert!(app.confirmed());
    press(&mut app, KeyCode::Enter);
    assert_eq!(app.screen, Screen::Writing);
}

#[test]
fn writing_cannot_be_interrupted() {
    let setup = setup();
    let mut app = App::new(&setup, &args()).unwrap();
    app.screen = Screen::Writing;
    app.key(
        KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL),
        None,
    );
    press(&mut app, KeyCode::Char('q'));
    assert!(!app.quit);
    assert_eq!(app.screen, Screen::Writing);
}

#[test]
fn a_file_target_needs_no_confirmation() {
    let setup = setup();
    let mut app = App::new(&setup, &args()).unwrap();
    app.prepared = Some(prepared());
    app.apply(Event::Note("x".into()), None);
    app.ready_to_confirm(None);
    assert_eq!(app.screen, Screen::Writing);
}

#[test]
fn a_too_small_drive_goes_back_to_the_list() {
    let setup = setup();
    let mut app = App::new(&setup, &args()).unwrap();
    let mut small = drive();
    small.size = 2_000_000_000;
    app.target = Some(Target::Drive(small));
    app.prepared = Some(prepared());
    app.ready_to_confirm(None);
    assert_eq!(app.screen, Screen::Drive);
    assert!(app.hint.as_deref().is_some_and(|h| h.contains("smaller")));
}
