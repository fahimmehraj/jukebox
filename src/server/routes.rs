use std::{net::SocketAddr, sync::Arc};

use axum::{
    extract::{self, ConnectInfo, Query, Request, State, WebSocketUpgrade},
    http::StatusCode,
    middleware::{self, Next},
    response::Response,
    routing::{any, get},
    Extension, Router,
};
use futures_util::StreamExt;
use serde::Deserialize;
use tokio::sync::mpsc::unbounded_channel;
use tower::ServiceBuilder;
use tower_http::trace::TraceLayer;
use tracing::{error, info};

use crate::{client::{payloads::ClientPayload, Client}, utils::handle_message};

use super::Headers;

pub fn app(password: impl Into<Arc<String>>) -> Router {
    let password = password.into();

    Router::new()
        .route("/loadtracks", get(loadtracks_handler))
        .route("/decodetrack", get(decodetracks_handler))
        .route("/", get(ws_handler))
        .layer(
            ServiceBuilder::new()
                .layer(middleware::from_fn_with_state(password, with_headers))
                .layer(TraceLayer::new_for_http()),
        )
}

fn get_header<'a>(req: &'a Request, header_name: &str) -> Result<&'a str, StatusCode> {
    req.headers()
        .get(header_name)
        .and_then(|header_value| header_value.to_str().ok())
        .ok_or(StatusCode::BAD_REQUEST)
}

async fn with_headers(
    extract::State(password): State<Arc<String>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    mut req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let auth_token = get_header(&req, "Authorization")?;
    let user_id = get_header(&req, "User-Id")?;
    let client_name = get_header(&req, "Client-Name")?;

    let final_headers = Headers::new(addr, auth_token, user_id, client_name)
        .verify(&password)
        .ok_or(StatusCode::UNAUTHORIZED)?;

    req.extensions_mut().insert(final_headers);

    Ok(next.run(req).await)
}

#[derive(Deserialize)]
struct LoadTracksParams {
    identifier: String,
}

async fn loadtracks_handler(Query(_): Query<LoadTracksParams>) -> String {
    todo!();
}

#[derive(Deserialize)]
struct DecodeTrackParams {
    encoded_track: String,
}

async fn decodetracks_handler(Query(_): Query<DecodeTrackParams>) -> String {
    todo!();
}

async fn ws_handler(ws: WebSocketUpgrade, Extension(headers): Extension<Headers>) -> Response {
    ws.on_upgrade(move |socket| create_client(headers, socket))
}

#[tracing::instrument(skip(websocket))]
async fn create_client(headers: Headers, websocket: axum::extract::ws::WebSocket) {
    info!("Websocket connected!");
    let (tx, mut rx) = websocket.split();
    let client = Arc::new(Client::new(headers, tx));

    let (_player_tx, mut player_rx) = unbounded_channel::<String>();

    let weak_client = Arc::downgrade(&client);
    tokio::spawn(async move {
        while let Some(msg) = player_rx.recv().await {
            error!("{:?}", msg);
            if let Some(client) = weak_client.upgrade() {
                client.send(axum::extract::ws::Message::text(msg)).await;
            }
        }
    });

    while let Ok(payload) = handle_message::<_, _, _, ClientPayload>(&mut rx).await {
        match payload.op {
            crate::client::payloads::Opcode::VoiceUpdate(voice_update) => {
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
