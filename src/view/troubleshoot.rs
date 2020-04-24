use crate::prelude::*;
use crate::tracker::Tracker;
use dialoguer::{theme::ColorfulTheme, Select};
use std::io::Read;
use std::path::PathBuf;

pub enum ActionOnMalformedFile {
    ReplaceOriginal,
    PrintFile,
    Cancel,
}

pub fn ask_malformed_action<'te>(p: PathBuf) -> (Tracker, PathBuf) {
    let c = &[
        "Ignore and exit",
        "Replace with default",
        "Print malformed file",
        // "Explore file system for a file", // not implemented
    ];
    loop {
        let choice = choices("File is malformed, possibly older version. What to do?", c);
        use ActionOnMalformedFile::*;
        let choice = match choice {
            0 => Cancel,
            1 => ReplaceOriginal,
            2 => PrintFile,
            _ => unreachable!(),
        };
        match choice {
            ReplaceOriginal => {
                warn!(
                    "Creating a default tracker in place of a malformed one based on user request"
                );
                let tracker = Tracker::empty();
                tracker.store_to_disk(&p);
                return (tracker, p);
            }
            Cancel => {
                info!("User requested cancellation upon encountering malformed tracker cache");
                println!("A tracker file is required to use the program. Exiting...");
                std::process::exit(0);
            }
            PrintFile => {
                let mut contents = String::new();
                std::fs::File::open(&p)
                    .unwrap()
                    .read_to_string(&mut contents)
                    .unwrap();
                println!("path: {}\n{:#?}", p.to_string_lossy(), contents);
                continue;
            }
        }
    }
}

pub fn choices(text: &str, choices: &[&str]) -> usize {
    Select::with_theme(&ColorfulTheme::default())
        .with_prompt(text)
        .default(0)
        .items(&choices[..])
        .interact()
        .unwrap()
}
