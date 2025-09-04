use async_trait::async_trait;
use mockall::mock;

use crate::services::HttpClientTrait;
use crate::utils::error::Result;

mock! {
    pub HttpClient {}

    #[async_trait]
    impl HttpClientTrait for HttpClient {
        async fn send_payload(
            &self,
            client_url: &str,
            payload: &serde_json::Value,
            request_id: &str,
        ) -> Result<reqwest::Response>;
    }
}