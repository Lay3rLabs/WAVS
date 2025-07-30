use std::sync::{atomic::AtomicBool, Arc};

use tokio::runtime::Runtime;
use tracing::instrument;

#[derive(Clone)]
pub struct AppContext {
    pub rt: Arc<Runtime>,
    killed: Arc<AtomicBool>,
    kill_sender: tokio::sync::broadcast::Sender<()>,
    // just to make sure we don't send in the case of "no receivers" accidentally
    _kill_receiver: Arc<tokio::sync::broadcast::Receiver<()>>,
}

impl Default for AppContext {
    fn default() -> Self {
        Self::new()
    }
}

impl AppContext {
    pub fn new() -> Self {
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
            _kill_receiver: Arc::new(kill_receiver),
            killed: Arc::new(AtomicBool::new(false)),
        }
    }

    /// The kill system is a way to signal to all running tasks that they should stop
    /// it can be used to gracefully shutdown the system in async code
    /// without relying on its parent to drop it
    #[instrument(level = "debug", skip(self), fields(subsys = "AppContext"))]
    pub fn get_kill_receiver(&self) -> tokio::sync::broadcast::Receiver<()> {
        self.kill_sender.subscribe()
    }

    /// This is typically only called from main or tests - it will kill the system gracefully
    #[instrument(level = "debug", skip(self), fields(subsys = "AppContext"))]
    pub fn kill(&self) {
        self.killed.store(true, std::sync::atomic::Ordering::SeqCst);
        self.kill_sender.send(()).unwrap();
    }

    pub fn killed(&self) -> bool {
        self.killed.load(std::sync::atomic::Ordering::SeqCst)
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
