use serde::{Deserialize, Serialize};
use tokio_tungstenite::tungstenite::Message;

use crate::crypto::EncryptionMode;

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "op", content = "d")]
pub enum DiscordPayload {
    #[serde(skip_deserializing)]
    #[serde(rename = "0")]
    Identify(Identify),

    #[serde(skip_deserializing)]
    #[serde(rename = "1")]
    SelectProtocol(SelectProtocol),

    #[serde(skip_serializing)]
    #[serde(rename = "2")]
    Ready(Ready),

    #[serde(skip_deserializing)]
    #[serde(rename = "3")]
    Heartbeat(Heartbeat),

    #[serde(skip_serializing)]
    #[serde(rename = "4")]
    SessionDescription(SessionDescription),

    #[serde(rename = "5")]
    Speaking(Speaking),

    #[serde(skip_serializing)]
    #[serde(rename = "6")]
    HeartbeatACK(HeartbeatACK),

    #[serde(skip_deserializing)]
    #[serde(rename = "7")]
    Resume(Resume),

    #[serde(skip_serializing)]
    #[serde(rename = "8")]
    Hello(Hello),

    #[serde(skip_serializing)]
    #[serde(rename = "9")]
    Resumed,

    #[serde(skip_serializing)]
    #[serde(rename = "13")]
    ClientDisconnect(ClientDisconnect),
}

impl From<DiscordPayload> for Message {
    fn from(payload: DiscordPayload) -> Self {
        Message::Text(serde_json::to_string(&payload).unwrap())
    }
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Identify {
    pub server_id: String,
    pub user_id: String,
    pub session_id: String,
    pub token: String,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SelectProtocol {
    pub protocol: String,
    pub data: SelectProtocolData,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SelectProtocolData {
    pub address: String,
    pub port: u16,
    pub mode: EncryptionMode,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Ready {
    pub ssrc: u32,
    pub ip: String,
    pub port: u16,
    pub modes: Vec<EncryptionMode>,
    pub heartbeat_interval: u64,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Heartbeat {
    pub nonce: u128,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SessionDescription {
    pub mode: EncryptionMode,
    pub secret_key: [u8; 32],
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Speaking {
    pub speaking: u8,
    pub delay: u8,
    pub ssrc: u32,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct HeartbeatACK {
    pub nonce: u64,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Resume {
    pub server_id: String,
    pub session_id: String,
    pub token: String,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Hello {
    pub heartbeat_interval: u64,
}

// Resumed has no data

// i am not sure what fields opcode 13 actually has
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ClientDisconnect {
    pub reason: String,
}
