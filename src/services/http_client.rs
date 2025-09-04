use async_trait::async_trait;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;

use crate::config::HttpConfig;
use crate::utils::error::{AppError, Result};
use crate::providers::logging::StructuredLogger;
use crate::services::AuthServiceTrait;

#[async_trait]
pub trait HttpClientTrait {
    async fn send_payload(
        &self,
        client_url: &str,
        payload: &serde_json::Value,
        request_id: &str,
    ) -> Result<reqwest::Response>;
}

#[derive(Clone)]
pub struct HttpClientService {
    client: reqwest::Client,
    config: HttpConfig,
    auth_service: Arc<dyn AuthServiceTrait + Send + Sync>,
}

impl HttpClientService {
    pub fn new(
        config: HttpConfig,
        auth_service: Arc<dyn AuthServiceTrait + Send + Sync>,
    ) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(config.timeout))
            .user_agent(&config.user_agent)
            .build()
            .expect("Failed to create HTTP client");

        Self {
            client,
            config,
            auth_service,
        }
    }

    async fn send_with_retry(
        &self,
        client_url: &str,
        payload: &serde_json::Value,
        request_id: &str,
    ) -> Result<reqwest::Response> {
        let mut last_error = None;

        for attempt in 1..=self.config.retry_attempts {
            match self.send_request(client_url, payload, request_id).await {
                Ok(response) => {
                    if response.status().is_success() {
                        StructuredLogger::log_info(
                            "HTTP request successful",
                            Some(request_id),
                            Some(request_id),
                            Some(serde_json::json!({
                                "client_url": client_url,
                                "status": response.status().as_u16(),
                                "attempt": attempt
                            })),
                        );
                        return Ok(response);
                    } else if response.status().as_u16() >= 500 {
                        StructuredLogger::log_warning(
                            &format!("Server error, retrying (attempt {})", attempt),
                            Some(request_id),
                            Some(request_id),
                        );
                        last_error = Some(AppError::message_processing(
                            format!("Server error: {}", response.status())
                        ));
                    } else {
                        StructuredLogger::log_error(
                            &format!("Client error: {}", response.status()),
                            Some(request_id),
                            Some(request_id),
                        );
                        return Err(AppError::message_processing(
                            format!("Client error: {}", response.status())
                        ));
                    }
                }
                Err(e) => {
                    StructuredLogger::log_warning(
                        &format!("HTTP request failed (attempt {}): {}", attempt, e),
                        Some(request_id),
                        Some(request_id),
                    );
                    last_error = Some(e);
                }
            }

            if attempt < self.config.retry_attempts {
                sleep(Duration::from_millis(self.config.retry_delay)).await;
            }
        }

        Err(last_error.unwrap_or_else(|| AppError::message_processing("All retry attempts failed")))
    }

    async fn send_request(
        &self,
        client_url: &str,
        payload: &serde_json::Value,
        request_id: &str,
    ) -> Result<reqwest::Response> {
        let token = self.auth_service.get_auth_token(client_url, request_id).await?;

        let response = self
            .client
            .post(client_url)
            .bearer_auth(token)
            .json(payload)
            .header("x-request-id", request_id)
            .send()
            .await?;

        Ok(response)
    }
}

#[async_trait]
impl HttpClientTrait for HttpClientService {
    async fn send_payload(
        &self,
        client_url: &str,
        payload: &serde_json::Value,
        request_id: &str,
    ) -> Result<reqwest::Response> {
        StructuredLogger::log_info(
            "Sending HTTP request",
            Some(request_id),
            Some(request_id),
            Some(serde_json::json!({
                "client_url": client_url,
                "payload_size": payload.to_string().len()
            })),
        );

        self.send_with_retry(client_url, payload, request_id).await
    }
}

