use std::{collections::HashMap, sync::Arc};

use warp::Filter;

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

fn gateway(
    password: Arc<String>,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::get()
        .and(with_headers(password))
        .and(warp::ws())
        .map(|headers, ws: warp::ws::Ws| {
            ws.on_upgrade(move |websocket| super::handle_websocket(headers, websocket))
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
