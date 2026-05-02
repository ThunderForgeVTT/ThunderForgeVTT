use base64::{engine::general_purpose, Engine as _};
use serde::{Deserialize, Serialize};
use std::str::from_utf8;

const SEPARATOR: &str = "~UwU~";

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct Credentials {
    id: Option<String>,
    pub username: String,
    password: String,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct User {
    id: String,
    username: String,
    password: Option<String>,
    first_name: String,
    last_name: String,
}

impl Credentials {
    pub fn new(id: Option<String>, username: String, password: String) -> Credentials {
        Credentials {
            id,
            username,
            password,
        }
    }

    pub async fn authenticate(&self) -> bool {
        true
    }

    #[cfg(feature = "client")]
    pub async fn login(&self) -> String {
        let body = reqwest_wasm::Body::from(self.to_string());
        let client = crate::utils::HttpClient::new();
        let request = client
            .post("/api/v1/authentication/basic")
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
                    "[{}]: An error has occurred!
{}",
                    error
                        .status()
                        .unwrap_or(reqwest_wasm::StatusCode::SEE_OTHER),
                    error.to_string()
                );
                web_sys::console::error_1(&message.into());
                String::from("failure")
            }
        }
    }
}

impl From<String> for Credentials {
    fn from(cred: String) -> Self {
        let cred_bytes = general_purpose::STANDARD.decode(cred).ok().unwrap();
        let cred_string: String = from_utf8(&cred_bytes).unwrap().to_string();
        let cred_parts: Vec<&str> = cred_string.split(&SEPARATOR).collect();
        Credentials {
            id: Option::Some(cred_parts[0].to_string()),
            username: cred_parts[1].to_string(),
            password: cred_parts[2].to_string(),
        }
    }
}

impl ToString for Credentials {
    fn to_string(&self) -> String {
        let id = match &self.id {
            Some(val) => val.to_owned(),
            None => String::new(),
        };
        let components = vec![
            id,
            String::from(&self.username),
            String::from(&self.password),
        ];
        // let contents = components.mapped( |value| String::from(value)).collect().join(&SEPARATOR).to_string();

        general_purpose::STANDARD.encode(components.join(SEPARATOR))
    }
}
