// SPDX-License-Identifier: GPL-3.0-or-later

//! The full-screen interface. `App` holds what is on screen and reacts to
//! keys and to events from the worker thread; `view` draws it. The slow work
//! (download, writing) runs on a scoped thread so the screen stays live.

mod view;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread::Scope;
use std::time::Duration;

use anyhow::Result;
use ratatui::crossterm::event::{
    self, Event as TermEvent, KeyCode, KeyEvent, KeyEventKind, KeyModifiers,
};
use ratatui::widgets::ListState;

use crate::drives::{self, Drive};
use crate::job::{self, IsoChoice, Prepared, Target};
use crate::report::Report;
use crate::{Args, Setup};

pub const NEXT_STEPS: [&str; 4] = [
    "Restart the T2 Mac and hold the Option key while it starts.",
    "Choose the orange \"EFI Boot\" entry of the USB stick.",
    "Install Fedora, remove the stick, and start the installed system.",
    "Sign in and connect to the internet; the VorT2x setup continues in a terminal.",
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Screen {
    Welcome,
    Edition,
    Drive,
    Preparing,
    Confirm,
    Writing,
    Done,
    Failed {
        message: String,
        drive_changed: bool,
    },
}

/// What the worker thread reports.
enum Event {
    Stage(String),
    Progress(u64, u64),
    Note(String),
    Prepared(Box<Result<Prepared>>),
    Burned(Result<()>),
}

struct ChannelReport(Sender<Event>);

impl Report for ChannelReport {
    fn stage(&self, title: &str) {
        let _ = self.0.send(Event::Stage(title.to_owned()));
    }
    fn progress(&self, done: u64, total: u64) {
        let _ = self.0.send(Event::Progress(done, total));
    }
    fn note(&self, text: &str) {
        let _ = self.0.send(Event::Note(text.to_owned()));
    }
}

pub struct App<'a> {
    pub setup: &'a Setup,
    pub screen: Screen,
    pub editions: ListState,
    pub drives: Vec<Drive>,
    pub drive_list: ListState,
    /// The drive or file the stick goes to, once chosen.
    pub target: Option<Target>,
    /// Set with --disk or --target-file: no drive menu.
    fixed_target: bool,
    pub typed: String,
    pub hint: Option<String>,
    pub stage: String,
    pub progress: (u64, u64),
    pub notes: Vec<String>,
    pub prepared: Option<Prepared>,
    pub quit: bool,
    worker: Option<Receiver<Event>>,
    cancel: Arc<AtomicBool>,
}

impl<'a> App<'a> {
    pub fn new(setup: &'a Setup, args: &Args) -> Result<Self> {
        let target = match (&args.target_file, &args.disk) {
            (Some(file), _) => Some(Target::File(file.clone())),
            (None, Some(disk)) => Some(Target::Drive(drives::find(disk)?)),
            _ => None,
        };
        Ok(Self {
            setup,
            screen: Screen::Welcome,
            editions: ListState::default().with_selected(Some(setup.kit.default_index())),
            drives: Vec::new(),
            drive_list: ListState::default(),
            fixed_target: target.is_some(),
            target,
            typed: String::new(),
            hint: None,
            stage: String::new(),
            progress: (0, 0),
            notes: Vec::new(),
            prepared: None,
            quit: false,
            worker: None,
            cancel: Arc::new(AtomicBool::new(false)),
        })
    }

    pub fn iso_choice(&self) -> IsoChoice {
        self.setup
            .iso
            .clone()
            .unwrap_or_else(|| IsoChoice::Edition(self.editions.selected().unwrap_or(0)))
    }

    pub fn target_label(&self) -> String {
        match &self.target {
            Some(Target::Drive(drive)) => format!(
                "{} ({}, {})",
                drive.path,
                drive.model,
                crate::report::gigabytes(drive.size)
            ),
            Some(Target::File(path)) => format!("file {}", path.display()),
            None => "not chosen yet".into(),
        }
    }

