// SPDX-License-Identifier: GPL-3.0-or-later

//! Drawing each screen. Colours are from the 256-colour palette, which
//! macOS Terminal shows correctly (it has no true colour).

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style, Stylize};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Gauge, List, ListItem, Paragraph, Wrap};

use super::{App, NEXT_STEPS, Screen};
use crate::job::{FirmwareSource, IsoChoice, Target};
use crate::report::gigabytes;

/// Close to the VorT2x accent `#38bdf8`.
const ACCENT: Color = Color::Indexed(39);
const MUTED: Color = Color::Indexed(245);

const LOGO: [&str; 2] = ["█ █ █▀█ █▀▄ ▀█▀ ▀▀█ ▀▄▀", "▀▄▀ █▄█ █▀▄  █  █▄▄ ▄▀▄"];

pub fn draw(frame: &mut Frame, app: &mut App) {
    let [header, body, footer] = Layout::vertical([
        Constraint::Length(4),
        Constraint::Min(6),
        Constraint::Length(2),
    ])
    .areas(frame.area());
    draw_header(frame, header, app);
    let block = Block::bordered()
        .title(Line::from(format!(" {} ", title(&app.screen))).bold())
        .border_style(Style::new().fg(MUTED))
        .padding(ratatui::widgets::Padding::horizontal(1));
    let inner = block.inner(body);
    frame.render_widget(block, body);
    match app.screen.clone() {
        Screen::Welcome => draw_welcome(frame, inner, app),
        Screen::Edition => draw_editions(frame, inner, app),
        Screen::Drive => draw_drives(frame, inner, app),
        Screen::Preparing | Screen::Writing => draw_progress(frame, inner, app),
        Screen::Confirm => draw_confirm(frame, inner, app),
        Screen::Done => draw_done(frame, inner, app),
        Screen::Failed {
            message,
            drive_changed,
        } => draw_failed(frame, inner, &message, drive_changed),
    }
    draw_footer(frame, footer, app);
}

fn title(screen: &Screen) -> &'static str {
    match screen {
        Screen::Welcome => "Welcome",
        Screen::Edition => "1. Choose a Fedora desktop",
        Screen::Drive => "2. Choose the USB drive",
        Screen::Preparing => "3. Getting everything ready",
        Screen::Confirm => "4. Erase the USB drive?",
        Screen::Writing => "5. Writing the stick",
        Screen::Done => "Done",
        Screen::Failed { .. } => "Something went wrong",
    }
}

fn draw_header(frame: &mut Frame, area: Rect, app: &App) {
    let accent = Style::new().fg(ACCENT).add_modifier(Modifier::BOLD);
    let plain = Style::new().add_modifier(Modifier::BOLD);
    // "VOR" plain, "T2" in the accent colour, "X" plain, like the wordmark.
    let logo_line = |line: &str| {
        let chars: Vec<char> = line.chars().collect();
        let part =
            |from: usize, to: usize| chars[from..to.min(chars.len())].iter().collect::<String>();
        Line::from(vec![
            Span::styled(part(0, 12), plain),
            Span::styled(part(12, 20), accent),
            Span::styled(part(20, chars.len()), plain),
        ])
    };
    let release = &app.setup.kit.target.fedora_release;
    let lines = vec![
        logo_line(LOGO[0]),
        logo_line(LOGO[1]),
        Line::from(vec![
            Span::raw("Fedora "),
            Span::raw(release.as_str()),
            Span::raw(" USB stick for T2 Macs").fg(MUTED),
        ]),
    ];
    frame.render_widget(
        Paragraph::new(lines),
        area.inner(ratatui::layout::Margin::new(1, 0)),
    );
}

fn draw_footer(frame: &mut Frame, area: Rect, app: &App) {
    let keys = match &app.screen {
        Screen::Welcome => "Enter continue   q quit",
        Screen::Edition => "↑↓ choose   Enter continue   Esc back   q quit",
        Screen::Drive => "↑↓ choose   Enter continue   r refresh   Esc back   q quit",
        Screen::Preparing => "Esc stop",
        Screen::Confirm => "type the words shown   Enter erase and write   Esc back",
        Screen::Writing => "please wait, do not unplug the drive",
        Screen::Done | Screen::Failed { .. } => "Enter quit",
    };
    let mut lines = vec![Line::from(keys).fg(MUTED)];
    if let Some(hint) = &app.hint {
        lines.insert(0, Line::from(hint.as_str()).fg(Color::Yellow));
    }
    frame.render_widget(
        Paragraph::new(lines),
        area.inner(ratatui::layout::Margin::new(1, 0)),
    );
}

