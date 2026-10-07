use base64::{Engine as _, engine::general_purpose};
use serde::{Deserialize, Serialize};
use std::path::Path;

pub mod oauth_env;

/// Fields are `pub` rather than `pub(crate)` because the binary that builds
/// `AppState` now lives in a different crate (`apps/thunderforge`) — see `lib.rs` for
/// why the server became a library. Nothing else about their meaning changed.
#[derive(Serialize, Deserialize, Clone)]
pub struct Config {
    pub secret: String,
    pub data_path: String,
    pub secure_cookies: bool,
}

/// What `THUNDERFORGE_SECRET` falls back to when it is unset, before encoding.
///
/// Public by being in this file. It exists so that `cargo run` works on a
/// laptop, and an instance still using it has session cookies and stored
/// credentials protected by a key anybody can read here.
const FALLBACK_SECRET_PLAINTEXT: &str =
    "Change me to something complex, overall it should be unique and greater than 64 characters.";

/// The default `compose.yml` supplies for `THUNDERFORGE_SECRET`, verbatim.
///
/// Just as public, and the one a real deployment is far more likely to be
/// running with: `docker compose up` works without setting anything. A test
/// reads `compose.yml` and fails if the two drift apart.
const COMPOSE_DEFAULT_SECRET: &str = "dGh1bmRlcmZvcmdlIGxvY2FsIGV4cGxvcmF0aW9uIGluc3RhbmNlIG9ubHksIG5vdCBhIGRlcGxveW1lbnQgc2VjcmV0IDAxMjM0NTY3ODk=";

impl Config {
    /// Whether the secret is one this project ships, rather than one the
    /// operator chose.
    ///
    /// Answers yes or no and nothing else. Readiness reports it, and readiness
    /// never names a value — least of all this one.
    pub fn secret_is_shipped_default(&self) -> bool {
        let secret = self.secret.trim();
        secret == COMPOSE_DEFAULT_SECRET
            || secret == general_purpose::STANDARD.encode(FALLBACK_SECRET_PLAINTEXT)
    }

    pub fn from_env() -> Config {
        let secret = std::env::var("THUNDERFORGE_SECRET").unwrap_or_else(|_| {
            // Silent on purpose: this runs before logging is configured, and
            // in every test. The readiness report is where an operator is
            // told (`readiness::deployment`).
            general_purpose::STANDARD.encode(FALLBACK_SECRET_PLAINTEXT)
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
    /// The built demo (spec 074), when this server was started with one.
    #[serde(default)]
    pub(crate) demo_files: Option<String>,
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
            demo_files: None,
            asset_directory: String::from(&base_dir.join("assets").to_str().unwrap().to_owned()),
            systems_dir: String::from(&systems_dir.to_str().unwrap().to_owned()),
            interface_packs_dir: String::from(&interface_packs_dir.to_str().unwrap().to_owned()),
        }
    }
}

impl Directories {
    /// Serve the web client from `static_dir` rather than `<data>/client`.
    ///
    /// The client is not data: an image carries it beside the binary, and the
    /// data directory is a volume that would hide it.
    pub fn with_static_files(mut self, static_dir: String) -> Self {
        self.static_files = static_dir;
        self
    }

    /// Serve the demo from `demo_dir` (spec 074 FR-015).
    ///
    /// There is no default: a server that was not told where a demo is has
    /// none, whatever the instance's setting says.
    pub fn with_demo_files(mut self, demo_dir: String) -> Self {
        self.demo_files = Some(demo_dir);
        self
    }

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

#[cfg(test)]
mod tests {
    use super::*;

    fn config(secret: &str) -> Config {
        Config {
            secret: secret.to_string(),
            data_path: String::new(),
            secure_cookies: false,
        }
    }

    #[test]
    fn both_shipped_secrets_are_recognised_and_a_chosen_one_is_not() {
        assert!(
            config(&general_purpose::STANDARD.encode(FALLBACK_SECRET_PLAINTEXT))
                .secret_is_shipped_default()
        );
        assert!(config(COMPOSE_DEFAULT_SECRET).secret_is_shipped_default());
        assert!(
            !config("b3BlcmF0b3ItY2hvc2VuLXNlY3JldC1vcGVyYXRvci1jaG9zZW4tc2VjcmV0")
                .secret_is_shipped_default()
        );
    }

    /// The constant is a copy of a line in another file, and a copy drifts.
    #[test]
    fn the_compose_default_is_the_one_compose_ships() {
        let compose = std::fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../compose.yml"),
        )
        .expect("compose.yml is at the repository root");
        assert!(
            compose.contains(&format!("THUNDERFORGE_SECRET:-{COMPOSE_DEFAULT_SECRET}}}")),
            "compose.yml's default secret changed and `COMPOSE_DEFAULT_SECRET` did not, so \
             readiness no longer recognises an instance running on the shipped one"
        );
    }
}
