mod transformations;

use serde::{Deserialize, Serialize};
use transformations::*;

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ClientPayload {
    pub guild_id: String,
    #[serde(flatten)]
    pub op: Opcode,
}

impl From<ClientPayload> for axum::extract::ws::Message {
    fn from(value: ClientPayload) -> Self {
        let json = serde_json::to_string(&value).unwrap();
        axum::extract::ws::Message::Text(json.into())
    }
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "op")]
pub enum Opcode {
    VoiceUpdate(VoiceUpdate),
    Play(Play),
    Stop(Stop),
    Pause(Pause),
    Seek(Seek),
    Volume(Volume),
    Filters(Filters),
    Destroy(Destroy),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct VoiceUpdate {
    pub session_id: String,
    pub event: VoiceUpdateEvent,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct VoiceUpdateEvent {
    pub token: String,
    pub guild_id: String,
    pub endpoint: String,
}

// possibly change track type to custom type

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Play {
    pub track: String,
    pub start_time: Option<u64>,
    pub end_time: Option<u64>,
    pub volume: Option<i16>,
    pub no_replace: Option<bool>,
    pub pause: Option<bool>,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Stop {}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Pause {
    pub pause: bool,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Seek {
    pub position: u64,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Volume {
    pub volume: i16,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Filters {
    pub volume: Option<f64>,
    pub equalizer: Option<Vec<EqualizerObject>>,
    pub karaoke: Option<Karaoke>,
    pub timescale: Option<Timescale>,
    pub tremolo: Option<Tremolo>,
    pub vibrato: Option<Vibrato>,
    pub distortion: Option<Distortion>,
    pub channel_mix: Option<ChannelMix>,
    pub low_pass: Option<LowPass>,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Destroy {}
