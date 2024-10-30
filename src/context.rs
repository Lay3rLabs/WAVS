use std::sync::{Arc, Mutex};

use tokio::runtime::Runtime;

#[derive(Clone)]
pub struct AppContext {
    pub rt: Arc<Runtime>,
    pub kill_switch: Arc<KillSwitch>,
}

pub struct KillSwitch {
    // for sending kill to http server, we need it to be over an async channel
    // but we want to be able to send it from either sync or async code
    // so we use tokio::sync::oneshot which satisfies both requirements
    // and consumes self, so we need to wrap it in a Mutex<Option<>> to be able to take it out
    pub http_receiver: Mutex<Option<tokio::sync::oneshot::Receiver<()>>>,
    http_sender: Mutex<Option<tokio::sync::oneshot::Sender<()>>>,
    // for sending kill to dispatcher, we only need it to be over a sync channel
    // so we use crossbeam, which doesn't have the same restrictions as tokio
    pub dispatcher_receiver: crossbeam_channel::Receiver<()>,
    dispatcher_sender: crossbeam_channel::Sender<()>,
}

impl KillSwitch {
    pub fn new() -> Self {
        let (http_sender, http_receiver) = tokio::sync::oneshot::channel();
        let (dispatcher_sender, dispatcher_receiver) = crossbeam_channel::bounded(1);

        Self {
            http_sender: Mutex::new(Some(http_sender)),
            http_receiver: Mutex::new(Some(http_receiver)),
            dispatcher_sender,
            dispatcher_receiver,
        }
    }

    pub fn kill(&self) {
        self.http_sender
            .lock()
            .unwrap()
            .take()
            .unwrap()
            .send(())
            .unwrap();
        self.dispatcher_sender.send(()).unwrap();
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

        let kill_switch = Arc::new(KillSwitch::new());

        Self { rt, kill_switch }
    }
}

impl Default for AppContext {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for KillSwitch {
    fn default() -> Self {
        Self::new()
    }
}
