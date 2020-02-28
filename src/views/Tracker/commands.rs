use super::error::CommandError;
use super::event::Id;
use super::Tracker;
use crate::views::tracker::ViewState;
use log::debug;
use serde::{Deserialize, Serialize};

/// Interpret user action as a command
pub fn match_command(input: &str) -> Option<CommandKind> {
    // Sanitize
    let input = input.trim();

    // Tokenize
    let mut tokens = input.split_whitespace();

    // No tokens? Early out
    if tokens.clone().count() == 0 {
        return None;
    }

    // Try to match a command from the first token
    use CommandKind::*;
    match tokens.nth(0).unwrap() {
        // Launch the event creation wizard
        "add" | "a" => {
            let cmd = match crate::views::tracker::create_event() {
                Some(cmd) => cmd,
                None => return None,
            };
            Some(Add(cmd))
        }
        // Create a remove event command
        "rm" => {
            let id = match tokens.nth(0) {
                None => {
                    println!("rm requires another parameter: <id>");
                    return None;
                }
                Some(id) => match id.parse::<usize>() {
                    Ok(id) => id,
                    Err(_) => return None,
                },
            };

            Some(Remove(RemoveCommand(Id(id))))
        }
        "show" | "s" => Some(Show(ShowCommand)),
        "hide" | "h" => Some(Hide(HideCommand)),
        "undo" | "u" => Some(Undo(UndoCommand)),
        "r" | "refresh" => Some(Refresh(RefreshCommand)),
        // Exit the program
        "exit" | "quit" | "e" | "q" => Some(Exit),
        first_token => {
            match first_token.parse::<usize>() {
                Ok(id) => Some(Complete(CompleteCommand(Id(id)))),

                // Nothing was matched, return None
                Err(_) => {
                    debug!("Nothing was matched from {}", input);
                    None
                }
            }
        }
    }
}

#[derive(Debug)]
pub enum CommandKind {
    Add(AddCommand),
    Remove(RemoveCommand),
    Complete(CompleteCommand),
    Show(ShowCommand),
    Hide(HideCommand),
    Undo(UndoCommand),
    Refresh(RefreshCommand),
    Exit,
}

pub trait Apply {
    fn apply(&self, tracker: &mut Tracker) -> Result<(), CommandError>;
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AddCommand(pub super::event::Event);

impl Apply for AddCommand {
    fn apply(&self, tracker: &mut Tracker) -> Result<(), CommandError> {
        unimplemented!();
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
pub struct RemoveCommand(Id);

impl Apply for RemoveCommand {
    fn apply(&self, tracker: &mut Tracker) -> Result<(), CommandError> {
        unimplemented!();
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
pub struct CompleteCommand(Id);

impl Apply for CompleteCommand {
    fn apply(&self, tracker: &mut Tracker) -> Result<(), CommandError> {
        unimplemented!();
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
pub struct ShowCommand;

impl Apply for ShowCommand {
    fn apply(&self, tracker: &mut Tracker) -> Result<(), CommandError> {
        let _prev = tracker.set_state(ViewState::Extended);
        Ok(())
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
pub struct HideCommand;

impl Apply for HideCommand {
    fn apply(&self, tracker: &mut Tracker) -> Result<(), CommandError> {
        let _prev = tracker.set_state(ViewState::Standard);
        Ok(())
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
pub struct UndoCommand;

impl Apply for UndoCommand {
    fn apply(&self, tracker: &mut Tracker) -> Result<(), CommandError> {
        unimplemented!();
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
pub struct RefreshCommand;

impl Apply for RefreshCommand {
    fn apply(&self, tracker: &mut Tracker) -> Result<(), CommandError> {
        unimplemented!();
    }
}
