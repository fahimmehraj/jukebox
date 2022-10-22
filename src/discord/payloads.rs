use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "op", content = "d")]
pub enum Payload {
    #[serde(rename = "0")]
    Identify(Identify),
    #[serde(rename = "1")]
    SelectProtocol(SelectProtocol),
    #[serde(rename = "2")]
    Ready(Ready),
    #[serde(rename = "3")]
    Heartbeat(Heartbeat),
    #[serde(rename = "4")]
    SessionDescription(SessionDescription),
    #[serde(rename = "5")]
    Speaking(Speaking),
    #[serde(rename = "6")]
    HeartbeatACK(HeartbeatACK),
    #[serde(rename = "7")]
    Resume(Resume),
    #[serde(rename = "8")]
    Hello(Hello),
    #[serde(rename = "9")]
    Resumed,
    #[serde(rename = "13")]
    ClientDisconnect(ClientDisconnect),
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Identify {
    pub server_id: String,
    pub user_id: String,
    pub session_id: String,
    pub token: String,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SelectProtocol {
    pub protocol: String,
    pub data: SelectProtocolData,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SelectProtocolData {
    pub address: String,
    pub port: u16,
    pub mode: String,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Ready {
    pub ssrc: u32,
    pub ip: String,
    pub port: u16,
    pub modes: Vec<String>,
    pub heartbeat_interval: u64,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Heartbeat {
    pub nonce: u64,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SessionDescription {
    pub mode: String,
    pub secret_key: Vec<u32>,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Speaking {
    pub speaking: u8,
    pub delay: u8,
    pub ssrc: u32,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct HeartbeatACK {
    pub nonce: u64,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Resume {
    pub server_id: String,
    pub session_id: String,
    pub token: String,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Hello {
    pub heartbeat_interval: u64,
}

// Resumed has no data

// i am not sure what fields opcode 13 actually has
#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ClientDisconnect {
    pub reason: String,
}
