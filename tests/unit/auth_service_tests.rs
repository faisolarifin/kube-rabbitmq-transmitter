use std::collections::HashMap;
use rabbitmq_consumer::{
    config::{AuthConfig, ClientCredentials},
    services::{AuthService, AuthServiceTrait},
};
use wiremock::{MockServer, Mock, ResponseTemplate};
use wiremock::matchers::{method, path, body_json};

fn create_auth_config() -> AuthConfig {
    let mut credentials = HashMap::new();
    credentials.insert("http://localhost:8080".to_string(), ClientCredentials {
        username: "testuser".to_string(),
        password: "testpass".to_string(),
    });

    AuthConfig {
        login_endpoint: "/auth/login".to_string(),
        credentials,
        token_cache_duration: 3600,
    }
}

#[tokio::test]
async fn test_successful_authentication() {
    let mock_server = MockServer::start().await;
    let mut config = create_auth_config();
    let server_url = mock_server.uri();
    config.credentials.insert(server_url.clone(), ClientCredentials {
        username: "testuser".to_string(),
        password: "testpass".to_string(),
    });

    Mock::given(method("POST"))
        .and(path("/auth/login"))
        .and(body_json(serde_json::json!({
            "username": "testuser",
            "password": "testpass"
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "token": "test_token_123",
            "expires_in": 3600
        })))
        .mount(&mock_server)
        .await;

    let http_client = reqwest::Client::new();
    let auth_service = AuthService::new(config, http_client);

    let result = auth_service.get_auth_token(&server_url, "test-req-1").await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "test_token_123");
}

#[tokio::test]
async fn test_authentication_failure() {
    let mock_server = MockServer::start().await;
    let mut config = create_auth_config();
    let server_url = mock_server.uri();
    config.credentials.insert(server_url.clone(), ClientCredentials {
        username: "wronguser".to_string(),
        password: "wrongpass".to_string(),
    });

    Mock::given(method("POST"))
        .and(path("/auth/login"))
        .respond_with(ResponseTemplate::new(401))
        .mount(&mock_server)
        .await;

    let http_client = reqwest::Client::new();
    let auth_service = AuthService::new(config, http_client);

    let result = auth_service.get_auth_token(&server_url, "test-req-2").await;
    assert!(result.is_err());
}