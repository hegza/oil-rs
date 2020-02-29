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
        // Launch the event creation wizard
        "add" | "a" => {
            let cmd = match crate::views::tracker::create_event_interact() {
                Some(cmd) => cmd,
                None => return None,
            };
            Some(ReversibleCommand(Box::new(cmd)))
        }
        // Create a remove event command
        "rm" => {
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
                    Err(_) => return None,
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
        "complete" | "c" => {
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
                    Err(_) => return None,
                },
            };
            let uid = match id_to_uid.get(&id) {
                Some(uid) => uid,
                None => {
                    println!("No item found for key {}", id);
                    return None;
                }
            };

            Some(ReversibleCommand(Box::new(CompleteCommand(*uid))))
        }
        "show" | "s" => Some(ReversibleCommand(Box::new(ShowCommand))),
        "hide" | "h" => Some(ReversibleCommand(Box::new(HideCommand))),
        "undo" | "u" => Some(Undo),
        "r" | "refresh" => Some(ReversibleCommand(Box::new(RefreshCommand))),
        // Exit the program
        "exit" | "quit" | "e" | "q" => Some(Exit),
        first_token => {
            match first_token.parse::<usize>() {
                Ok(id) => Some(ReversibleCommand(Box::new(CompleteCommand(
                    id_to_uid[&TrackerId(id)],
                )))),

                // Nothing was matched, return None
                Err(_) => {
                    debug!("Nothing was matched from {}", input);
                    None
                }
            }
        }
    }
}

pub enum CommandKind {
    ReversibleCommand(Box<dyn Apply>),
    Undo,
    Exit,
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
        let uid = tracker.register_event(event);

        // Undo
        Ok(Box::new(move |tracker| {
            tracker.unregister_event(uid);
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
        let (event, state) = match tracker.unregister_event(uid) {
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
            tracker.register_event_with_state(event, state);
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
