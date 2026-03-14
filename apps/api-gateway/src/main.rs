use anyhow::Result;
use config::AppConfig;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    let config = AppConfig::from_env()?;
    tracing::info!(
        host = %config.host,
        port = config.port,
        "api-gateway placeholder configured"
    );
    Ok(())
}
