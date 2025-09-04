use serde_json::Value;
use std::sync::Arc;

use crate::models::QueueMessage;
use crate::services::MessageProcessorTrait;
use crate::utils::error::AppError;
use crate::providers::logging::StructuredLogger;

pub struct WebhookHandler {
    message_processor: Arc<dyn MessageProcessorTrait + Send + Sync>,
}

impl WebhookHandler {
    pub fn new(message_processor: Arc<dyn MessageProcessorTrait + Send + Sync>) -> Self {
        Self { message_processor }
    }

    pub async fn process_webhook(
        &self,
        message: QueueMessage,
        request_id: &str,
    ) -> Result<Value, AppError> {
        StructuredLogger::log_info(
            "Processing webhook request",
            Some(request_id),
            Some(request_id),
            Some(serde_json::json!({
                "client_url": message.client_url,
                "webhook_type": message.webhook_type
            })),
        );

        self.message_processor
            .process_single_message(message, request_id)
            .await?;

        Ok(serde_json::json!({
            "status": "success",
            "message": "Webhook processed successfully",
            "request_id": request_id
        }))
    }
}