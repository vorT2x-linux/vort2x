// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 André Eikmeyer <andre.eikmeyer@kait2en.org>

use std::{
    fs::{File, OpenOptions},
    os::{
        fd::{AsFd, AsRawFd, OwnedFd},
        unix::fs::OpenOptionsExt,
    },
    path::Path,
    sync::{
        Arc, OnceLock,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
};

use anyhow::{Context, Result, anyhow};
use input::{
    Libinput, LibinputInterface,
    event::{
        Event, EventTrait,
        keyboard::{KeyState, KeyboardEvent, KeyboardEventTrait},
    },
};
use input_linux::Key;
use libc::{O_ACCMODE, O_RDONLY, O_RDWR, O_WRONLY, pollfd};

struct Interface;

impl LibinputInterface for Interface {
    fn open_restricted(&mut self, path: &Path, flags: i32) -> std::result::Result<OwnedFd, i32> {
        let mode = flags & O_ACCMODE;
        OpenOptions::new()
            .custom_flags(flags)
            .read(mode == O_RDONLY || mode == O_RDWR)
            .write(mode == O_WRONLY || mode == O_RDWR)
            .open(path)
            .map(Into::into)
            .map_err(|error| error.raw_os_error().unwrap_or(libc::EIO))
    }

    fn close_restricted(&mut self, fd: OwnedFd) {
        drop(File::from(fd));
    }
}

/// Whether a match window is open. The keyboard thread keeps libinput
/// suspended outside it, so it doesn't wake for every mouse and trackpad
/// event all day; setting it wakes the thread.
#[derive(Default)]
pub struct Verifying {
    active: AtomicBool,
    watcher: OnceLock<thread::Thread>,
}

impl Verifying {
    pub fn set(&self, active: bool) {
        self.active.store(active, Ordering::Release);
        if let Some(watcher) = self.watcher.get() {
            watcher.unpark();
        }
    }

    fn get(&self) -> bool {
        self.active.load(Ordering::Acquire)
    }
}

pub fn watch_escape(verifying: Arc<Verifying>, cancel_requested: Arc<AtomicBool>) -> Result<()> {
    let (ready_sender, ready_receiver) = mpsc::sync_channel(1);
    let watched = Arc::clone(&verifying);
    let handle = thread::Builder::new()
        .name("touch-id-keyboard".to_owned())
        .spawn(move || {
            let mut input = Libinput::new_with_udev(Interface);
            if input.udev_assign_seat("seat0").is_err() {
                let _ = ready_sender.send(Err("assign libinput seat0"));
                return;
            }
            if ready_sender.send(Ok(())).is_err() {
                return;
            }
            if let Err(error) = listen(&mut input, &verifying, &cancel_requested) {
                eprintln!("t2-touchid: keyboard monitor stopped: {error:#}");
            }
        })?;
    let _ = watched.watcher.set(handle.thread().clone());
    ready_receiver
        .recv()
        .context("start keyboard monitor")?
        .map_err(anyhow::Error::msg)
}

/// How long poll waits for input during a match window before checking
/// whether the window has closed.
const WINDOW_POLL_MS: i32 = 500;

fn listen(
    input: &mut Libinput,
    verifying: &Verifying,
    cancel_requested: &AtomicBool,
) -> Result<()> {
    loop {
        if !verifying.get() {
            input.suspend();
            // unpark() from Verifying::set ends the wait; a spurious wake
            // just checks again
            while !verifying.get() {
                thread::park();
            }
            input
                .resume()
                .map_err(|()| anyhow!("resume keyboard input"))?;
        }
        let mut descriptor = pollfd {
            fd: input.as_fd().as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        let result = unsafe { libc::poll(&mut descriptor, 1, WINDOW_POLL_MS) };
        if result < 0 {
            let error = std::io::Error::last_os_error();
            if error.kind() == std::io::ErrorKind::Interrupted {
                continue;
            }
            return Err(error).context("wait for keyboard input");
        }
        input
            .dispatch()
            .map_err(|error| anyhow!("dispatch keyboard input: {error}"))?;
        for event in &mut *input {
            if let Event::Keyboard(KeyboardEvent::Key(key)) = event
                && key.key() == Key::Esc as u32
                && key.key_state() == KeyState::Pressed
                && verifying.get()
            {
                eprintln!(
                    "t2-touchid: Escape observed during verification from {}",
                    key.device().name()
                );
                cancel_requested.store(true, Ordering::Release);
            }
        }
    }
}
