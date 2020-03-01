use super::error::CommandError;
use super::event_store::Uid as EventUid;
use super::Id as TrackerId;
use super::{Tracker, ViewState};
use crate::event::{Event, State};
use crate::prelude::*;
use chrono::Local;
use dialoguer::Confirmation;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt::{Display, Formatter};
use std::ops::Deref;

#[derive(Debug)]
pub enum CommandKind {
    ReversibleCommand(Box<dyn Apply>),
    Undo,
    Exit,
}

macro_rules! cmd {
    // Match expressions like: cmd!("", Enum::Variant, "", Enum::Variant)
    ( $name:expr, $matcher:path, $short_desc:expr, $com_input:path ) => {
        CommandKey {
            name: $name,
            keys: $matcher,
            short_desc: $short_desc,
            command_input: $com_input,
        }
    };
    // Match expressions like: cmd!("", ["", ""], "", Enum::Variant)
    ( $name:expr, $keys:expr, $short_desc:expr, $com_input:path ) => {
        CommandKey {
            name: $name,
            keys: KeyMatcher::List(&$keys),
            short_desc: $short_desc,
            command_input: $com_input,
        }
    };
}

/// This list is used to determine the user interface for the tracker, and the availability and parsing order of commands.
///
/// To add a new command:
/// 1. write the required fields to cmd macro
/// 2. add enum variant
/// 3. implement parser in match_command (as prompted by compiler)
/// cmd!('key on UI', 'acceptable keys', 'short decription', 'CommandInput enum for matching to functionality')
pub static COMMAND_KEYS: CommandKeys = {
    use CommandInput::*;
    CommandKeys(&[
        cmd!(
            "<id>",
            KeyMatcher::Usize,
            "set event as completed",
            Complete
        ),
        cmd!(
            "create",
            ["create", "c"],
            "create an event interactively",
            Create
        ),
        cmd!("rm <id>", ["rm"], "remove registered event", Remove),
        cmd!(
            "alter <id>",
            ["alter", "a"],
            "alter an existing event",
            Alter
        ),
        cmd!(
            "trigger <id>",
            ["trigger", "trig", "t"],
            "manually trigger an event now",
            Trigger
        ),
        cmd!(
            "show",
            ["show", "s"],
            "show all events and extended status",
            Show
        ),
        cmd!(
            "hide",
            ["hide", "h"],
            "hide untriggered events and extended event status",
            Hide
        ),
        cmd!("undo", ["undo", "u"], "undo last action", Undo),
        cmd!(
            "exit",
            ["exit", "quit", "q"],
            "exit interactive client",
            Exit
        ),
    ])
};

#[derive(Clone, Copy)]
enum CommandInput {
    Create,
    Remove,
    Alter,
    Trigger,
    Show,
    Hide,
    Undo,
    Complete,
    Exit,
}

/// Represents the interface between user interaction and internal commands
///
/// Used for user-interaction displays and for resolving which command to execute.
pub struct CommandKey {
    /// Name is displayable in UI, 5 characters or so
    pub name: &'static str,
    /// The keys are the valid "first words" of input
    keys: KeyMatcher,
    /// Short description displayable in UI as a one-liner
    pub short_desc: &'static str,
    /// An enum for matching the inputs to functionality
    command_input: CommandInput,
}

enum KeyMatcher {
    List(&'static [&'static str]),
    Usize,
}

