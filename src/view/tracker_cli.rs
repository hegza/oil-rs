use crate::datamodel::*;
use crate::prelude::*;
use crate::tracker;
use chrono::{DateTime, Duration, Local, Timelike, Weekday};
use dialoguer::theme;
use std::cmp::Ordering;
use std::convert::TryInto;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::string::ToString;
use tracker::{command, event_store::TrackedEvent, Tracker, Uid};

pub const LOOK_AHEAD_FRAC: f64 = 1. / 12.;

pub struct TrackerCli {
    state: ViewState,
    pub tracker: Tracker,
}

impl TrackerCli {
    pub fn new(tracker: Tracker) -> TrackerCli {
        TrackerCli {
            state: ViewState::Extended,
            tracker,
        }
    }

    pub fn call<S>(&mut self, input: S)
    where
        S: AsRef<str>,
    {
        debug!("Out-of-order 'call' starts");

        let now = Local::now();
        let visible_events = self.generate_events_list(&now);
        let cmd = self.interpret(input.as_ref(), &visible_events);

        debug!("Applying command...");
        match self.apply_command(cmd) {
            ControlAction::Proceed => {}
            ControlAction::Exit => return,
            ControlAction::Input => return,
        }
    }

    /// 1. Display tracker, 2. get input (modal), 3. refresh status from disk,
    /// 4. apply changes.
    pub fn interact_modal<P>(&mut self, path: P)
    where
        P: AsRef<Path>,
    {
        loop {
            // Hint at loop reset
            debug!("Interact loop starts");
            println!();
            println!();

            // Set up cached variables
            let now = Local::now();

            trace!("Refreshing tracked events");
            self.tracker.update_events();

            // List of events, filtered and sorted for ui. Vector index indicates UI ID.
            trace!("Generating list of events for UI");
            let visible_events = self.generate_events_list(&now);

            // 1. Display tracker (main UI)
            debug!("Visualizing tracker (main 1/5)");
            self.visualize(&visible_events);

            // 2. Get input from user (main interface)
            debug!("Getting user input (main 2/5)");
            let input = dialoguer::Input::<String>::with_theme(
                &theme::CustomPromptCharacterTheme::new('>'),
            )
            .allow_empty(true)
            .interact()
            .expect("could not parse string from user input");
            let cmd = self.interpret(&input, &visible_events);

            // Check for exit
            if let Some(command::CommandKind::Exit) = cmd {
                return;
            }

            // 3. Refresh status from disk
            debug!("User input received... refreshing state from disk before applying (main 3/5)");
            match self.refresh_tracker_from_disk(&path) {
                ControlAction::Proceed => {}
                ControlAction::Exit => return,
                ControlAction::Input => continue,
            }

            // 4. Attempt to apply command
            debug!("Applying command... (main 4/5)");
            match self.apply_command(cmd) {
                ControlAction::Proceed => {}
                ControlAction::Exit => return,
                ControlAction::Input => continue,
            }

            // 5. Store to disk on success
            debug!("Command applied succesfully, storing state to disk (main 5/5)");
            self.tracker.store_to_disk(&path);
        }
    }

    fn interpret<S>(
        &self,
        input: S,
        visible_events: &[(Uid, &TrackedEvent)],
    ) -> Option<command::CommandKind>
    where
        S: AsRef<str>,
    {
        let input = input.as_ref();

        debug!("User input: '{}'", &input);
        let cmd = command::match_command(
            &input,
            &visible_events
                .iter()
                .map(|(uid, _)| *uid)
                .collect::<Vec<tracker::Uid>>(),
        );

        if let Some(ref cmd) = cmd {
            debug!("Matched command '{}'", cmd);
        }

        cmd
    }

    fn apply_command(&mut self, cmd: Option<command::CommandKind>) -> ControlAction {
        // If no command was parsed, return to input step
        let cmd = match cmd {
            Some(c) => c,
            None => return ControlAction::Input,
        };

        // Match and apply command
        use command::*;
        match cmd {
            CommandKind::Exit => return ControlAction::Exit,
            CommandKind::CliCommand(cmd) => self.apply(cmd),
            CommandKind::Undo => self.tracker.undo(),
            CommandKind::DataCommand(cmd) => match self.tracker.apply_command(&*cmd) {
                Ok(()) => {}
                Err(e) => {
                    println!("Apply failed: {}", e.to_string());
                    // Go back to input phase
                    return ControlAction::Input;
                }
            },
        }
        ControlAction::Proceed
    }

