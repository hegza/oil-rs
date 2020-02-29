use super::DEFAULT_CONFIG_NAME;
use crate::prelude::*;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{Read, Write};
use std::path::PathBuf;

#[derive(Deserialize, Serialize)]
pub struct Config {
    pub last_open: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Config { last_open: None }
    }
}

fn make_config_path() -> PathBuf {
    let dir = dirs::config_dir().expect("Home directory not found");
    fs::create_dir_all(&dir).expect("Could not recursively create default config directory");
    // Try write the default config file
    let mut config_path = dir;
    config_path.push(DEFAULT_CONFIG_NAME);
    config_path
}

impl Config {
    pub fn store_default(&self) {
        // Create the default config directory if it doesn't exist
        let config_path = make_config_path();

        let config_str = serde_yaml::to_string(self).expect("Cannot serialize config to yaml");
        trace!("Writing: {}", &config_str);

        let mut file = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .append(false)
            .truncate(true)
            .open(&config_path)
            .expect("Could not create default config file");
        file.write_all(&config_str.as_bytes())
            .expect("Could not write config to file");
    }
    pub fn load_default() -> Config {
        // Create the default config directory if it doesn't exist
        let config_path = make_config_path();

        // Open default config file
        if let Ok(mut file) = fs::OpenOptions::new().read(true).open(&config_path) {
            let mut config_str = String::new();
            file.read_to_string(&mut config_str)
                .expect("Could not read config from file");
            if config_str.is_empty() {
                Config::default()
            } else {
                serde_yaml::from_str(&config_str).expect("Could not open config from yaml")
            }
        }
        // Default config file does not exist, create it
        else {
            info!(
                "Creating a default config file at {:?} as it cannot be loaded from default location",
                &config_path
            );

            // Create default config and serialize to yaml
            let config = Config::default();
            let config_str = serde_yaml::to_string(&config).expect(
                "Cannot serialize default config to yaml, this is a bug in the yaml library",
            );

            // Write the file
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create(true)
                .append(false)
                .truncate(true)
                .open(&config_path)
                .expect("Could not create default config file");
            file.write_all(&config_str.as_bytes())
                .expect("Could not write config to file");
            config
        }
    }
}
