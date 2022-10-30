use std::net::SocketAddr;
use std::str::FromStr;

use anyhow::Result;
use serde::Deserialize;
use thiserror::Error;
use tokio::fs;

use crate::server::Server;

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("Failed to read config file: {0}")]
    ReadError(#[from] std::io::Error),
    #[error("Failed to parse config file: {0}")]
    YamlParseError(#[from] serde_yaml::Error),
    #[error("Failed to parse server address: {0}")]
    AddressParseError(#[from] std::net::AddrParseError),
}

#[derive(Deserialize)]
pub struct Configuration {
    pub server: ServerConfiguration,
    pub media: MediaConfiguration,
}

impl Default for Configuration {
    fn default() -> Self {
        Self {
            server: ServerConfiguration::default(),
            media: MediaConfiguration::default(),
        }
    }
}

impl Configuration {
    pub async fn parse_from_file(path: &str) -> Result<Self, ConfigError> {
        let data = fs::read(path).await?;
        let config: Configuration = serde_yaml::from_slice(&data)?;
        Ok(config)
    }

    pub fn compose(self) -> Result<Server, ConfigError> {
        let addr = SocketAddr::from_str(&format!("{}:{}", self.server.address, self.server.port))?;
        Ok(Server::_new(self.media.server.password, addr))
    }
}

#[derive(Deserialize)]
pub struct ServerConfiguration {
    pub port: u16,
    pub address: String,
}

impl Default for ServerConfiguration {
    fn default() -> Self {
        Self {
            port: 2333,
            address: "0.0.0.0".to_string(),
        }
    }
}

#[derive(Deserialize)]
pub struct MediaConfiguration {
    pub server: MediaServerConfiguration,
}

impl Default for MediaConfiguration {
    fn default() -> Self {
        Self {
            server: MediaServerConfiguration::default(),
        }
    }
}

#[derive(Deserialize)]
pub struct MediaServerConfiguration {
    pub password: String,
}

impl Default for MediaServerConfiguration {
    fn default() -> Self {
        Self {
            password: "youshallnotpass".to_string(),
        }
    }
}
