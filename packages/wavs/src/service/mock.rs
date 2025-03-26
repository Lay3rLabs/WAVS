use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use wavs_types::{Service, ServiceMetadataSource, Submit};

use crate::apis::service::ServiceCache;

use super::error::ServiceError;

#[derive(Clone, Default)]
pub struct MockServiceCache {
    lookup: Arc<Mutex<Vec<Service>>>,
}

impl MockServiceCache {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_service(&self, service: Service) {
        self.lookup.lock().unwrap().push(service);
    }
}

#[async_trait]
impl ServiceCache for MockServiceCache {
    async fn get(
        &self,
        source: &wavs_types::ServiceMetadataSource,
    ) -> Result<wavs_types::Service, ServiceError> {
        let (source_chain_name, source_address) = match source {
            ServiceMetadataSource::EthereumServiceManager {
                chain_name,
                contract_address,
            } => (chain_name, contract_address),
        };
        Ok(self
            .lookup
            .lock()
            .unwrap()
            .iter()
            .find(|service| {
                service
                    .workflows
                    .values()
                    .any(|workflow| match &workflow.submit {
                        Submit::EthereumContract {
                            chain_name,
                            address,
                            ..
                        } => chain_name == source_chain_name && address == source_address,
                        _ => false,
                    })
            })
            .unwrap()
            .clone())
    }
}
