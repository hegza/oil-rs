use crate::prelude::*;
use crate::tracker;
use dialoguer::{theme::ColorfulTheme, Input, Select};
use std::fs;
use std::path::PathBuf;
use std::str::FromStr;
use tracker::Tracker;

/// Displays a UI for finding or creating a Tracker cache file.
pub fn ask_tracker_file() -> (Tracker, PathBuf) {
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
            let default_path = match selection {
                // Create
                0 => [
                    dirs::document_dir().unwrap_or_else(|| {
                        PathBuf::from(shellexpand::full(".").unwrap().into_owned())
                    }),
                    PathBuf::from("events.yaml"),
                ]
                .iter()
                .collect::<PathBuf>(),
                // Open
                1 => PathBuf::new(),
                _ => unreachable!(),
            };
            match prompt_path(Some(default_path.to_string_lossy().into_owned())) {
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
                        let tracker = Tracker::empty();
                        tracker.store_to_disk(&path);
                        return (tracker, path);
                    }
                    // Try to read the file and return its contents tracker
                    1 => {
                        trace!(
                            "Attempting to open file for reading at {}",
                            &path.to_str().unwrap()
                        );
                        match Tracker::from_path(&path) {
                            // Tracker could be parsed: return it
                            Ok(tracker) => return (tracker, path),
                            // File was found but contents are malformed: prompt user for action
                            Err(tracker::LoadError::FileContentsMalformed(
                                _,
                                mal_path,
                                _contents,
                            )) => {
                                return crate::view::troubleshoot::ask_malformed_action(
                                    PathBuf::from_str(&mal_path).unwrap(),
                                )
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
