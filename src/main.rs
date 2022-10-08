use std::collections::HashMap;

use futures_util::{FutureExt, StreamExt};
use warp::Filter;

use jukebox::Payload;

/*
https://github.com/freyacodes/Lavalink/blob/master/IMPLEMENTATION.md

TODO: Websocket serve
Connection open: Accepts password, User-Id, Client-Name
Receive these opcodes: voiceUpdate, play, stop, pause, seek, volume, filters, destroy, configureResuming
Send these opcodes: playerUpdate, stats?, event

TODO: Host the following endpoints that require authorization header:
GET /loadtracks?identifier=dQw4w9WgXcQ
GET /decodetrack?track=trackid
POST /decodetracks
    body: [trackids]

TODO: implement ip rotation
*/
#[tokio::main]
async fn main() {
    let gateway = warp::get().and(warp::ws()).map(|ws: warp::ws::Ws| {
        // This will call our function if the handshake succeeds.
        ws.on_upgrade(|websocket| {
            let (tx, rx) = websocket.split();
            rx.forward(tx).map(|result| {
                if let Err(e) = result {
                    eprintln!("websocket error: {}", e);
                }
            })
        })
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
