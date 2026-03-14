use std::{env, net::SocketAddr};

use anyhow::{Context, Result};

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub host: String,
    pub port: u16,
    pub database_url: String,
    pub nats_url: String,
    pub redis_url: String,
    pub s3_endpoint: String,
    pub auth_secret: String,
}

impl AppConfig {
    pub fn from_env() -> Result<Self> {
        let _ = dotenvy::dotenv();

        Ok(Self {
            host: env::var("APP_HOST").unwrap_or_else(|_| "127.0.0.1".to_string()),
            port: env::var("APP_PORT")
                .ok()
                .and_then(|value| value.parse().ok())
                .unwrap_or(3001),
            database_url: env::var("DATABASE_URL").unwrap_or_else(|_| {
                "postgres://postgres:postgres@127.0.0.1:5432/daven".to_string()
            }),
            nats_url: env::var("NATS_URL").unwrap_or_else(|_| "nats://127.0.0.1:4222".to_string()),
            redis_url: env::var("REDIS_URL")
                .unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string()),
            s3_endpoint: env::var("S3_ENDPOINT")
                .unwrap_or_else(|_| "http://127.0.0.1:9000".to_string()),
            auth_secret: env::var("AUTH_SECRET").unwrap_or_else(|_| "local-dev-secret".to_string()),
        })
    }

    pub fn socket_addr(&self) -> Result<SocketAddr> {
        format!("{}:{}", self.host, self.port)
            .parse()
            .with_context(|| "failed to parse APP_HOST/APP_PORT into a socket address")
    }
}
