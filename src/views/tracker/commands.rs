use super::error::CommandError;
use super::event_store::Uid as EventUid;
use super::Id as TrackerId;
use super::{Tracker, ViewState};
use crate::event::Event;
use crate::prelude::*;
use dialoguer::Confirmation;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt::{Display, Formatter};
use std::ops::Deref;

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
        cmd!("add", ["add", "a"], "create an event interactively", Add),
        cmd!("rm <id>", ["rm"], "remove registered event", Remove),
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
    Add,
    Remove,
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
                    // Add launches the event creation wizard
                    CommandInput::Add => {
                        let cmd = match crate::views::tracker::create_event_interact() {
                            Some(cmd) => cmd,
                            None => return None,
                        };
                        Some(ReversibleCommand(Box::new(cmd)))
                    }
                    // Rm removes an event with id
                    CommandInput::Remove => {
                        let id = match tokens.nth(0) {
                            None => {
                                Confirmation::new()
                                    .with_text("rm requires another parameter: <uid>")
                                    .show_default(false)
                                    .interact()
                                    .unwrap();
                                return None;
                            }
                            Some(id) => match id.parse::<usize>() {
                                Ok(id) => TrackerId(id),
                                Err(_) => {
                                    println!("Could not parse {} into an id", id);
                                    return None;
                                }
                            },
                        };
                        let uid = match id_to_uid.get(&id) {
                            Some(uid) => uid,
                            None => {
                                println!("No item found for key {}", id);
                                return None;
                            }
                        };

                        Some(ReversibleCommand(Box::new(RemoveCommand(*uid))))
                    }
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

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AddCommand(pub Event);

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

impl Display for CommandKind {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            CommandKind::ReversibleCommand(apply) => write!(f, "{}", apply),
            CommandKind::Undo => write!(f, "{}", "cmd-undo"),
            CommandKind::Exit => write!(f, "{}", "cmd-exit"),
        }
    }
}

pub trait Apply: Display {
    fn apply(&self, tracker: &mut Tracker) -> Result<Box<dyn FnOnce(&mut Tracker)>, CommandError>;
}

impl Display for AddCommand {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let event = self.0.clone();
        write!(f, "cmd-add({})", event)
    }
}

impl Apply for AddCommand {
    fn apply(&self, tracker: &mut Tracker) -> Result<Box<dyn FnOnce(&mut Tracker)>, CommandError> {
        let event = self.0.clone();

        // Op
        let uid = tracker.add_event(event);

        // Undo
        Ok(Box::new(move |tracker| {
            tracker.remove_event(uid);
        }))
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
        let (op_id, _time) = match tracker.complete_now(uid) {
            Some(x) => x,
            None => return Err(CommandError::EventNotFound(uid)),
        };

        // Undo
        Ok(Box::new(move |tracker| tracker.rewind_complete(op_id)))
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