pub struct CommandKeys(&'static [CommandKey]);

impl CommandKeys {
    fn is_match(&self, s: &str) -> bool {
        for key in self.0 {
            match key.keys {
                KeyMatcher::List(keys) => {
                    if keys.contains(&s) {
                        return true;
                    }
                }
                KeyMatcher::Usize => {
                    if s.parse::<usize>().is_ok() {
                        return true;
                    }
                }
            }
        }
        false
    }
    fn command_input(&self, s: &str) -> Option<CommandInput> {
        for key in self.0 {
            match key.keys {
                KeyMatcher::List(keys) => {
                    if keys.contains(&s) {
                        return Some(key.command_input);
                    }
                }
                // If the key just parses as a valid usize, return the matching CommandInput
                KeyMatcher::Usize => {
                    if s.parse::<usize>().is_ok() {
                        return Some(key.command_input);
                    }
                }
            }
        }
        None
    }
}

impl Deref for CommandKeys {
    type Target = &'static [CommandKey];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// Interpret user action as a command
pub fn match_command(
    input: &str,
    id_to_uid: &BTreeMap<TrackerId, EventUid>,
) -> Option<CommandKind> {
    // Sanitize
    let input = input.trim();

    // Tokenize
    let mut tokens = input.split_whitespace();

    use CommandKind::*;

    // No tokens? Early out
    if tokens.clone().count() == 0 {
        // Refresh by default
        return Some(ReversibleCommand(Box::new(RefreshCommand)));
    }

    // Try to match a command from the first token
    match tokens.nth(0).unwrap() {
        first_token if COMMAND_KEYS.is_match(first_token) => {
            match COMMAND_KEYS.command_input(first_token) {
                Some(cmd_input) => match cmd_input {
                    // 'Create' launches the event creation wizard
                    CommandInput::Create => {
                        let cmd = match crate::views::tracker::create_event_interact() {
                            Some(cmd) => cmd,
                            None => return None,
                        };
                        Some(ReversibleCommand(Box::new(cmd)))
                    }
                    // Rm removes an event with id
                    CommandInput::Remove => id_token_to_uid_interact(&mut tokens, id_to_uid)
                        .map(|uid| ReversibleCommand(Box::new(RemoveCommand(uid)))),
                    CommandInput::Alter => {
                        // Resolve uid first, and early out if not found
                        let uid = match id_token_to_uid_interact(&mut tokens, id_to_uid) {
                            Some(uid) => uid,
                            None => return None,
                        };

                        // TODO: create a real interface for the alter command
                        println!("Hack: using 'create event' interface to replace the old event");
                        let cmd = match crate::views::tracker::create_event_interact() {
                            Some(cmd) => AlterCommand(uid, cmd.0),
                            None => return None,
                        };
                        Some(ReversibleCommand(Box::new(cmd)))
                    }
                    CommandInput::Trigger => id_token_to_uid_interact(&mut tokens, id_to_uid)
                        .map(|uid| ReversibleCommand(Box::new(TriggerCommand(uid)))),
                    CommandInput::Show => Some(ReversibleCommand(Box::new(ShowCommand))),
                    CommandInput::Hide => Some(ReversibleCommand(Box::new(HideCommand))),
                    CommandInput::Undo => Some(Undo),
                    // Exit the program
                    CommandInput::Exit => Some(Exit),
                    //CommandInput::Refresh => Some(ReversibleCommand(Box::new(RefreshCommand))),
                    // Set item status as 'completed'
                    CommandInput::Complete => {
                        // Try to match a number for "complete command"
                        match first_token.parse::<usize>() {
                            Ok(id) => {
                                let uid = match id_to_uid.get(&TrackerId(id)) {
                                    Some(uid) => uid,
                                    None => {
                                        println!("No event found for key {}", id);
                                        return None;
                                    }
                                };

                                Some(ReversibleCommand(Box::new(CompleteCommand(*uid))))
                            }

                            // Nothing was matched, return None
                            Err(_) => {
                                debug!("Nothing was matched from {}", input);
                                println!("Nothing matched from {}", input);
                                None
                            }
                        }
                    }
                },
                None => unreachable!(),
            }
        }
        first_token => {
            debug!(
                "Nothing was matched from '{}', because first token '{}' does not match to a command",
                input, first_token
            );
            None
        }
    }
}

pub trait Apply: Display + std::fmt::Debug {
    fn apply(&self, tracker: &mut Tracker) -> Result<Box<dyn FnOnce(&mut Tracker)>, CommandError>;
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
pub struct RemoveCommand(pub EventUid);

#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
pub struct CompleteCommand(pub EventUid);

#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
pub struct ShowCommand;

#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
pub struct HideCommand;

#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
pub struct RefreshCommand;

/// Generate a type of the form `struct Item;` or `struct Item(Param)`.
macro_rules! gen_type(
    ( $(#[$attr:meta])* $Cmd:ident ) => {
        $(#[$attr])*
        #[derive(Serialize, Deserialize, Clone, Debug)]
        pub struct $Cmd;
    };
    ( $(#[$attr:meta])* $Cmd:ident($( $type:ty ),*) ) => {
        $(#[$attr])*
        #[derive(Serialize, Deserialize, Clone, Debug)]
        pub struct $Cmd($(
            pub $type,
        )*);
    };
);

macro_rules! impl_cmd_body{
    ($Cmd:ident, $sel:ident, $tracker:ident, $apply:block) => {
        impl Apply for $Cmd {
            fn apply(&$sel, $tracker: &mut Tracker) -> Result<Box<dyn FnOnce(&mut Tracker)>, CommandError> $apply
        }

        impl Display for $Cmd {
            fn fmt(&$sel, f: &mut Formatter<'_>) -> std::fmt::Result {
                // TODO: implement a better version using macro parameters if necessary
                write!(f, "{}", stringify!($Cmd))
            }
        }
    }
}

// Define command general implementations
macro_rules! impl_cmd(
    ( $(#[$attr:meta])* $Cmd:ident($( $type:ty ),*), |$sel:ident, $tracker:ident| $apply:block ) => {
        gen_type!($(#[$attr])* $Cmd($($type),*));
        impl_cmd_body!($Cmd, $sel, $tracker, $apply);
    };
    ( $(#[$attr:meta])* $Cmd:ident, |$sel:ident, $tracker:ident| $apply:block ) => {
        gen_type!($(#[$attr])* $Cmd);
        impl_cmd_body!($Cmd, $sel, $tracker, $apply);
    };
);

impl_cmd!(
    /// CreateCommand creates the given event with a UID
    /// Undo will remove the event with the UID
    CreateCommand(Event),
    |self, tracker| {
        let event = self.0.clone();

        // Op
        let uid = tracker.add_event(event);

        // Undo
        Ok(Box::new(move |tracker| {
            tracker.remove_event(uid);
        }))
    }
);

impl_cmd!(AlterCommand(EventUid, Event), |self, tracker| {
    let old_uid = self.0;

    // Op
    // Perform a replacement
    let new_event = &self.1;
    let old_event = match tracker.remove_event(old_uid) {
        None => {
            warn!("AlterCommand failed because the event being altered did not exist");
            return Err(CommandError::EventNotFound(old_uid));
        }
        Some(e) => e,
    };
    let new_uid = tracker.add_event_with_state(new_event.clone(), old_event.1.clone());

    // Undo
    Ok(Box::new(move |tracker| {
        tracker.remove_event(new_uid);
        tracker.add_event_with_state(old_event.0, old_event.1);
    }))
});

impl_cmd!(TriggerCommand(EventUid), |self, tracker| {
    let uid = self.0;

    // Op
    let old_state = match tracker.get_event_state_mut(uid) {
        None => {
            warn!("TriggerCommand failed because the event being triggered did not exist");
            return Err(CommandError::EventNotFound(uid));
        }
        Some(state) => {
            let old_state = state.clone();
            state.trigger_now(Local::now());
            old_state
        }
    };

    // Undo
    Ok(Box::new(move |tracker| {
        match tracker.get_event_state_mut(uid) {
            None => warn!(
                "Undo failed for TriggerCommand with uid {} because uid did not exist",
                uid
            ),
            Some(state) => {
                *state = old_state;
            }
        }
    }))
});

impl Display for CommandKind {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            CommandKind::ReversibleCommand(apply) => write!(f, "{}", apply),
            CommandKind::Undo => write!(f, "{}", "cmd-undo"),
            CommandKind::Exit => write!(f, "{}", "cmd-exit"),
        }
    }
}

impl Display for RemoveCommand {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let id = self.0;
        write!(f, "cmd-rm({})", id)
    }
}

impl Apply for RemoveCommand {
    fn apply(&self, tracker: &mut Tracker) -> Result<Box<dyn FnOnce(&mut Tracker)>, CommandError> {
        let uid = self.0;

        // Op
        let (event, state) = match tracker.remove_event(uid) {
            None => {
                warn!(
                    "Tried to unregister a non-existing event with unique-uid {} -> no-op",
                    uid
                );
                return Err(CommandError::EventNotFound(uid));
            }
            Some(x) => x,
        };

        // Undo
        Ok(Box::new(move |tracker| {
            tracker.add_event_with_state(event, state);
        }))
    }
}

impl Display for CompleteCommand {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let uid: EventUid = self.0;
        write!(f, "cmd-complete({})", uid)
    }
}

impl Apply for CompleteCommand {
    fn apply(&self, tracker: &mut Tracker) -> Result<Box<dyn FnOnce(&mut Tracker)>, CommandError> {
        let uid = self.0;

        // Op
        let old_state = match tracker.get_event_state_mut(uid) {
            Some(state) => {
                let old_state = state.clone();
                *state = State::Completed(Local::now());
                old_state
            }
            None => return Err(CommandError::EventNotFound(uid)),
        };

        // Undo
        Ok(Box::new(move |tracker| {
            match tracker.get_event_state_mut(uid) {
                None => warn!(
                    "Undo failed for CompleteCommand with uid {} because uid did not exist",
                    uid
                ),
                Some(state) => {
                    *state = old_state;
                }
            }
        }))
    }
}

impl Display for ShowCommand {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "cmd-show")
    }
}

impl Apply for ShowCommand {
    fn apply(&self, tracker: &mut Tracker) -> Result<Box<dyn FnOnce(&mut Tracker)>, CommandError> {
        let prev = tracker.set_state(ViewState::Extended);
        Ok(Box::new(move |tracker| {
            tracker.set_state(prev);
        }))
    }
}

impl Display for HideCommand {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "cmd-hide")
    }
}

impl Apply for HideCommand {
    fn apply(&self, tracker: &mut Tracker) -> Result<Box<dyn FnOnce(&mut Tracker)>, CommandError> {
        let prev = tracker.set_state(ViewState::Standard);
        Ok(Box::new(move |tracker| {
            tracker.set_state(prev);
        }))
    }
}

impl Display for RefreshCommand {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "cmd-refresh")
    }
}

impl Apply for RefreshCommand {
    fn apply(&self, _tracker: &mut Tracker) -> Result<Box<dyn FnOnce(&mut Tracker)>, CommandError> {
        // Op: no-op

        // Undo: no-op
        Ok(Box::new(move |_| {}))
    }
}

fn match_id_interact<'i, I>(input: &'i mut I) -> Option<TrackerId>
where
    I: Iterator<Item = &'i str>,
{
    match input.nth(0) {
        None => {
            Confirmation::new()
                .with_text("rm requires another parameter: <uid>")
                .show_default(false)
                .interact()
                .unwrap();
            None
        }
        Some(id) => match id.parse::<usize>() {
            Ok(id) => Some(TrackerId(id)),
            Err(_) => {
                println!("Could not parse {} into an id", id);
                None
            }
        },
    }
}

/// Returns the mapped UID based on the UI Tracker ID received as input
fn id_token_to_uid_interact<'i, I>(
    input: &'i mut I,
    id_to_uid: &BTreeMap<TrackerId, EventUid>,
) -> Option<EventUid>
where
    I: Iterator<Item = &'i str>,
{
    let id = match match_id_interact(input) {
        Some(id) => id,
        None => return None,
    };
    match id_to_uid.get(&id) {
        Some(uid) => Some(*uid),
        None => {
            println!("No item found for key {}", id);
            return None;
        }
    }
}
