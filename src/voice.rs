mod gateway;
mod payloads;
mod udp;

use std::{
    io::ErrorKind,
    net::SocketAddr,
    sync::Arc,
};

use anyhow::Result;
use crypto_secretbox::{KeyInit, XSalsa20Poly1305};
use derivative::Derivative;
use futures_util::StreamExt;
use payloads::{DiscordPayload, SelectProtocol, SelectProtocolData, Speaking};
use thiserror::Error;
use tracing::{error, info, trace};

use tokio::{
    fs::File,
    sync::mpsc::{unbounded_channel, Sender, UnboundedReceiver, UnboundedSender},
    time,
};

use crate::{
    client::payloads::VoiceUpdate,
    webm_parse::WebmStream,
};

use gateway::VoiceGateway;
use udp::{UDPMessage, VoiceUDP};

#[derive(Error, Debug)]
pub enum VoiceError {
    #[error("invalid endpoint provided: {0}")]
    InvalidEndpoint(#[from] url::ParseError),
    #[error("websocket gateway error: {0}")]
    WebsocketError(#[from] tokio_tungstenite::tungstenite::Error),
    #[error("udp error: {0}")]
    UdpError(#[from] std::io::Error),
    #[error("gateway dropped: {0}")]
    SendError(#[from] tokio::sync::mpsc::error::SendError<DiscordPayload>),
    #[error("discord violated protocol: ")]
    UnexpectedProtocolError(String),

    #[error("discord sent invalid payload: {0}")]
    InvalidPayloadError(#[from] crate::utils::ReadMessageError),
}

// Essentially a handle to both the websocket-based voice gateway and the udp
// socket for sending audio
#[derive(Derivative)]
#[derivative(Debug)]
pub struct VoiceManager {
    ssrc: u32,
    #[derivative(Debug = "ignore")]
    from_gateway_rx: UnboundedReceiver<DiscordPayload>,
    #[derivative(Debug = "ignore")]
    to_gateway_tx: UnboundedSender<DiscordPayload>,
    #[derivative(Debug = "ignore")]
    udp_tx: Arc<Sender<UDPMessage>>,
}

// i gotta clean this up
impl VoiceManager {
    /// Initializes the voice gateway and UDP connection. The returned connection is fully
    /// authenticated and ready to send and receive audio.
    #[tracing::instrument]
    pub async fn new(
        user_id: impl Into<String> + std::fmt::Debug,
        voice_update_payload: VoiceUpdate,
    ) -> Result<Self, VoiceError> {
        let (to_manager_tx, mut from_gateway_rx) = unbounded_channel();
        let to_gateway_tx =
            VoiceGateway::connect(user_id.into(), voice_update_payload, to_manager_tx).await?;

        let ready_payload = match from_gateway_rx.recv().await {
            Some(ready_payload) => match ready_payload {
                DiscordPayload::Ready(p) => p,
                _ => {
                    return Err(VoiceError::UnexpectedProtocolError(
                        "ready payload was not sent first".to_owned(),
                    ))
                }
            },
            None => {
                return Err(VoiceError::UnexpectedProtocolError(
                    "websocket unexpectedly closed".to_owned(),
                ))
            }
        };

        let dest_addr: SocketAddr = format!("{}:{}", ready_payload.ip, ready_payload.port)
            .parse()
            .map_err(|e| {
                std::io::Error::new(
                    ErrorKind::InvalidData,
                    format!("discord did not provide valid udp address {}", e),
                )
            })?;

        let mode = ready_payload
            .modes
            .into_iter()
            .min()
            .expect("Modes should not be empty");

        let (mut udp, udp_tx) = VoiceUDP::connect(ready_payload.ssrc, dest_addr, mode).await?;

        let test_payload = DiscordPayload::SelectProtocol(SelectProtocol {
            protocol: "udp".to_string(),
            data: SelectProtocolData {
                address: udp.local_addr().ip().to_string(),
                port: udp.local_addr().port(),
                mode,
            },
        });
        trace!(
            "About to send select protocol thing, {:?}",
            serde_json::to_string(&test_payload).unwrap()
        );
        to_gateway_tx.send(test_payload)?;
        let sd = match from_gateway_rx.recv().await {
            Some(p) => match p {
                DiscordPayload::SessionDescription(p) => p,
                _ => {
                    return Err(VoiceError::UnexpectedProtocolError(
                        "SessionDescription was not sent after ready".to_owned(),
                    ))
                }
            },
            None => {
                return Err(VoiceError::UnexpectedProtocolError(
                    "websocket unexpectedly closed".to_owned(),
                ))
            }
        };

        *udp.cipher_mut() = Some(
            XSalsa20Poly1305::new_from_slice(&sd.secret_key)
                .expect("32 bytes should be correct key size"),
        );
        tokio::spawn(async move {
            if let Err(e) = udp.run().await {
                error!("{}", e);
            }
        });

        Ok(Self {
            ssrc: ready_payload.ssrc,
            from_gateway_rx,
            to_gateway_tx,
            udp_tx: Arc::new(udp_tx),
        })
    }

    #[tracing::instrument]
    pub async fn play_audio(&self, path: impl Into<String> + std::fmt::Debug) -> Result<()> {
        let path: String = path.into();
        self.to_gateway_tx.send(DiscordPayload::Speaking(Speaking {
            speaking: 1,
            delay: Some(0),
            user_id: None,
            ssrc: self.ssrc,
        }))?;

        info!("started playing audio");
        let weak_udp_tx = Arc::downgrade(&self.udp_tx);

        // Create a bounded channel for buffering audio packets
        let (packet_tx, mut packet_rx) = tokio::sync::mpsc::channel(32); // Buffer size of 32 packets

        // Spawn a separate task for reading from OggStream
        tokio::spawn(async move {
            let f = File::open(path).await.unwrap();
            //let mut stream = OggStream::new(f);
            let mut stream = WebmStream::new(f);

            while let Some(packet) = stream.next().await {
                if packet_tx.send(packet).await.is_err() {
                    // Channel closed, receiver dropped
                    break;
                }
            }
        });

        // Main playback loop
        tokio::spawn(async move {
            let mut interval = time::interval(time::Duration::from_millis(20));

            loop {
                interval.tick().await;

                if let Some(udp_tx) = weak_udp_tx.upgrade() {
                    match packet_rx.recv().await {
                        Some(packet) => {
                            if let Err(e) = udp_tx.send(UDPMessage::Audio(packet)).await {
                                error!("error sending audio: {}", e);
                                break;
                            }
                        }
                        None => {
                            // Channel closed, no more packets
                            _ = udp_tx.send(UDPMessage::Silence).await;
                            break;
                        }
                    }
                } else {
                    break;
                }
            }
            info!("finished playing audio");
        });

        Ok(())
    }
}
