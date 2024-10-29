pub mod apis;
pub mod args;
pub mod config;
mod digest;
pub mod dispatcher; // where we have the high-level dispatcher
pub mod engine; // where we manage and execute wasm
pub mod http;
pub mod storage;
pub mod submission; // where we submit the results to the chain
pub mod triggers; // where we handle the trigger runtime
pub use digest::Digest;

// This section is called from both main and end-to-end tests
use dispatcher::core::CoreDispatcher;
use std::sync::Arc;

pub fn start(dispatcher: Arc<CoreDispatcher>) {
    ctrlc::set_handler({
        let dispatcher = dispatcher.clone();
        move || {
            dispatcher.kill();
        }
    })
    .unwrap();

    // start the http server in its own thread
    let server_handle = std::thread::spawn({
        let dispatcher = dispatcher.clone();
        move || {
            http::server::start(dispatcher).unwrap();
        }
    });

    let dispatcher_handle = std::thread::spawn({
        let dispatcher = dispatcher.clone();
        move || {
            dispatcher.start().unwrap();
        }
    });

    // wait for all threads to finish

    server_handle.join().unwrap();
    dispatcher_handle.join().unwrap();
}
