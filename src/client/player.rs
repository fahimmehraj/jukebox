use std::{
    io::{Error, ErrorKind},
    net::{IpAddr, SocketAddr},
    sync::{Arc, Weak},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use futures_util::{
    stream::{SplitSink, SplitStream},
    FutureExt, SinkExt, StreamExt,
};
use tokio::{
    net::{TcpStream, UdpSocket},
    sync::{
        mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender},
        RwLock,
    },
    time,
};
use tokio_tungstenite::{
    connect_async,
    tungstenite::{Error as WebsocketError, Message},
    MaybeTlsStream,
};

use crate::{
    crypto::EncryptionMode,
    discord::payloads::{Heartbeat, Ready},
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
    socket: Option<Arc<UdpSocket>>,
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
    ip: IpAddr,
    port: u16,
    mode: EncryptionMode,
    heartbeat_interval: Option<Duration>,
    secret_key: [u8; 32],
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
                socket: None,
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
            if let DiscordPayload::Ready(ready_payload) = payload {
                let udp_addr: SocketAddr = format!("{}:{}", ready_payload.ip, ready_payload.port)
                    .parse()
                    .unwrap();
                let socket = UdpSocket::bind("0.0.0.0:0").await?;
                socket.connect(udp_addr).await?;
                self.send(DiscordPayload::SelectProtocol(
                    Self::select_protocol_payload(ready_payload.modes, socket.local_addr()?),
                ));
                while let Some(payload) = handle_message(&mut self.ws_read).await {
                    if let DiscordPayload::SessionDescription(session_desc_payload) = payload {
                        self.connection_details = Some(ConnectionDetails {
                            ssrc: ready_payload.ssrc,
                            ip: socket.local_addr()?.ip(),
                            port: socket.local_addr()?.port(),
                            mode: session_desc_payload.mode,
                            secret_key: session_desc_payload.secret_key,
                        });
                        self.socket = Some(Arc::new(socket));
                        break;
                    }
                }
            } else {
                self.handle_discord_payload(payload).await;
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

    fn select_protocol_payload(
        modes: Vec<EncryptionMode>,
        socket_addr: SocketAddr,
    ) -> SelectProtocol {
        SelectProtocol {
            protocol: "udp".to_string(),
            data: SelectProtocolData {
                address: socket_addr.ip().to_string(),
                port: socket_addr.port(),
                mode: modes.into_iter().min().unwrap(),
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

struct PlayerConnection {
    gateway: PlayerGateway,
    udp: PlayerUDP,
}

impl PlayerConnection {
    async fn new(player: &Player) {
        let gateway = PlayerGateway::new(player.endpoint.clone());

    }
}

struct PlayerGateway {
    endpoint: String,
    write: Arc<RwLock<SplitSink<WebSocketStream, Message>>>,
    read: RwLock<SplitStream<WebSocketStream>>,
    heartbeat_interval: Duration,
}

impl PlayerGateway {
    async fn new(endpoint: String) -> Result<PlayerGateway, Error> {
        let url = match url::Url::parse(&format!("wss://{}?v=4", endpoint)) {
            Ok(url) => url,
            Err(e) => return Err(Error::new(ErrorKind::InvalidInput, e.to_string())),
        };
        let ws_stream = match connect_async(url).await {
            Ok((ws_stream, _)) => ws_stream,
            Err(e) => {
                return Err(Error::new(
                    ErrorKind::ConnectionRefused,
                    "Could not connect to voice endpoint.",
                ));
            }
        };
        let (write, mut read) = ws_stream.split();
        if let Some(payload) = handle_message(&mut read).await {
            if let DiscordPayload::Hello(payload) = payload {
                let gateway = Self {
                    endpoint,
                    write: Arc::new(RwLock::new(write)),
                    read: RwLock::new(read),
                    heartbeat_interval: Duration::from_millis(payload.heartbeat_interval),
                };
                gateway.start_heartbeating();
                Ok(gateway)
            } else {
            Err(Error::new(
                ErrorKind::Unsupported,
                "Did not receive hello payload first",
            ))
            }
        } else {
            Err(Error::new(
                ErrorKind::NotConnected,
                "Websocket not functioning",
            ))
        }
    }

    async fn send(&self, payload: DiscordPayload) -> Result<(), Error> {
        if let Err(e) = self.write.write().await.send(payload.into()).await {
            return Err(Error::new(ErrorKind::Other, e.to_string()));
        }
        Ok(())
    }

    pub async fn identify(&self, player: &Player) -> Result<(), Error> {
        let inner_payload = Identify {
            server_id: player.guild_id(),
            user_id: player.user_id(),
            session_id: player.session_id(),
            token: player.session_id(),
        };
        self.send(DiscordPayload::Identify(inner_payload)).await
    }

    fn heartbeat() -> DiscordPayload {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis();
        DiscordPayload::Heartbeat(Heartbeat { nonce })
    }

    fn start_heartbeating(&self) {
        let weak_sender = Arc::downgrade(&self.write);
        let heartbeat_interval = self.heartbeat_interval;
        tokio::spawn(async move {
            let mut interval = time::interval(heartbeat_interval);
            loop {
                interval.tick().await;
                match weak_sender.upgrade() {
                    Some(write) => write.write().await.send(Self::heartbeat().into()),
                    None => {
                        eprintln!("Websocket writer is dropped");
                        break;
                    }
                };
            }
        });
    }
}

struct PlayerUDP {
    ssrc: u32,
    dest_ip: IpAddr,
    src_ip: IpAddr,
    mode: EncryptionMode,
    secret_key: [u8; 32]
}
