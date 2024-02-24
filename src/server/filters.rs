use std::{collections::HashMap, sync::Arc};

use log::{error, info};
use warp::Filter;

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

use super::{Headers, Unauthorized};

pub fn routes(
    password: Arc<String>,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    gateway(password.clone())
        .or(loadtracks(password.clone()))
        .or(decodetrack(password.clone()))
        .or(decodetracks(password.clone()))
        .recover(|err: warp::Rejection| async move {
            if let Some(Unauthorized) = err.find() {
                Ok::<_, warp::Rejection>(warp::reply::with_status(
                    "Unauthorized",
                    warp::http::StatusCode::UNAUTHORIZED,
                ))
            } else {
                Err(err)
            }
        })
}

fn with_headers(
    password: Arc<String>,
) -> impl Filter<Extract = (Headers,), Error = warp::Rejection> + Clone {
    warp::any()
        .map(move || password.clone())
        .and(warp::header::<String>("Authorization"))
        .and(warp::header::<String>("User-Id"))
        .and(warp::header::<String>("Client-Name"))
        .and_then(
            move |password, authorization, user_id, client_name| async move {
                if let Some(headers) =
                    Headers::new(authorization, user_id, client_name).verify(password)
                {
                    Ok(headers)
                } else {
                    Err(warp::reject::custom(Unauthorized))
                }
            },
        )
}

async fn handle_websocket(headers: Headers, websocket: warp::ws::WebSocket) {
    info!("Websocket connected: {:?}", headers);
    let (tx, mut rx) = websocket.split();
    let client = Arc::new(Client::new(headers, tx));

    let (_player_tx, mut player_rx) = unbounded_channel::<String>();

    let weak_client = Arc::downgrade(&client);
    tokio::spawn(async move {
        while let Some(msg) = player_rx.recv().await {
            error!("{:?}", msg);
            if let Some(client) = weak_client.upgrade() {
                client.send(Message::text(msg)).await;
            }
        }
    });

    while let Ok(payload) = handle_message::<_, _, _, ClientPayload>(&mut rx).await {
        match payload.op {
            Opcode::VoiceUpdate(voice_update) => {
                info!("Voice update: {:?}", voice_update);
                if let Err(e) = client.add_player(voice_update).await {
                    error!("Error adding player: {}", e);
                }
            }
            _ => {
                info!("recieved");
                if let Err(e) = client.send_to_player(payload).await {
                    info!("Error sending to player: {}", e);
                }
            }
        }
    }
}

fn gateway(
    password: Arc<String>,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::get()
        .and(with_headers(password))
        .and(warp::ws())
        .map(|headers, ws: warp::ws::Ws| {
            ws.on_upgrade(move |websocket| handle_websocket(headers, websocket))
        })
}

fn loadtracks(
    password: Arc<String>,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path!("loadtracks")
        .and(with_headers(password))
        .and(warp::query::<HashMap<String, String>>())
        .map(
            |_, params: HashMap<String, String>| match params.get("identifier") {
                Some(identifier) => format!("Loading tracks with identifier {}", identifier),
                None => "No identifier provided".to_string(),
            },
        )
}

fn decodetrack(
    password: Arc<String>,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path!("decodetrack")
        .and(with_headers(password))
        .and(warp::query::<HashMap<String, String>>())
        .map(
            |_, params: HashMap<String, String>| match params.get("track") {
                Some(track) => format!("Decoding track {}", track),
                None => "No track provided".to_string(),
            },
        )
}

fn decodetracks(
    password: Arc<String>,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path!("decodetracks")
        .and(with_headers(password))
        .and(warp::body::json())
        .map(|_, tracks: Vec<String>| format!("Decoding tracks {:?}", tracks))
}
