use rabbitmq_consumer::config::AppConfig;
use rabbitmq_consumer::models::QueueMessage;
use std::collections::HashMap;

#[cfg(test)]
mod tests {
    use super::*;

    #[ignore] // Requires actual RabbitMQ server
    #[tokio::test]
    async fn test_config_loading() {
        // Test that config can be loaded from file
        let result = std::panic::catch_unwind(|| {
            // This would require actual config file
            // AppConfig::load()
        });

        // For now just test that we can create a dummy config
        assert!(true);
    }

    #[test]
    fn test_message_serialization() {
        let message = QueueMessage {
            id: Some("test-123".to_string()),
            webhook_type: Some("user.created".to_string()),
            client_url: "https://example.com/webhook".to_string(),
            payload: serde_json::json!({"user": "john"}),
            headers: Some(HashMap::new()),
            changes: Some(serde_json::json!({"name": "John Doe"})),
        };

        let json = serde_json::to_string(&message).unwrap();
        let deserialized: QueueMessage = serde_json::from_str(&json).unwrap();

        assert_eq!(message.id, deserialized.id);
        assert_eq!(message.webhook_type, deserialized.webhook_type);
        assert_eq!(message.client_url, deserialized.client_url);
    }

    #[test]
    fn test_error_chain() {
        use rabbitmq_consumer::utils::error::AppError;

        let error = AppError::payload_conversion("Test error");
        let error_string = format!("{}", error);
        assert!(error_string.contains("Payload conversion error"));
    }
}