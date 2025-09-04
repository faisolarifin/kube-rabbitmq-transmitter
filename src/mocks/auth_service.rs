use async_trait::async_trait;
use mockall::mock;

use crate::services::AuthServiceTrait;
use crate::utils::error::Result;

mock! {
    pub AuthService {}

    #[async_trait]
    impl AuthServiceTrait for AuthService {
        async fn get_auth_token(&self, client_url: &str, request_id: &str) -> Result<String>;
        async fn is_token_valid(&self, client_url: &str) -> bool;
    }
}