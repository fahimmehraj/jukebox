mod gateway;
mod udp;

use std::{
    io::{Error, ErrorKind},
    net::SocketAddr,
    sync::Arc, task::Context,
};

use std::task::Poll::Ready;
use anyhow::Result;
use futures_util::StreamExt;
use log::{error, info};
use ogg::reading::async_api::PacketReader;
use tokio::{
    fs::File,
    io::{AsyncReadExt, BufReader},
    sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender},
    time,
};
use xsalsa20poly1305::{XSalsa20Poly1305, KeyInit};

use crate::{client::player::Player, server::Headers};

use super::payloads::*;

pub use gateway::VoiceGateway;
pub use udp::{UDPMessage, VoiceUDP};

const FRAME_SIZE_IN_BYTES: usize = 200;

pub struct VoiceManager {
    user_id: Arc<String>,
    ssrc: u32,
    gateway_rx: UnboundedReceiver<DiscordPayload>,
    gateway_tx: UnboundedSender<DiscordPayload>,
    udp_tx: Arc<UnboundedSender<UDPMessage>>,
}

// i gotta clean this up
impl VoiceManager {
    /// Initializes the voice gateway and UDP connection. The returned connection is fully
    /// authenticated and ready to send and receive audio.
    pub async fn new(player: &Player) -> Result<Self> {
        let (manager_tx, mut gateway_rx) = unbounded_channel();
        let (mut gateway, gateway_tx) = VoiceGateway::connect(player, manager_tx).await?;
        tokio::spawn(async move {
            if let Err(e) = gateway.run().await {
                error!("{}", e);
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
                // cache the ssrc
                let ssrc = payload.ssrc;
                let (mut udp, udp_tx) = VoiceUDP::connect(payload.ssrc, dest_addr, mode).await?;
                let test_payload = DiscordPayload::SelectProtocol(SelectProtocol {
                    protocol: "udp".to_string(),
                    data: SelectProtocolData {
                        address: udp.local_addr().ip().to_string(),
                        port: udp.local_addr().port(),
                        mode,
                    },
                });
                info!("About to send select protocol thing, {:?}", serde_json::to_string(&test_payload).unwrap());
                gateway_tx.send(test_payload)?;
                info!("Waiting on Session Description");
                if let Some(payload) = gateway_rx.recv().await {
                    if let DiscordPayload::SessionDescription(payload) = payload {
                        *udp.cipher_mut() = Some(XSalsa20Poly1305::new_from_slice(&payload.secret_key)?);
                        tokio::spawn(async move {
                            if let Err(e) = udp.run().await {
                                error!("{}", e);
                            }
                        });
                        return Ok(Self {
                            user_id: player.user_id(),
                            ssrc,
                            gateway_rx,
                            gateway_tx,
                            udp_tx: Arc::new(udp_tx),
                        });
                    } else {
                        return Err(Error::new(
                            ErrorKind::ConnectionAborted,
                            "Expected SessionDescription payload",
                        ))?;
                    }
                } else {
                    return Err(Error::new(
                        ErrorKind::ConnectionAborted,
                        "No payload received",
                    ))?;
                }
            } else {
                return Err(Error::new(
                    ErrorKind::ConnectionAborted,
                    "Expected Ready payload",
                ))?;
            }
        } else {
            return Err(Error::new(
                ErrorKind::ConnectionAborted,
                "No payload received",
            ))?;
        }
    }

    pub async fn play_audio(&self, path: &str) -> Result<()> {
        self.gateway_tx.send(DiscordPayload::Speaking(Speaking {
            speaking: 1,
            delay: Some(0),
            user_id: None,
            ssrc: self.ssrc,
        }))?;
        info!("started playing audio");
        let audio_file = File::open(path).await?;
        let mut reader = PacketReader::new(audio_file);
        let mut interval = time::interval(time::Duration::from_millis(20));
        let weak_udp_tx = Arc::downgrade(&self.udp_tx);
        tokio::spawn(async move {
            loop {
                interval.tick().await;
                if let Some(udp_tx) = weak_udp_tx.upgrade() {
                    // let mut buffer = vec![0; FRAME_SIZE_IN_BYTES];
                    // let bytes_read = reader.read(&mut buffer).await.unwrap();
                    // if bytes_read == 0 {
                    //     break;
                    // }
                    if let Some(Ok(packet)) = reader.next().await {
                    udp_tx.send(UDPMessage::Audio(packet.data)).unwrap();
                    info!("sent audio");
                    } else {
                        break;
                    }
                } else {
                    break;
                }
            }
        });

        Ok(())
    }
}
