use serde::{Deserialize, Serialize};
use serde_with::{serde_as, VecSkipError};
use tokio_tungstenite::tungstenite::Message;

use crate::crypto::EncryptionMode;

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "op", content = "d")]
pub enum DiscordPayload {
    #[serde(rename = 0)]
    Identify(Identify),

    #[serde(rename = 1)]
    SelectProtocol(SelectProtocol),

    #[serde(rename = 2)]
    Ready(Ready),

    #[serde(rename = 3)]
    Heartbeat(u128),

    #[serde(rename = 4)]
    SessionDescription(SessionDescription),

    #[serde(rename = 5)]
    Speaking(Speaking),

    #[serde(rename = 6)]
    HeartbeatACK(u128),

    #[serde(rename = 7)]
    Resume(Resume),

    #[serde(rename = 8)]
    Hello(Hello),

    #[serde(rename = 9)]
    Resumed,

    #[serde(rename = 10)]
    ClientDisconnect(ClientDisconnect),

    #[serde(rename = 11)]
    ClientConnect(serde_json::Value),

    #[serde(rename = 13)]
    OtherClientDisconnect(serde_json::Value),

    #[serde(rename = 18)]
    ClientFlags(serde_json::Value),

    #[serde(rename = 20)]
    ClientPlatform(serde_json::Value),

    #[serde(untagged)]
    Unknown(serde_json::Value),
}

impl From<DiscordPayload> for Message {
    fn from(payload: DiscordPayload) -> Self {
        let text = serde_json::to_string(&payload).unwrap();
        Message::Text(text.into())
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Identify {
    pub server_id: String,
    pub user_id: String,
    pub session_id: String,
    pub token: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SelectProtocol {
    pub protocol: String,
    pub data: SelectProtocolData,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SelectProtocolData {
    pub address: String,
    pub port: u16,
    pub mode: EncryptionMode,
}

#[serde_as()]
#[derive(Serialize, Deserialize, Debug)]
pub struct Ready {
    pub ssrc: u32,
    pub ip: String,
    pub port: u16,
    #[serde_as(as = "VecSkipError<_>")]
    pub modes: Vec<EncryptionMode>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SessionDescription {
    pub mode: EncryptionMode,
    pub secret_key: [u8; 32],
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Speaking {
    #[serde(skip_serializing)]
    pub user_id: Option<String>,
    pub speaking: u8,
    pub delay: Option<u8>,
    pub ssrc: u32,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct HeartbeatACK {
    pub d: u64,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Resume {
    pub server_id: String,
    pub session_id: String,
    pub token: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Hello {
    pub v: u8,
    pub heartbeat_interval: u64,
}

// Resumed has no data

// i am not sure what fields opcode 13 actually has
#[derive(Serialize, Deserialize, Debug)]
pub struct ClientDisconnect {
    pub user_id: String,
}
