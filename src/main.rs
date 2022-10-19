use jukebox::client::player::Player;
use serde_json::Error;
use std::collections::HashMap;
use std::sync::Arc;
use warp::ws::Message;

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

async fn handle_websocket(websocket: warp::ws::WebSocket, mut client: Client) {
    let (mut sender, mut receiver) = websocket.split();

    // first payload should be voiceUpdate
    let voice_update = if let Some(msg) = receiver.next().await {
        let msg = match msg {
            Ok(msg) => msg,
            Err(e) => {
                eprintln!("Websocket error: {}", e);
                return
            },
        };
        let payload = match serde_json::from_slice::<Payload>(&msg.as_bytes()) {
            Ok(payload) => payload,
            Err(e) => {
                eprintln!("Error deserializing payload: {}", e);
                return
            },
        };
        match payload {
            Payload::VoiceUpdate(voice_update) => voice_update,
            _ => {
                eprintln!("First payload should be voiceUpdate");
                return
            },
        }
    } else {
        eprintln!("websocket closed?");
        return
    };

    let player = Player::from(voice_update);
    client.add_player(player);

    while let Some(msg) = receiver.next().await {
        let msg = match msg {
            Ok(msg) => msg,
            Err(e) => {
                eprintln!("websocket error: {}", e);
                break;
            }
        };
        let payload: Result<Payload, Error> = serde_json::from_slice(&msg.as_bytes());
        match payload {
            Ok(payload) => {
                println!("payload: {:?}", payload);
                handle_payload(payload);
                sender.send(msg).await.unwrap();
            }
            Err(e) => {
                eprintln!("Error deserializing payload: {}", e);
            }
        }
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
