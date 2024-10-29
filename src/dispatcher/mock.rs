use crate::{apis::dispatcher::DispatchManager, config::Config};

use super::core::DispatcherError;

pub struct MockDispatcher {
    pub config: Config,
}

impl MockDispatcher {
    pub fn new(config: Config) -> Self {
        Self { config }
    }
}

// TODO: provide a mock implementation of the trait here for easier testing.
impl DispatchManager for MockDispatcher {
    type Error = DispatcherError;

    fn async_runtime_handle(&self) -> tokio::runtime::Handle {
        tokio::runtime::Handle::current()
    }

    fn config(&self) -> &Config {
        &self.config
    }

    fn store_component(
        &self,
        _source: crate::apis::dispatcher::WasmSource,
    ) -> Result<crate::Digest, Self::Error> {
        todo!()
    }

    fn add_service(&self, _service: crate::apis::dispatcher::Service) -> Result<(), Self::Error> {
        todo!()
    }

    fn remove_service(&self, _id: crate::apis::ID) -> Result<(), Self::Error> {
        todo!()
    }

    fn list_services(&self) -> Result<Vec<crate::apis::dispatcher::Service>, Self::Error> {
        todo!()
    }
}
