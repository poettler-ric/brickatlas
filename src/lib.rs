#![deny(missing_docs)]
#![deny(deprecated)]
#![warn(missing_doc_code_examples)]

//! `brickatlas` watches the PoE log file. If the user enters a configured map a
//! alert notification is shown to not complete the map.
//!
//! Inspired by this [Python script](https://pastebin.com/emFNyUXe).

use clap::{App, Arg};
use notify::{self, DebouncedEvent, RecursiveMode, Watcher};
use notify_rust::{self, Notification, NotificationUrgency, Timeout};
use std::error;
use std::fmt;
use std::fs::File;
use std::io::prelude::*;
use std::io::{BufReader, SeekFrom};
use std::path::Path;
use std::sync::mpsc;
use std::time::Duration;

/// An error thrown during execution of the program
#[derive(Debug)]
pub enum AtlasError {
    /// Something went wrong during notification on file changes
    FsNotifyError(notify::Error),
    /// Something went wrong when reading the log file
    IoError(std::io::Error),
    /// Something went wrong when notifying the user
    NotifyError(notify_rust::error::Error),
    /// Configuration is not usable
    ConfigError(String),
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
            AtlasError::NotifyError(e) => write!(f, "AtlasError::NotifyError: {}", e),
            AtlasError::ConfigError(e) => write!(f, "AtlasError::ConfigError: {}", e),
        }
    }
}

impl error::Error for AtlasError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            AtlasError::FsNotifyError(e) => Some(e),
            AtlasError::IoError(e) => Some(e),
            AtlasError::NotifyError(e) => Some(e),
            AtlasError::ConfigError(_) => None,
        }
    }
}

/// Stores the configuration for the application.
#[derive(Debug, Default)]
pub struct Config {
    logfile: String,
    maps: Vec<String>,
    bad_map_messages: Option<Vec<String>>,
}

impl Config {
    /// Updates the configuration from command line arguments.
    ///
    /// The first argument is interpreted as the file to watch. The second one
    /// the maps to look for.
    pub fn new_from_args() -> Result<Config, &'static str> {
        let matches = App::new("brickatlas")
            .arg(
                Arg::with_name("logfile")
                    .short("l")
                    .help("log file to analyze")
                    .takes_value(true),
            )
            .arg(
                Arg::with_name("maps")
                    .short("m")
                    .help("maps to avoid")
                    .takes_value(true)
                    .multiple(true),
            )
            .get_matches();

        let logfile = String::from(matches.value_of("logfile").unwrap_or(""));

        let maps = match matches.values_of("maps") {
            Some(v) => v.map(String::from).collect(),
            None => vec![],
        };

        Ok(Config {
            logfile: logfile,
            maps: maps,
            bad_map_messages: None,
        })
    }

    fn bad_map_messages(&mut self) -> &[String] {
        let Self {
            maps,
            bad_map_messages,
            ..
        } = self;
        bad_map_messages.get_or_insert_with(|| {
            maps.iter()
                .map(|m| format!("You have entered {}", m))
                .collect()
        })
    }
}

fn handle_event(
    event: DebouncedEvent,
    config: &mut Config,
    file: &mut BufReader<std::fs::File>,
) -> Result<(), AtlasError> {
    if let DebouncedEvent::Write(_) = event {
        for line in file.lines() {
            let line = line?;
            if config.bad_map_messages().iter().any(|bmm| bmm == &line) {
                notify()?;
            }
        }
    }
    Ok(())
}

fn notify() -> Result<(), AtlasError> {
    Notification::new()
        .summary("brickatlas alert")
        .body("Do NOT complete map!")
        .timeout(Timeout::Milliseconds(5000))
        .urgency(NotificationUrgency::Critical)
        .show()?;
    Ok(())
}

/// Runs the application given a certain configuration.
pub fn run(config: &mut Config) -> Result<(), AtlasError> {
    if !Path::new(&config.logfile).exists() {
        return Err(AtlasError::ConfigError(format!(
            "watchfile ({}) doesn't exist",
            &config.logfile
        )));
    }

    let (tx, rx) = mpsc::channel();
    let mut watcher = notify::watcher(tx, Duration::from_secs(1))?;
    watcher.watch(&config.logfile, RecursiveMode::NonRecursive)?;

    let f = File::open(&config.logfile)?;
    let mut f = BufReader::new(f);
    f.seek(SeekFrom::End(0))?;

    for event in rx {
        handle_event(event, config, &mut f)?;
    }
    Ok(())
}
