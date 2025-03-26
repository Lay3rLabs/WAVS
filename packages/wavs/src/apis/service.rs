use async_trait::async_trait;
use wavs_types::{Service, ServiceMetadataSource};

use crate::service::error::ServiceError;

#[async_trait]
pub trait ServiceCache: Send + Sync {
    async fn get(&self, source: &ServiceMetadataSource) -> Result<Service, ServiceError>;
}
