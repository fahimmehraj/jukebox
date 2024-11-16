use std::{
    io::{Error, ErrorKind},
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use anyhow::Result;
use futures_util::{
    stream::{SplitSink, SplitStream},
    SinkExt, StreamExt,
};
use tracing::{error};
use tokio::{
    net::TcpStream,
    sync::{
        mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender},
        RwLock,
    },
    time,
};
use tokio_tungstenite::{connect_async, tungstenite::Message, MaybeTlsStream};

use crate::{client::player::Player, utils::handle_message};

use super::super::payloads::*;

type WebSocketStream = tokio_tungstenite::WebSocketStream<MaybeTlsStream<TcpStream>>;
pub struct VoiceGateway {
    endpoint: Arc<String>,
    write: Arc<RwLock<SplitSink<WebSocketStream, Message>>>,
    read: SplitStream<WebSocketStream>,
    manager_rx: UnboundedReceiver<DiscordPayload>,
    heartbeat_interval: Option<Duration>,
    manager_tx: UnboundedSender<DiscordPayload>,
}

impl VoiceGateway {
    /// Connects to the voice gateway, sends identify payload, and returns a [`VoiceGateway`]
    /// as well as a [`UnboundedSender`] to send payloads to the gateway.
    pub async fn connect(
        player: &Player,
        tx: UnboundedSender<DiscordPayload>,
    ) -> Result<(VoiceGateway, UnboundedSender<DiscordPayload>)> {
        let url = match url::Url::parse(&format!("wss://{}?v=7", player.endpoint())) {
            Ok(url) => url,
            Err(e) => return Err(Error::new(ErrorKind::InvalidInput, e.to_string()))?,
        };
        let (ws_stream, _) = connect_async(url).await?;
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

    async fn send(&self, payload: DiscordPayload) -> Result<()> {
        if let Err(e) = self.write.write().await.send(payload.into()).await {
            return Err(Error::new(ErrorKind::Other, e.to_string()))?;
        }
        Ok(())
    }

    /// Runs the event loop for the voice gateway. It responds to gateway events
    /// and messages sent from the [`super::VoiceManager`].
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
                            error!("frick");
                            return Err(e);
                        }
                    }
                },
            }
        }
    }

    async fn identify(&self, player: &Player) -> Result<()> {
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
                        error!("Websocket writer is dropped");
                        break;
                    }
                };
            }
        });
    }
}
