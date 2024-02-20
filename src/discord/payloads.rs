use log::info;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use serde_with::{serde_as, DefaultOnError};
use tokio_tungstenite::tungstenite::Message;

use crate::crypto::EncryptionMode;

#[derive(Debug)]
pub enum DiscordPayload {
    Identify(Identify),

    SelectProtocol(SelectProtocol),

    Ready(Ready),

    Heartbeat(Heartbeat),

    SessionDescription(SessionDescription),

    Speaking(Speaking),

    HeartbeatACK(HeartbeatACK),

    Resume(Resume),

    Hello(Hello),

    Resumed,

    ClientDisconnect(ClientDisconnect),

    Generic(Value),
}

impl Serialize for DiscordPayload {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        #[derive(Serialize)]
        #[serde(untagged)]
        enum DiscordPayload_<'a> {
            Identify(&'a Identify),
            SelectProtocol(&'a SelectProtocol),
            Heartbeat(&'a Heartbeat),
            Speaking(&'a Speaking),
            Resume(&'a Resume),
        }

        #[derive(Serialize)]
        struct TypedDiscordPayload<'a> {
            op: u8,
            d: DiscordPayload_<'a>,
        }

        match self {
            DiscordPayload::Identify(payload) => TypedDiscordPayload {
                op: 0,
                d: DiscordPayload_::Identify(payload),
            }
            .serialize(serializer),
            DiscordPayload::SelectProtocol(payload) => TypedDiscordPayload {
                op: 1,
                d: DiscordPayload_::SelectProtocol(payload),
            }
            .serialize(serializer),
            DiscordPayload::Heartbeat(payload) => TypedDiscordPayload {
                op: 3,
                d: DiscordPayload_::Heartbeat(payload),
            }
            .serialize(serializer),
            DiscordPayload::Speaking(payload) => TypedDiscordPayload {
                op: 5,
                d: DiscordPayload_::Speaking(payload),
            }
            .serialize(serializer),
            DiscordPayload::Resume(payload) => TypedDiscordPayload {
                op: 7,
                d: DiscordPayload_::Resume(payload),
            }
            .serialize(serializer),
            _ => Err(serde::ser::Error::custom("Cannot serialize this payload")),
        }
    }
}

// Generated with GitHub Copilot
impl<'de> serde::Deserialize<'de> for DiscordPayload {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct DiscordPayloadWrapper {
            op: u8,
            d: serde_json::Value,
        }

        let wrapper = DiscordPayloadWrapper::deserialize(deserializer)?;

        match wrapper.op {
            2 => Ok(DiscordPayload::Ready(
                serde_json::from_value(wrapper.d).unwrap(),
            )),
            4 => Ok(DiscordPayload::SessionDescription(
                serde_json::from_value(wrapper.d).unwrap(),
            )),
            5 => Ok(DiscordPayload::Speaking(
                serde_json::from_value(wrapper.d).unwrap(),
            )),
            6 => Ok(DiscordPayload::HeartbeatACK(
                serde_json::from_value(wrapper.d).unwrap(),
            )),
            8 => Ok(DiscordPayload::Hello(
                serde_json::from_value(wrapper.d).unwrap(),
            )),
            9 => Ok(DiscordPayload::Resumed),
            13 => Ok(DiscordPayload::ClientDisconnect(
                serde_json::from_value(wrapper.d).unwrap(),
            )),
            _ => Ok(DiscordPayload::Generic(
                serde_json::from_value(wrapper.d).unwrap(),
            )),
        }
    }
}

impl From<DiscordPayload> for Message {
    fn from(payload: DiscordPayload) -> Self {
        let text = serde_json::to_string(&payload).unwrap();
        info!("{:#?}", text);
        Message::Text(text)
    }
}

#[derive(Serialize, Debug)]
pub struct Identify {
    pub server_id: String,
    pub user_id: String,
    pub session_id: String,
    pub token: String,
}

#[derive(Serialize, Debug)]
pub struct SelectProtocol {
    pub protocol: String,
    pub data: SelectProtocolData,
}

#[derive(Serialize, Debug)]
pub struct SelectProtocolData {
    pub address: String,
    pub port: u16,
    pub mode: EncryptionMode,
}

#[derive(Deserialize, Debug)]
pub struct Ready {
    pub ssrc: u32,
    pub ip: String,
    pub port: u16,
    #[serde(deserialize_with = "skip_on_error")]
    pub modes: Vec<EncryptionMode>,
}
fn skip_on_error<'de, D>(deserializer: D) -> Result<Vec<EncryptionMode>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[serde_as]
    #[derive(Deserialize, Debug)]
    struct MayBeT(#[serde_as(as = "DefaultOnError")] Option<EncryptionMode>);

    let values: Vec<MayBeT> = Deserialize::deserialize(deserializer)?;

    Ok(values.into_iter().filter_map(|t| t.0).collect())
}

#[derive(Serialize, Debug)]
pub struct Heartbeat {
    pub nonce: u128,
}

#[derive(Deserialize, Debug)]
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

#[derive(Deserialize, Debug)]
pub struct HeartbeatACK {
    pub nonce: u64,
}

#[derive(Serialize, Debug)]
pub struct Resume {
    pub server_id: String,
    pub session_id: String,
    pub token: String,
}

#[derive(Deserialize, Debug)]
pub struct Hello {
    pub heartbeat_interval: f64,
}

// Resumed has no data

// i am not sure what fields opcode 13 actually has
#[derive(Deserialize, Debug)]
pub struct ClientDisconnect {
    pub user_id: String,
}
