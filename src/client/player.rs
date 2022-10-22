use std::{
    io::{Error, ErrorKind},
    time::Duration,
};

use futures_util::SinkExt;
use tokio::{net::TcpStream, sync::mpsc::UnboundedSender};
use tokio_tungstenite::{connect_async, MaybeTlsStream, WebSocketStream};

use super::super::discord::payloads::{Payload as DiscordPayload, Identify};
use super::payloads::{Payload as ClientPayload, VoiceUpdate};

pub struct Player {
    user_id: String,
    guild_id: String,
    session_id: String,
    token: String,
    endpoint: String,
    ws_stream: WebSocketStream<MaybeTlsStream<TcpStream>>,
    sender: UnboundedSender<ClientPayload>,
    client_sender: UnboundedSender<String>,
    track: Option<String>,
    start_time: Option<Duration>,
    end_time: Option<Duration>,
    volume: Option<i16>,
    no_replace: Option<bool>,
    pause: Option<bool>,
}

impl Player {
    pub async fn new(
        user_id: String,
        voice_update: VoiceUpdate,
        tx: UnboundedSender<ClientPayload>,
        player_tx: UnboundedSender<String>,
    ) -> Result<Self, Error> {
        if let Some(endpoint) = voice_update.event.endpoint {
            let url = url::Url::parse(&format!("wss://{}", endpoint.clone())).unwrap();
            let ws_stream = match connect_async(url).await {
                Ok((ws_stream, _)) => ws_stream,
                Err(e) => {
                    eprintln!("Could not connect to voice endpoint, {}", e);
                    player_tx.send(format!("Could not connect to voice endpoint, {}", e)).unwrap();
                    return Err(Error::new(
                        ErrorKind::ConnectionRefused,
                        "Could not connect to voice endpoint.",
                    ));
                }
            };
            Ok(Self {
                user_id,
                guild_id: voice_update.event.guild_id,
                session_id: voice_update.session_id,
                token: voice_update.event.token,
                endpoint,
                sender: tx,
                client_sender: player_tx,
                ws_stream,
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

    async fn init(&mut self) {
        self.send(DiscordPayload::Identify(self.derive_idenitfy_payload())).await;

    }

    fn derive_idenitfy_payload(&self) -> Identify {
        Identify {
            server_id: self.guild_id(),
            user_id: self.user_id(),
            session_id: self.session_id(),
            token: self.token(),
        }
    }

    async fn send(&mut self, payload: DiscordPayload) -> Result<(), tokio_tungstenite::tungstenite::Error> {
        let message = serde_json::to_string(&payload).unwrap();
        self.ws_stream.send(message.into()).await
        // if let Err(e) = self.ws_stream.send(message.into()).await {
        //     eprintln!("Could not send payload, {}", e);
        //     self.client_sender
        //         .send(format!("Could not send payload, {}", e));
        // };
    }

    pub fn user_id(&self) -> String {
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

    pub fn sender(&self) -> UnboundedSender<ClientPayload> {
        self.sender.clone()
    }
}