fn good(text: &str) -> Line<'static> {
    Line::from(vec![
        Span::raw("✓ ").fg(Color::Green),
        Span::raw(text.to_owned()),
    ])
}

fn warning(text: &str) -> Line<'static> {
    Line::from(vec![
        Span::raw("! ").fg(Color::Yellow).bold(),
        Span::raw(text.to_owned()),
    ])
}

fn draw_welcome(frame: &mut Frame, area: Rect, app: &App) {
    let setup = app.setup;
    let mut text = vec![
        Line::from("This makes a USB stick that installs Fedora with VorT2x on a T2 Mac."),
        Line::from("The stick is erased. Nothing on this computer is changed.").fg(MUTED),
        Line::default(),
    ];
    text.push(good(&match &setup.firmware {
        FirmwareSource::ThisMac => "Wi-Fi firmware: taken from this Mac's macOS.".to_owned(),
        FirmwareSource::Dir(dir) => format!("Wi-Fi firmware: from {}", dir.display()),
    }));
    for (ok, check) in &setup.mac_checks {
        text.push(if *ok { good(check) } else { warning(check) });
    }
    if let Some(IsoChoice::File(path)) = &setup.iso {
        text.push(good(&format!("Fedora ISO: {}", path.display())));
    }
    text.push(Line::default());
    text.push(Line::from(crate::CREDIT).fg(MUTED));
    frame.render_widget(Paragraph::new(text).wrap(Wrap { trim: true }), area);
}

fn draw_editions(frame: &mut Frame, area: Rect, app: &mut App) {
    let kit = &app.setup.kit;
    let items: Vec<ListItem> = kit
        .editions
        .iter()
        .map(|edition| {
            let mut line = vec![Span::raw(edition.name.clone())];
            if edition.id == kit.target.default_edition {
                line.push(Span::raw("  recommended").fg(ACCENT));
            }
            line.push(Span::raw(format!("  {}", gigabytes(edition.size))).fg(MUTED));
            ListItem::new(Line::from(line))
        })
        .collect();
    let [intro, list] = Layout::vertical([Constraint::Length(2), Constraint::Min(1)]).areas(area);
    frame.render_widget(
        Paragraph::new(format!(
            "The ISO is downloaded to {} if it isn't there yet.",
            app.setup.download_dir.display()
        ))
        .fg(MUTED)
        .wrap(Wrap { trim: true }),
        intro,
    );
    frame.render_stateful_widget(selection_list(items), list, &mut app.editions);
}

fn draw_drives(frame: &mut Frame, area: Rect, app: &mut App) {
    let [intro, list] = Layout::vertical([Constraint::Length(2), Constraint::Min(1)]).areas(area);
    frame.render_widget(
        Paragraph::new(
            "Only external drives are shown. Everything on the chosen drive will be erased.",
        )
        .wrap(Wrap { trim: true }),
        intro,
    );
    if app.drives.is_empty() {
        frame.render_widget(
            Paragraph::new(
                "No external drive found. Connect a USB drive of at least 8 GB and press r.",
            )
            .fg(Color::Yellow),
            list,
        );
        return;
    }
    let items: Vec<ListItem> = app
        .drives
        .iter()
        .map(|drive| {
            ListItem::new(Line::from(vec![
                Span::raw(format!("{:<12}", drive.path)),
                Span::raw(format!("{:>9}  ", gigabytes(drive.size))).fg(MUTED),
                Span::raw(drive.model.clone()),
            ]))
        })
        .collect();
    frame.render_stateful_widget(selection_list(items), list, &mut app.drive_list);
}

fn selection_list(items: Vec<ListItem>) -> List {
    List::new(items)
        .highlight_symbol("› ")
        .highlight_style(Style::new().fg(ACCENT).add_modifier(Modifier::BOLD))
}

