use futures_util::stream::SplitStream;
use jukebox::client::player::Player;
use serde_json::Error;
use std::collections::HashMap;
use std::sync::Arc;
use warp::ws::{Message, WebSocket};

use futures_util::{SinkExt, StreamExt};
use tokio::sync::{mpsc, RwLock};
use warp::Filter;

use jukebox::client::{Client, Headers};
use jukebox::Payload;

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
            if let Some(client) = Headers::new(authorization, user_id, client_name).build(PASSWORD)
            {
                Ok(client)
            } else {
                Err(warp::reject::custom(Unauthorized))
            }
        });

    // ws://127.0.0.1/
    let gateway = warp::get()
        .and(headers)
        .and(warp::ws())
        .map(|client, ws: warp::ws::Ws| {
            ws.on_upgrade(move |websocket| handle_websocket(websocket, client))
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
async fn handle_websocket(websocket: warp::ws::WebSocket, mut client: Client) {
    let (mut tx, mut rx) = websocket.split();

    let payload = match handle_message(&mut rx).await {
        Some(payload) => payload,
        None => return,
    };

    let voice_update = match payload {
        Payload::VoiceUpdate(voice_update) => {
            if voice_update.event.endpoint.is_none() {
                eprintln!("No endpoint provided");
                return;
            }
            voice_update
        }
        _ => {
            eprintln!("First payload should be voiceUpdate");
            return;
        }
    };

    let player = Player::from(voice_update);
    client.add_player(player);

    // handler for this player
    tokio::spawn(async move {
        while let Some(payload) = handle_message(&mut rx).await {
            println!("payload: {:?}", payload);
            tx.send(Message::text(format!("received payload: {:?}", payload))).await;
            handle_payload(payload);
        }
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
    match payload {
        Payload::VoiceUpdate(voice_update) => {
            println!("VoiceUpdate: {:?}", voice_update);
        }
        Payload::Play(play) => {
            println!("Play: {:?}", play);
        }
        Payload::Stop(stop) => {
            println!("Stop: {:?}", stop);
        }
        Payload::Pause(pause) => {
            println!("Pause: {:?}", pause);
        }
        Payload::Seek(seek) => {
            println!("Seek: {:?}", seek);
        }
        Payload::Volume(volume) => {
            println!("Volume: {:?}", volume);
        }
        Payload::Filters(filters) => {
            println!("Filters: {:?}", filters);
        }
        Payload::Destroy(destroy) => {
            println!("Destroy: {:?}", destroy);
        }
    }
}
