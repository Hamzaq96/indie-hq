use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type")]
pub enum ClientMessage {
    Join { name: String },
    Move { x: i32, y: i32 },
    Chat { message: String },
    SetStatus { status: String},
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type")]
pub enum ServerMessage {
    Welcome { id: String },
    Presence {
        id: String,
        name: String,
        x: i32,
        y: i32,
        status: String,
    },
    Chat { from: String, message: String, zone: Zone},
    PlayerLeft { id: String },
}

#[derive(Serialize, PartialEq, Deserialize, Debug, Clone)]
pub enum Zone {
    Focus,
    Open,
    Office,
}

pub fn zone_at(x: i32) -> Zone {
    if x < 10 {
        Zone::Focus
    }
    else if x < 20 {
        Zone::Open
    } else {
        Zone::Office
    }
}
