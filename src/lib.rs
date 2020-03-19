#![deny(missing_docs)]

//! `brickatlas` watches the PoE log file. If the user enters a configured map a
//! alert notification is shown to not complete the map.

use notify::{self, DebouncedEvent, RecursiveMode, Watcher};
use notify_rust::{self, Notification, NotificationUrgency, Timeout};
use std::env;
use std::fmt;
use std::fs::File;
use std::io::prelude::*;
use std::io::{BufReader, SeekFrom};
use std::sync::mpsc;
use std::time::Duration;

/// An error thrown during execution of the program
#[derive(Debug)]
pub enum AtlasError {
    /// Something went wrong during notification on file changes
    FsNotifyError(notify::Error),
    /// Something went wrong when reading the log file
    IoError(std::io::Error),
    /// Something went wrong when reading from the channel
    RecvError(std::sync::mpsc::RecvError),
    /// Something went wrong when notifying the user
    NotifyError(notify_rust::error::Error),
}

impl From<notify::Error> for AtlasError {
    fn from(e: notify::Error) -> Self {
        AtlasError::FsNotifyError(e)
    }
}

impl From<std::io::Error> for AtlasError {
    fn from(e: std::io::Error) -> Self {
        AtlasError::IoError(e)
    }
}

impl From<std::sync::mpsc::RecvError> for AtlasError {
    fn from(e: std::sync::mpsc::RecvError) -> Self {
        AtlasError::RecvError(e)
    }
}

impl From<notify_rust::error::Error> for AtlasError {
    fn from(e: notify_rust::error::Error) -> Self {
        AtlasError::NotifyError(e)
    }
}

impl fmt::Display for AtlasError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AtlasError::FsNotifyError(e) => write!(f, "AtlasError::FsNotifyError: {}", e),
            AtlasError::IoError(e) => write!(f, "AtlasError::IoError: {}", e),
            AtlasError::RecvError(e) => write!(f, "AtlasError::RecvError: {}", e),
            AtlasError::NotifyError(e) => write!(f, "AtlasError::NotifyError: {}", e),
        }
    }
}

/// Stores the configuration for the application.
pub struct Config {
    watch_file: String,
    bad_map_messages: Vec<String>,
}

impl Config {
    /// Creates a new configuration from command line arguments.
    ///
    /// The first argument is interpreted as the file to watch. The second one
    /// the maps to look for.
    pub fn new_from_args() -> Result<Config, &'static str> {
        let mut args = env::args().skip(1);

        let watch_file = match args.next() {
            Some(v) => v,
            None => return Err("no file to watch given"),
        };

        let template = String::from("You have entered {}");
        let bad_map_messages = args.map(|m| template.replace("{}", &m)).collect::<Vec<_>>();
        if bad_map_messages.is_empty() {
            return Err("no bad maps given");
        }

        Ok(Config {
            watch_file,
            bad_map_messages,
        })
    }
}

fn handle_event(
    event: DebouncedEvent,
    config: &Config,
    file: &mut BufReader<std::fs::File>,
) -> Result<(), AtlasError> {
    if let DebouncedEvent::Write(p) = event {
        for line in file.lines() {
            let line = line?;
            if config.bad_map_messages.iter().any(|bmm| bmm == &line) {
                notify()?;
            }
        }
    }
    Ok(())
}

fn notify() -> Result<(), notify_rust::error::Error> {
    Notification::new()
        .summary("brickatlas alert")
        .body("Do NOT complete map!")
        .timeout(Timeout::Milliseconds(5000))
        .urgency(NotificationUrgency::Critical)
        .show()?;
    Ok(())
}

/// Runs the application given a certain configuration.
///
/// Inspired by https://pastebin.com/emFNyUXe
pub fn run(config: Config) -> Result<(), AtlasError> {
    let (tx, rx) = mpsc::channel();
    let mut watcher = notify::watcher(tx, Duration::from_secs(5))?;
    watcher.watch(&config.watch_file, RecursiveMode::NonRecursive)?;

    let f = File::open(&config.watch_file)?;
    let mut f = BufReader::new(f);
    f.seek(SeekFrom::End(0))?;

    loop {
        match rx.recv() {
            Ok(event) => handle_event(event, &config, &mut f)?,
            Err(err) => return Err(AtlasError::RecvError(err)),
        }
    }
}
