mod commands;
mod error;
mod event;

use crate::config::Config;
use crate::views::tracker::commands::{match_command, AddCommand};
use crate::views::tracker::event::{AnnualDay, Event};
use chrono::Timelike;
use dialoguer::{
    theme::{ColorfulTheme, CustomPromptCharacterTheme},
    Confirmation, Input, Select,
};
pub use error::*;
use event::{Id, Interval};
use log::{info, warn};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::convert::TryFrom;
use std::fs;
use std::io::Read;
use std::path::Path;
use std::path::PathBuf;

#[derive(Clone, Copy)]
pub enum ViewState {
    Standard,
    Extended,
}

pub struct Tracker {
    tracked_events: BTreeMap<Id, Event>,
    state: ViewState,
}

impl Tracker {
    pub fn with_events(tracked_events: BTreeMap<Id, Event>) -> Tracker {
        Tracker {
            tracked_events,
            state: ViewState::Extended,
        }
    }
    pub fn empty() -> Tracker {
        Tracker {
            tracked_events: BTreeMap::new(),
            state: ViewState::Extended,
        }
    }

    pub fn interact<P>(&mut self, path: P)
    where
        P: AsRef<Path>,
    {
        loop {
            // 0. Display tracker
            self.visualize();

            // 1. Get input
            let input = Input::<String>::with_theme(&CustomPromptCharacterTheme::new('>'))
                .with_prompt("add, rm <id>, <id>, show, hide, refresh, exit")
                .allow_empty(true)
                .interact()
                .expect("could not parse string from user input");
            let cmd = match_command(&input);

            // TODO: 2. Refresh status from disk

            // TODO: 3. Attempt to apply command
            // If no command was parsed, return to input step
            let cmd = match cmd {
                Some(c) => c,
                None => continue,
            };

            // Match and apply command
            use commands::*;
            match cmd {
                CommandKind::Exit => break,
                CommandKind::Show(c) => {
                    c.apply(self);
                }
                CommandKind::Hide(c) => {
                    c.apply(self);
                }
                c => {
                    Confirmation::new()
                        .with_text(&format!(
                            "Command {:?} is not implemented (press anything to continue)",
                            c
                        ))
                        .show_default(false)
                        .interact()
                        .unwrap();
                }
            }

            // TODO: 4. Store to disk on success
        }
    }

    fn visualize(&self) {
        let state_str = match self.state {
            ViewState::Standard => "standard",
            ViewState::Extended => "extended",
        };

        // Print status
        println!("=== Events ({}) ===", state_str);
        for (id, event) in &self.tracked_events {
            println!("#{} - {}", id, event.text());
        }

        // Print commands
        println!("=== Commands ===");
        println!(
            "\
            add     - create an event interactively\n\
            rm <id> - remove registered event\n\
            <id>    - set event as completed\n\
            show    - show untriggered events and extended event status\n\
            hide    - hide untriggered events and extended event status\n\
            refresh - refresh events from disk\n\
            undo    - undo last action\n\
            exit    - exit interactive client\n\
            "
        );
    }

    pub fn from_path<P>(path: P) -> Result<Tracker, LoadError>
    where
        P: AsRef<Path>,
    {
        match fs::OpenOptions::new().read(true).open(&path) {
            Ok(mut file) => {
                let mut contents = String::new();
                file.read_to_string(&mut contents)
                    .expect("cannot read file to string");

                if contents.is_empty() {
                    return Err(LoadError::FileEmpty);
                }

                // Try load tracker from file
                let map = serde_yaml::from_str(&contents)
                    .map_err(|e| LoadError::FileContentsMalformed(Box::new(e)));
                match map {
                    Ok(events) => Ok(Tracker::with_events(events)),
                    Err(e) => Err(e),
                }
            }
            _ => Err(LoadError::FileDoesNotExist),
        }
    }

    /// Returns the currently tracker cached in the config, and it's file, none if no tracker is cached
    pub fn from_config(config: &Config) -> Option<(Tracker, PathBuf)> {
        let last_open = match &config.last_open {
            Some(p) => p,
            None => return None,
        };
        let path =
            PathBuf::try_from(last_open).expect("cannot parse path from cached 'last_open' string");

        match Tracker::from_path(&path) {
            Ok(tracker) => Some((tracker, path)),
            Err(LoadError::FileEmpty) => {
                warn!("A tracker file was found in cache but it was empty, replacing with Tracker::empty()");
                Some((Tracker::empty(), path))
            }
            Err(LoadError::FileDoesNotExist) => {
                warn!(
                    "A tracker file was found in cache but not in filesystem at {}",
                    path.to_str().expect("cannot make path into a string")
                );
                None
            }
            Err(LoadError::FileContentsMalformed(_)) => {
                use crate::views::prompt_file::{ask_malformed_action, ActionOnMalformedFile};
                match ask_malformed_action() {
                    ActionOnMalformedFile::ReplaceOriginal => {
                        warn!("Creating a default tracker in place of a malformed one based on user request");
                        Some((Tracker::empty(), path))
                    }
                    ActionOnMalformedFile::Cancel => {
                        info!(
                            "User requested cancellation upon encountering malformed tracker cache"
                        );
                        panic!(
                            "User requested cancellation upon encountering malformed tracker cache"
                        );
                    }
                }
            }
        }
    }

