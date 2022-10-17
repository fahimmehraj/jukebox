use serde_json::Error;
use warp::ws::Message;
use std::collections::HashMap;
use std::sync::Arc;

use futures_util::{SinkExt, StreamExt};
use tokio::sync::mpsc;
use warp::Filter;

use jukebox::Payload;
use jukebox::client::Client;

type Clients = Arc<HashMap<Client, mpsc::UnboundedSender<Message>>>;

// https://github.com/freyacodes/Lavalink/blob/master/IMPLEMENTATION.md

#[tokio::main]
async fn main() {
    let clients: Clients = Clients::default();
    let clients = warp::any().map(move || clients.clone());

    let gateway = warp::get()
        .and(warp::header::<String>("Authorization"))
        .and(warp::header::<String>("User-Id"))
        .and(warp::header::<String>("Client-Name"))
        .and(clients)
        .and(warp::ws())
        .map(|authorization, user_id, client_name, clients, ws: warp::ws::Ws| {
            // This will call our function if the handshake succeeds.
            ws.on_upgrade(move |websocket| handle_websocket(websocket, clients));
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

    let routes = gateway.or(loadtracks).or(decodetrack).or(decodetracks);

    warp::serve(routes).run(([127, 0, 0, 1], 3030)).await;
}

async fn handle_websocket<'a>(websocket: warp::ws::WebSocket, clients: Clients) {
    let (mut sender, mut receiver) = websocket.split();
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
    };
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
