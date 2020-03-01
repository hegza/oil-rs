mod commands;
mod error;
mod event_store;
#[cfg(test)]
mod tests;

use crate::event::{AnnualDay, Event, Interval, State};
use crate::prelude::*;
use chrono::{Local, Timelike, Weekday};
use commands::{match_command, AddCommand, CommandKind};
use dialoguer::{
    theme::{ColorfulTheme, CustomPromptCharacterTheme},
    Confirmation, Input, Select,
};
pub use error::*;
use event_store::{EventStore, TrackedEvent, Uid as EventUid};
use std::collections::BTreeMap;
use std::convert::TryInto;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::string::ToString;
use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Clone, Copy)]
pub enum ViewState {
    Standard,
    Extended,
}

pub struct Tracker {
    tracked_events: EventStore,
    id_to_uid: BTreeMap<Id, EventUid>,
    state: ViewState,
    undo_buffer: Vec<Box<dyn FnOnce(&mut Tracker)>>,
}

impl Tracker {
    pub fn with_events(tracked_events: EventStore) -> Tracker {
        let id_to_uid = make_ids(&tracked_events);

        Tracker {
            tracked_events,
            id_to_uid,
            state: ViewState::Extended,
            undo_buffer: Vec::new(),
        }
    }
    pub fn empty() -> Tracker {
        Tracker {
            tracked_events: EventStore::new(),
            id_to_uid: BTreeMap::new(),
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

    pub fn interact<P>(&mut self, path: P)
    where
        P: AsRef<Path>,
    {
        loop {
            debug!("Interact loop starts");

            // Hint at loop reset
            println!();

            // 0. Display tracker (main UI)
            debug!("Visualizing tracker (main 0/4)");
            self.tracked_events.refresh_state();
            self.visualize();

            // 1. Get input from user (main interface)
            debug!("Getting user input (main 1/4)");
            let input = Input::<String>::with_theme(&CustomPromptCharacterTheme::new('>'))
                .allow_empty(true)
                .interact()
                .expect("could not parse string from user input");
            let cmd = match_command(&input, &self.id_to_uid);

            // Check for exit
            if let Some(CommandKind::Exit) = cmd {
                return;
            }

            // 2. Refresh status from disk
            debug!("User input received... refreshing state from disk (main 2/4)");
            match self.refresh_from_disk(&path) {
                ControlAction::Proceed => {}
                ControlAction::Exit => return,
                ControlAction::Input => continue,
            }

            // 3. Attempt to apply command
            debug!("Applying command... (main 3/4)");
            match self.apply_command(cmd) {
                ControlAction::Proceed => {}
                ControlAction::Exit => return,
                ControlAction::Input => continue,
            }

            // 4. Store to disk on success
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
        self.id_to_uid = make_ids(&self.tracked_events);
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

    fn undo(&mut self) {
        // No-op if nothing in buffer
        if self.undo_buffer.is_empty() {
            debug!("Attempted to undo with empty undo buffer");
            println!("Cannot undo, undo buffer is empty");
            return;
        }

        let undo_op = self.undo_buffer.pop().unwrap();
        undo_op(self);
    }

    fn visualize(&self) {
        let now = Local::now();

        let state_str = match self.state {
            ViewState::Standard => "standard",
            ViewState::Extended => "extended",
        };

        // Print status
        println!("=== Events ({}) ===", state_str);
        for (idx, (_uid, event)) in self.tracked_events.events().iter().enumerate() {
            match self.state {
                ViewState::Standard => match event.state() {
                    // Only show triggered entries
                    State::Triggered(_) => {
                        println!("* ({id:>2}) - {text}", id = idx, text = event.text());
                    }
                    _ => {}
                },
                ViewState::Extended => {
                    println!(
                        "{trig} ({id:>2}) {next:>16} - {text} ({state:?})",
                        id = idx,
                        text = event.text(),
                        next = match event.next_trigger_time(&now) {
                            None => "Not scheduled".to_string(),
                            Some(time) => format!("{}", time.format("%a %d.%m. %H:%M")),
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
            // Found: also remove from ID's and separate the return value
            Ok(te) => {
                self.id_to_uid = make_ids(&self.tracked_events);
                Some((te.event().clone(), te.state().clone()))
            }
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
            Ok(()) => {
                self.id_to_uid = make_ids(&self.tracked_events);
                uid
            }
            Err(ItemAlreadyExistsError(k, ov, _)) => {
                panic!(
                    "Attempted to register an event with UID {} that was already reserved for: {:#?}", k, ov
                );
            }
        }
    }

    // Returns None if event not found
    pub fn complete_now(&mut self, uid: EventUid) -> Option<(OpId, LocalTime)> {
        match self.tracked_events.get_mut(uid) {
            Ok(ev) => Some((OpId::next(), ev.complete_now())),
            Err(_) => None,
        }
    }

    pub fn rewind_complete(&mut self, op_id: OpId) {
        println!("Rewinding completed items is not implemented");
    }
}

enum ControlAction {
    Proceed,
    Input,
    Exit,
}

pub fn create_event_interact() -> Option<AddCommand> {
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

    Some(AddCommand(Event::new(interval, text)))
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

#[derive(Clone, Copy)]
pub struct OpId(pub usize);

static OP_ID_COUNTER: AtomicUsize = AtomicUsize::new(0);

impl OpId {
    pub fn next() -> OpId {
        {
            OpId(OP_ID_COUNTER.fetch_add(1, Ordering::Relaxed))
        }
    }
}

#[derive(Clone, Copy, PartialOrd, Eq, PartialEq, Ord)]
pub struct Id(pub usize);

impl std::fmt::Display for Id {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

fn make_ids(events: &EventStore) -> BTreeMap<Id, EventUid> {
    events
        .events()
        .iter()
        .map(|(&uid, _)| uid)
        .enumerate()
        .map(|(id, uid)| (Id(id), uid))
        .collect::<BTreeMap<Id, EventUid>>()
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
