use std::path::PathBuf;
use directories::ProjectDirs;
use serde_derive::{Serialize, Deserialize};
use lazy_static::lazy_static;
//use crate::ui::Theme;

const CONFIG_FILENAME: &str = concat!(env!("CARGO_PKG_NAME"), ".toml");

lazy_static! {
	static ref CONFIG_PATH: PathBuf = {
        let mut path = ProjectDirs::from("com", "jsbarretto", "zte")
            .unwrap()
            .config_dir()
            .to_owned();
        path.push(CONFIG_FILENAME);
        path
    };
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Config {
    //theme: Theme,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            //theme: Theme::default()
        }
    }
}

impl Config {
    /// Load the configuration from disk, if it exists
    pub fn load() -> Result<Self, config::ConfigError> {
        let mut config = config::Config::new();
        config.merge(config::File::with_name(CONFIG_FILENAME))?;
        config.try_into()
    }
}
