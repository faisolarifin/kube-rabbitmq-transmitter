use std::sync::Arc;
use std::collections::HashMap;
use rabbitmq_consumer::{
    models::QueueMessage,
    services::{MessageProcessor, MessageProcessorTrait},
    mocks::MockHttpClient,
    utils::error::AppError,
};
use mockall::predicate::*;

fn create_test_message() -> QueueMessage {
    QueueMessage {
        id: Some("msg-123".to_string()),
        webhook_type: Some("user.created".to_string()),
        client_url: "http://example.com/webhook".to_string(),
        payload: serde_json::json!({"user": "john", "email": "john@example.com"}),
        headers: Some(HashMap::new()),
        changes: Some(serde_json::json!({"name": "John Doe", "status": "active"})),
    }
}

fn create_invalid_message() -> QueueMessage {
    QueueMessage {
        id: Some("msg-456".to_string()),
        webhook_type: None, // Missing webhook_type
        client_url: "http://example.com/webhook".to_string(),
        payload: serde_json::json!({"user": "jane"}),
        headers: Some(HashMap::new()),
        changes: None, // Missing changes
    }
}

#[tokio::test]
async fn test_successful_message_processing() {
    let mut mock_http_client = MockHttpClient::new();
    
    mock_http_client
        .expect_send_payload()
        .with(
            eq("http://example.com/webhook"),
            always(),
            eq("msg-123")
        )
        .times(1)
        .returning(|_, _, _| {
            let response_body = "";
            let mock_response = http::Response::builder()
                .status(200)
                .body(response_body)
                .unwrap();
            Ok(reqwest::Response::from(mock_response))
        });

    let processor = MessageProcessor::new(Arc::new(mock_http_client));
    let message = create_test_message();

    let result = processor.process_single_message(message, "msg-123").await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_message_validation_failure() {
    let mock_http_client = MockHttpClient::new();
    let processor = MessageProcessor::new(Arc::new(mock_http_client));
    let message = create_invalid_message();

    let result = processor.process_single_message(message, "msg-456").await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_convert_to_webhook_payload() {
    let mock_http_client = MockHttpClient::new();
    let processor = MessageProcessor::new(Arc::new(mock_http_client));
    let message = create_test_message();

    let result = processor.convert_to_webhook_payload(&message, "test-req").await;
    assert!(result.is_ok());

    let webhook_payload = result.unwrap();
    assert_eq!(webhook_payload.webhook_type, "user.created");
    assert!(webhook_payload.changes.is_some());
    assert_eq!(webhook_payload.data, serde_json::json!({"user": "john", "email": "john@example.com"}));
}

#[tokio::test]
async fn test_validate_message_missing_changes() {
    let mock_http_client = MockHttpClient::new();
    let processor = MessageProcessor::new(Arc::new(mock_http_client));
    
    let mut message = create_test_message();
    message.changes = None;

    let result = processor.validate_message(&message, "test-req").await;
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), AppError::WebhookType { .. }));
}

#[tokio::test]
async fn test_validate_message_missing_webhook_type() {
    let mock_http_client = MockHttpClient::new();
    let processor = MessageProcessor::new(Arc::new(mock_http_client));
    
    let mut message = create_test_message();
    message.webhook_type = None;

    let result = processor.validate_message(&message, "test-req").await;
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), AppError::WebhookType { .. }));
}