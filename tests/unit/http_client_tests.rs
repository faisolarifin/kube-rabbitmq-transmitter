use std::sync::Arc;
use rabbitmq_consumer::{
    config::HttpConfig,
    services::{AuthServiceTrait, HttpClientService, HttpClientTrait},
    mocks::MockAuthService,
    utils::error::Result,
};
use mockall::predicate::*;
use wiremock::{MockServer, Mock, ResponseTemplate};
use wiremock::matchers::{method, header, body_json};

fn create_http_config() -> HttpConfig {
    HttpConfig {
        timeout: 30,
        retry_attempts: 3,
        retry_delay: 1000,
        user_agent: "rabbitmq-consumer/1.0".to_string(),
    }
}

#[tokio::test]
async fn test_successful_request() {
    let mock_server = MockServer::start().await;
    let server_url = mock_server.uri();

    Mock::given(method("POST"))
        .and(header("authorization", "Bearer test_token"))
        .and(header("x-request-id", "test-req-1"))
        .and(body_json(serde_json::json!({"test": "data"})))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status": "success"
        })))
        .mount(&mock_server)
        .await;

    let mut mock_auth = MockAuthService::new();
    let server_url_clone = server_url.clone();
    mock_auth
        .expect_get_auth_token()
        .withf(move |url, req_id| url == server_url_clone.as_str() && req_id == "test-req-1")
        .times(1)
        .returning(|_, _| Ok("test_token".to_string()));

    let config = create_http_config();
    let http_service = HttpClientService::new(config, Arc::new(mock_auth));

    let payload = serde_json::json!({"test": "data"});
    let result = http_service.send_payload(&server_url, &payload, "test-req-1").await;

    assert!(result.is_ok());
    let response = result.unwrap();
    assert_eq!(response.status(), 200);
}

#[tokio::test]
async fn test_retry_on_server_error() {
    let mock_server = MockServer::start().await;
    let server_url = mock_server.uri();

    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(500))
        .expect(3)
        .mount(&mock_server)
        .await;

    let mut mock_auth = MockAuthService::new();
    mock_auth
        .expect_get_auth_token()
        .returning(|_, _| Ok("test_token".to_string()));

    let config = create_http_config();
    let http_service = HttpClientService::new(config, Arc::new(mock_auth));

    let payload = serde_json::json!({"test": "data"});
    let result = http_service.send_payload(&server_url, &payload, "test-req-1").await;

    assert!(result.is_err());
}