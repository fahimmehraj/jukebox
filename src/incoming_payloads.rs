mod transformations;

use serde::Deserialize;
use std::time::Duration;
use transformations::*;

type Snowflake = String;

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Payload {
    pub guild_id: String,
    #[serde(flatten)]
    pub op: Opcode
}

#[derive(Deserialize, Debug)]
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

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct VoiceUpdate {
    pub guild_id: Snowflake,
    pub session_id: Snowflake,
    pub event: VoiceUpdateEvent,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct VoiceUpdateEvent {
    pub token: String,
    pub guild_id: Snowflake,
    pub endpoint: Option<String>,
}

// possibly change track type to custom type

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Play {
    pub guild_id: Snowflake,
    pub track: String,
    #[serde(with = "serde_millis")]
    pub start_time: Option<Duration>,
    #[serde(with = "serde_millis")]
    pub end_time: Option<Duration>,
    pub volume: Option<i16>,
    pub no_replace: Option<bool>,
    pub pause: Option<bool>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Stop {
    pub guild_id: Snowflake,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Pause {
    pub guild_id: Snowflake,
    pub pause: bool,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Seek {
    pub guild_id: Snowflake,
    #[serde(with = "serde_millis")]
    pub position: Duration,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Volume {
    pub guild_id: Snowflake,
    pub volume: i16,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Filters {
    pub guild_id: Snowflake,
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

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Destroy {
    pub guild_id: Snowflake,
}
