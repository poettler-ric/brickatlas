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
use serde::Deserialize;
use std::error;
use std::fmt;
use std::fs::{self, File};
use std::io::prelude::*;
use std::io::{BufReader, SeekFrom};
use std::path::Path;
use std::sync::mpsc;
use std::time::Duration;
// use toml;

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
    /// Something went wrong while parsing the configuration
    TomlError(toml::de::Error),
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

impl From<toml::de::Error> for AtlasError {
    fn from(e: toml::de::Error) -> Self {
        AtlasError::TomlError(e)
    }
}

impl fmt::Display for AtlasError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AtlasError::FsNotifyError(e) => write!(f, "AtlasError::FsNotifyError: {}", e),
            AtlasError::IoError(e) => write!(f, "AtlasError::IoError: {}", e),
            AtlasError::NotifyError(e) => write!(f, "AtlasError::NotifyError: {}", e),
            AtlasError::ConfigError(e) => write!(f, "AtlasError::ConfigError: {}", e),
            AtlasError::TomlError(e) => write!(f, "AtlasError::TomlError: {}", e),
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
            AtlasError::TomlError(e) => Some(e),
        }
    }
}

/// Stores the configuration for the application.
#[derive(Debug, Default, Deserialize)]
pub struct Config {
    #[serde(default)]
    logfile: String,
    #[serde(default)]
    maps: Vec<String>,
    bad_map_messages: Option<Vec<String>>,
}

impl Config {
    /// Parses the configuration from command line arguments.
    ///
    /// The first argument is interpreted as the file to watch. The second one
    /// the maps to look for.
    pub fn new_from_args() -> Result<Config, AtlasError> {
        let matches = App::new("brickatlas")
            .arg(
                Arg::with_name("configfile")
                    .short("c")
                    .help("config file to use")
                    .takes_value(true),
            )
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

        let mut config = if let Some(file) = matches.value_of("configfile") {
            Self::new_from_file(file)?
        } else {
            Default::default()
        };

        if let Some(logfile) = matches.value_of("logfile") {
            config.logfile = String::from(logfile);
        }

        if let Some(maps) = matches.values_of("maps") {
            config.maps.extend(maps.map(String::from));
        }

        Ok(config)
    }

    /// Parse configuration from a toml file.
    pub fn new_from_file(file: &str) -> Result<Config, AtlasError> {
        Ok(toml::from_str::<Config>(
            fs::read_to_string(file)?.as_str(),
        )?)
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
