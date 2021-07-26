use crate::utils::HttpClient;

#[cfg(feature = "client")]
use wasm_bindgen::prelude::*;

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

#[cfg_attr(feature = "client", wasm_bindgen)]
#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
#[cfg_attr(feature = "server", derive(rocket::form::FromForm))]
pub struct TokenEvent {
    id: String,
    pub x: u32,
    pub y: u32,
    pub z: u32,
}

#[cfg_attr(feature = "client", wasm_bindgen)]
impl TokenEvent {
    #[cfg_attr(feature = "client", wasm_bindgen(constructor))]
    pub fn new(id: String) -> TokenEvent {
        TokenEvent {
            id,
            x: 0,
            y: 0,
            z: 0,
        }
    }

    #[cfg_attr(feature = "client", wasm_bindgen(getter))]
    pub fn id(&self) -> String {
        String::from(&self.id)
    }

    #[cfg_attr(feature = "client", wasm_bindgen(setter))]
    pub fn set_id(&mut self, id: String) {
        self.id = id;
    }
}

#[cfg_attr(feature = "client", wasm_bindgen)]
#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
#[cfg_attr(feature = "server", derive(rocket::form::FromForm))]
pub struct WorldEvent {
    pub event_code: u32,
    world_id: Option<String>,
    token: Option<TokenEvent>, // pub scene: Option<>
                               // pub effect: Option<>
                               // pub audio: Option<>
}

#[cfg_attr(feature = "client", wasm_bindgen)]
impl WorldEvent {
    #[cfg_attr(feature = "client", wasm_bindgen(constructor))]
    pub fn new(id: String) -> WorldEvent {
        WorldEvent {
            event_code: 0,
            world_id: None,
            token: None,
        }
    }

    #[cfg_attr(feature = "client", wasm_bindgen(getter))]
    pub fn world_id(&self) -> Option<String> {
        self.world_id.clone()
    }

    #[cfg_attr(feature = "client", wasm_bindgen(setter))]
    pub fn set_world_id(&mut self, world_id: String) {
        self.world_id = Some(world_id);
    }

    #[cfg_attr(feature = "client", wasm_bindgen(getter))]
    pub fn token(&self) -> Option<TokenEvent> {
        self.token.clone()
    }

    #[cfg_attr(feature = "client", wasm_bindgen(setter))]
    pub fn set_token(&mut self, token_event: TokenEvent) {
        self.token = Some(token_event);
    }

    // pub async fn subscribe(world_id: String) -> reqwest_wasm::Result<Response> {
    //     let url = format!("/api/v1/world/{}/events", world_id);
    //     HttpClient::new().get(&url).send().await
    // }
    //
    // pub async fn publish_event(
    //     &self,
    //     host: &str,
    //     world_id: String,
    // ) -> reqwest_wasm::Result<Response> {
    //     let url = format!("{}/api/v1/world/{}/event", host, world_id);
    //     let client = reqwest_wasm::Client::new();
    //     client.post(&url).form(&self).send().await
    // }
    //
    // pub fn create_token_event(token: TokenEvent) -> WorldEvent {
    //     WorldEvent {
    //         event_code: WorldEvents::TokenUpdate.to_u32(),
    //         world_id: None,
    //         token: Option::from(token),
    //     }
    // }
}
