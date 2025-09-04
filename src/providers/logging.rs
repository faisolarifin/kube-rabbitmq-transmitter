use chrono::{Local, Utc};
use serde_json::{json, Value};
use std::fs::{File, OpenOptions};
use std::io::BufWriter;
use std::sync::{Arc, RwLock, OnceLock};
use tracing::{event, Level};
use tracing_subscriber::fmt::writer::MakeWriterExt;

use crate::config::LoggerConfig;
use crate::utils::error::Result;

pub struct ConfigurableFileWriter {
    config: LoggerConfig,
}

impl ConfigurableFileWriter {
    fn new(config: LoggerConfig) -> Result<Self> {
        // Create log directory if it doesn't exist
        std::fs::create_dir_all(&config.dir)?;
        Ok(Self { config })
    }
    
}

// Implement MakeWriter trait for ConfigurableFileWriter
impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for ConfigurableFileWriter {
    type Writer = BufWriter<File>;

    fn make_writer(&'a self) -> Self::Writer {
        let today = if self.config.local_time {
            Local::now().format("%Y-%m-%d").to_string()
        } else {
            Utc::now().format("%Y-%m-%d").to_string()
        };
        
        let log_file_path = format!("{}/{}.{}.log", 
            self.config.dir.trim_end_matches('/'), 
            self.config.file_name,
            today
        );
        
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_file_path)
            .unwrap_or_else(|e| panic!("Failed to open log file {}: {}", log_file_path, e));
            
        BufWriter::new(file)
    }
}

pub struct StructuredLogger;

static LOGGER_CONFIG: OnceLock<Arc<RwLock<Option<LoggerConfig>>>> = OnceLock::new();

impl StructuredLogger {
    pub fn init(level: &str, logger_config: Option<LoggerConfig>) -> Result<()> {
        let filter = match level.to_lowercase().as_str() {
            "error" => "error",
            "warn" => "warn",
            "info" => "info",
            "debug" => "debug",
            "trace" => "trace",
            _ => "info",
        };

        // Store config globally for use in logging functions
        let config_lock = LOGGER_CONFIG.get_or_init(|| Arc::new(RwLock::new(None)));
        if let Ok(mut config_guard) = config_lock.write() {
            *config_guard = logger_config.clone();
        }

        if let Some(config) = logger_config {
            // Create custom file writer with all config options
            let file_writer = ConfigurableFileWriter::new(config)?;
            
            // Create a writer that only writes ERROR level logs to file
            let error_file_writer = file_writer.with_max_level(Level::ERROR);
            
            tracing_subscriber::fmt()
                .json()
                .with_env_filter(filter)
                .with_writer(std::io::stdout.and(error_file_writer))
                .init();
        } else {
            tracing_subscriber::fmt()
                .json()
                .with_env_filter(filter)
                .init();
        }

        Ok(())
    }

    pub fn log_error(
        error: &str,
        unique_id: Option<&str>,
        request_id: Option<&str>,
    ) {
        let use_local_time = LOGGER_CONFIG
            .get()
            .and_then(|config_lock| config_lock.read().ok())
            .and_then(|config_guard| config_guard.as_ref().map(|c| c.local_time))
            .unwrap_or(false);
        
        let timestamp = if use_local_time {
            Local::now().format("%Y-%m-%d %H:%M:%S%.3f").to_string()
        } else {
            Utc::now().format("%Y-%m-%d %H:%M:%S%.3f").to_string()
        };
        
        let unique_id = unique_id.unwrap_or("unknown");
        let request_id = request_id.unwrap_or(unique_id);

        let log_entry = json!({
            "message": {
                "error": error
            },
            "timestamp": timestamp,
            "uniqueId": unique_id,
            "x-request-id": request_id
        });

        event!(Level::ERROR, "{}", log_entry);
    }

    pub fn log_info(
        message: &str,
        unique_id: Option<&str>,
        request_id: Option<&str>,
        additional_data: Option<Value>,
    ) {
        let use_local_time = LOGGER_CONFIG
            .get()
            .and_then(|config_lock| config_lock.read().ok())
            .and_then(|config_guard| config_guard.as_ref().map(|c| c.local_time))
            .unwrap_or(false);
        
        let timestamp = if use_local_time {
            Local::now().format("%Y-%m-%d %H:%M:%S%.3f").to_string()
        } else {
            Utc::now().format("%Y-%m-%d %H:%M:%S%.3f").to_string()
        };
        
        let unique_id = unique_id.unwrap_or("unknown");
        let request_id = request_id.unwrap_or(unique_id);

        let mut log_entry = json!({
            "message": message,
            "timestamp": timestamp,
            "uniqueId": unique_id,
            "x-request-id": request_id
        });

        if let Some(data) = additional_data {
            if let Value::Object(ref mut map) = log_entry {
                if let Value::Object(data_map) = data {
                    for (key, value) in data_map {
                        map.insert(key, value);
                    }
                }
            }
        }

        event!(Level::INFO, "{}", log_entry);
    }

    pub fn log_warning(
        message: &str,
        unique_id: Option<&str>,
        request_id: Option<&str>,
    ) {
        let use_local_time = LOGGER_CONFIG
            .get()
            .and_then(|config_lock| config_lock.read().ok())
            .and_then(|config_guard| config_guard.as_ref().map(|c| c.local_time))
            .unwrap_or(false);
        
        let timestamp = if use_local_time {
            Local::now().format("%Y-%m-%d %H:%M:%S%.3f").to_string()
        } else {
            Utc::now().format("%Y-%m-%d %H:%M:%S%.3f").to_string()
        };
        
        let unique_id = unique_id.unwrap_or("unknown");
        let request_id = request_id.unwrap_or(unique_id);

        let log_entry = json!({
            "message": message,
            "timestamp": timestamp,
            "uniqueId": unique_id,
            "x-request-id": request_id
        });

        event!(Level::WARN, "{}", log_entry);
    }
}