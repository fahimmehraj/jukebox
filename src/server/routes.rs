use std::{net::SocketAddr, sync::Arc};

use axum::{
    extract::{self, ConnectInfo, Query, Request, State, WebSocketUpgrade},
    http::StatusCode,
    middleware::{self, Next},
    response::Response,
    routing::get,
    Extension, Router,
};
use serde::Deserialize;
use tower::ServiceBuilder;
use tower_http::trace::TraceLayer;
use tracing::info;

use crate::client::Client;

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

    if auth_token != *password {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let user_id = get_header(&req, "User-Id")?;
    let client_name = get_header(&req, "Client-Name")?;

    let final_headers = Headers::new(addr, user_id, client_name);

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
    ws.on_upgrade(move |socket| spawn_client_session(headers, socket))
}

#[tracing::instrument(skip(websocket))]
async fn spawn_client_session(headers: Headers, websocket: axum::extract::ws::WebSocket) {
    info!("Connection with {} established", headers.user_id);
    let mut client = Client::new(headers, websocket);
    client.listen().await;
    info!("Connection with {} closed", client.user_id());
}
