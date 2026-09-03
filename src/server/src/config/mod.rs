use base64::{Engine as _, engine::general_purpose};
use serde::{Deserialize, Serialize};
use std::path::Path;

pub mod oauth_env;

/// Fields are `pub` rather than `pub(crate)` because the binary that builds
/// `AppState` now lives in a different crate (`src/app`) — see `lib.rs` for
/// why the server became a library. Nothing else about their meaning changed.
#[derive(Serialize, Deserialize, Clone)]
pub struct Config {
    pub secret: String,
    pub data_path: String,
    pub secure_cookies: bool,
}

impl Config {
    pub fn from_env() -> Config {
        let secret = std::env::var("THUNDERFORGE_SECRET").unwrap_or_else(|_| {
            general_purpose::STANDARD.encode(
                "Change me to something complex, overall it should be unique and greater than 64 characters.",
            )
        });
        let data_path = std::env::var("THUNDERFORGE_DATA_PATH").unwrap_or_else(|_| {
            std::env::current_dir()
                .unwrap()
                .as_path()
                .join("data")
                .to_str()
                .unwrap()
                .to_string()
        });
        let secure_cookies = std::env::var("THUNDERFORGE_SECURE_COOKIES")
            .ok()
            .map(|value| {
                matches!(
                    value.trim().to_ascii_lowercase().as_str(),
                    "1" | "true" | "yes" | "on"
                )
            })
            .unwrap_or(false);
        Config {
            secret,
            data_path,
            secure_cookies,
        }
    }
}

impl Default for Config {
    fn default() -> Config {
        Config::from_env()
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Directories {
    pub(crate) base_dir: String,
    pub(crate) config_directory: String,
    pub(crate) manifest_file: String,
    pub(crate) user_database: String,
    pub(crate) world_basedir: String,
    pub(crate) modules_basedir: String,
    pub(crate) static_files: String,
    pub(crate) asset_directory: String,
    pub(crate) databases_basedir: String,
    pub(crate) systems_dir: String,
    /// Where interface packs live — presentation-only packs, siblings of the
    /// system packs above.
    ///
    /// A separate directory rather than a `type` field in one shared tree,
    /// because spec 032's FR-002 makes the two pack types exclusive and a
    /// directory that cannot hold both is the cheapest enforcement of that
    /// there is.
    pub(crate) interface_packs_dir: String,
}

impl From<String> for Directories {
    fn from(data_path: String) -> Directories {
        let base_dir = Path::new(&data_path);
        let databases_dir = &base_dir.join("databases");
        let config_dir = &base_dir.join("config");
        let systems_dir = &base_dir.join("packs").join("systems");
        let interface_packs_dir = &base_dir.join("packs").join("interface");
        Directories {
            base_dir: String::from(&base_dir.to_str().unwrap().to_owned()),
            config_directory: String::from(&config_dir.to_str().unwrap().to_owned()),
            manifest_file: String::from(
                &config_dir
                    .join("manifest.json")
                    .to_str()
                    .unwrap()
                    .to_owned(),
            ),
            databases_basedir: String::from(&databases_dir.to_str().unwrap().to_owned()),
            user_database: String::from(
                &databases_dir
                    .join("users.json")
                    .to_str()
                    .unwrap()
                    .to_owned(),
            ),
            world_basedir: String::from(&base_dir.join("worlds").to_str().unwrap().to_owned()),
            modules_basedir: String::from(&base_dir.join("modules").to_str().unwrap().to_owned()),
            static_files: String::from(&base_dir.join("client").to_str().unwrap().to_owned()),
            asset_directory: String::from(&base_dir.join("assets").to_str().unwrap().to_owned()),
            systems_dir: String::from(&systems_dir.to_str().unwrap().to_owned()),
            interface_packs_dir: String::from(&interface_packs_dir.to_str().unwrap().to_owned()),
        }
    }
}

impl Directories {
    pub fn create_if_not_present(&self) {
        let directories = vec![
            &self.asset_directory,
            &self.config_directory,
            &self.databases_basedir,
            &self.modules_basedir,
            &self.static_files,
            &self.world_basedir,
            &self.systems_dir,
            &self.interface_packs_dir,
        ];
        for directory in directories {
            let dir_path = Path::new(&directory);
            if !dir_path.exists() {
                match std::fs::create_dir_all(dir_path) {
                    Ok(_) => continue,
                    Err(_) => panic!("Failed to create: {}\nAre permissions correct?", directory),
                }
            }
        }
    }
}
