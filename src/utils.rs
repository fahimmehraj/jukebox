use std::fmt::Debug;

use bytes::Bytes;
use futures_util::{stream::SplitStream, StreamExt};
use thiserror::Error;
use tracing::error;

#[derive(Error, Debug)]
pub enum ReadMessageError {
    #[error("failed to deserialize message: {0}")]
    SerializationError(String),

    #[error("websocket stream error: {0}")]
    WebsocketStreamError(#[from] Box<dyn std::error::Error + Send + Sync>),

    #[error("websocket closed")]
    WebsocketClosed,
}

pub trait IntoRawData {
    fn as_bytes(self) -> Bytes;
}

impl IntoRawData for axum::extract::ws::Message {
    fn as_bytes(self) -> Bytes {
        self.into_data()
    }
}

impl IntoRawData for tokio_tungstenite::tungstenite::Message {
    fn as_bytes(self) -> Bytes {
        self.into_data()
    }
}

pub async fn parse_msg<
    T: IntoRawData + Debug,
    E: std::error::Error + Send + Sync + 'static,
    R: serde::de::DeserializeOwned,
>(
    msg: Option<Result<T, E>>,
) -> Result<R, ReadMessageError> {
    match msg {
        Some(Ok(msg)) => {
            let bytes = msg.as_bytes();
            let result = serde_json::from_slice(&bytes).map_err(|_| {
                ReadMessageError::SerializationError(String::from_utf8_lossy(&bytes).into_owned())
            })?;
            Ok(result)
        }
        Some(Err(e)) => Err(ReadMessageError::WebsocketStreamError(e.into())),
        None => Err(ReadMessageError::WebsocketClosed),
    }
}

pub async fn handle_message<T, M, E, R>(rx: &mut SplitStream<T>) -> Result<R, ReadMessageError>
where
    T: Unpin,
    SplitStream<T>: StreamExt<Item = Result<M, E>> + Unpin,
    E: std::error::Error + Send + Sync + 'static,
    M: IntoRawData + Debug,
    R: serde::de::DeserializeOwned + Debug,
{
    match rx.next().await {
        Some(Ok(msg)) => {
            let bytes = msg.as_bytes();
            let result = serde_json::from_slice(&bytes).map_err(|_| {
                ReadMessageError::SerializationError(String::from_utf8_lossy(&bytes).into_owned())
            })?;
            Ok(result)
        }
        Some(Err(e)) => Err(ReadMessageError::WebsocketStreamError(e.into())),
        None => Err(ReadMessageError::WebsocketClosed),
    }
}
