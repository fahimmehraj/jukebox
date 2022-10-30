use std::{net::SocketAddr, sync::Arc};

use futures_util::StreamExt;
use tokio::sync::mpsc::unbounded_channel;
use warp::ws::Message;

use crate::{
    client::{
        payloads::{ClientPayload, Opcode},
        Client,
    },
    utils::handle_message,
};

mod filters;

#[derive(Debug)]
struct Unauthorized;

impl warp::reject::Reject for Unauthorized {}

#[derive(Debug)]
pub struct Headers {
    pub authorization: String,
    pub user_id: String,
    pub client_name: String,
}

impl Headers {
    pub fn new(authorization: String, user_id: String, client_name: String) -> Self {
        Self {
            authorization,
            user_id,
            client_name,
        }
    }

    pub fn verify(self, authorization: Arc<String>) -> Option<Self> {
        if self.authorization != *authorization {
            return None;
        }
        Some(self)
    }
}

/// More fields coming soon
pub struct Server {
    password: String,
    address: SocketAddr,
}

impl Server {
    pub fn _new(password: String, address: SocketAddr) -> Self {
        Self { password, address }
    }

    pub async fn run(self) {
        let password = Arc::new(self.password);
        let routes = filters::routes(password);
        warp::serve(routes).run(self.address).await;
    }
}

async fn handle_websocket(headers: Headers, websocket: warp::ws::WebSocket) {
    println!("Websocket connected: {:?}", headers);
    let (tx, mut rx) = websocket.split();
    let client = Arc::new(Client::new(headers, tx));

    let (player_tx, mut player_rx) = unbounded_channel::<String>();

    let weak_client = Arc::downgrade(&client);
    tokio::spawn(async move {
        while let Some(msg) = player_rx.recv().await {
            eprintln!("{:?}", msg);
            if let Some(client) = weak_client.upgrade() {
                client.send(Message::text(msg)).await;
            }
        }
    });

    while let Ok(payload) = handle_message::<_, _, _, ClientPayload>(&mut rx).await {
        match payload.op {
            Opcode::VoiceUpdate(voice_update) => {
                println!("Voice update: {:?}", voice_update);
                if let Err(e) = client.add_player(voice_update).await {
                    eprintln!("Error adding player: {}", e);
                }
            }
            _ => {
                println!("recieved");
                if let Err(e) = client.send_to_player(payload).await {
                    println!("Error sending to player: {}", e);
                }
            }
        }
    }
}
