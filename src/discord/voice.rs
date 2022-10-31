mod gateway;
mod udp;

use std::{
    io::{Error, ErrorKind},
    net::SocketAddr,
};

use anyhow::Result;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};

use crate::client::player::Player;

use super::payloads::*;

pub use gateway::VoiceGateway;
pub use udp::{UDPMessage, VoiceUDP};

pub struct VoiceManager {
    gateway_rx: UnboundedReceiver<DiscordPayload>,
    gateway_tx: UnboundedSender<DiscordPayload>,
    udp_tx: UnboundedSender<UDPMessage>,
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
                let (mut udp, udp_tx) = VoiceUDP::connect(payload.ssrc, dest_addr, mode).await?;
                gateway_tx.send(DiscordPayload::SelectProtocol(SelectProtocol {
                    protocol: "udp".to_string(),
                    data: SelectProtocolData {
                        address: udp.local_addr().ip().to_string(),
                        port: udp.local_addr().port(),
                        mode,
                    },
                }))?;
                if let Some(payload) = gateway_rx.recv().await {
                    if let DiscordPayload::SessionDescription(payload) = payload {
                        *udp.secret_key_mut() = Some(payload.secret_key);
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
}
