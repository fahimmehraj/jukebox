use std::{
    fmt::{Debug, Display},
    io::ErrorKind,
};

use anyhow::Result;
use futures_util::{stream::SplitStream, Stream, StreamExt};
use tokio_tungstenite::tungstenite::Message as DiscordMessage;
use warp::ws::Message as ClientMessage;

pub trait Representable {
    fn represent(self) -> Result<Vec<u8>>;
}

impl Representable for ClientMessage {
    fn represent(self) -> Result<Vec<u8>> {
        if self.is_close() {
            return Err(
                anyhow::anyhow!(ErrorKind::ConnectionAborted).context(format!(
                    "{:?}",
                    self.close_frame()
                        .unwrap_or((0, "No close reason provided"))
                )),
            );
        }
        Ok(self.as_bytes().to_vec())
    }
}

impl Representable for DiscordMessage {
    fn represent(self) -> Result<Vec<u8>> {
        if self.is_close() {
            return Err(anyhow::anyhow!(ErrorKind::ConnectionAborted).context(self.into_text()?));
        }
        Ok(self.into_data())
    }
}

pub async fn handle_message<T, M, E, R>(rx: &mut SplitStream<T>) -> Result<R>
where
    T: Stream,
    SplitStream<T>: StreamExt<Item = Result<M, E>>,
    E: std::error::Error + Send + Sync + 'static,
    M: Representable + Debug,
    R: serde::de::DeserializeOwned + Debug,
{
    if let Some(msg) = rx.next().await {
        let msg = msg?;
        eprintln!("received message: {:#?}", msg);
        Ok(serde_json::from_slice(&msg.represent()?[..])?)
    } else {
        eprintln!("websocket closed?");
        return Err(anyhow::anyhow!(ErrorKind::ConnectionAborted).context("websocket closed?"));
    }
}