    /// Returns previous state
    pub fn set_state(&mut self, s: ViewState) -> ViewState {
        let prev = self.state;
        self.state = s;
        prev
    }
}

pub fn create_event() -> Option<AddCommand> {
    // What?
    let text = Input::<String>::new()
        .with_prompt("What? (type text)")
        .allow_empty(false)
        .interact()
        .expect("cannot parse string from user input");
    println!();

    // Interval type?
    let choices = &[
        // FromLastCompletion(Duration)
        "A constant time after the last completion of the event",
        "Annual(AnnualDay, Time)",
        "Monthly(MonthlyDay, Time)",
        //"Weekly(Weekday, Time)", // Not implemented!
    ];

    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Choose when to trigger the event")
        .default(0)
        .items(&choices[..])
        .interact()
        .unwrap();
    let interval = match selection {
        0 => match create_timedelta() {
            None => {
                println!("Aborting 'add event'");
                return None;
            }
            Some(td) => Interval::FromLastCompletion(td),
        },
        1 => {
            let month = match input_number("Which month? (number)") {
                Some(m) => m,
                None => {
                    println!("Aborting 'add event'");
                    return None;
                }
            };
            let day = match input_number("Which day? (number)") {
                Some(m) => m,
                None => {
                    println!("Aborting 'add event'");
                    return None;
                }
            };
            let time = match input_time("At what time?") {
                Some(t) => t,
                None => {
                    println!("Aborting 'add event'");
                    return None;
                }
            };

            Interval::Annual(AnnualDay { month, day }, time)
        }
        2 => {
            let day = match input_number("Which day? (number)") {
                Some(d) => d,
                None => {
                    println!("Aborting 'add event'");
                    return None;
                }
            };
            let time = match input_time("At what time?") {
                Some(t) => t,
                None => {
                    println!("Aborting 'add event'");
                    return None;
                }
            };

            Interval::Monthly(event::MonthlyDay { day }, time)
        }
        _ => unreachable!(),
    };

    Some(AddCommand(Event::new(interval, text)))
}

pub fn create_timedelta() -> Option<event::TimeDelta> {
    let choices = &[
        // "Days(i32)"
        "Trigger every N days",
        // "Hms { hours: i32, minutes: i32, seconds: i32 }"
        "Trigger every 'hours, minutes, seconds'",
    ];

    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Choose the kind of timer to use for triggering the event")
        .default(0)
        .items(&choices[..])
        .interact()
        .unwrap();
    match selection {
        0 => {
            let days = input_number("Input a number of days for the interval");
            match days {
                None => return None,
                Some(d) => Some(event::TimeDelta::Days(d)),
            }
        }
        1 => {
            let time = input_time("Input a time interval, eg. 2:15 for 2 hours 15 minutes");
            match time {
                None => return None,
                Some(t) => Some(event::TimeDelta::Hms {
                    hours: t.hour(),
                    minutes: t.minute(),
                    seconds: t.second(),
                }),
            }
        }
        _ => unreachable!(),
    }
}

pub fn input_number(prompt: &str) -> Option<i32> {
    loop {
        let input = Input::<String>::new()
            .with_prompt(prompt)
            .allow_empty(true)
            .interact()
            .expect("unable to parse string from user input");
        if input.is_empty() {
            return None;
        }

        // Parse integer from input
        match input.parse() {
            Ok(i) => return Some(i),
            Err(_) => {
                println!("Cannot parse int from {}\n", input);
                continue;
            }
        }
    }
}

const TIME_FORMATS: &[&str] = &[
    "%H:%M:%S",
    "%H.%M.%S",
    "%H, %M, %S",
    "%H:%M",
    "%H.%M",
    "%H %M",
];

pub fn input_time(prompt: &str) -> Option<chrono::NaiveTime> {
    loop {
        let input = Input::<String>::new()
            .with_prompt(prompt)
            .allow_empty(true)
            .interact()
            .expect("unable to parse string from user input");
        if input.is_empty() {
            return None;
        }

        // Parse time from input
        for fmt in TIME_FORMATS {
            if let Ok(time) = chrono::NaiveTime::parse_from_str(&input, fmt) {
                return Some(time);
            }
        }
        println!("Cannot parse int from {}\n", input);
        continue;
    }
}
