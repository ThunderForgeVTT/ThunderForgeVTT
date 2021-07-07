use crate::utils::HttpClient;
use reqwest_wasm::Response;
use serde::{Deserialize, Serialize};

pub enum WorldEvents {
    TokenUpdate,
}

impl WorldEvents {
    pub fn to_u32(&self) -> u32 {
        use WorldEvents::TokenUpdate;
        match &self {
            TokenUpdate => 400,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
#[cfg_attr(feature = "server", derive(rocket::form::FromForm))]
pub struct TokenEvent {
    pub id: String,
    pub x: u32,
    pub y: u32,
    pub z: u32,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
#[cfg_attr(feature = "server", derive(rocket::form::FromForm))]
pub struct WorldEvent {
    pub event_code: u32,
    pub world_id: Option<String>,
    pub token: Option<TokenEvent>, // pub scene: Option<>
                                   // pub effect: Option<>
                                   // pub audio: Option<>
}

impl WorldEvent {
    pub async fn subscribe(world_id: String) -> reqwest_wasm::Result<Response> {
        let url = format!("/api/v1/world/{}/events", world_id);
        HttpClient::new().get(&url).send().await
    }
    pub async fn publish_event(
        &self,
        host: &str,
        world_id: String,
    ) -> reqwest_wasm::Result<Response> {
        let url = format!("{}/api/v1/world/{}/event", host, world_id);
        let client = reqwest_wasm::Client::new();
        client.post(&url).form(&self).send().await
    }
    pub fn create_token_event(token: TokenEvent) -> WorldEvent {
        WorldEvent {
            event_code: WorldEvents::TokenUpdate.to_u32(),
            world_id: None,
            token: Option::from(token),
        }
    }
}
