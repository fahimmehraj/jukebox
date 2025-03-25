use std::fmt::Debug;

use anyhow::Result;
use axum::extract::ws::Message as ClientMessage;
use futures_util::{stream::SplitStream, Stream, StreamExt};
use tokio_tungstenite::tungstenite::Message as DiscordMessage;
use tracing::{debug, error};

pub trait Representable {
    fn represent(self) -> Result<Vec<u8>>;
}

impl Representable for ClientMessage {
    fn represent(self) -> Result<Vec<u8>> {
        if let ClientMessage::Close(e) = self {
            return Err(
                anyhow::anyhow!(std::io::ErrorKind::ConnectionAborted).context(format!(
                    "{:?}",
                    e.unwrap_or(axum::extract::ws::CloseFrame {
                        code: 0,
                        reason: "No Close reason provided".into()
                    })
                )),
            );
        }
        Ok(self.into_data().to_vec())
    }
}

impl Representable for DiscordMessage {
    fn represent(self) -> Result<Vec<u8>> {
        if self.is_close() {
            return Err(
                anyhow::anyhow!(std::io::ErrorKind::ConnectionAborted).context(self.into_text()?)
            );
        }
        Ok(self.into_data().to_vec())
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
        debug!("received message: {:#?}", msg);
        Ok(serde_json::from_slice(&msg.represent()?[..])?)
    } else {
        error!("websocket closed?");
        return Err(
            anyhow::anyhow!(std::io::ErrorKind::ConnectionAborted).context("websocket closed?")
        );
    }
}
