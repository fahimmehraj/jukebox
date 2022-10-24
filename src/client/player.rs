use std::{
    io::{Error, ErrorKind},
    time::Duration,
};

use futures_util::{
    stream::{SplitSink, SplitStream},
    SinkExt, StreamExt,
};
use tokio::{
    net::TcpStream,
    sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender},
};
use tokio_tungstenite::{
    connect_async,
    tungstenite::{Error as WebsocketError, Message},
    MaybeTlsStream,
};

use crate::{crypto::EncryptionMode, discord::payloads::Ready, utils::handle_message};

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

#[derive(Clone)]
struct ConnectionDetails {
    ssrc: u32,
    ip: String,
    port: u16,
    mode: EncryptionMode,
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

    async fn setup(&mut self) -> Result<(), WebsocketError> {
        self.send(DiscordPayload::Identify(self.identify_payload()))
            .await?;
        while let Some(payload) = handle_message(&mut self.ws_read).await {
            if let DiscordPayload::Ready(payload) = payload {
                self.send(DiscordPayload::SelectProtocol(
                    Self::select_protocol_payload(payload),
                ));
            }
        }
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

    fn select_protocol_payload(ready_payload: Ready) -> SelectProtocol {
        SelectProtocol {
            protocol: "udp".to_string(),
            data: SelectProtocolData {
                address: ready_payload.ip,
                port: ready_payload.port,
                mode: ready_payload.modes.into_iter().min().unwrap(),
            },
        }
    }

    // fn select_protocol_payload(&mut self) -> Result<SelectProtocol, Error> {
    //     if let Some(mut connection_details) = self.connection_details.as_mut() {
    //         if let Some(possible_modes) = &connection_details.possible_modes {
    //             connection_details.mode = Some(possible_modes.iter().min().unwrap().clone());
    //             Ok(SelectProtocol {
    //                 protocol: "udp".to_string(),
    //                 data: SelectProtocolData {
    //                     address: connection_details.ip.clone().unwrap(),
    //                     port: connection_details.port.unwrap(),
    //                     mode: connection_details.mode.unwrap(),
    //                 },
    //             })
    //         } else {
    //             Err(Error::new(
    //                 ErrorKind::ConnectionAborted,
    //                 "No possible modes provided",
    //             ))
    //         }
    //     } else {
    //         Err(Error::new(
    //             ErrorKind::ConnectionAborted,
    //             "No connection details provided",
    //         ))
    //     }
    // }

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
                    ssrc: payload.ssrc,
                    ip: payload.ip,
                    port: payload.port,
                    mode: payload.modes.into_iter().min().unwrap(),
                    heartbeat_interval: None,
                    secret_key: None,
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
