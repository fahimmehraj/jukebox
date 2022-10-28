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
    /// A handle for sending payloads from the client to this player
    sender: UnboundedSender<ClientPayload>,
    /// A handle for receiving payloads from the client to this player
    receiver: UnboundedReceiver<ClientPayload>,
    /// A handle for sending payloads from this player to the client
    client_sender: UnboundedSender<String>,
    player_connection: Option<PlayerConnection>,
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
        player_tx: UnboundedSender<String>,
    ) -> Result<Self, Error> {
        if let Some(endpoint) = voice_update.event.endpoint {
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
                player_connection: None,
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
        if let Err(e) = PlayerConnection::new(&self).await {
            eprintln!("{}", e);
        }
        todo!()
    }

    async fn setup(&mut self) -> Result<(), WebsocketError> {
        todo!()
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

    async fn send(&mut self, payload: DiscordPayload) -> Result<(), WebsocketError> {
        todo!()
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
    from_gateway: UnboundedReceiver<DiscordPayload>,
    from_udp: UnboundedReceiver<DiscordPayload>,
}

impl PlayerConnection {
    async fn new(player: &Player) -> Result<Self, Error> {
        let (to_player, from_gateway) = unbounded_channel();
        let (mut gateway, to_gateway) = PlayerGateway::new(player, to_player).await?;
        tokio::spawn(async move {
            if let Err(e) = gateway.run().await {
                eprintln!("{}", e);
            }
        });
        let (to_player, from_udp) = unbounded_channel();
        let (mut udp, to_udp) = PlayerUdp::new(player, to_player).await?;
        tokio::spawn(async move {
            if let Err(e) = udp.run().await {
                eprintln!("{}", e);
            }
        });
        todo!()
    }
}

struct PlayerGateway {
    endpoint: String,
    write: Arc<RwLock<SplitSink<WebSocketStream, Message>>>,
    read: SplitStream<WebSocketStream>,
    from_player: UnboundedReceiver<DiscordPayload>,
    heartbeat_interval: Option<Duration>,
    to_player: UnboundedSender<DiscordPayload>,
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
        let (to_self, from_player) = unbounded_channel();
        let gateway = Self {
            endpoint: player.endpoint(),
            write: Arc::new(RwLock::new(write)),
            read,
            heartbeat_interval: None,
            from_player,
            to_player: tx,
        };
        gateway.identify(player).await?;
        Ok((gateway, to_self))
    }

    async fn send(&self, payload: DiscordPayload) -> Result<(), Error> {
        if let Err(e) = self.write.write().await.send(payload.into()).await {
            return Err(Error::new(ErrorKind::Other, e.to_string()));
        }
        Ok(())
    }

    /*
    pub async fn init(&mut self) -> Result<PlayerUDP, Error> {
        if let Some(payload) = self.get_next_message().await {
            if let DiscordPayload::Ready(payload) = payload {
                let (udp_tx, udp_rx) = unbounded_channel();
                let dest_addr: SocketAddr = format!("{}:{}", payload.ip, payload.port)
                    .parse()
                    .expect("Discord did not provide valid udp address");
                let mode = payload
                    .modes
                    .into_iter()
                    .min()
                    .expect("Modes should not be empty");
                let src_addr: SocketAddr = "0.0.0.0:0".parse().unwrap();
                let udp = PlayerUDP::new(payload.ssrc, dest_addr, src_addr, mode);
                self.send(DiscordPayload::SelectProtocol(SelectProtocol {
                    protocol: "udp".to_string(),
                    data: SelectProtocolData {
                        address: ready.ip,
                        port: ready.port,
                        mode: EncryptionMode::xsalsa20_poly1305,
                    },
                }))
                .await?;
                Ok(udp)
            } else {
                Err(Error::new(
                    ErrorKind::ConnectionAborted,
                    "Expected Ready payload first",
                ))
            }
        } else {
            Err(Error::new(
                ErrorKind::ConnectionAborted,
                "Expected Ready payload first",
            ))
        }
    } */

    pub async fn run(mut self) -> Result<(), Error> {
        loop {
            tokio::select! {
                sent_payload = self.from_player.recv() => {
                    if let Some(payload) = sent_payload {
                        self.send(payload).await?;
                    }
                },

                Some(payload) = handle_message(&mut self.read) => {
                    match payload {
                        DiscordPayload::Ready(_) => {
                            self.to_player
                                .send(payload)
                                .expect("Receiver should not be dropped");
                        }
                        DiscordPayload::Hello(payload) => {
                            self.heartbeat_interval =
                                Some(Duration::from_millis(payload.heartbeat_interval));
                            self.start_heartbeating();
                        }
                        DiscordPayload::SessionDescription(_) => {
                            self.to_player
                                .send(payload)
                                .expect("Receiver should not be dropped");
                        }
                        DiscordPayload::Speaking(_) => {}
                        DiscordPayload::Resumed => {}
                        DiscordPayload::ClientDisconnect(_) => {}
                        _ => {}
                    }
                }
            }
        }
    }

    async fn identify(&self, player: &Player) -> Result<(), Error> {
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
    secret_key: Option<[u8; 32]>,
}

impl PlayerUDP {
    fn new(ssrc: u32, dest_ip: SocketAddr, src_ip: SocketAddr, mode: EncryptionMode) -> Self {
        Self {
            ssrc,
            dest_ip,
            src_ip,
            mode,
            secret_key: None,
        }
    }
}