    fn apply(&mut self, cmd: Box<dyn command::Apply>) {
        let result = cmd.apply(command::CommandReceiver::TrackerCli(self));
        match result {
            Ok(apply) => {
                if let Some(_) = apply {
                    warn!("Apply::apply returned an undo callback, but undo is not implemented for TrackerCli");
                }
            }
            Err(err) => {
                println!("Could not apply command: {}", err);
            }
        }
    }

    fn generate_events_list(&self, now: &DateTime<Local>) -> Vec<(tracker::Uid, &TrackedEvent)> {
        let mut events = self.tracker.events();
        match self.state {
            // Extended mode: show all events, sorted by next trigger time
            ViewState::Extended => {
                events.sort_by(|(_, te1), (_, te2)| sort_by_next_trigger(te1, te2));
            }
            // Standard mode: show triggered events + lookahead, sorted by next trigger time
            ViewState::Standard => {
                let mut filtered_events = events
                    .iter()
                    .filter(|&(_, event)| {
                        if event.1.is_triggered() {
                            true
                        } else {
                            // Show other entries if their next trigger is within look-ahead scope
                            match event.fraction_of_interval_remaining(&now) {
                                Some(remaining) if remaining < LOOK_AHEAD_FRAC => true,
                                _ => false,
                            }
                        }
                    })
                    .map(|&x| x)
                    .collect::<Vec<(tracker::Uid, &TrackedEvent)>>();
                filtered_events.sort_by(|(_, te1), (_, te2)| sort_by_next_trigger(te1, te2));
                events = filtered_events;
            }
        }
        events
    }

    fn visualize(&self, visible_events: &[(Uid, &TrackedEvent)]) {
        let now = Local::now();

        let state_str = match self.state {
            ViewState::Standard => "standard",
            ViewState::Extended => "extended",
        };

        // Figure out status
        if visible_events.len() != 0 {
            let visible_events: Vec<(usize, &(Uid, &TrackedEvent))> =
                visible_events.as_ref().iter().enumerate().collect();
            let split_idx = visible_events.iter().position(|(_idx, (_uid, event))| {
                match event.0.interval().to_duration_heuristic() {
                    Some(duration) => duration >= Duration::days(1) + Duration::hours(1),
                    None => false,
                }
            });

            let (daily_events, other_events) = match split_idx {
                None => (visible_events.as_slice(), None),
                Some(idx) => {
                    let (a, b) = visible_events.split_at(idx);
                    (a, Some(b))
                }
            };

            // Print status
            println!("=== Daily Events ({})) ===", state_str);
            for (idx, (_, event)) in daily_events {
                self.print_event_line(*idx, event, &now);
            }
            if let Some(events) = other_events {
                println!();
                println!("=== Events ({})) ===", state_str);
                for (idx, (_, event)) in events {
                    self.print_event_line(*idx, event, &now);
                }
            }
        }
        // No events: print something else
        else {
            println!("=== No Events ({})) ===", state_str);
        }

        // Print commands
        println!();
        println!("=== Commands ===");
        for cmd in command::COMMAND_KEYS.iter() {
            println!("{:<10} - {}", cmd.name, cmd.short_desc);
        }
    }

