use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Serialize, Deserialize)]
pub struct OAuth2Config {
    pub(crate) id: String,
    pub(crate) secret: String,
    pub(crate) auth_url: String,
    pub(crate) token_url: String,
    pub(crate) scopes: Vec<String>,
}

#[derive(Serialize, Deserialize)]
pub struct SupportedAuthentication {
    pub(crate) oauth2: Option<Vec<OAuth2Config>>,
}

#[derive(Serialize, Deserialize)]
pub struct Config {
    pub(crate) secret: String,
    pub(crate) data_path: String,
    pub(crate) authentication: Option<SupportedAuthentication>,
}

impl Default for Config {
    fn default() -> Config {
        Config {
            secret: base64::encode("Change me to something complex, overall it should be unique and greater than 64 characters."),
            data_path: std::env::current_dir().unwrap().as_path().join("data").to_str().unwrap().to_string(),
            authentication: None,
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct Directories {
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
        Directories {
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
