use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::utils::error::Result;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub amqp: AmqpConfig,
    pub http: HttpConfig,
    pub auth: AuthConfig,
    pub logger: LoggerConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AmqpConfig {
    pub scheme: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub concurrent: u16,
    pub prefetch_count: u16,
    pub prefetch_size: u32,
    pub global: bool,
    pub queue_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpConfig {
    pub timeout: u64,
    pub retry_attempts: u8,
    pub retry_delay: u64,
    pub user_agent: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    pub login_endpoint: String,
    pub credentials: HashMap<String, ClientCredentials>,
    pub token_cache_duration: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientCredentials {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggerConfig {
    pub dir: String,
    pub file_name: String,
    pub max_backups: u32,
    pub max_size: u32,
    pub max_age: u32,
    pub compress: bool,
    pub local_time: bool,
}

impl AppConfig {
    pub fn load() -> Result<Self> {
        let settings = config::Config::builder()
            .add_source(config::File::with_name("config.yaml"))
            .add_source(config::Environment::with_prefix("APP"))
            .build()?;

        Ok(settings.try_deserialize()?)
    }
}