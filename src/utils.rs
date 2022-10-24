use std::fmt::{Debug, Display};

use futures_util::{stream::SplitStream, StreamExt, Stream};
use warp::ws::Message as ClientMessage;
use tokio_tungstenite::tungstenite::Message as DiscordMessage;

pub trait Representable {
    fn represent(self) -> Vec<u8>;
}

impl Representable for ClientMessage {
    fn represent(self) -> Vec<u8> {
        self.as_bytes().to_vec()
    }
}

impl Representable for DiscordMessage {
    fn represent(self) -> Vec<u8> {
        self.into_data()
    }
}

pub async fn handle_message<T, M, E, R>(rx: &mut SplitStream<T>) -> Option<R>
where
    T: Stream,
    SplitStream<T>: StreamExt<Item = Result<M, E>>,
    E: Display,
    M: Representable,
    R: serde::de::DeserializeOwned + Debug,
{
    if let Some(msg ) = rx.next().await {
        let msg = match msg {
            Ok(msg) => msg,
            Err(e) => {
                eprintln!("websocket error: {}", e);
                return None;
            }
        };
        match serde_json::from_slice(&msg.represent()[..]) {
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
