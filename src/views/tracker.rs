mod commands;
mod error;
mod event_store;
#[cfg(test)]
mod tests;

use crate::event::{AnnualDay, Event, Interval, State};
use crate::prelude::*;
use crate::views::tracker::commands::COMMAND_KEYS;
use chrono::{Local, Timelike, Weekday};
use commands::{match_command, CommandKind, CreateCommand};
use dialoguer::{
    theme::{ColorfulTheme, CustomPromptCharacterTheme},
    Confirmation, Input, Select,
};
pub use error::*;
use event_store::{EventStore, TrackedEvent, Uid as EventUid};
use std::cmp::Ordering;
use std::convert::TryInto;
use std::iter::FromIterator;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::string::ToString;

pub struct Tracker {
    tracked_events: EventStore,
    state: ViewState,
    undo_buffer: Vec<Box<dyn FnOnce(&mut Tracker)>>,
}

#[derive(Clone, Copy)]
pub enum ViewState {
    Standard,
    Extended,
}

pub const LOOK_AHEAD_FRAC: f64 = 1. / 12.;

impl Tracker {
    pub fn with_events(tracked_events: EventStore) -> Tracker {
        Tracker {
            tracked_events,
            state: ViewState::Extended,
            undo_buffer: Vec::new(),
        }
    }
    pub fn empty() -> Tracker {
        Tracker {
            tracked_events: EventStore::new(),
            state: ViewState::Extended,
            undo_buffer: Vec::new(),
        }
    }
    pub fn from_path<P>(path: P) -> Result<Tracker, LoadError>
    where
        P: AsRef<Path>,
    {
        debug!(
            "Reading events for tracker from path: {}",
            path.as_ref().to_string_lossy()
        );
        let events = EventStore::from_file(path);
        match events {
            Ok(events) => Ok(Tracker::with_events(events)),
            Err(e) => Err(e),
        }
    }

    /// 1. Display tracker, 2. get input (modal), 3. refresh status from disk, 4. apply changes.
    pub fn interact<P>(&mut self, path: P)
    where
        P: AsRef<Path>,
    {
        loop {
            // Hint at loop reset
            debug!("Interact loop starts");
            println!();

            // Set up cached variables
            let now = Local::now();

            trace!("Refreshing tracked events from disk");
            self.tracked_events.update_events();

            // List of events, filtered and sorted for ui. Vector index indicates UI ID.
            trace!("Generating list of events for UI");
            let events_list: Vec<(&EventUid, &TrackedEvent)> = {
                let mut events = Vec::from_iter(self.tracked_events.iter());
                match self.state {
                    // Extended mode: show all events, sorted by next trigger time
                    ViewState::Extended => {
                        events.sort_by(|(_, te1), (_, te2)| sort_by_next_trigger(te1, te2, &now));
                    }
                    // Standard mode: show triggered events + lookahead, sorted by next trigger time
                    ViewState::Standard => {
                        let mut filtered_events = events
                            .iter()
                            .filter(|&(_, event)| match event.state() {
                                State::Triggered(_) => true,
                                // Show other entries if their next trigger is within look-ahead scope
                                _ => match event.fraction_of_interval_remaining(&now) {
                                    Some(remaining) if remaining < LOOK_AHEAD_FRAC => true,
                                    _ => false,
                                },
                            })
                            .map(|&x| x)
                            .collect::<Vec<(&EventUid, &TrackedEvent)>>();
                        filtered_events
                            .sort_by(|(_, te1), (_, te2)| sort_by_next_trigger(te1, te2, &now));
                        events = filtered_events;
                    }
                }
                events
            };

            // 1. Display tracker (main UI)
            debug!("Visualizing tracker (main 0/4)");
            self.visualize(&events_list);

            // 2. Get input from user (main interface)
            debug!("Getting user input (main 1/4)");
            let input = Input::<String>::with_theme(&CustomPromptCharacterTheme::new('>'))
                .allow_empty(true)
                .interact()
                .expect("could not parse string from user input");
            let cmd = match_command(
                &input,
                &events_list
                    .iter()
                    .map(|(&uid, _)| uid)
                    .collect::<Vec<EventUid>>(),
            );

            // Check for exit
            if let Some(CommandKind::Exit) = cmd {
                return;
            }

            // 3. Refresh status from disk
            debug!("User input received... refreshing state from disk (main 2/4)");
            match self.refresh_from_disk(&path) {
                ControlAction::Proceed => {}
                ControlAction::Exit => return,
                ControlAction::Input => continue,
            }

            // 4. Attempt to apply command
            debug!("Applying command... (main 3/4)");
            match self.apply_command(cmd) {
                ControlAction::Proceed => {}
                ControlAction::Exit => return,
                ControlAction::Input => continue,
            }

            // 5. Store to disk on success
            debug!("Command applied succesfully, storing state to disk (main 4/4)");
            self.store_to_disk(&path);
        }
    }

