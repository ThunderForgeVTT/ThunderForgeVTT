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
pub struct TokenEvent {
    id: String,
    pub x: u32,
    pub y: u32,
    pub z: u32,
}

impl TokenEvent {
    pub fn new(id: String) -> TokenEvent {
        TokenEvent {
            id,
            x: 0,
            y: 0,
            z: 0,
        }
    }

    pub fn id(&self) -> String {
        String::from(&self.id)
    }

    pub fn set_id(&mut self, id: String) {
        self.id = id;
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub struct WorldEvent {
    pub event_code: u32,
    world_id: Option<String>,
    token: Option<TokenEvent>, // pub scene: Option<>
                               // pub effect: Option<>
                               // pub audio: Option<>
}

impl WorldEvent {
    pub fn new(_id: String) -> WorldEvent {
        WorldEvent {
            event_code: 0,
            world_id: None,
            token: None,
        }
    }

    pub fn world_id(&self) -> Option<String> {
        self.world_id.clone()
    }

    pub fn set_world_id(&mut self, world_id: String) {
        self.world_id = Some(world_id);
    }

    pub fn token(&self) -> Option<TokenEvent> {
        self.token.clone()
    }

    pub fn set_token(&mut self, token_event: TokenEvent) {
        self.token = Some(token_event);
    }
}
