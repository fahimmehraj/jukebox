pub mod payloads;
pub mod player;

use std::collections::HashMap;

use anyhow::Result;
use derivative::Derivative;
use futures_util::{
    stream::{SplitSink, SplitStream},
    SinkExt, StreamExt,
};
use player::Player;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};
use tracing::{error, info};

use payloads::ClientPayload;

use crate::{
    server::Headers,
    utils::{parse_msg, ReadMessageError},
};

// Axum Websocket, not Tungstenite
type WebSocket = axum::extract::ws::WebSocket;
type Message = axum::extract::ws::Message;

#[derive(Derivative)]
#[derivative(Debug)]
pub struct Client {
    user_id: String,
    client_name: String,
    players: HashMap<String, Player>,

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
            user_id: headers.user_id,
            client_name: headers.client_name,
            players: HashMap::new(),
            to_client_tx,
            from_players_rx,
            ws_reader,
            ws_writer,
        }
    }

    pub fn user_id(&self) -> &str {
        &self.user_id
    }

    pub fn client_name(&self) -> &str {
        &self.client_name
    }

    #[tracing::instrument(level = "trace")]
    pub async fn add_player(&mut self, voice_update: payloads::VoiceUpdate) -> Result<()> {
        let guild_id = voice_update.event.guild_id.clone();
        let player = Player::new(
            &self.user_id,
            voice_update,
        )
        .await?;
        player.connection_manager.play_audio("Ghost Town.webm").await?;
        self.players.insert(guild_id, player);
        Ok(())
    }

    #[tracing::instrument(level = "debug")]
    pub async fn send_to_player(&mut self, client_payload: ClientPayload) -> Result<()> {
        if let payloads::Opcode::Destroy(_) = client_payload.op {
            self.players.remove(&client_payload.guild_id);
            return Ok(());
        }
        match self.players.get_mut(&client_payload.guild_id) {
            None => Err(anyhow::anyhow!(
                "no player found for guild {}",
                client_payload.guild_id
            )),
            Some(player) => {
                player.handle_client_payload(client_payload).await?;
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
                            ReadMessageError::WebsocketClosed => break,
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
