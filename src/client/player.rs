use std::{
    io::{Error, ErrorKind},
    time::Duration,
};

use tokio::sync::mpsc::UnboundedSender;

use crate::{incoming_payloads::VoiceUpdate, Payload};

pub struct Player {
    guild_id: String,
    session_id: String,
    token: String,
    endpoint: String,
    sender: UnboundedSender<Payload>,
    track: Option<String>,
    start_time: Option<Duration>,
    end_time: Option<Duration>,
    volume: Option<i16>,
    no_replace: Option<bool>,
    pause: Option<bool>,
}

impl Player {
    pub fn new(
        voice_update: VoiceUpdate,
        tx: UnboundedSender<Payload>
    ) -> Result<Self, Error> {
        if let Some(endpoint) = voice_update.event.endpoint {
            Ok(Self {
                guild_id: voice_update.event.guild_id,
                session_id: voice_update.session_id,
                token: voice_update.event.token,
                endpoint: endpoint,
                sender: tx,
                track: None,
                start_time: None,
                end_time: None,
                volume: None,
                no_replace: None,
                pause: None,
            })
        } else {
            Err(Error::new(
                ErrorKind::ConnectionAborted,
                "No endpoint provided",
            ))
        }
    }

    pub fn guild_id(&self) -> String {
        self.guild_id.clone()
    }

    pub fn session_id(&self) -> String {
        self.session_id.clone()
    }

    pub fn token(&self) -> String {
        self.token.clone()
    }

    pub fn endpoint(&self) -> String {
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

    pub fn sender(&self) -> UnboundedSender<Payload> {
        self.sender.clone()
    }
}
