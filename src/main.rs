use std::collections::HashMap;
use std::sync::Arc;

use futures_util::StreamExt;
use jukebox::utils::handle_message;
use tokio::sync::mpsc::{unbounded_channel, UnboundedSender};
use tokio_tungstenite::connect_async;
use warp::ws::{Message, WebSocket};
use warp::Filter;

use jukebox::client::payloads::{ClientPayload, Opcode, VoiceUpdate};
use jukebox::client::player::Player;
use jukebox::client::{Client, Headers};

use jukebox::discord::payloads::{DiscordPayload, Identify};

const PASSWORD: &str = "youshallnotpass";

#[derive(Debug)]
struct Unauthorized;

impl warp::reject::Reject for Unauthorized {}

// https://github.com/freyacodes/Lavalink/blob/master/IMPLEMENTATION.md

#[tokio::main]
async fn main() {
    let headers = warp::any()
        .and(warp::header::<String>("Authorization"))
        .and(warp::header::<String>("User-Id"))
        .and(warp::header::<String>("Client-Name"))
        .and_then(|authorization, user_id, client_name| async move {
            if let Some(headers) =
                Headers::new(authorization, user_id, client_name).verify(PASSWORD)
            {
                Ok(headers)
            } else {
                Err(warp::reject::custom(Unauthorized))
            }
        });

    // ws://127.0.0.1/
    let gateway = warp::get()
        .and(headers)
        .and(warp::ws())
        .map(|headers, ws: warp::ws::Ws| {
            ws.on_upgrade(move |websocket| handle_websocket(websocket, headers))
        });

    // GET /loadtracks?identifier=dQw4w9WgXcQ
    let loadtracks = warp::path!("loadtracks")
        .and(warp::query::<HashMap<String, String>>())
        .map(
            |params: HashMap<String, String>| match params.get("identifier") {
                Some(identifier) => format!("Loading tracks with identifier {}", identifier),
                None => "No identifier provided".to_string(),
            },
        );

    // GET /decodetrack?track=<trackid>
    let decodetrack = warp::path!("decodetrack")
        .and(warp::query::<HashMap<String, String>>())
        .map(
            |params: HashMap<String, String>| match params.get("track") {
                Some(track) => format!("Decoding track {}", track),
                None => "No track provided".to_string(),
            },
        );

    // POST /decodetracks
    let decodetracks = warp::path!("decodetracks")
        .and(warp::body::json())
        .map(|tracks: Vec<String>| format!("Decoding tracks {:?}", tracks));

    let routes = gateway
        .or(loadtracks)
        .or(decodetrack)
        .or(decodetracks)
        .recover(|err: warp::Rejection| async move {
            if let Some(Unauthorized) = err.find() {
                Ok::<_, warp::Rejection>(warp::reply::with_status(
                    "Unauthorized",
                    warp::http::StatusCode::UNAUTHORIZED,
                ))
            } else {
                Err(err)
            }
        });

    warp::serve(routes).run(([127, 0, 0, 1], 3030)).await;
}

// first payload should be voiceUpdate
async fn handle_websocket(websocket: warp::ws::WebSocket, headers: Headers) {
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

    while let Ok(payload) =
        // jesus
        handle_message::<_, _, _, ClientPayload>(&mut rx).await
    {
        println!("{:?}", payload);
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
