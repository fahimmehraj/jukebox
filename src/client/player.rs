use std::{
    io::{Error, ErrorKind},
    net::{IpAddr, SocketAddr},
    ptr::null,
    str::FromStr,
    sync::{Arc, Weak},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use anyhow::Result;
use byteorder::{ByteOrder, NetworkEndian, WriteBytesExt};
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
use warp::Buf;

use crate::{
    crypto::EncryptionMode,
    discord::payloads::{Heartbeat, Ready},
    utils::handle_message,
};

use super::payloads::{ClientPayload, VoiceUpdate};
use super::{
    super::discord::payloads::{DiscordPayload, Identify, SelectProtocol, SelectProtocolData},
    Client,
};

type WebSocketStream = tokio_tungstenite::WebSocketStream<MaybeTlsStream<TcpStream>>;

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
    connection_manager: Option<ConnectionManager>,
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

    pub async fn start(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Send the identify payload
        // Receive Ready payload
        // Send Select Protocol payload
        // Receive Session Description payload
        self.connection_manager = Some(ConnectionManager::new(&self).await?);
        loop {
            tokio::select! {
                client_payload = self.client_rx.recv() => {
                    println!("Received client payload: {:?}", client_payload);
                }
            }
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

struct ConnectionManager {
    gateway_rx: UnboundedReceiver<DiscordPayload>,
    gateway_tx: UnboundedSender<DiscordPayload>,
    udp_tx: UnboundedSender<DiscordPayload>,
}

// i gotta clean this up
impl ConnectionManager {
    async fn new(player: &Player) -> Result<Self, Box<dyn std::error::Error>> {
        let (manager_tx, mut gateway_rx) = unbounded_channel();
        let (mut gateway, gateway_tx) = PlayerGateway::new(player, manager_tx).await?;
        tokio::spawn(async move {
            if let Err(e) = gateway.run().await {
                eprintln!("{}", e);
            }
        });
        if let Some(payload) = gateway_rx.recv().await {
            if let DiscordPayload::Ready(payload) = payload {
                let dest_addr: SocketAddr = format!("{}:{}", payload.ip, payload.port)
                    .parse()
                    .expect("Discord did not provide valid udp address");
                let mode = payload
                    .modes
                    .into_iter()
                    .min()
                    .expect("Modes should not be empty");
                let (mut udp, udp_tx) = PlayerUDP::new(payload.ssrc, dest_addr, mode).await?;
                gateway_tx.send(DiscordPayload::SelectProtocol(SelectProtocol {
                    protocol: "udp".to_string(),
                    data: SelectProtocolData {
                        address: udp.src_ip.ip().to_string(),
                        port: udp.src_ip.port(),
                        mode,
                    },
                }))?;
                if let Some(payload) = gateway_rx.recv().await {
                    if let DiscordPayload::SessionDescription(payload) = payload {
                        udp.secret_key = Some(payload.secret_key);
                        udp.mode = payload.mode;
                        tokio::spawn(async move {
                            if let Err(e) = udp.run().await {
                                eprintln!("{}", e);
                            }
                        });
                        return Ok(Self {
                            gateway_rx,
                            gateway_tx,
                            udp_tx,
                        });
                    } else {
                        return Err(Box::new(Error::new(
                            ErrorKind::ConnectionAborted,
                            "Expected SessionDescription payload",
                        )));
                    }
                } else {
                    return Err(Box::new(Error::new(
                        ErrorKind::ConnectionAborted,
                        "No payload received",
                    )));
                }
            } else {
                return Err(Box::new(Error::new(
                    ErrorKind::ConnectionAborted,
                    "Expected Ready payload",
                )));
            }
        } else {
            return Err(Box::new(Error::new(
                ErrorKind::ConnectionAborted,
                "No payload received",
            )));
        }
    }
}

struct PlayerGateway {
    endpoint: Arc<String>,
    write: Arc<RwLock<SplitSink<WebSocketStream, Message>>>,
    read: SplitStream<WebSocketStream>,
    manager_rx: UnboundedReceiver<DiscordPayload>,
    heartbeat_interval: Option<Duration>,
    manager_tx: UnboundedSender<DiscordPayload>,
}

impl PlayerGateway {
    async fn new(
        player: &Player,
        tx: UnboundedSender<DiscordPayload>,
    ) -> Result<(PlayerGateway, UnboundedSender<DiscordPayload>), Error> {
        let url = match url::Url::parse(&format!("wss://{}?v=4", player.endpoint())) {
            Ok(url) => url,
            Err(e) => return Err(Error::new(ErrorKind::InvalidInput, e.to_string())),
        };
        let ws_stream = match connect_async(url).await {
            Ok((ws_stream, _)) => ws_stream,
            Err(e) => {
                return Err(Error::new(
                    ErrorKind::ConnectionRefused,
                    format!("Failed to connect to gateway: {}", e),
                ))
            }
        };
        let (write, read) = ws_stream.split();
        let (gateway_tx, manager_rx) = unbounded_channel();
        let gateway = Self {
            endpoint: player.endpoint(),
            write: Arc::new(RwLock::new(write)),
            read,
            heartbeat_interval: None,
            manager_rx,
            manager_tx: tx,
        };
        gateway.identify(player).await?;
        Ok((gateway, gateway_tx))
    }

    async fn send(&self, payload: DiscordPayload) -> Result<(), Error> {
        if let Err(e) = self.write.write().await.send(payload.into()).await {
            return Err(Error::new(ErrorKind::Other, e.to_string()));
        }
        Ok(())
    }

    pub async fn run(&mut self) -> Result<()> {
        loop {
            tokio::select! {
                sent_payload = self.manager_rx.recv() => {
                    if let Some(payload) = sent_payload {
                        self.send(payload).await?;
                    }
                },

                response = handle_message(&mut self.read) => {
                    match response {
                        Ok(payload) => {
                            match payload {
                                DiscordPayload::Ready(_) => {
                                    self.manager_tx
                                        .send(payload)
                                        .expect("Receiver should not be dropped");
                                }
                                DiscordPayload::Hello(payload) => {
                                    self.heartbeat_interval =
                                        Some(Duration::from_secs_f64(payload.heartbeat_interval / 1000.0));
                                    self.start_heartbeating();
                                }
                                DiscordPayload::SessionDescription(_) => {
                                    self.manager_tx
                                        .send(payload)
                                        .expect("Receiver should not be dropped");
                                }
                                DiscordPayload::Speaking(_) => {}
                                DiscordPayload::Resumed => {}
                                DiscordPayload::ClientDisconnect(_) => {}
                                _ => {}
                            }
                        },
                        Err(e) => {
                            return Err(e);
                        }
                    }
                },
            }
        }
    }

    async fn identify(&self, player: &Player) -> Result<(), Error> {
        eprintln!("Identifying: {:?}", player.session_id());
        let inner_payload = Identify {
            server_id: player.guild_id(),
            user_id: player.user_id().to_string(),
            session_id: player.session_id(),
            token: player.token(),
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
        let heartbeat_interval = self
            .heartbeat_interval
            .expect("Started heartbeating before Hello");
        tokio::spawn(async move {
            let mut interval = time::interval(heartbeat_interval);
            loop {
                interval.tick().await;
                match weak_sender.upgrade() {
                    Some(write) => write.write().await.send(Self::heartbeat().into()).await,
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
    dest_ip: SocketAddr,
    src_ip: SocketAddr,
    mode: EncryptionMode,
    player_rx: UnboundedReceiver<DiscordPayload>,
    socket: Arc<UdpSocket>,
    secret_key: Option<[u8; 32]>,
}

impl PlayerUDP {
    async fn new(
        ssrc: u32,
        dest_ip: SocketAddr,
        mode: EncryptionMode,
    ) -> Result<(Self, UnboundedSender<DiscordPayload>)> {
        let (udp_tx, player_rx) = unbounded_channel();
        let socket = Arc::new(UdpSocket::bind("0.0.0.0:0").await?);
        socket.connect(dest_ip).await?;
        let src_ip = Self::ip_discovery(Arc::clone(&socket), ssrc).await?;

        Ok((
            Self {
                ssrc,
                dest_ip,
                src_ip,
                mode,
                player_rx,
                socket,
                secret_key: None,
            },
            udp_tx,
        ))
    }

    async fn run(&mut self) -> Result<(), Error> {
        while let Some(msg) = self.player_rx.recv().await {}
        Ok(())
    }

    async fn ip_discovery(socket: Arc<UdpSocket>, ssrc: u32) -> Result<SocketAddr> {
        let mut discovery_buf = vec![0u8; 74];
        NetworkEndian::write_u16(&mut discovery_buf[0..2], 0x1);
        NetworkEndian::write_u16(&mut discovery_buf[2..4], 70);
        NetworkEndian::write_u32(&mut discovery_buf[4..8], ssrc);

        socket.send(&discovery_buf).await?;
        socket.recv(&mut discovery_buf).await?;
        if NetworkEndian::read_u16(&discovery_buf[0..2]) == 0x2 {
            if NetworkEndian::read_u16(&discovery_buf[2..4]) == 70 {
                if NetworkEndian::read_u32(&discovery_buf[4..8]) == ssrc {
                    let null_byte_index = &discovery_buf
                        .iter()
                        .skip(8)
                        .position(|&x| x == 0)
                        .expect("No null byte");
                    let ip = std::str::from_utf8(&discovery_buf[8..8 + null_byte_index])?;
                    let port = NetworkEndian::read_u16(&discovery_buf[discovery_buf.len() - 2..]);
                    return Ok(SocketAddr::from_str(&format!("{}:{}", ip, port))?);
                }
            }
        }
        Err(Error::new(ErrorKind::Other, "Failed to discover IP"))?
    }
}
