use std::time::Duration;

use crate::incoming_payloads::VoiceUpdate;

pub struct Player {
    guild_id: String,
    session_id: String,
    token: String,
    endpoint: Option<String>,
    track: Option<String>,
    start_time: Option<Duration>,
    end_time: Option<Duration>,
    volume: Option<i16>,
    no_replace: Option<bool>,
    pause: Option<bool>,
}

impl From<VoiceUpdate> for Player {
    fn from(voice_update: VoiceUpdate) -> Self {
        Self {
            guild_id: voice_update.guild_id,
            session_id: voice_update.session_id,
            token: voice_update.event.token,
            endpoint: voice_update.event.endpoint,
            track: None,
            start_time: None,
            end_time: None,
            volume: None,
            no_replace: None,
            pause: None,
        }
    }
}

impl Player {
    pub fn guild_id(&self) -> String {
        self.guild_id.clone()
    }

    pub fn session_id(&self) -> String {
        self.session_id.clone()
    }

    pub fn token(&self) -> String {
        self.token.clone()
    }

    pub fn endpoint(&self) -> Option<String> {
        self.endpoint.clone()
    }

    pub fn track(&self) -> Option<String> {
        self.track.clone()
    }

    pub fn start_time(&self) -> Option<Duration> {
        self.start_time
    }

    pub fn end_time(&self) -> Option<Duration> {
        self.end_time
    }

    pub fn volume(&self) -> Option<i16> {
        self.volume
    }

    pub fn no_replace(&self) -> Option<bool> {
        self.no_replace
    }

    pub fn pause(&self) -> Option<bool> {
        self.pause
    }
}