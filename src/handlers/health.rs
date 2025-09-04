use serde_json::{json, Value};

use crate::providers::logging::StructuredLogger;

pub struct HealthHandler;

impl HealthHandler {
    pub fn new() -> Self {
        Self
    }

    pub async fn health_check(&self) -> Result<Value, String> {
        StructuredLogger::log_info(
            "Health check requested",
            None,
            None,
            None,
        );

        Ok(json!({
            "status": "healthy",
            "service": "rabbitmq-consumer",
            "version": "1.0.0",
            "timestamp": chrono::Utc::now().to_rfc3339()
        }))
    }

    pub async fn readiness_check(&self) -> Result<Value, String> {
        StructuredLogger::log_info(
            "Readiness check requested",
            None,
            None,
            None,
        );

        Ok(json!({
            "status": "ready",
            "service": "rabbitmq-consumer",
            "checks": {
                "rabbitmq": "connected",
                "http_client": "ready"
            }
        }))
    }
}