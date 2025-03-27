pub mod payloads;
pub mod player;

use std::{collections::HashMap, sync::Arc};

use anyhow::Result;
use derivative::Derivative;
use futures_util::{
    stream::{SplitSink, SplitStream},
    SinkExt, StreamExt,
};
use player::Player;
use tokio::sync::{
    mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender},
    RwLock,
};
use tracing::{error, info};

use payloads::ClientPayload;

use crate::{
    server::Headers,
    utils::{handle_message, parse_msg, ReadMessageError},
};

use self::payloads::{Destroy, VoiceUpdate};

// Axum Websocket, not Tungstenite
type WebSocket = axum::extract::ws::WebSocket;
type Message = axum::extract::ws::Message;

#[derive(Derivative)]
#[derivative(Debug)]
pub struct Client {
    user_id: Arc<String>,
    client_name: String,
    players: HashMap<String, UnboundedSender<ClientPayload>>,

    #[derivative(Debug = "ignore")]
    from_players_rx: UnboundedReceiver<ClientPayload>,
    #[derivative(Debug = "ignore")]
    to_client_tx: UnboundedSender<ClientPayload>,

    #[derivative(Debug = "ignore")]
    ws_reader: SplitStream<WebSocket>,
    #[derivative(Debug = "ignore")]
    ws_writer: SplitSink<WebSocket, Message>,
}

impl Client {
    pub fn new(headers: Headers, ws: WebSocket) -> Self {
        let (ws_writer, ws_reader) = ws.split();
        let (to_client_tx, from_players_rx) = unbounded_channel();
        Self {
            user_id: Arc::new(headers.user_id),
            client_name: headers.client_name,
            players: HashMap::new(),
            to_client_tx,
            from_players_rx,
            ws_reader,
            ws_writer,
        }
    }

    pub fn user_id(&self) -> Arc<String> {
        self.user_id.clone()
    }

    pub fn client_name(&self) -> String {
        self.client_name.clone()
    }

    #[tracing::instrument(level = "trace")]
    pub async fn add_player(&mut self, voice_update: VoiceUpdate) -> Result<()> {
        let guild_id = voice_update.event.guild_id.clone();
        let (mut player, to_player_tx) =
            Player::new(self.user_id(), voice_update, self.to_client_tx.clone()).await?;
        tokio::spawn(async move {
            if let Err(e) = player.start().await {
                error!("Player error: {}", e);
            }
        });
        self.players.insert(guild_id, to_player_tx);
        Ok(())
    }

    pub async fn remove_player(&mut self, guild_id: &str) {
        self.players.remove(guild_id);
    }

    #[tracing::instrument(level = "debug")]
    pub async fn send_to_player(&mut self, client_payload: ClientPayload) -> Result<()> {
        match self.players.get(&client_payload.guild_id) {
            None => Err(anyhow::anyhow!(
                "no player found for guild {}",
                client_payload.guild_id
            )),
            Some(player_tx) => {
                player_tx.send(client_payload)?;
                Ok(())
            }
        }
    }

    #[tracing::instrument(level = "trace")]
    async fn send(&mut self, message: Message) {
        if let Err(e) = self.ws_writer.send(message).await {
            error!("Error sending message: {}", e);
        }
    }

    #[tracing::instrument]
    pub async fn listen(&mut self) -> () {
        loop {
            tokio::select! {
                msg = self.ws_reader.next() => {
                    let parsed_msg: Result<ClientPayload, ReadMessageError> = parse_msg(msg).await;
                    match parsed_msg {
                        Ok(payload) => self.handle_payload(payload).await,
                        Err(e)=> match e {
                            ReadMessageError::WebsocketClosed => {
                                self.destroy_players().await;
                                break;
                            },
                            ReadMessageError::SerializationError(e) => error!(e),
                            ReadMessageError::WebsocketStreamError(e) => error!(e),
                        }
                    }
                },
                msg = self.from_players_rx.recv() => match msg {
                    Some(client_payload) => self.send(client_payload.into()).await,
                    None => unreachable!("a copy of the associated tx always exists inside client"),
                },
            }
        }
    }

    async fn destroy_players(&self) {
        for (guild_id, to_player_tx) in &self.players {
            let destroy_payload = ClientPayload {
                guild_id: guild_id.clone(),
                op: payloads::Opcode::Destroy(Destroy {}),
            };
            if let Err(e) = to_player_tx.send(destroy_payload) {
                error!("{}", e);
            }
        }
    }

    async fn handle_payload(&mut self, payload: ClientPayload) {
        match payload.op {
            payloads::Opcode::VoiceUpdate(voice_update) => {
                info!("Voice update: {:?}", voice_update);
                if let Err(e) = self.add_player(voice_update).await {
                    error!("Error adding player: {}", e);
                }
            }
            _ => {
                info!("recieved: {:?}", payload);
                if let Err(e) = self.send_to_player(payload).await {
                    error!("Error sending to player: {}", e);
                }
            }
        }
    }
}
