use std::{
    io::{Error, ErrorKind},
    sync::Arc,
    time::Duration,
};

use futures_util::{
    stream::{SplitSink, SplitStream},
    SinkExt, StreamExt,
};
use tokio::{
    net::TcpStream,
    sync::{
        mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender},
        RwLock,
    },
};
use tokio_tungstenite::{
    connect_async,
    tungstenite::{Error as WebsocketError, Message},
    MaybeTlsStream,
};

use crate::{
    crypto::EncryptionMode,
    discord::payloads::{Hello, SessionDescription},
    utils::handle_message,
};

use super::super::discord::payloads::{
    DiscordPayload, Identify, SelectProtocol, SelectProtocolData,
};
use super::payloads::{ClientPayload, VoiceUpdate};

type WebSocketStream = tokio_tungstenite::WebSocketStream<MaybeTlsStream<TcpStream>>;

pub struct Player {
    user_id: String,
    guild_id: String,
    session_id: String,
    token: String,
    endpoint: String,
    ws_write: SplitSink<WebSocketStream, Message>,
    ws_read: SplitStream<WebSocketStream>,
    /// A handle for sending payloads from the client to this player
    sender: UnboundedSender<ClientPayload>,
    /// A handle for receiving payloads from the client to this player
    receiver: UnboundedReceiver<ClientPayload>,
    /// A handle for sending payloads from this player to the client
    client_sender: UnboundedSender<String>,
    connection_details: Option<ConnectionDetails>,
    track: Option<String>,
    start_time: Option<Duration>,
    end_time: Option<Duration>,
    volume: Option<i16>,
    no_replace: Option<bool>,
    pause: Option<bool>,
}

#[derive(Clone, Default)]
struct ConnectionDetails {
    ssrc: Option<u32>,
    ip: Option<String>,
    port: Option<u16>,
    mode: Option<EncryptionMode>,
    possible_modes: Option<Vec<EncryptionMode>>,
    heartbeat_interval: Option<Duration>,
    secret_key: Option<[u8; 32]>,
}

impl Player {
    pub async fn new(
        user_id: String,
        voice_update: VoiceUpdate,
        player_tx: UnboundedSender<String>,
    ) -> Result<Self, Error> {
        if let Some(endpoint) = voice_update.event.endpoint {
            let url = url::Url::parse(&format!("wss://{}?v=4", endpoint.clone())).unwrap();
            let ws_stream = match connect_async(url).await {
                Ok((ws_stream, _)) => ws_stream,
                Err(e) => {
                    eprintln!("Could not connect to voice endpoint, {}", e);
                    player_tx
                        .send(format!("Could not connect to voice endpoint, {}", e))
                        .unwrap();
                    return Err(Error::new(
                        ErrorKind::ConnectionRefused,
                        "Could not connect to voice endpoint.",
                    ));
                }
            };
            let (ws_write, ws_read) = ws_stream.split();
            let (tx, rx) = unbounded_channel();
            Ok(Self {
                user_id,
                guild_id: voice_update.event.guild_id,
                session_id: voice_update.session_id,
                token: voice_update.event.token,
                endpoint,
                sender: tx,
                receiver: rx,
                client_sender: player_tx,
                ws_write,
                ws_read,
                connection_details: None,
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

    pub async fn start(mut self) -> Result<(), WebsocketError> {
        // Send the identify payload
        // Receive Ready payload
        // Send Select Protocol payload
        // Receive Session Description payload

        self.send(DiscordPayload::Identify(self.identify_payload()))
            .await?;

        tokio::spawn(async move {
            while let Some(payload) =
                handle_message::<_, _, _, DiscordPayload>(&mut self.ws_read).await
            {
                self.handle_discord_payload(payload).await;
            }
            let client_listener = tokio::spawn(async move {
                while let Some(payload) = self.receiver.recv().await {
                    println!("payload: {:?}", payload);
                }
            });
        });

        Ok(())
    }

    fn identify_payload(&self) -> Identify {
        Identify {
            server_id: self.guild_id(),
            user_id: self.user_id(),
            session_id: self.session_id(),
            token: self.token(),
        }
    }

    fn select_protocol_payload(&self) -> Result<SelectProtocol, Error> {
        if let Some(connection_details) = self.connection_details {
            if let Some(possible_modes) = connection_details.possible_modes {
                let mode = possible_modes
                    .iter()
                    .find(|mode| mode == &&EncryptionMode::XSalsa20Poly1305)
                    .unwrap_or(&EncryptionMode::XSalsa20Poly1305);
                Ok(SelectProtocol {
                    protocol: "udp".to_string(),
                    data: SelectProtocolData {
                        address: connection_details.ip.unwrap(),
                        port: connection_details.port.unwrap(),
                        mode: mode.to_string(),
                    },
                })
            } else {
                Err(Error::new(
                    ErrorKind::ConnectionAborted,
                    "No possible modes provided",
                ))
            }
        } else {
            Err(Error::new(
                ErrorKind::ConnectionAborted,
                "No connection details provided",
            ))
        }
    }

    async fn send(&mut self, payload: DiscordPayload) -> Result<(), WebsocketError> {
        let message = serde_json::to_string(&payload).unwrap();
        self.ws_write.send(message.into()).await
        // if let Err(e) = self.ws_stream.send(message.into()).await {
        //     eprintln!("Could not send payload, {}", e);
        //     self.client_sender
        //         .send(format!("Could not send payload, {}", e));
        // };
    }

    async fn handle_discord_payload(&mut self, payload: DiscordPayload) {
        match payload {
            DiscordPayload::Ready(payload) => {
                self.connection_details = Some(ConnectionDetails {
                    ssrc: Some(payload.ssrc),
                    ip: Some(payload.ip),
                    port: Some(payload.port),
                    possible_modes: Some(payload.modes),
                    ..self.connection_details.clone().unwrap_or_default()
                })
            }
            DiscordPayload::Hello(payload) => {
                self.connection_details = Some(ConnectionDetails {
                    heartbeat_interval: Some(Duration::from_millis(payload.heartbeat_interval)),
                    ..self.connection_details.clone().unwrap_or_default()
                })
            }
            DiscordPayload::SessionDescription(payload) => {
                self.connection_details = Some(ConnectionDetails {
                    mode: Some(payload.mode),
                    secret_key: Some(payload.secret_key),
                    ..self.connection_details.clone().unwrap_or_default()
                })
            }
            _ => {}
        }
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
