use std::time::Duration;

use anyhow::Result;

use derivative::Derivative;
use tracing::info;

use crate::voice::{VoiceError, VoiceManager};

use super::payloads::{ClientPayload, Opcode, VoiceUpdate};

#[derive(Derivative)]
#[derivative(Debug)]
pub struct Player {
    user_id: String,
    guild_id: String,
    session_id: String,
    #[derivative(Debug = "ignore")]
    token: String,

    pub connection_manager: VoiceManager,

    track: Option<String>,
    start_time: Option<Duration>,
    end_time: Option<Duration>,
    volume: Option<i16>,
    no_replace: Option<bool>,
    pause: Option<bool>,
}

impl Player {
    #[tracing::instrument]
    pub async fn new(
        user_id: &str,
        voice_update: VoiceUpdate,
    ) -> Result<Self, VoiceError> {
        let connection_manager = VoiceManager::new(user_id, voice_update.clone()).await?;
        Ok(Self {
            connection_manager,
            user_id: user_id.to_owned(),
            guild_id: voice_update.event.guild_id,
            session_id: voice_update.session_id,
            token: voice_update.event.token,
            track: None,
            start_time: None,
            end_time: None,
            volume: None,
            no_replace: None,
            pause: None,
        })
    }

    pub async fn handle_client_payload(&mut self, client_payload: ClientPayload) -> Result<()> {
        match client_payload.op {
            Opcode::Destroy(_) => {
                info!("Destroying player");
                Err(anyhow::anyhow!("Destroying player"))
            }
            Opcode::Play(_) => {
                info!("Playing track");
                self.connection_manager
                    .play_audio(String::from("Ghost Town.webm"))
                    .await?;
                Ok(())
            }
            _ => {
                info!("Received client payload: {:?}", client_payload);
                Ok(())
            }
        }
    }

    pub fn user_id(&self) -> &str {
        &self.user_id
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
