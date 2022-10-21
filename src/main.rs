use futures_util::stream::{SplitSink, SplitStream};
use jukebox::client::player::Player;
use serde_json::Error;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::mpsc::unbounded_channel;
use warp::ws::{Message, WebSocket};

use futures_util::{SinkExt, StreamExt};
use tokio::sync::{mpsc, RwLock};
use warp::Filter;

use jukebox::client::{Client, Headers};
use jukebox::{Opcode, Payload, VoiceUpdate};

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

    while let Some(payload) = handle_message(&mut rx).await {
        match payload.op {
            Opcode::VoiceUpdate(voice_update) => create_player(client.clone(), voice_update).await,
            _ => {
                println!("recieved");
                match client.get_player_sender(&payload.guild_id).await {
                    // receiver will never be dropped so long as player is alive
                    Some(sender) => sender.send(payload).unwrap(),
                    None => {
                        client
                            .send(Message::text("No player associated with this guild_id"))
                            .await
                    }
                }
            }
        }
    }
}

async fn create_player(client: Arc<Client>, voice_update: VoiceUpdate) {
    let (tx, mut rx) = unbounded_channel();
    let player = match Player::new(voice_update, tx) {
        Ok(player) => player,
        Err(e) => {
            eprintln!("{}", e);
            return;
        }
    };
    client.add_player(player).await;

    let weak_client = Arc::downgrade(&client);

    // handler for this player
    tokio::spawn(async move {
        while let Some(payload) = rx.recv().await {
            println!("payload: {:?}", payload);
            if let Some(client) = weak_client.upgrade() {
                client
                    .send(Message::text(format!("received payload: {:?}", payload)))
                    .await;
                handle_payload(payload);
            } else {
                // this code is never reached which is ok
                println!("client dropped");
                break;
            }
        }
        println!("end reached")
    });
}

async fn handle_message(rx: &mut SplitStream<WebSocket>) -> Option<Payload> {
    if let Some(msg) = rx.next().await {
        let msg = match msg {
            Ok(msg) => msg,
            Err(e) => {
                eprintln!("websocket error: {}", e);
                return None;
            }
        };
        let payload: Result<Payload, Error> = serde_json::from_slice(&msg.as_bytes());
        match payload {
            Ok(payload) => {
                println!("payload: {:?}", payload);
                Some(payload)
            }
            Err(e) => {
                eprintln!("Error deserializing payload: {}", e);
                return None;
            }
        }
    } else {
        eprintln!("websocket closed?");
        return None;
    }
}

fn handle_payload(payload: Payload) {
    match payload.op {
        Opcode::VoiceUpdate(voice_update) => {
            println!("VoiceUpdate: {:?}", voice_update);
        }
        Opcode::Play(play) => {
            println!("Play: {:?}", play);
        }
        Opcode::Stop(stop) => {
            println!("Stop: {:?}", stop);
        }
        Opcode::Pause(pause) => {
            println!("Pause: {:?}", pause);
        }
        Opcode::Seek(seek) => {
            println!("Seek: {:?}", seek);
        }
        Opcode::Volume(volume) => {
            println!("Volume: {:?}", volume);
        }
        Opcode::Filters(filters) => {
            println!("Filters: {:?}", filters);
        }
        Opcode::Destroy(destroy) => {
            println!("Destroy: {:?}", destroy);
        }
    }
}
