use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::Result;
use derivative::Derivative;
use futures_util::{
    stream::{SplitSink, SplitStream},
    SinkExt, StreamExt,
};
use tokio::{
    net::TcpStream,
    sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender},
    time,
};
use tokio_tungstenite::{connect_async, tungstenite::Message, MaybeTlsStream};
use tracing::{debug, error, Instrument};

use crate::{
    client::payloads::VoiceUpdate,
    utils::{handle_message, ReadMessageError},
};

use super::{
    payloads::{self, DiscordPayload, Identify},
    VoiceError,
};

type WebSocketStream = tokio_tungstenite::WebSocketStream<MaybeTlsStream<TcpStream>>;

const WEBSOCET_VERSION: u8 = 7;

#[derive(Derivative)]
#[derivative(Debug)]
pub struct VoiceGateway {
    user_id: String,
    guild_id: String,
    endpoint: String,
    session_id: String,

    #[derivative(Debug = "ignore")]
    write: SplitSink<WebSocketStream, Message>,
    #[derivative(Debug = "ignore")]
    read: SplitStream<WebSocketStream>,
    #[derivative(Debug = "ignore")]
    from_manager_rx: UnboundedReceiver<DiscordPayload>,
    #[derivative(Debug = "ignore")]
    heartbeat_interval: Duration,
    #[derivative(Debug = "ignore")]
    to_manager_tx: UnboundedSender<DiscordPayload>,
}

type Error = super::VoiceError;

impl VoiceGateway {
    /// Connects to the voice gateway, sends identify payload, and returns a [`VoiceGateway`]
    /// as well as a [`UnboundedSender`] to send payloads to the gateway.
    #[tracing::instrument(skip(to_manager_tx))]
    pub async fn connect(
        user_id: impl Into<String> + std::fmt::Debug,
        voice_update_payload: VoiceUpdate,
        to_manager_tx: UnboundedSender<DiscordPayload>,
    ) -> Result<UnboundedSender<DiscordPayload>, Error> {
        let url = url::Url::parse(&format!(
            "wss://{}?v={}",
            voice_update_payload.event.endpoint.clone(),
            WEBSOCET_VERSION
        ))?;
        let (ws_stream, _) = connect_async(url.as_str()).await?;
        let (write, mut read) = ws_stream.split();
        let (to_gateway_tx, from_manager_rx) = unbounded_channel();

        // await hello to determine heartbeat interval
        let payload: payloads::Hello = match handle_message(&mut read).await? {
            DiscordPayload::Hello(payload) => Ok(payload),
            _ => Err(VoiceError::UnexpectedProtocolError(
                "first message was not Hello".to_owned(),
            )),
        }?;

        let mut gateway = Self {
            user_id: user_id.into(),
            guild_id: voice_update_payload.event.guild_id,
            endpoint: voice_update_payload.event.endpoint,
            session_id: voice_update_payload.session_id,

            write,
            read,
            heartbeat_interval: Duration::from_millis(payload.heartbeat_interval),
            from_manager_rx,
            to_manager_tx,
        };

        gateway.identify(voice_update_payload.event.token).await?;

        let gateway_info = format!(
            "user_id: {}, guild_id: {}, endpoint: {}, session_id: {}",
            gateway.user_id, gateway.guild_id, gateway.endpoint, gateway.session_id
        );
        tokio::spawn(
            async move {
                if let Err(e) = gateway.run().await {
                    error!("{}", e);
                }
                debug!(
                    "Gateway closed for client {} at guild {}",
                    &gateway.user_id, &gateway.guild_id
                )
            }
            .instrument(tracing::info_span!("voice_gateway", "gateway" = %gateway_info)),
        );

        Ok(to_gateway_tx)
    }

    #[tracing::instrument(level = "trace")]
    async fn send(&mut self, payload: DiscordPayload) -> Result<(), Error> {
        self.write.send(payload.into()).await?;
        Ok(())
    }

    /// Runs the event loop for the voice gateway. It responds to gateway events
    /// and messages sent from the [`super::VoiceManager`].
    #[tracing::instrument]
    pub async fn run(&mut self) -> Result<()> {
        let mut interval = time::interval(self.heartbeat_interval);

        loop {
            tokio::select! {
                biased;

                sent_payload = self.from_manager_rx.recv() => {
                    match sent_payload {
                        Some(payload) => self.send(payload).await?,
                        // VoiceManager is dropped
                        None => break,
                    }
                },

                _ = interval.tick() => {
                    self.send(Self::heartbeat()).await?;
                }

                response = handle_message(&mut self.read) => {
                    match response {
                        Ok(payload) => {
                            match payload {
                                DiscordPayload::Ready(_) => {
                                    self.to_manager_tx
                                        .send(payload)
                                        .expect("Receiver should not be dropped");
                                }
                                DiscordPayload::SessionDescription(_) => {
                                    self.to_manager_tx
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
                            match e {
                                ReadMessageError::WebsocketClosed => break,
                                ReadMessageError::SerializationError(e) => error!(e),
                                ReadMessageError::WebsocketStreamError(e) => error!(e),
                            }
                        }
                    }
                },
            }
        }
        Ok(())
    }

    async fn identify(&mut self, token: String) -> Result<(), Error> {
        let inner_payload = Identify {
            server_id: self.guild_id.clone(),
            user_id: self.user_id.clone(),
            session_id: self.session_id.clone(),
            token,
        };
        self.send(DiscordPayload::Identify(inner_payload)).await?;
        Ok(())
    }

    fn heartbeat() -> DiscordPayload {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis();
        DiscordPayload::Heartbeat(nonce)
    }
}
