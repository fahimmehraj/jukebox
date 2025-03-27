use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct EqualizerObject {
    pub band: i8,
    pub gain: f64,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Karaoke {
    pub level: f64,
    pub mono_level: f64,
    pub filter_band: f64,
    pub filter_width: f64,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Timescale {
    pub speed: f64,
    pub pitch: f64,
    pub rate: f64,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Tremolo {
    pub frequency: f64,
    pub depth: f64,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Vibrato {
    pub frequency: f64,
    pub depth: f64,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Rotation {
    pub rotation_hz: f64,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Distortion {
    pub sin_offset: f64,
    pub sin_scale: f64,
    pub cos_offset: f64,
    pub cos_scale: f64,
    pub tan_offset: f64,
    pub tan_scale: f64,
    pub offset: f64,
    pub scale: f64,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ChannelMix {
    pub left_to_left: f64,
    pub left_to_right: f64,
    pub right_to_left: f64,
    pub right_to_right: f64,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct LowPass {
    pub smoothing: f64,
}
