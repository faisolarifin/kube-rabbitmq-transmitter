use async_trait::async_trait;
use chrono::{Duration, Utc};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::config::AuthConfig;
use crate::models::{AuthContext, AuthRequest, AuthResponse};
use crate::utils::error::{AppError, Result};
use crate::providers::logging::StructuredLogger;

#[async_trait]
pub trait AuthServiceTrait {
    async fn get_auth_token(&self, client_url: &str, request_id: &str) -> Result<String>;
    async fn is_token_valid(&self, client_url: &str) -> bool;
}

#[derive(Clone)]
pub struct AuthService {
    config: AuthConfig,
    http_client: reqwest::Client,
    token_cache: Arc<RwLock<HashMap<String, AuthContext>>>,
}

impl AuthService {
    pub fn new(config: AuthConfig, http_client: reqwest::Client) -> Self {
        Self {
            config,
            http_client,
            token_cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    async fn authenticate(&self, client_url: &str, request_id: &str) -> Result<AuthContext> {
        let credentials = self.config.credentials.get(client_url)
            .ok_or_else(|| AppError::authentication_failed(
                format!("No credentials found for client: {}", client_url)
            ))?;

        let auth_request = AuthRequest {
            username: credentials.username.clone(),
            password: credentials.password.clone(),
        };

        let login_url = format!("{}{}", client_url, self.config.login_endpoint);
        
        StructuredLogger::log_info(
            "Attempting authentication",
            Some(request_id),
            Some(request_id),
            Some(serde_json::json!({
                "client_url": client_url,
                "login_url": login_url
            })),
        );

        let response = self.http_client
            .post(&login_url)
            .json(&auth_request)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_msg = format!("Authentication failed with status: {}", response.status());
            StructuredLogger::log_error(&error_msg, Some(request_id), Some(request_id));
            return Err(AppError::authentication_failed(error_msg));
        }

        let auth_response: AuthResponse = response.json().await?;
        
        let expires_at = auth_response.expires_in.map(|expires_in| {
            Utc::now() + Duration::seconds(expires_in - 60) // 1 minute buffer
        });

        let auth_context = AuthContext {
            token: auth_response.token,
            client_url: client_url.to_string(),
            expires_at,
        };

        StructuredLogger::log_info(
            "Authentication successful",
            Some(request_id),
            Some(request_id),
            Some(serde_json::json!({
                "client_url": client_url
            })),
        );

        Ok(auth_context)
    }
}

#[async_trait]
impl AuthServiceTrait for AuthService {
    async fn get_auth_token(&self, client_url: &str, request_id: &str) -> Result<String> {
        {
            let cache = self.token_cache.read().await;
            if let Some(auth_context) = cache.get(client_url) {
                if let Some(expires_at) = auth_context.expires_at {
                    if Utc::now() < expires_at {
                        return Ok(auth_context.token.clone());
                    }
                } else {
                    let cache_duration = Duration::seconds(self.config.token_cache_duration as i64);
                    if Utc::now() < (Utc::now() + cache_duration) {
                        return Ok(auth_context.token.clone());
                    }
                }
            }
        }

        let auth_context = self.authenticate(client_url, request_id).await?;
        let token = auth_context.token.clone();

        {
            let mut cache = self.token_cache.write().await;
            cache.insert(client_url.to_string(), auth_context);
        }

        Ok(token)
    }

    async fn is_token_valid(&self, client_url: &str) -> bool {
        let cache = self.token_cache.read().await;
        if let Some(auth_context) = cache.get(client_url) {
            if let Some(expires_at) = auth_context.expires_at {
                return Utc::now() < expires_at;
            }
            return true;
        }
        false
    }
}

