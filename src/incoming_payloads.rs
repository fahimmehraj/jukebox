mod transformations;

use serde::Deserialize;
use std::time::Duration;
use transformations::*;

type Snowflake = String;

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "op")]
pub enum Payload {
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
    guild_id: Snowflake,
    session_id: Snowflake,
    event: VoiceUpdateEvent,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct VoiceUpdateEvent {
    token: String,
    guild_id: Snowflake,
    endpoint: Option<String>,
}

// possibly change track type to custom type

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Play {
    guild_id: Snowflake,
    track: String,
    #[serde(with = "serde_millis")]
    start_time: Duration,
    #[serde(with = "serde_millis")]
    end_time: Duration,
    volume: i16,
    no_replace: bool,
    pause: bool,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Stop {
    guild_id: Snowflake,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Pause {
    guild_id: Snowflake,
    pause: bool,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Seek {
    guild_id: Snowflake,
    #[serde(with = "serde_millis")]
    position: Duration,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Volume {
    guild_id: Snowflake,
    volume: i16,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Filters {
    guild_id: Snowflake,
    volume: Option<f64>,
    equalizer: Option<Vec<EqualizerObject>>,
    karaoke: Option<Karaoke>,
    timescale: Option<Timescale>,
    tremolo: Option<Tremolo>,
    vibrato: Option<Vibrato>,
    distortion: Option<Distortion>,
    channel_mix: Option<ChannelMix>,
    low_pass: Option<LowPass>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Destroy {
    guild_id: Snowflake,
}
