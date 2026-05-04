use base64::{Engine as _, engine::general_purpose};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Serialize, Deserialize, Clone)]
pub struct OAuth2Config {
    pub(crate) id: String,
    pub(crate) secret: String,
    pub(crate) auth_url: String,
    pub(crate) token_url: String,
    pub(crate) scopes: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct SupportedAuthentication {
    pub(crate) oauth2: Option<Vec<OAuth2Config>>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Config {
    pub(crate) secret: String,
    pub(crate) data_path: String,
    pub(crate) authentication: Option<SupportedAuthentication>,
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
        let authentication = std::env::var("THUNDERFORGE_AUTHENTICATION")
            .ok()
            .and_then(|auth_str| serde_json::from_str(&auth_str).ok());

        Config {
            secret,
            data_path,
            authentication,
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
}

impl From<String> for Directories {
    fn from(data_path: String) -> Directories {
        let base_dir = Path::new(&data_path);
        let databases_dir = &base_dir.join("databases");
        let config_dir = &base_dir.join("config");
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
