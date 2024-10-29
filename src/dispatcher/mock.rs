use crate::{apis::dispatcher::DispatchManager, config::Config};

use super::core::DispatcherError;

pub struct MockDispatcher {
    pub config: Config,
    kill_receiver: std::sync::Mutex<Option<tokio::sync::broadcast::Receiver<()>>>,
    pub kill_sender: tokio::sync::broadcast::Sender<()>,
}

impl MockDispatcher {
    pub fn new(config: Config) -> Self {
        let (kill_sender, kill_receiver) = tokio::sync::broadcast::channel(1);

        Self {
            config,
            kill_receiver: std::sync::Mutex::new(Some(kill_receiver)),
            kill_sender,
        }
    }
}

// TODO: provide a mock implementation of the trait here for easier testing.
impl DispatchManager for MockDispatcher {
    type Error = DispatcherError;

    fn config(&self) -> &Config {
        &self.config
    }

    fn kill_receiver(&self) -> tokio::sync::broadcast::Receiver<()> {
        // first try to hand out the original receiver
        // if we've already done that, we need to subscribe to a new one
        let mut lock = self.kill_receiver.lock().unwrap();
        match lock.take() {
            Some(rx) => rx,
            None => self.kill_sender.subscribe(),
        }
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
