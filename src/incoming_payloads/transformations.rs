use serde::Deserialize;

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct EqualizerObject {
    band: i8,
    gain: f64,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Karaoke {
    level: f64,
    mono_level: f64,
    filter_band: f64,
    filter_width: f64,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Timescale {
    speed: f64,
    pitch: f64,
    rate: f64,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Tremolo {
    frequency: f64,
    depth: f64,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Vibrato {
    frequency: f64,
    depth: f64,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Rotation {
    rotation_hz: f64,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Distortion {
    sin_offset: f64,
    sin_scale: f64,
    cos_offset: f64,
    cos_scale: f64,
    tan_offset: f64,
    tan_scale: f64,
    offset: f64,
    scale: f64,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ChannelMix {
    left_to_left: f64,
    left_to_right: f64,
    right_to_left: f64,
    right_to_right: f64,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct LowPass {
    smoothing: f64,
}
