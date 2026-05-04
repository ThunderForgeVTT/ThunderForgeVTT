use base64::{Engine as _, engine::general_purpose};
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter, Result};
use std::str::from_utf8;

const SEPARATOR: &str = "~UwU~";

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct Credentials {
    id: Option<String>,
    pub username: String,
    pub password: String,
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

impl Display for Credentials {
    fn fmt(&self, f: &mut Formatter) -> Result {
        let id = match &self.id {
            Some(val) => val.to_owned(),
            None => String::new(),
        };
        let components = [
            id,
            String::from(&self.username),
            String::from(&self.password),
        ];
        // let contents = components.mapped( |value| String::from(value)).collect().join(&SEPARATOR).to_string();

        let result = general_purpose::STANDARD.encode(components.join(SEPARATOR));
        write!(f, "{}", result)
    }
}
