use anyhow::Result;
use jukebox::config::Configuration;
use tracing::info;
use tracing_subscriber::fmt::format::FmtSpan;

#[tokio::main]
async fn main() -> Result<()> {
    // let (non_blocking, _guard) = tracing_appender::non_blocking(std::io::stdout());
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("tungstenite=off".parse()?)
                .add_directive("tokio-tungstenite=off".parse()?),
        )
        // .with_writer(non_blocking)
        .compact()
        .with_file(true)
        .with_line_number(true)
        .with_thread_ids(true)
        .with_target(false)
        .init();
    info!("Starting");
    let server_config = Configuration::parse_from_file("application.yml").await?;
    let server = server_config.compose()?;
    server.run().await?;
    Ok(())
}
