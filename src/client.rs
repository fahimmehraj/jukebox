pub mod payloads;
pub mod player;

use std::{collections::HashMap, sync::Arc};

use futures_util::{stream::SplitSink, SinkExt};
use player::Player;
use tokio::sync::{mpsc::{UnboundedSender, unbounded_channel}, RwLock};
use warp::ws::{Message, WebSocket};
use anyhow::Result;

use payloads::ClientPayload;

use self::payloads::VoiceUpdate;

pub struct Headers {
    authorization: String,
    user_id: String,
    client_name: String,
}

impl Headers {
    pub fn new(authorization: String, user_id: String, client_name: String) -> Self {
        Self {
            authorization,
            user_id,
            client_name,
        }
    }

    pub fn verify(self, authorization: &str) -> Option<Self> {
        if self.authorization != authorization {
            return None;
        }
        Some(self)
    }
}

pub struct Client {
    user_id: Arc<String>,
    client_name: String,
    players: RwLock<HashMap<String, UnboundedSender<ClientPayload>>>,
    sender: RwLock<SplitSink<WebSocket, Message>>,
}

impl Client {
    pub fn new(headers: Headers, sender: SplitSink<WebSocket, Message>) -> Self {
        Self {
            user_id: Arc::new(headers.user_id),
            client_name: headers.client_name,
            players: RwLock::new(HashMap::new()),
            sender: RwLock::new(sender),
        }
    }

    pub fn user_id(&self) -> Arc<String> {
        self.user_id.clone()
    }

    pub fn client_name(&self) -> String {
        self.client_name.clone()
    }

    pub async fn add_player(&self, voice_update: VoiceUpdate) -> Result<()> {
        let guild_id = voice_update.event.guild_id.clone();
        let (client_tx, player_rx) = unbounded_channel();
        let (mut player, player_tx) = Player::new(self.user_id(), voice_update, client_tx).await?;
        tokio::spawn(async move {
            if let Err(e) = player.start().await {
                eprintln!("Player error: {}", e);
            }
        });
        self.players
            .write()
            .await
            .insert(guild_id, player_tx);
        Ok(())
    }

    pub async fn remove_player(&self, guild_id: &str) {
        self.players.write().await.remove(guild_id);
    }

    pub async fn send_to_player(&self, client_payload: ClientPayload) -> Result<()> {
        println!("{:?}", self.players.read().await);
        match self.players.read().await.get(&client_payload.guild_id) {
            None => Err(anyhow::anyhow!("No player found for guild {}", client_payload.guild_id)),
            Some(player_tx) => {
                player_tx.send(client_payload)?;
                Ok(())
            }
        }
    }

    pub async fn send(&self, message: Message) {
        if let Err(e) = self.sender.write().await.send(message).await {
            println!("Error sending message: {}", e);
        }
    }
}
