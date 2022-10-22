pub mod payloads;
pub mod player;

use std::collections::HashMap;

use futures_util::{stream::SplitSink, SinkExt};
use player::Player;
use tokio::sync::{RwLock, mpsc::UnboundedSender};
use warp::ws::{WebSocket, Message};

use payloads::Payload;

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
            return None
        }
        Some(self)
    }
}

pub struct Client {
    user_id: String,
    client_name: String,
    players: RwLock<HashMap<String, Player>>,
    sender: RwLock<SplitSink<WebSocket, Message>>
}

impl Client {
    pub fn new(headers: Headers, sender: SplitSink<WebSocket, Message>) -> Self {
        Self {
            user_id: headers.user_id,
            client_name: headers.client_name,
            players: RwLock::new(HashMap::new()),
            sender: RwLock::new(sender),
        }
    }

    pub fn id(&self) -> String {
        self.user_id.clone()
    }

    pub fn client_name(&self) -> String {
        self.client_name.clone()
    }

    pub async fn add_player(&self, player: Player) {
        self.players.write().await.insert(player.guild_id(), player);
    }

    pub async fn remove_player(&self, guild_id: &str) {
        self.players.write().await.remove(guild_id);
    }

    pub async fn get_player_sender(&self, guild_id: &str) -> Option<UnboundedSender<Payload>> {
        match self.players.read().await.get(guild_id) {
            Some(player) => Some(player.sender()),
            None => None,
        }
    }

    pub async fn send(&self, message: Message) {
        if let Err(e) = self.sender.write().await.send(message).await {
            println!("Error sending message: {}", e);
        }
    } 
}