use std::sync::{Arc, Mutex};

use tokio::runtime::Runtime;

use crate::config::Config;

#[derive(Clone)]
pub struct AppContext {
    pub rt: Arc<Runtime>,
    kill_sender: tokio::sync::broadcast::Sender<()>,
    // keep the first receiver alive as long as context is alive
    // subsequent receivers will subscribe from the sender
    kill_receiver: Arc<Mutex<Option<tokio::sync::broadcast::Receiver<()>>>>,
}

impl AppContext {
    pub fn new(_config: &Config) -> Self {
        let rt = Arc::new(
            tokio::runtime::Builder::new_multi_thread()
                .worker_threads(4) // TODO: make configurable?
                .enable_all()
                .build()
                .unwrap(),
        );

        let (kill_sender, kill_receiver) = tokio::sync::broadcast::channel(1);

        Self {
            rt,
            kill_sender,
            kill_receiver: Arc::new(Mutex::new(Some(kill_receiver))),
        }
    }

    pub fn get_kill_receiver(&self) -> tokio::sync::broadcast::Receiver<()> {
        match self.kill_receiver.lock().unwrap().take() {
            Some(rx) => rx,
            None => self.kill_sender.subscribe(),
        }
    }

    pub fn kill(&self) {
        self.kill_sender.send(()).unwrap();
    }
}

#[cfg(test)]
mod test {
    #[test]
    fn kill_switch_drop_fails() {
        let sender = {
            let (sender, _) = tokio::sync::broadcast::channel::<&'static str>(1);
            sender
        };

        sender.send("hello").unwrap_err();
    }

    #[test]
    fn kill_switch_hold_succeeds() {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let (sender, mut receiver) = tokio::sync::broadcast::channel::<&'static str>(1);

        sender.send("hello").unwrap();

        runtime.block_on(async move {
            let msg = receiver.recv().await;

            assert_eq!("hello", msg.unwrap());
        });
    }
}