    fn refresh_tracker_from_disk<P>(&mut self, path: P) -> ControlAction
    where
        P: AsRef<Path>,
    {
        match self.tracker.refresh_from_disk(&path) {
            Ok(()) => ControlAction::Proceed,
            Err(e) => {
                let text = &format!(
                    "Could not refresh event status from disk before attempting to apply command:\n{:#?}",
                    &e
                );
                let choices = ["Cancel"];
                match crate::view::troubleshoot::choices(text, &choices) {
                    0 => {
                        return ControlAction::Input;
                    }
                    _ => unreachable!(),
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

    fn print_event_line(&self, idx: usize, event: &TrackedEvent, now: &LocalTime) {
        match self.state {
            ViewState::Standard => match event.1.status {
                // Show triggered entries
                StatusKind::Triggered => {
                    println!("* ({id:>2})   {text}", id = idx, text = event.text());
                }
                // Show non-triggered if close to triggering, HACK: unless they're "Skip"
                _ => {
                    if let Some(_) = event.fraction_of_interval_remaining(now) {
                        if let StatusKind::Skip(_) = event.1.status {
                            return;
                        } else {
                            println!(
                                "  ({id:>2})   ({text}) - (triggers {time})",
                                id = idx,
                                text = event.text(),
                                time = {
                                    let t = event.next_trigger_time().unwrap();
                                    if is_today(&t) {
                                        t.format("today at %H:%M")
                                    } else {
                                        t.format("on %d.%m. at %H:%M")
                                    }
                                }
                            );
                        }
                    }
                }
            },
            ViewState::Extended => {
                println!(
                    "{trig} ({id:>2}) {next} - {text} ({interval}, current: {state:?})",
                    id = idx,
                    text = event.text(),
                    interval = event.0.interval(),
                    next = match &event.next_trigger_time() {
                        None => format!("{:>16}", "Not scheduled"),
                        Some(time) => format!(
                            "{:<10} {:<5}",
                            time.format("%a %-d.%-m."),
                            time.format("%H:%M")
                        ),
                    },
                    state = &event.1,
                    trig = match event.1.status {
                        StatusKind::Triggered => "*",
                        _ => " ",
                    }
                );
            }
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum ViewState {
    Standard,
    Extended,
}

enum ControlAction {
    Proceed,
    Input,
    Exit,
}

pub fn create_event_interact() -> Option<command::CreateCommand> {
    // What?
    let text = dialoguer::Input::<String>::new()
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

    let selection = dialoguer::Select::with_theme(&theme::ColorfulTheme::default())
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
            Interval::Periodic(TimePeriod::Daily(time))
        }
        // Weekly
        2 => {
            let weekday = match crate::view::troubleshoot::choices(
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

            Interval::Periodic(TimePeriod::Weekly(weekday, time))
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

            Interval::Periodic(TimePeriod::Monthly(MonthlyDay { day }, time))
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

            Interval::Periodic(TimePeriod::Annual(AnnualDay { month, day }, time))
        }
        _ => unreachable!(),
    };

    Some(command::CreateCommand(EventData::new(interval, text)))
}

pub fn create_timedelta() -> Option<TimeDelta> {
    let choices = &[
        // "Days(i32)"
        "Trigger every N days",
        // "Hm { hours: i32, minutes: i32 }"
        "Trigger every h:mm hours and minutes",
    ];

    let selection = dialoguer::Select::with_theme(&theme::ColorfulTheme::default())
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
                Some(d) => Some(TimeDelta::Days(d)),
            }
        }
        1 => {
            let time = input_time("Input a time interval, eg. 2:15 for 2 hours 15 minutes");
            match time {
                None => return None,
                Some(t) => Some(TimeDelta::Hm(
                    t.hour().try_into().unwrap(),
                    t.minute().try_into().unwrap(),
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
        let input = dialoguer::Input::<String>::new()
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
        let input = dialoguer::Input::<String>::new()
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

fn sort_by_next_trigger(te1: &TrackedEvent, te2: &TrackedEvent) -> Ordering {
    match (te1.next_trigger_time(), te2.next_trigger_time()) {
        // Both are going to trigger in the future: later trigger == greater (goes later in list)
        (Some(t1), Some(t2)) => t1.cmp(&t2),
        // First one has a time when it's going to trigger, the other one is probably triggered
        // already => First is greater (goes later in list)
        (Some(_), None) => Ordering::Greater,
        // First one triggered, the second
        (None, Some(_)) => Ordering::Less,
        // Both have triggered, order by general interval duration
        (None, None) => te1
            .0
            .interval()
            .to_duration_heuristic()
            .cmp(&te2.0.interval().to_duration_heuristic()),
    }
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
        Err(tracker::LoadError::FileDoesNotExist) => {
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
        Err(tracker::LoadError::FileContentsMalformed(_, mal_path, _contents)) => {
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
