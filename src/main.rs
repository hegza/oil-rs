mod config;
pub mod datamodel;
pub mod prelude;
mod tracker;
mod view;

use config::Config;
use prelude::*;
use simplelog::Config as LogConfig;
use simplelog::*;
use view::tracker_cli::TrackerCli;

const DEFAULT_CONFIG_NAME: &str = "oil.yaml";

fn main() {
    CombinedLogger::init(vec![
        TermLogger::new(LevelFilter::Warn, LogConfig::default(), TerminalMode::Mixed).unwrap(),
        WriteLogger::new(
            LevelFilter::Trace,
            LogConfig::default(),
            std::fs::File::create(".oil.log").unwrap(),
        ),
    ])
    .unwrap();

    println!("This application will track recurring events as defined by the user");

    // Load the default config file or create it
    info!("Loading default config");
    let mut config = Config::load_default();

    // Try to open the last opened file from config cache
    info!("Looking for last opened path in config");
    let last_path = {
        config.last_open.map(|last| std::path::PathBuf::try_from(last)
                    .expect("cannot parse path from cached 'last_open' string"))
    };

    // Set up the tracker or prompt the user for one
    let (tracker, path) = match last_path {
        // Store the tracker file as the last used one and return
        None => {
            info!("No 'last opened' in config, asking user for a tracker file");
            let (tracker, path) = view::prompt_file::ask_tracker_file();
            config.last_open = Some(
                path.canonicalize()
                    .expect("cannot canonicalize path")
                    .to_string_lossy()
                    .to_string(),
            );
            config.store_default();
            (tracker, path)
        }
        Some(p) => {
            info!(
                "Setting up tracker from last opened path at: {}",
                p.canonicalize()
                    .expect("cannot canonicalize path")
                    .to_string_lossy()
            );
            view::tracker_cli::set_up_at(p)
        }
    };

    info!("User starts interaction with tracker");
    let mut gui = TrackerCli::new(tracker);
    gui.interact_modal(&path);
}