    fn refresh_from_disk<P>(&mut self, path: P) -> ControlAction
    where
        P: AsRef<Path>,
    {
        self.tracked_events = match EventStore::from_file(&path) {
            Ok(ev) => ev,
            Err(e) => {
                warn!("Could not refresh events from disk: {:?}", e);
                let text = &format!(
                "Could not refresh event status from disk before attempting to apply command:\n{:#?}",
                &e
            );
                let choices = ["Cancel"];
                match crate::views::troubleshoot::choices(text, &choices) {
                    0 => {
                        return ControlAction::Input;
                    }
                    _ => unreachable!(),
                }
            }
        };
        ControlAction::Proceed
    }

    pub fn store_to_disk<P>(&self, path: P)
    where
        P: AsRef<Path>,
    {
        match self.tracked_events.to_file(&path) {
            Ok(()) => {}
            Err(_) => {
                Confirmation::new()
                    .with_text("Failed to write to disk. Last operation will be cancelled.")
                    .show_default(false)
                    .interact()
                    .unwrap();
            }
        }
    }

    fn apply_command(&mut self, cmd: Option<CommandKind>) -> ControlAction {
        // If no command was parsed, return to input step
        let cmd = match cmd {
            Some(c) => c,
            None => return ControlAction::Input,
        };

        // Match and apply command
        use commands::*;
        match cmd {
            CommandKind::Exit => return ControlAction::Exit,
            CommandKind::Undo => self.undo(),
            CommandKind::ReversibleCommand(cmd) => {
                let undo_op = match cmd.apply(self) {
                    Ok(f) => f,
                    Err(e) => {
                        println!("Apply failed: {}", e.to_string());
                        // Go back to input phase
                        return ControlAction::Input;
                    }
                };
                self.undo_buffer.push(undo_op);
            }
        }
        ControlAction::Proceed
    }

    fn visualize(&self, visible_events: &[(&EventUid, &TrackedEvent)]) {
        let now = Local::now();

        let state_str = match self.state {
            ViewState::Standard => "standard",
            ViewState::Extended => "extended",
        };

        // Print status
        println!("=== Events ({}) ===", state_str);
        for (idx, (_, event)) in visible_events.iter().enumerate() {
            match self.state {
                ViewState::Standard => match event.state() {
                    // Show triggered entries
                    State::Triggered(_) => {
                        println!("* ({id:>2})   {text}", id = idx, text = event.text());
                    }
                    // Show non-triggered with details if requested
                    _ => {
                        if let Some(_) = event.fraction_of_interval_remaining(&now) {
                            println!(
                                "  ({id:>2})   ({text}) - (triggers {time})",
                                id = idx,
                                text = event.text(),
                                time = event
                                    .next_trigger_time(&now)
                                    .unwrap()
                                    .format("on %d.%m. at %H:%M")
                            );
                        }
                    }
                },
                ViewState::Extended => {
                    println!(
                        "{trig} ({id:>2}) {next} - {text} ({interval}, current: {state:?})",
                        id = idx,
                        text = event.text(),
                        interval = event.event().interval(),
                        next = match &event.next_trigger_time(&now) {
                            None => format!("{:>16}", "Not scheduled"),
                            Some(time) => format!(
                                "{:<10} {:<5}",
                                time.format("%a %-d.%-m."),
                                time.format("%H:%M")
                            ),
                        },
                        state = event.state(),
                        trig = match event.state() {
                            State::Triggered(_) => "*",
                            _ => " ",
                        }
                    );
                }
            }
        }

        // Print commands
        println!("=== Commands ===");
        for cmd in COMMAND_KEYS.iter() {
            println!("{:<10} - {}", cmd.name, cmd.short_desc);
        }
    }

    fn undo(&mut self) {
        trace!("Undo starts");

        // No-op if nothing in buffer
        if self.undo_buffer.is_empty() {
            debug!("Attempted to undo with empty undo buffer");
            println!("Cannot undo, undo buffer is empty");
            return;
        }

        let undo_op = self.undo_buffer.pop().unwrap();
        undo_op(self);
    }

    /// Returns previous state
    pub fn set_state(&mut self, s: ViewState) -> ViewState {
        let prev = self.state;
        self.state = s;
        prev
    }

    pub fn add_event(&mut self, event: Event) -> EventUid {
        self.add_event_with_state(event, State::default())
    }

    // Returns None if an event was not found with id
    pub fn remove_event(&mut self, uid: EventUid) -> Option<(Event, State)> {
        match self.tracked_events.remove(uid) {
            // Found: separate the return value
            Ok(te) => Some((te.event().clone(), te.state().clone())),
            // Not found: return None
            Err(_) => None,
        }
    }

    pub fn add_event_with_state(&mut self, event: Event, state: State) -> EventUid {
        let uid = self.tracked_events.next_free_uid();
        debug!("Registering a new event with UID {}: {:?}", uid, event);
        let tracked_event = TrackedEvent::with_state(event, state);
        trace!("Created TrackedEvent: {:?}", &tracked_event);

        match self.tracked_events.add(uid, tracked_event) {
            Ok(()) => uid,
            Err(ItemAlreadyExistsError(k, ov, _)) => {
                panic!(
                    "Attempted to register an event with UID {} that was already reserved for: {:#?}", k, ov
                );
            }
        }
    }

