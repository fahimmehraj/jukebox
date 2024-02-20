use log::info;
use anyhow::Result;
use jukebox::config::Configuration;

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();
    info!("Starting");
    let server_config = Configuration::parse_from_file("application.yml").await?;
    let server = server_config.compose()?;
    server.run().await;
    Ok(())
}
