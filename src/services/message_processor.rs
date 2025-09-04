use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::models::{QueueMessage, WebhookPayload};
use crate::utils::error::{AppError, Result};
use crate::providers::logging::StructuredLogger;
use crate::services::HttpClientTrait;

#[async_trait]
pub trait MessageProcessorTrait {
    async fn start_processing(&self, mut receiver: mpsc::Receiver<QueueMessage>) -> Result<()>;
    async fn process_single_message(&self, message: QueueMessage, request_id: &str) -> Result<()>;
}

#[derive(Clone)]
pub struct MessageProcessor {
    http_client: Arc<dyn HttpClientTrait + Send + Sync>,
}

impl MessageProcessor {
    pub fn new(http_client: Arc<dyn HttpClientTrait + Send + Sync>) -> Self {
        Self { http_client }
    }

    async fn validate_message(&self, message: &QueueMessage, request_id: &str) -> Result<()> {
        if message.client_url.is_empty() {
            return Err(AppError::payload_conversion("client_url is empty".to_string()));
        }

        if message.webhook_type.is_none() {
            StructuredLogger::log_error(
                "convert payload error: get webhook type error: no webhook_type param found",
                Some(request_id),
                Some(request_id),
            );
            return Err(AppError::webhook_type("no webhook_type param found".to_string()));
        }

        if message.changes.is_none() {
            StructuredLogger::log_error(
                "convert payload error: get webhook type error: no changes param found",
                Some(request_id),
                Some(request_id),
            );
            return Err(AppError::webhook_type("no changes param found".to_string()));
        }

        Ok(())
    }

    async fn convert_to_webhook_payload(
        &self,
        message: &QueueMessage,
        request_id: &str,
    ) -> Result<WebhookPayload> {
        let webhook_type = message.webhook_type
            .as_ref()
            .ok_or_else(|| AppError::webhook_type("webhook_type is missing".to_string()))?;

        let changes = message.changes
            .as_ref()
            .ok_or_else(|| AppError::webhook_type("changes is missing".to_string()))?;

        let webhook_payload = WebhookPayload {
            webhook_type: webhook_type.clone(),
            data: message.payload.clone(),
            changes: Some(changes.clone()),
            timestamp: chrono::Utc::now(),
        };

        StructuredLogger::log_info(
            "Converted message to webhook payload",
            Some(request_id),
            Some(request_id),
            Some(serde_json::json!({
                "webhook_type": webhook_type,
                "has_changes": changes.is_object() || changes.is_array(),
                "data_size": message.payload.to_string().len()
            })),
        );

        Ok(webhook_payload)
    }

    async fn send_webhook(&self, 
        client_url: &str, 
        payload: &WebhookPayload, 
        request_id: &str
    ) -> Result<()> {
        let payload_json = serde_json::to_value(payload)?;

        match self.http_client.send_payload(client_url, &payload_json, request_id).await {
            Ok(response) => {
                let status = response.status();
                if status.is_success() {
                    StructuredLogger::log_info(
                        "Webhook sent successfully",
                        Some(request_id),
                        Some(request_id),
                        Some(serde_json::json!({
                            "client_url": client_url,
                            "status_code": status.as_u16(),
                            "webhook_type": payload.webhook_type
                        })),
                    );
                } else {
                    let error_msg = format!("Webhook failed with status: {}", status);
                    StructuredLogger::log_error(
                        &error_msg,
                        Some(request_id),
                        Some(request_id),
                    );
                    return Err(AppError::message_processing(error_msg));
                }
            }
            Err(e) => {
                StructuredLogger::log_error(
                    &format!("Failed to send webhook: {}", e),
                    Some(request_id),
                    Some(request_id),
                );
                return Err(e);
            }
        }

        Ok(())
    }
}

#[async_trait]
impl MessageProcessorTrait for MessageProcessor {
    async fn process_single_message(&self, message: QueueMessage, request_id: &str) -> Result<()> {
        StructuredLogger::log_info(
            "Processing message",
            Some(request_id),
            Some(request_id),
            Some(serde_json::json!({
                "client_url": message.client_url,
                "webhook_type": message.webhook_type,
                "message_id": message.id
            })),
        );

        // Validate message
        if let Err(e) = self.validate_message(&message, request_id).await {
            StructuredLogger::log_error(
                &format!("Message validation failed: {}", e),
                Some(request_id),
                Some(request_id),
            );
            return Err(e);
        }

        // Convert to webhook payload
        let webhook_payload = match self.convert_to_webhook_payload(&message, request_id).await {
            Ok(payload) => payload,
            Err(e) => {
                StructuredLogger::log_error(
                    &format!("Payload conversion failed: {}", e),
                    Some(request_id),
                    Some(request_id),
                );
                return Err(e);
            }
        };

        // Send webhook
        self.send_webhook(&message.client_url, &webhook_payload, request_id).await?;

        StructuredLogger::log_info(
            "Message processed successfully",
            Some(request_id),
            Some(request_id),
            Some(serde_json::json!({
                "client_url": message.client_url,
                "webhook_type": message.webhook_type
            })),
        );

        Ok(())
    }

    async fn start_processing(&self, mut receiver: mpsc::Receiver<QueueMessage>) -> Result<()> {
        StructuredLogger::log_info(
            "Message processor started",
            None,
            None,
            None,
        );

        while let Some(message) = receiver.recv().await {
            let request_id = message.id
                .clone()
                .unwrap_or_else(|| format!("req-{}", Uuid::new_v4()));

            tokio::spawn({
                let processor = self.clone();
                let request_id = request_id.clone();
                async move {
                    if let Err(e) = processor.process_single_message(message, &request_id).await {
                        StructuredLogger::log_error(
                            &format!("Failed to process message: {}", e),
                            Some(&request_id),
                            Some(&request_id),
                        );
                    }
                }
            });
        }

        StructuredLogger::log_info(
            "Message processor stopped - receiver closed",
            None,
            None,
            None,
        );

        Ok(())
    }
}