    /// One key press. `scope` starts the worker when a step needs it.
    pub fn key<'s>(&mut self, key: KeyEvent, scope: Option<&'s Scope<'s, '_>>)
    where
        'a: 's,
    {
        if key.kind != KeyEventKind::Press {
            return;
        }
        let ctrl_c =
            key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c');
        self.hint = None;
        match &self.screen {
            Screen::Writing => {
                if ctrl_c {
                    self.hint =
                        Some("The stick is being written; please wait until it is done.".into());
                }
            }
            Screen::Preparing => {
                if ctrl_c || matches!(key.code, KeyCode::Esc | KeyCode::Char('q')) {
                    self.cancel.store(true, Ordering::Relaxed);
                    self.hint = Some("Stopping...".into());
                }
            }
            Screen::Confirm => self.confirm_key(key, ctrl_c, scope),
            _ if ctrl_c => self.quit = true,
            Screen::Welcome => match key.code {
                KeyCode::Enter => self.after_welcome(),
                KeyCode::Esc | KeyCode::Char('q') => self.quit = true,
                _ => {}
            },
            Screen::Edition => match key.code {
                KeyCode::Up | KeyCode::Char('k') => self.editions.select_previous(),
                KeyCode::Down | KeyCode::Char('j') => {
                    let last = self.setup.kit.editions.len() - 1;
                    self.editions
                        .select(Some((self.editions.selected().unwrap_or(0) + 1).min(last)));
                }
                KeyCode::Enter => self.after_edition(scope),
                KeyCode::Esc => self.screen = Screen::Welcome,
                KeyCode::Char('q') => self.quit = true,
                _ => {}
            },
            Screen::Drive => match key.code {
                KeyCode::Up | KeyCode::Char('k') => self.drive_list.select_previous(),
                KeyCode::Down | KeyCode::Char('j') if !self.drives.is_empty() => {
                    let next = self.drive_list.selected().map_or(0, |i| i + 1);
                    self.drive_list
                        .select(Some(next.min(self.drives.len() - 1)));
                }
                KeyCode::Char('r') => self.refresh_drives(),
                KeyCode::Enter => match self.drive_list.selected().and_then(|i| self.drives.get(i))
                {
                    Some(drive) => {
                        self.target = Some(Target::Drive(drive.clone()));
                        self.after_drive(scope);
                    }
                    None => self.hint = Some("Connect a USB drive, then press r.".into()),
                },
                KeyCode::Esc => {
                    self.screen = if self.setup.iso.is_some() {
                        Screen::Welcome
                    } else {
                        Screen::Edition
                    }
                }
                KeyCode::Char('q') => self.quit = true,
                _ => {}
            },
            Screen::Done | Screen::Failed { .. } => {
                if matches!(key.code, KeyCode::Enter | KeyCode::Esc | KeyCode::Char('q')) {
                    self.quit = true;
                }
            }
        }
    }

    fn after_welcome(&mut self) {
        if self.setup.iso.is_none() {
            self.screen = Screen::Edition;
        } else {
            self.after_edition(None);
        }
    }

    fn after_edition<'s>(&mut self, scope: Option<&'s Scope<'s, '_>>)
    where
        'a: 's,
    {
        // Another edition means another ISO; prepare again.
        self.prepared = None;
        if self.fixed_target {
            self.after_drive(scope);
        } else {
            self.refresh_drives();
            self.screen = Screen::Drive;
        }
    }

    fn after_drive<'s>(&mut self, scope: Option<&'s Scope<'s, '_>>)
    where
        'a: 's,
    {
        if self.prepared.is_some() {
            self.ready_to_confirm(scope);
            return;
        }
        self.screen = Screen::Preparing;
        self.notes.clear();
        let Some(scope) = scope else { return };
        let (sender, receiver) = channel();
        self.worker = Some(receiver);
        let setup = self.setup;
        let iso = self.iso_choice();
        let cancel = Arc::clone(&self.cancel);
        let firmware = setup.firmware.clone();
        scope.spawn(move || {
            let report = ChannelReport(sender.clone());
            let result = job::prepare(
                &setup.kit,
                &iso,
                &setup.download_dir,
                &firmware,
                &setup.user,
                &cancel,
                &report,
            );
            let _ = sender.send(Event::Prepared(Box::new(result)));
        });
    }

    fn ready_to_confirm<'s>(&mut self, scope: Option<&'s Scope<'s, '_>>)
    where
        'a: 's,
    {
        let (Some(prepared), Some(target)) = (&self.prepared, &self.target) else {
            return;
        };
        match target {
            Target::Drive(drive) => {
                if let Err(error) = job::fits(drive, prepared.iso_size) {
                    self.hint = Some(format!("{error:#}"));
                    self.screen = Screen::Drive;
                    return;
                }
                self.typed.clear();
                self.screen = Screen::Confirm;
            }
            // A file erases nothing: no confirmation needed.
            Target::File(_) => self.start_burn(scope),
        }
    }

    fn confirm_key<'s>(&mut self, key: KeyEvent, ctrl_c: bool, scope: Option<&'s Scope<'s, '_>>)
    where
        'a: 's,
    {
        if ctrl_c {
            self.quit = true;
            return;
        }
        match key.code {
            KeyCode::Char(c) if self.typed.len() < 40 => self.typed.push(c),
            KeyCode::Backspace => {
                self.typed.pop();
            }
            KeyCode::Esc if self.fixed_target => self.quit = true,
            KeyCode::Esc => {
                self.refresh_drives();
                self.screen = Screen::Drive;
            }
            KeyCode::Enter => {
                if self.confirmed() {
                    self.start_burn(scope);
                } else {
                    self.hint = Some(format!("That did not match '{}'.", self.confirm_phrase()));
                }
            }
            _ => {}
        }
    }

    pub fn confirm_phrase(&self) -> String {
        match &self.target {
            Some(Target::Drive(drive)) => drive.confirm_phrase(),
            _ => String::new(),
        }
    }

    pub fn confirmed(&self) -> bool {
        matches!(self.target, Some(Target::Drive(_))) && self.typed == self.confirm_phrase()
    }

    fn start_burn<'s>(&mut self, scope: Option<&'s Scope<'s, '_>>)
    where
        'a: 's,
    {
        self.screen = Screen::Writing;
        self.notes.clear();
        let (Some(scope), Some(prepared), Some(target)) =
            (scope, self.prepared.take(), self.target.clone())
        else {
            return;
        };
        let (sender, receiver) = channel();
        self.worker = Some(receiver);
        scope.spawn(move || {
            let report = ChannelReport(sender.clone());
            let result = job::burn(&prepared, &target, &report);
            let _ = sender.send(Event::Burned(result));
        });
    }

    fn refresh_drives(&mut self) {
        match drives::list() {
            Ok(list) => {
                self.drives = list;
                self.drive_list
                    .select((!self.drives.is_empty()).then_some(0));
            }
            Err(error) => self.hint = Some(format!("Could not list drives: {error:#}")),
        }
    }

    /// Takes everything the worker sent since the last frame.
    fn drain_worker<'s>(&mut self, scope: Option<&'s Scope<'s, '_>>)
    where
        'a: 's,
    {
        let Some(worker) = &self.worker else { return };
        let events: Vec<Event> = worker.try_iter().collect();
        for event in events {
            self.apply(event, scope);
        }
    }

    fn apply<'s>(&mut self, event: Event, scope: Option<&'s Scope<'s, '_>>)
    where
        'a: 's,
    {
        match event {
            Event::Stage(stage) => {
                self.stage = stage;
                self.progress = (0, 0);
            }
            Event::Progress(done, total) => self.progress = (done, total),
            Event::Note(note) => self.notes.push(note),
            Event::Prepared(result) => {
                self.worker = None;
                match *result {
                    Ok(prepared) => {
                        self.prepared = Some(prepared);
                        self.ready_to_confirm(scope);
                    }
                    Err(_) if self.cancel.load(Ordering::Relaxed) => self.quit = true,
                    Err(error) => self.fail(error, false),
                }
            }
            Event::Burned(result) => {
                self.worker = None;
                match result {
                    Ok(()) => self.screen = Screen::Done,
                    Err(error) => self.fail(error, true),
                }
            }
        }
    }

    fn fail(&mut self, error: anyhow::Error, drive_changed: bool) {
        self.screen = Screen::Failed {
            message: format!("{error:#}"),
            drive_changed,
        };
    }
}

pub fn run(setup: &Setup, args: &Args) -> Result<()> {
    let mut app = App::new(setup, args)?;
    let mut terminal = ratatui::init();
    let outcome = std::thread::scope(|scope| -> Result<()> {
        while !app.quit {
            app.drain_worker(Some(scope));
            terminal.draw(|frame| view::draw(frame, &mut app))?;
            if event::poll(Duration::from_millis(100))?
                && let TermEvent::Key(key) = event::read()?
            {
                app.key(key, Some(scope));
            }
        }
        Ok(())
    });
    ratatui::restore();
    outcome?;
    // Leave the result on the normal screen too.
    match &app.screen {
        Screen::Done => {
            println!("The VorT2x Fedora USB stick is ready.");
            for step in NEXT_STEPS {
                println!("  - {step}");
            }
        }
        Screen::Failed {
            message,
            drive_changed,
        } => {
            eprintln!("Error: {message}");
            if !drive_changed {
                eprintln!("The USB drive was not changed.");
            }
            std::process::exit(1);
        }
        _ => {}
    }
    Ok(())
}

#[cfg(test)]
mod tests;