fn draw_progress(frame: &mut Frame, area: Rect, app: &App) {
    let [stage, gauge, _, notes] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(1),
    ])
    .areas(area);
    frame.render_widget(Paragraph::new(app.stage.as_str()).bold(), stage);
    let (done, total) = app.progress;
    let (ratio, label) = if total > 0 {
        let ratio = (done as f64 / total as f64).clamp(0.0, 1.0);
        let label = if total > 50_000_000 {
            format!(
                "{:.0} %  ·  {} of {}",
                ratio * 100.0,
                gigabytes(done),
                gigabytes(total)
            )
        } else {
            format!("{:.0} %", ratio * 100.0)
        };
        (ratio, label)
    } else {
        (0.0, "working...".into())
    };
    frame.render_widget(
        Gauge::default()
            .ratio(ratio)
            .label(label)
            .gauge_style(Style::new().fg(ACCENT).bg(Color::Indexed(236))),
        gauge,
    );
    let shown = notes.height as usize;
    let lines: Vec<Line> = app
        .notes
        .iter()
        .skip(app.notes.len().saturating_sub(shown))
        .map(|note| Line::from(note.as_str()).fg(MUTED))
        .collect();
    frame.render_widget(Paragraph::new(lines), notes);
}

fn draw_confirm(frame: &mut Frame, area: Rect, app: &App) {
    let Some(prepared) = &app.prepared else {
        return;
    };
    let mut text = vec![
        Line::from(vec![
            Span::raw("Fedora:   "),
            Span::raw(prepared.edition.name.clone()).bold(),
        ]),
        Line::from(vec![
            Span::raw("Drive:    "),
            Span::raw(app.target_label()).bold(),
        ]),
        Line::from(vec![
            Span::raw("Firmware: "),
            Span::raw(prepared.firmware_names.join(", ")).fg(MUTED),
        ]),
    ];
    for note in &prepared.warnings {
        text.push(warning(note));
    }
    let phrase = app.confirm_phrase();
    text.push(Line::default());
    text.push(
        Line::from(vec![
            Span::raw("Everything on "),
            Span::raw(app.target_label()).bold(),
            Span::raw(" will be erased."),
        ])
        .fg(Color::Yellow),
    );
    text.push(Line::from(vec![
        Span::raw("Type "),
        Span::raw(phrase.clone()).bold(),
        Span::raw(" to continue:"),
    ]));
    let typed_style = if app.confirmed() {
        Style::new().fg(Color::Green)
    } else {
        Style::new()
    };
    text.push(Line::from(vec![
        Span::raw("› ").fg(ACCENT),
        Span::styled(app.typed.clone(), typed_style),
        Span::raw("█").fg(ACCENT),
    ]));
    frame.render_widget(
        Paragraph::new(Text::from(text)).wrap(Wrap { trim: true }),
        area,
    );
}

fn draw_done(frame: &mut Frame, area: Rect, app: &App) {
    let mut text = vec![
        good("The VorT2x Fedora USB stick is ready and checked."),
        Line::default(),
        Line::from("Next:").bold(),
    ];
    for (number, step) in NEXT_STEPS.iter().enumerate() {
        text.push(Line::from(format!("{}. {step}", number + 1)));
    }
    if let Some(Target::File(path)) = &app.target {
        text.push(Line::default());
        text.push(Line::from(format!("Image written to {}", path.display())).fg(MUTED));
    }
    frame.render_widget(Paragraph::new(text).wrap(Wrap { trim: true }), area);
}

fn draw_failed(frame: &mut Frame, area: Rect, message: &str, drive_changed: bool) {
    let mut text = vec![Line::from(vec![
        Span::raw("✗ ").fg(Color::Red).bold(),
        Span::raw(message.to_owned()),
    ])];
    text.push(Line::default());
    text.push(if drive_changed {
        Line::from("The USB drive was partly written. Run vort2x-usb again to make it usable.")
            .fg(Color::Yellow)
    } else {
        Line::from("The USB drive was not changed.").fg(MUTED)
    });
    frame.render_widget(Paragraph::new(text).wrap(Wrap { trim: true }), area);
}
