use std::sync::{atomic::AtomicBool, Arc};

use tokio::runtime::{Handle, Runtime};
use tracing::instrument;

#[derive(Clone)]
pub struct AppContext {
    pub rt: AnyRuntime,
    killed: Arc<AtomicBool>,
    kill_sender: tokio::sync::broadcast::Sender<()>,
    // just to make sure we don't send in the case of "no receivers" accidentally
    _kill_receiver: Arc<tokio::sync::broadcast::Receiver<()>>,
}

#[derive(Clone)]
pub enum AnyRuntime {
    Tokio(Arc<Runtime>),
    TokioHandle(Handle),
}

impl AnyRuntime {
    pub fn block_on<F: std::future::Future>(&self, fut: F) -> F::Output {
        match self {
            AnyRuntime::Tokio(rt) => rt.block_on(fut),
            AnyRuntime::TokioHandle(handle) => handle.block_on(fut),
        }
    }

    pub fn spawn<F>(&self, fut: F) -> tokio::task::JoinHandle<F::Output>
    where
        F: std::future::Future + Send + 'static,
        F::Output: Send + 'static,
    {
        match self {
            AnyRuntime::Tokio(rt) => rt.spawn(fut),
            AnyRuntime::TokioHandle(handle) => handle.spawn(fut),
        }
    }
}

impl Default for AppContext {
    fn default() -> Self {
        Self::new()
    }
}

impl AppContext {
    pub fn new() -> Self {
        Self::new_with_runtime(AnyRuntime::Tokio(Arc::new(
            tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .unwrap(),
        )))
    }
    pub fn new_with_runtime(rt: AnyRuntime) -> Self {
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
    #[instrument(skip(self), fields(subsys = "AppContext"))]
    pub fn get_kill_receiver(&self) -> tokio::sync::broadcast::Receiver<()> {
        self.kill_sender.subscribe()
    }

    /// This is typically only called from main or tests - it will kill the system gracefully
    #[instrument(skip(self), fields(subsys = "AppContext"))]
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
