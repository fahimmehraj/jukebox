use std::{
    io::{Error, ErrorKind},
    sync::Arc,
    time::Duration,
};

use anyhow::Result;

use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};

use crate::discord::voice::VoiceManager;

use super::payloads::{ClientPayload, Opcode, VoiceUpdate};

pub struct Player {
    user_id: Arc<String>,
    guild_id: String,
    session_id: String,
    token: String,
    endpoint: Arc<String>,
    // A channel for receiving messages from the client to this player
    client_rx: UnboundedReceiver<ClientPayload>,
    // A channel for sending messages from this player to the client
    client_tx: UnboundedSender<ClientPayload>,
    connection_manager: Option<VoiceManager>,
    track: Option<String>,
    start_time: Option<Duration>,
    end_time: Option<Duration>,
    volume: Option<i16>,
    no_replace: Option<bool>,
    pause: Option<bool>,
}

impl Player {
    pub async fn new(
        user_id: Arc<String>,
        voice_update: VoiceUpdate,
        client_tx: UnboundedSender<ClientPayload>,
    ) -> Result<(Self, UnboundedSender<ClientPayload>), Error> {
        if let Some(endpoint) = voice_update.event.endpoint {
            let (player_tx, client_rx) = unbounded_channel();
            Ok((
                Self {
                    user_id,
                    guild_id: voice_update.event.guild_id,
                    session_id: voice_update.session_id,
                    token: voice_update.event.token,
                    endpoint: Arc::new(endpoint),
                    client_rx,
                    client_tx,
                    connection_manager: None,
                    track: None,
                    start_time: None,
                    end_time: None,
                    volume: None,
                    no_replace: None,
                    pause: None,
                },
                player_tx,
            ))
        } else {
            Err(Error::new(
                ErrorKind::ConnectionAborted,
                "No endpoint provided",
            ))
        }
    }

    pub async fn start(&mut self) -> Result<()> {
        // Send the identify payload
        // Receive Ready payload
        // Send Select Protocol payload
        // Receive Session Description payload
        self.connection_manager = Some(VoiceManager::new(&self).await?);
        if let Some(connection_manager) = &self.connection_manager {
            connection_manager.play_audio("NothingNew.opus").await?;
        }
        loop {
            tokio::select! {
                client_payload = self.client_rx.recv() => {
                    if let Some(payload) = client_payload {
                        self.handle_client_payload(payload).await?;
                    } else {
                        return Err(anyhow::anyhow!("Client disconnected before destroy"));
                    }
                }
            }
        }
    }

    async fn handle_client_payload(&mut self, client_payload: ClientPayload) -> Result<()> {
        if let Some(connection_manager) = &self.connection_manager {
        match client_payload.op {
            Opcode::Destroy(_) => {
                println!("Destroying player");
                Err(anyhow::anyhow!("Destroying player"))
            },
            Opcode::Play(_) => {
                println!("Playing track");
                connection_manager.play_audio("NothingNew.opus").await?;
                Ok(())
            }
            _ => {
                println!("Received client payload: {:?}", client_payload);
                Ok(())
            }
        }
        } else {
           Err(anyhow::anyhow!("No connection maanager"))
        }
    }

    pub fn user_id(&self) -> Arc<String> {
        self.user_id.clone()
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

    pub fn endpoint(&self) -> Arc<String> {
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
