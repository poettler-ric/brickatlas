#![deny(missing_docs)]

//! `brickatlas` watches the PoE log file. If the user enters a configured map a
//! alert notification is shown to not complete the map.

use notify::{self, DebouncedEvent, RecursiveMode, Watcher};
use notify_rust::{self, Notification, NotificationUrgency, Timeout};
use std::env;
use std::error::Error;
use std::fs::File;
use std::io::prelude::*;
use std::io::{BufReader, SeekFrom};
use std::sync::mpsc;
use std::time::Duration;

/// Stores the configuration for the application.
pub struct Config {
    watch_file: String,
    bad_maps: Vec<String>,
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

        let bad_maps = args.collect::<Vec<_>>();
        if bad_maps.is_empty() {
            return Err("no bad maps given");
        }

        Ok(Config {
            watch_file,
            bad_maps,
        })
    }
}

fn handle_event<R>(event: DebouncedEvent, config: &Config, file: &BufReader<R>) {}

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
pub fn run(config: Config) -> Result<(), Box<dyn Error>> {
    let (tx, rx) = mpsc::channel();
    let mut watcher = notify::watcher(tx, Duration::from_secs(5))?;
    watcher.watch(&config.watch_file, RecursiveMode::NonRecursive)?;

    let f = File::open(&config.watch_file)?;
    let mut f = BufReader::new(f);
    f.seek(SeekFrom::End(0))?;

    loop {
        match rx.recv() {
            Ok(event) => handle_event(event, &config, &f),
            Err(err) => return Err(Box::new(err)),
        }
    }
}
