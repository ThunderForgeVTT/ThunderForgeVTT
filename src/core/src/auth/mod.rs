use crate::utils::HttpClient;
use base64::{decode, encode};
use reqwest_wasm::{Body, StatusCode};
use rocket::http::Cookie;
use serde::{Deserialize, Serialize};
use std::str::from_utf8;

const SEPARATOR: &str = "~UwU~";

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
#[cfg_attr(feature = "server", derive(rocket::form::FromForm))]
pub struct Credentials {
    id: String,
    pub username: String,
    password: String,
}

impl Credentials {
    #[cfg(feature = "server")]
    pub fn id_to_uuid(&self) -> rocket::serde::uuid::Uuid {
        use rocket::serde::uuid::Uuid;
        use std::str::FromStr;
        Uuid::from_str(&self.id).unwrap()
    }

    #[cfg(feature = "server")]
    pub fn create_cookie(&self) -> Cookie<'static> {
        let mut cookie = Cookie::new("session", "");
        cookie.set_secure(true);
        cookie
    }

    pub fn new(id: String, username: String, password: String) -> Credentials {
        Credentials {
            id,
            username,
            password,
        }
    }

    #[cfg(feature = "server")]
    pub async fn authenticate(&self) -> bool {
        return true;
    }

    #[cfg(feature = "client")]
    pub async fn login(&self) -> String {
        let body = Body::from(self.to_string());

        let client = HttpClient::new();
        let request = client
            .post(&format!("/api/v1/authentication/basic"))
            .body(body)
            .send()
            .await;
        match request {
            Ok(response) => response
                .text()
                .await
                .unwrap_or_else(|_| String::from("error")),
            Err(error) => {
                let message = format!(
                    "[{}]: An error has occurred!\n{}",
                    error.status().unwrap_or_else(|| StatusCode::SEE_OTHER),
                    error.to_string()
                );
                yew::services::ConsoleService::error(&message);
                String::from("failure")
            }
        }
    }
}

impl From<String> for Credentials {
    fn from(cred: String) -> Self {
        let cred_bytes = decode(cred).ok().unwrap();
        let cred_string: String = from_utf8(&cred_bytes).unwrap().to_string();
        let cred_parts: Vec<&str> = cred_string.split(&SEPARATOR).collect();
        Credentials {
            id: cred_parts[0].to_string(),
            username: cred_parts[1].to_string(),
            password: cred_parts[2].to_string(),
        }
    }
}

impl ToString for Credentials {
    fn to_string(&self) -> String {
        let components = vec![
            String::from(&self.id.to_string()),
            String::from(&self.username),
            String::from(&self.password),
        ];
        // let contents = components.mapped( |value| String::from(value)).collect().join(&SEPARATOR).to_string();

        encode(components.join(&SEPARATOR).to_string())
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
#[cfg_attr(feature = "server", derive(rocket::form::FromForm))]
pub struct User {
    id: String,
    username: String,
    password: Option<String>,
    first_name: String,
    last_name: String,
}
