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

    pub fn decode(encoded: &str) -> std::result::Result<Credentials, String> {
        let cred_bytes = general_purpose::STANDARD
            .decode(encoded)
            .map_err(|_| "Credentials were not valid base64".to_string())?;
        let cred_string = from_utf8(&cred_bytes)
            .map_err(|_| "Credentials payload was not valid UTF-8".to_string())?
            .to_string();
        let cred_parts: Vec<&str> = cred_string.split(SEPARATOR).collect();
        if cred_parts.len() != 3 {
            return Err("Credentials payload was malformed".to_string());
        }

        Ok(Credentials {
            id: Some(cred_parts[0].to_string()),
            username: cred_parts[1].to_string(),
            password: cred_parts[2].to_string(),
        })
    }

    pub async fn authenticate(&self) -> bool {
        true
    }
}

impl From<String> for Credentials {
    fn from(cred: String) -> Self {
        Credentials::decode(&cred).unwrap_or_else(|_| Credentials {
            id: Some(String::new()),
            username: String::new(),
            password: String::new(),
        })
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

#[cfg(test)]
mod tests {
    use super::Credentials;

    #[test]
    fn credentials_roundtrip() {
        let credentials = Credentials::new(
            Some("id-1".to_string()),
            "mage".to_string(),
            "secret".to_string(),
        );

        let encoded = credentials.to_string();
        let decoded = Credentials::decode(&encoded).expect("credentials should decode");

        assert_eq!(decoded.username, "mage");
        assert_eq!(decoded.password, "secret");
    }

    #[test]
    fn credentials_decode_rejects_invalid_input() {
        let error = Credentials::decode("not-base64").expect_err("invalid credentials must fail");
        assert!(error.contains("base64"));
    }
}
