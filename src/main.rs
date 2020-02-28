mod config;
mod views;

use config::Config;
use log::info;
use views::tracker::Tracker;

const DEFAULT_CONFIG_NAME: &str = "oil.yaml";

fn main() {
    println!("This application will track recurring events as defined by the user");

    // Load the default config file or create it
    info!("Loading default config");
    let mut config = Config::load_default();

    // Try to open the last opened file from config cache
    info!("Looking for last opened file");
    let cached = Tracker::from_config(&config);

    // Set up the tracker or prompt the user for one
    let (mut tracker, path) = cached.unwrap_or_else(|| {
        info!("No last opened file available, asking the user to provide a file");
        // No "last open file", prompt for one
        if let Some((tracker, path)) = views::prompt_file::ask_tracker_file() {
            // Store path in cache
            config.last_open = Some(
                path.to_str()
                    .expect("cannot parse path into string")
                    .to_owned(),
            );
            config.store_default();
            (tracker, path)
        } else {
            // Exit
            println!("Bye!");
            std::process::exit(0);
        }
    });

    tracker.interact(&path);
}