    /// Returns the event as mutable if it exists with given UID
    #[allow(dead_code)]
    pub fn get_event_mut(&mut self, uid: EventUid) -> Option<&mut TrackedEvent> {
        self.tracked_events.get_mut(uid).ok()
    }

    /// Gets the state of the event as mutable if event exists with given UID
    pub fn get_event_state_mut(&mut self, uid: EventUid) -> Option<&mut State> {
        self.get_event_mut(uid).map(|e| e.state_mut())
    }
}

enum ControlAction {
    Proceed,
    Input,
    Exit,
}

pub fn create_event_interact() -> Option<CreateCommand> {
    // What?
    let text = Input::<String>::new()
        .with_prompt("What? (type text)")
        .allow_empty(true)
        .interact()
        .expect("cannot parse string from user input");
    if text.is_empty() {
        return None;
    }
    println!();

    // Interval type?
    let choices = &[
        "A constant time after the last completion of the event",
        "Daily",
        "Weekly",
        "Monthly",
        "Annually",
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
        // Daily
        1 => {
            let time = match input_time("At what time?") {
                Some(t) => t,
                None => {
                    println!("Aborting 'add event'");
                    return None;
                }
            };
            Interval::Daily(time)
        }
        // Weekly
        2 => {
            let weekday = match crate::views::troubleshoot::choices(
                "Which day of the week? (number)",
                &[
                    "Monday",
                    "Tuesday",
                    "Wednesday",
                    "Thursday",
                    "Friday",
                    "Saturday",
                    "Sunday",
                ],
            ) {
                0 => Weekday::Mon,
                1 => Weekday::Tue,
                2 => Weekday::Wed,
                3 => Weekday::Thu,
                4 => Weekday::Fri,
                5 => Weekday::Sat,
                6 => Weekday::Sun,
                _ => unreachable!(),
            };
            let time = match input_time("At what time?") {
                Some(t) => t,
                None => {
                    println!("Aborting 'add event'");
                    return None;
                }
            };

            Interval::Weekly(weekday, time)
        }
        // Monthly
        3 => {
            let day = match input("Which day? (number)") {
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

            Interval::Monthly(crate::event::MonthlyDay { day }, time)
        }
        // Annually
        4 => {
            let month = match input("Which month? (number)") {
                Some(m) => m,
                None => {
                    println!("Aborting 'add event'");
                    return None;
                }
            };
            let day = match input("Which day? (number)") {
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
        _ => unreachable!(),
    };

    Some(CreateCommand(Event::new(interval, text)))
}

pub fn create_timedelta() -> Option<crate::event::TimeDelta> {
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
            let days = input("Input a number of days for the interval");
            match days {
                None => return None,
                Some(d) => Some(crate::event::TimeDelta::Days(d)),
            }
        }
        1 => {
            let time = input_time("Input a time interval, eg. 2:15 for 2 hours 15 minutes");
            match time {
                None => return None,
                Some(t) => Some(crate::event::TimeDelta::Hms(
                    t.hour().try_into().unwrap(),
                    t.minute().try_into().unwrap(),
                    t.second().try_into().unwrap(),
                )),
            }
        }
        _ => unreachable!(),
    }
}

pub fn input<T>(prompt: &str) -> Option<T>
where
    T: FromStr,
{
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

fn sort_by_next_trigger(te1: &TrackedEvent, te2: &TrackedEvent, now: &LocalTime) -> Ordering {
    te1.next_trigger_time(&now)
        .cmp(&te2.next_trigger_time(&now))
}

pub fn set_up_at(path: PathBuf) -> (Tracker, PathBuf) {
    debug!(
        "Attempting to create Tracker from {}",
        path.canonicalize()
            .expect("cannot canonicalize path")
            .to_string_lossy()
    );
    let (tracker, path) = match Tracker::from_path(&path) {
        Ok(t) => (t, path),
        Err(LoadError::FileDoesNotExist) => {
            warn!(
                "A tracker file was found in cache but not in filesystem at \"{}\", the user may have removed it",
                path.to_str().expect("cannot make path into a string")
            );
            println!(
                "Last used file does not exist at \"{}\", creating an empty tracker",
                path.to_string_lossy()
            );
            (Tracker::empty(), path)
        }
        Err(LoadError::FileContentsMalformed(_, mal_path, _contents)) => {
            warn!("File contents malformed, asking user for action");
            super::troubleshoot::ask_malformed_action(PathBuf::from_str(&mal_path).unwrap())
        }
    };

    debug!(
        "Set up complete, storing tracker on disk at: {}",
        path.canonicalize()
            .expect("cannot canonicalize path")
            .to_string_lossy()
    );
    tracker.store_to_disk(&path);
    (tracker, path)
}
