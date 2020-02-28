use crate::views::prompt_file::ActionOnMalformedFile::{Cancel, ReplaceOriginal};
use crate::views::tracker;
use crate::views::tracker::Tracker;
use dialoguer::{theme::ColorfulTheme, Input, Select};
use log::{info, trace, warn};
use std::convert::TryFrom;
use std::fs;
use std::path::PathBuf;

/// Displays a UI for finding or creating a Tracker cache file.
///
/// Returns Some(tracker, path) if a valid path is wizarded, and None if user wishes to exit  
pub fn ask_tracker_file() -> Option<(Tracker, PathBuf)> {
    let choices = &[
        "Create a new file for storing events...",
        "Open an existing file with stored events...",
        // "Explore file system for a file", // not implemented
    ];

    'top: loop {
        let selection = Select::with_theme(&ColorfulTheme::default())
            .with_prompt("Please set a file to be used for storing user-defined events, this file will be loaded by default on next startup, navigate with arrow keys")
            .default(0)
            .items(&choices[..])
            .interact()
            .unwrap();

        'inner: loop {
            let default_path = shellexpand::tilde("~/.events.yaml").to_string();
            match prompt_path(Some(default_path)) {
                // Empty path: user wants to exit
                PromptPathResult::Empty => {
                    println!("Path empty, moving up\n");
                    continue 'top;
                }
                // Invalid path: re-try
                PromptPathResult::Failure(path_str) => {
                    println!("Invalid path: {}", &path_str);
                    continue 'inner;
                }
                PromptPathResult::CouldNotExpand => {
                    println!("Could not expand shell variables");
                    continue 'inner;
                }
                // Valid path: try to use the file
                PromptPathResult::Path(path) => match selection {
                    // Try to create the file based on a default tracker
                    0 => {
                        if let Some(parent) = path.parent() {
                            fs::create_dir_all(&parent)
                                .expect("could not recursively create the directory for the path");
                        }
                        // Create the file
                        match fs::OpenOptions::new().write(true).create(true).open(&path) {
                            Ok(_) => {
                                return Some((Tracker::empty(), path));
                            }
                            Err(e) => {
                                println!(
                                    "Could not create file '{}': {}",
                                    &path.to_str().expect("could not parse string from path"),
                                    e.to_string()
                                );
                                continue 'inner;
                            }
                        }
                    }
                    // Try to read the file and return its contents tracker
                    1 => {
                        trace!(
                            "Attempting to open file for reading at {}",
                            &path.to_str().unwrap()
                        );
                        match Tracker::from_path(&path) {
                            // Tracker could be parsed: return it
                            Ok(tracker) => return Some((tracker, path)),
                            // File empty, create default tracker
                            Err(tracker::LoadError::FileEmpty) => {
                                warn!("A tracker file was found at {} but it was empty, replacing with Tracker::empty()", &path.to_str().expect("could not parse string from path"));
                                return Some((Tracker::empty(), path));
                            }
                            // File was found but contents are malformed: prompt user for action
                            Err(tracker::LoadError::FileContentsMalformed(_)) => {
                                match ask_malformed_action() {
                                    ActionOnMalformedFile::ReplaceOriginal => {
                                        warn!("Creating a default tracker in place of a malformed one based on user request");
                                        return Some((Tracker::empty(), path));
                                    }
                                    ActionOnMalformedFile::Cancel => {
                                        info!("User requested cancellation upon encountering malformed tracker cache");
                                        panic!("User requested cancellation upon encountering malformed tracker cache");
                                    }
                                }
                            }
                            // File does not exist, retry
                            Err(tracker::LoadError::FileDoesNotExist) => {
                                println!("File not found");
                                continue 'inner;
                            }
                        }
                    }
                    _ => unreachable!(),
                },
            }
        }
    }
}

pub enum ActionOnMalformedFile {
    ReplaceOriginal,
    Cancel,
}

pub fn ask_malformed_action() -> ActionOnMalformedFile {
    let choices = &[
        "Ignore and exit",
        "Replace with default",
        // "Explore file system for a file", // not implemented
    ];
    let choice = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("File is malformed, probably older version. What to do?")
        .default(0)
        .items(&choices[..])
        .interact()
        .unwrap();
    match choice {
        0 => Cancel,
        1 => ReplaceOriginal,
        _ => unreachable!(),
    }
}

pub enum PromptPathResult {
    Path(PathBuf),
    Failure(String),
    CouldNotExpand,
    Empty,
}

pub fn prompt_path(initial: Option<String>) -> PromptPathResult {
    let mut input = Input::<String>::new();
    input.with_prompt("Choose file").allow_empty(true);
    if let Some(s) = initial {
        input.with_initial_text(&s);
    }

    if let Ok(path_str) = input.interact() {
        if path_str.is_empty() {
            return PromptPathResult::Empty;
        }

        let expanded = match shellexpand::full(&path_str) {
            Ok(e) => e,
            Err(_) => return PromptPathResult::CouldNotExpand,
        };
        match PathBuf::try_from(expanded.to_string()) {
            Ok(p) => PromptPathResult::Path(p),
            Err(_) => PromptPathResult::Failure(path_str),
        }
    } else {
        panic!("could not parse string from user input");
    }
}
