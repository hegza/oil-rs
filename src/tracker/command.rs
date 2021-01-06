use super::error::CommandError;
use super::event_store::Uid;
use super::Tracker;
use crate::prelude::*;
use crate::view::tracker_cli::{TrackerCli, ViewState};
use dialoguer::Confirmation;
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};
use std::ops::Deref;

#[derive(Debug)]
pub enum CommandKind {
    CliCommand(Box<dyn Apply>),
    DataCommand(Box<dyn Apply>),
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

/// This list is used to determine the user interface for the tracker, and the
/// availability and parsing order of commands.
///
/// To add a new command:
/// 1. write the required fields to cmd macro
/// 2. add enum variant
/// 3. implement parser in match_command (as prompted by compiler)
/// cmd!('key on UI', 'acceptable keys', 'short decription', 'CommandInput enum
/// for matching to functionality')
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
            "trig <id>",
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
/// Used for user-interaction displays and for resolving which command to
/// execute.
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
pub fn match_command(input: &str, id_to_uid: &[Uid]) -> Option<CommandKind> {
    // Sanitize
    let input = input.trim();

    // Tokenize and trim tokens
    let tokens = input.split_whitespace().map(|token| token.trim());

    // No tokens? Early out
    if tokens.clone().count() == 0 {
        // Refresh by default
        return Some(DataCommand(Box::new(RefreshCommand)));
    }

    // Try to match a command from the first token
    use CommandKind::*;
    match tokens.clone().nth(0).unwrap() {
        first_token if COMMAND_KEYS.is_match(first_token) => {
            match COMMAND_KEYS.command_input(first_token) {
                Some(cmd_input) => match cmd_input {
                    // 'Create' launches the event creation wizard
                    CommandInput::Create => {
                        let cmd = match crate::view::tracker_cli::create_event_interact() {
                            Some(cmd) => cmd,
                            None => return None,
                        };
                        Some(DataCommand(Box::new(cmd)))
                    }
                    // Rm removes an event with id
                    CommandInput::Remove => {
                        id_token_to_uid_interact(&mut tokens.skip(1), id_to_uid)
                            .map(|uid| DataCommand(Box::new(RemoveCommand(vec![uid]))))
                    }
                    CommandInput::Alter => {
                        // Resolve uid first, and early out if not found
                        let uid = match id_token_to_uid_interact(&mut tokens.skip(1), id_to_uid) {
                            Some(uid) => uid,
                            None => return None,
                        };

                        // TODO: create a real interface for the alter command
                        println!("Hack: using 'create event' interface to replace the old event");
                        let cmd = match crate::view::tracker_cli::create_event_interact() {
                            Some(cmd) => AlterCommand(uid, cmd.0),
                            None => return None,
                        };
                        Some(DataCommand(Box::new(cmd)))
                    }
                    CommandInput::Trigger => {
                        id_token_to_uid_interact(&mut tokens.skip(1), id_to_uid)
                            .map(|uid| DataCommand(Box::new(TriggerCommand(uid))))
                    }
                    CommandInput::Show => Some(CliCommand(Box::new(ShowCommand))),
                    CommandInput::Hide => Some(CliCommand(Box::new(HideCommand))),
                    CommandInput::Undo => Some(Undo),
                    // Exit the program
                    CommandInput::Exit => Some(Exit),
                    //CommandInput::Refresh => Some(ReversibleCommand(Box::new(RefreshCommand))),
                    // Set item status as 'completed'
                    CommandInput::Complete => {
                        // Try to match a number ID from all elements
                        let uids = tokens
                            .filter_map(|token| {
                                token.parse::<usize>().ok().and_then(|id| id_to_uid.get(id))
                            })
                            .cloned()
                            .collect::<Vec<Uid>>();

                        // Set up the command
                        Some(DataCommand(Box::new(CompleteCommand(uids))))
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

pub enum CommandReceiver<'r> {
    Tracker(&'r mut Tracker),
    TrackerCli(&'r mut TrackerCli),
}

impl std::fmt::Debug for CommandReceiver<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use CommandReceiver::*;
        match self {
            Tracker(_) => write!(f, "CommandReceiver::Tracker"),
            TrackerCli(_) => write!(f, "CommandReceiver::TrackerCli"),
        }
    }
}

pub type FnApply = Box<dyn FnOnce(&mut Tracker)>;
pub type CommandResult = Result<Option<FnApply>, CommandError>;

pub trait Apply: Display + std::fmt::Debug {
    fn apply(&self, target: CommandReceiver) -> CommandResult;
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct RemoveCommand(pub Vec<Uid>);

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CompleteCommand(pub Vec<Uid>);

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
    ($Cmd:ident, $sel:ident, $target:ident, $apply:block) => {
        impl Apply for $Cmd {
            fn apply(& $sel, $target: CommandReceiver) -> CommandResult $apply
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
    ( $(#[$attr:meta])* $Cmd:ident($( $type:ty ),*), |$sel:ident, $target:ident| $apply:block ) => {
        gen_type!($(#[$attr])* $Cmd($($type),*));
        impl_cmd_body!($Cmd, $sel, $target, $apply);
    };
    ( $(#[$attr:meta])* $Cmd:ident, |$sel:ident, $target:ident| $apply:block ) => {
        gen_type!($(#[$attr])* $Cmd);
        impl_cmd_body!($Cmd, $sel, $target, $apply);
    };
);

impl_cmd!(
    /// CreateCommand creates the given event with a UID
    /// Undo will remove the event with the UID
    CreateCommand(EventData),
    |self, target| {
        match target {
            CommandReceiver::Tracker(tracker) => {
                let event = self.0.clone();

                // Op
                let uid = tracker.add_event(event);

                // Undo
                Ok(Some(Box::new(move |tracker: &mut Tracker| {
                    tracker.remove_event(uid);
                })))
            }
            target => Err(CommandError::InvalidReceiver(format!("{:?}", target))),
        }
    }
);

impl_cmd!(AlterCommand(Uid, EventData), |self, target| {
    match target {
        CommandReceiver::Tracker(tracker) => {
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
            let new_uid = tracker.add_event_with_status(new_event.clone(), old_event.1.clone());

            // Undo
            Ok(Some(Box::new(move |tracker: &mut Tracker| {
                tracker.remove_event(new_uid);
                tracker.add_event_with_status(old_event.0, old_event.1);
            })))
        }
        CommandReceiver::TrackerCli(_) => {
            Err(CommandError::InvalidReceiver(format!("{:?}", target)))
        }
    }
});

impl_cmd!(TriggerCommand(Uid), |self, target| {
    match target {
        CommandReceiver::Tracker(tracker) => {
            let uid = self.0;

            // Op
            let old_state = match tracker.event_mut(uid) {
                None => {
                    warn!("TriggerCommand failed because the event being triggered did not exist");
                    return Err(CommandError::EventNotFound(uid));
                }
                Some(TrackedEvent(_, state)) => {
                    let old_state = state.clone();
                    state.trigger_now();
                    old_state
                }
            };

            // Undo
            Ok(Some(Box::new(move |tracker: &mut Tracker| {
                match tracker.event_mut(uid) {
                    None => warn!(
                        "Undo failed for TriggerCommand with uid {} because uid did not exist",
                        uid
                    ),
                    Some(TrackedEvent(_, ref mut state)) => {
                        *state = old_state;
                    }
                }
            })))
        }
        CommandReceiver::TrackerCli(_) => {
            Err(CommandError::InvalidReceiver(format!("{:?}", target)))
        }
    }
});

impl Display for CommandKind {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            CommandKind::CliCommand(apply) => write!(f, "{}", apply),
            CommandKind::DataCommand(apply) => write!(f, "{}", apply),
            CommandKind::Undo => write!(f, "{}", "cmd-undo"),
            CommandKind::Exit => write!(f, "{}", "cmd-exit"),
        }
    }
}

impl Display for RemoveCommand {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "cmd-rm({:?})", &self.0)
    }
}

impl Apply for RemoveCommand {
    fn apply(&self, target: CommandReceiver) -> CommandResult {
        match target {
            CommandReceiver::Tracker(tracker) => {
                let uids = &self.0;

                let mut events = Vec::with_capacity(uids.len());
                for &uid in uids {
                    // Op
                    match tracker.remove_event(uid) {
                        None => {
                            warn!(
                                "Tried to unregister a non-existing event with unique-uid {} -> no-op",
                                uid
                            );
                            return Err(CommandError::EventNotFound(uid));
                        }
                        Some(x) => events.push(x),
                    }
                }

                // Undo
                Ok(Some(Box::new(move |tracker| {
                    for event in events {
                        tracker.add_event_with_status(event.0, event.1);
                    }
                })))
            }
            CommandReceiver::TrackerCli(_) => {
                Err(CommandError::InvalidReceiver(format!("{:?}", target)))
            }
        }
    }
}

impl Display for CompleteCommand {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "cmd-complete({:?})", &self.0)
    }
}

impl Apply for CompleteCommand {
    fn apply(&self, target: CommandReceiver) -> CommandResult {
        match target {
            CommandReceiver::Tracker(tracker) => {
                let uids = &self.0;

                let mut states = Vec::with_capacity(uids.len());
                for &uid in uids {
                    // Op
                    let old_state = match tracker.event_mut(uid) {
                        Some(TrackedEvent(ev, state)) => {
                            let old_state = state.clone();
                            match ev.interval() {
                                // If event is timespan-based, set it complete, post-poning next
                                // triggering
                                Interval::FromLastCompletion(_) => {
                                    state.complete_now();
                                }
                                // If event is periodic, set it as skipped, canceling the next
                                // triggering
                                Interval::Periodic(_) => {
                                    state.skip_now();
                                }
                            }
                            old_state
                        }
                        None => return Err(CommandError::EventNotFound(uid)),
                    };
                    states.push((uid, old_state));
                }

                // Undo
                Ok(Some(Box::new(move |tracker| {
                    for (uid, old_state) in states {
                        match tracker.event_mut(uid) {
                            None => warn!(
                            "Undo failed for CompleteCommand with uid {} because uid did not exist",
                            uid
                        ),
                            Some(TrackedEvent(_, state)) => {
                                *state = old_state;
                            }
                        }
                    }
                })))
            }
            CommandReceiver::TrackerCli(_) => {
                Err(CommandError::InvalidReceiver(format!("{:?}", target)))
            }
        }
    }
}

impl Display for ShowCommand {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "cmd-show")
    }
}

impl Apply for ShowCommand {
    fn apply(&self, target: CommandReceiver) -> CommandResult {
        match target {
            CommandReceiver::TrackerCli(cli) => {
                cli.set_state(ViewState::Extended);
                Ok(None)
            }
            CommandReceiver::Tracker(_) => {
                Err(CommandError::InvalidReceiver(format!("{:?}", target)))
            }
        }
    }
}

impl Display for HideCommand {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "cmd-hide")
    }
}

impl Apply for HideCommand {
    fn apply(&self, target: CommandReceiver) -> CommandResult {
        match target {
            CommandReceiver::Tracker(_) => {
                Err(CommandError::InvalidReceiver(format!("{:?}", target)))
            }
            CommandReceiver::TrackerCli(cli) => {
                cli.set_state(ViewState::Standard);
                Ok(None)
            }
        }
    }
}

impl Display for RefreshCommand {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "cmd-refresh")
    }
}

impl Apply for RefreshCommand {
    fn apply(&self, _target: CommandReceiver) -> CommandResult {
        // Op: no-op

        // Undo: no-op
        Ok(Some(Box::new(move |_| {})))
    }
}

fn match_id_interact<'i, I>(input: &'i mut I) -> Option<usize>
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
            Ok(id) => Some(id),
            Err(_) => {
                println!("Could not parse {} into an id", id);
                None
            }
        },
    }
}

/// Returns the mapped UID based on the UI Tracker ID received as input
fn id_token_to_uid_interact<'i, I>(input: &'i mut I, id_to_uid: &[Uid]) -> Option<Uid>
where
    I: Iterator<Item = &'i str>,
{
    let id = match match_id_interact(input) {
        Some(id) => id,
        None => return None,
    };
    match id_to_uid.get(id) {
        Some(uid) => Some(*uid),
        None => {
            println!("No item found for key {}", id);
            return None;
        }
    }
}
