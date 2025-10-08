use std::{sync::Arc, time::Duration};

use futures::{
    stream::{SplitSink, SplitStream},
    SinkExt, StreamExt,
};
use thiserror::Error;
use tokio::{
    net::TcpStream,
    sync::{mpsc::UnboundedSender, oneshot},
};
use tokio_tungstenite::{connect_async, tungstenite::Message, MaybeTlsStream, WebSocketStream};

use super::{channels::ConnectionChannels, rpc_types::outbound::RpcRequest};

// A handle for managing WebSocket connections with intelligent retry logic
//
// This loops forever, attempting to connect to one of the provided WebSocket endpoints.
//
// ## Retry Strategy
//
// - **Endpoint Cycling**: When a connection fails, it cycles through all provided endpoints
//   at the current backoff level before increasing the backoff time
// - **Backoff Timing**: Starts with a 1-second backoff, doubles after each complete cycle
//   of endpoint failures, with a maximum cap of 30 seconds
// - **Fair Treatment**: All endpoints within the same retry cycle get the same backoff time
// - **Reset on Success**: Both backoff time and cycle tracking reset when any connection succeeds
//
// ## Example Behavior
//
// With endpoints [A, B, C] and failures on all:
// 1. Try A (fail) → sleep 1s
// 2. Try B (fail) → sleep 1s
// 3. Try C (fail) → sleep 1s → increase backoff to 2s for next cycle
// 4. Try A (fail) → sleep 2s
// 5. Try B (success) → reset backoff to 1s, reset cycle
// -> disconnection happens
// 6. Try C (fail) → sleep 1s
// 7. Try A (success)
// ... and so on
#[allow(dead_code)]
pub struct Connection {
    handles: Option<[tokio::task::JoinHandle<()>; 2]>,
    current_endpoint: Arc<std::sync::RwLock<Option<String>>>,
    shutdown_txs: Option<[oneshot::Sender<()>; 2]>,
}

pub enum ConnectionData {
    Text(String),
    Binary(Vec<u8>),
}

pub enum ConnectionState {
    Connected(String),
    Disconnected,
}

impl Connection {
    pub const BACKOFF_BASE: Duration = Duration::from_secs(1);
    pub const BACKOFF_CAP: Duration = Duration::from_secs(30);

    pub fn new(endpoints: Vec<String>, channels: ConnectionChannels) -> Self {
        let ConnectionChannels {
            connection_send_rpc_rx,
            connection_data_tx,
            connection_state_tx,
        } = channels;

        let current_sink = Arc::new(tokio::sync::Mutex::new(None));
        let current_endpoint = Arc::new(std::sync::RwLock::new(None));

        let (main_shutdown_tx, main_shutdown_rx) = oneshot::channel();
        let (message_shutdown_tx, message_shutdown_rx) = oneshot::channel();

        let main_handle = tokio::spawn(connection_loop(
            endpoints,
            current_sink.clone(),
            current_endpoint.clone(),
            main_shutdown_rx,
            connection_data_tx,
            connection_state_tx,
        ));

        let message_handle = tokio::spawn(message_loop(
            connection_send_rpc_rx,
            current_sink.clone(),
            message_shutdown_rx,
        ));

        Self {
            handles: Some([main_handle, message_handle]),
            current_endpoint,
            shutdown_txs: Some([main_shutdown_tx, message_shutdown_tx]),
        }
    }

    pub fn current_endpoint(&self) -> Option<String> {
        self.current_endpoint.read().unwrap().clone()
    }
}

impl Drop for Connection {
    fn drop(&mut self) {
        tracing::info!("EVM: connection dropped");

        if let Some(txs) = self.shutdown_txs.take() {
            for tx in txs {
                let _ = tx.send(());
            }
        }

        if let Some(handles) = self.handles.take() {
            for mut handle in handles {
                tokio::spawn(async move {
                    if tokio::time::timeout(Duration::from_millis(500), &mut handle)
                        .await
                        .is_err()
                    {
                        tracing::warn!("EVM: connection loop did not shut down in time, aborting");
                        handle.abort();
                    }
                });
            }
        }
    }
}

async fn connection_loop(
    endpoints: Vec<String>,
    current_sink: Arc<
        tokio::sync::Mutex<Option<SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>>>,
    >,
    current_endpoint: Arc<std::sync::RwLock<Option<String>>>,
    mut shutdown_rx: oneshot::Receiver<()>,
    connection_data_tx: UnboundedSender<ConnectionData>,
    connection_state_tx: UnboundedSender<ConnectionState>,
) {
    let mut endpoint_idx = 0;
    let mut current_backoff = Connection::BACKOFF_BASE;
    let mut failures_in_cycle = 0;

    loop {
        tokio::select! {
            _ = &mut shutdown_rx => {
                tracing::info!("EVM: shutdown requested, exiting connection loop");
                if let Err(e) = connection_state_tx.send(ConnectionState::Disconnected) {
                    tracing::error!("Failed to send disconnected state: {}", e);
                }
                break;
            }

            (result, endpoint) = async {
                let endpoint = endpoints[endpoint_idx % endpoints.len()].clone();
                tracing::info!("EVM: connecting to {endpoint}");
                let result = connect_async(&endpoint).await;
                (result, endpoint)
            } => {
                match result {
                    Ok((ws, _)) => {
                        tracing::info!("EVM: connected {endpoint}");
                        current_backoff = Connection::BACKOFF_BASE; // reset backoff on success
                        failures_in_cycle = 0; // reset failure count

                        let (sink, stream) = ws.split();

                        *current_sink.lock().await = Some(sink);
                        *current_endpoint.write().unwrap() = Some(endpoint.clone());

                        if let Err(e) = connection_state_tx.send(ConnectionState::Connected(endpoint.clone())) {
                            tracing::error!("Failed to send connected state: {}", e);
                        }
                        // Handle the connection until it disconnects
                        if let Err(err) = handle_connection(stream, connection_data_tx.clone()).await {
                            tracing::error!("EVM connection lost from {endpoint}: {err:?}");
                        } else {
                            tracing::info!("EVM: disconnected {endpoint}");
                        }

                        if let Err(e) = connection_state_tx.send(ConnectionState::Disconnected) {
                            tracing::error!("Failed to send disconnected state: {}", e);
                        }

                        *current_sink.lock().await = None;
                        *current_endpoint.write().unwrap() = None;

                        endpoint_idx += 1; // cycle to next endpoint on disconnection
                    }
                    Err(err) => {
                        if let Err(e) = connection_state_tx.send(ConnectionState::Disconnected) {
                            tracing::error!("Failed to send disconnected state: {}", e);
                        }
                        tracing::error!("EVM: connect error to {endpoint}: {err:?}");
                        failures_in_cycle += 1;
                        endpoint_idx += 1; // cycle the endpoints

                        // backoff before trying next endpoint
                        tokio::time::sleep(current_backoff).await;

                        // Check if we've tried all endpoints in this cycle
                        if failures_in_cycle >= endpoints.len() {
                            // Completed a full cycle without success, increase backoff
                            current_backoff = (current_backoff * 2).min(Connection::BACKOFF_CAP);
                            failures_in_cycle = 0; // reset for new cycle
                        }
                    }
                }
            }
        }
    }
}

async fn handle_connection(
    mut stream: SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>,
    connection_data_tx: UnboundedSender<ConnectionData>,
) -> Result<(), ConnectionError> {
    // Keep the connection alive and handle messages
    while let Some(msg) = stream.next().await {
        match msg.map_err(|e| ConnectionError::WebSocketError(e.to_string()))? {
            Message::Text(msg) => {
                connection_data_tx
                    .send(ConnectionData::Text(msg.to_string()))
                    .map_err(|e| ConnectionError::SendError(e.to_string()))?;
            }
            Message::Binary(msg) => {
                connection_data_tx
                    .send(ConnectionData::Binary(msg.to_vec()))
                    .map_err(|e| ConnectionError::SendError(e.to_string()))?;
            }
            // tungstenite automatically responds to pings
            Message::Ping(_) => {}
            Message::Close(_) => {
                tracing::info!("EVM: WebSocket closed gracefully");
                break;
            }
            _ => {}
        }
    }

    Ok(())
}

async fn message_loop(
    mut connection_send_rpc_rx: tokio::sync::mpsc::UnboundedReceiver<RpcRequest>,
    current_sink: Arc<
        tokio::sync::Mutex<Option<SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>>>,
    >,
    mut shutdown_rx: oneshot::Receiver<()>,
) {
    loop {
        tokio::select! {
            _ = &mut shutdown_rx => {
                tracing::info!("EVM: shutdown requested, exiting message loop");
                let mut guard = current_sink.lock().await;
                if let Some(sink) = guard.as_mut() {
                    let _ = sink.close().await;
                }
                break;
            }
            Some(msg) = connection_send_rpc_rx.recv() => {
                match serde_json::to_string(&msg) {
                    Ok(msg) => {

                        tracing::info!("EVM: sending message: {}", msg);
                        let mut guard = current_sink.lock().await;
                        if let Some(sink) = guard.as_mut() {
                            match sink.send(Message::Text(msg.into())).await {
                                Ok(_) => {},
                                Err(e) => {
                                    tracing::error!("Failed to send message: {}", e);
                                }
                            }
                        } else {
                            tracing::error!("{:#?}", ConnectionError::NoActiveConnection);
                        }
                    },
                    Err(e) => {
                        tracing::error!("Failed to serialize message: {}", e);
                    }
                }
            }
            else => break, // Exit if the channel is closed
        }
    }
}

#[derive(Debug, Error)]
pub enum ConnectionError {
    #[error("No active connection")]
    NoActiveConnection,
    #[error("Send error: {0}")]
    SendError(String),
    #[error("WebSocket error: {0}")]
    WebSocketError(String),
}

#[cfg(test)]
mod test {
    use crate::{
        init_tracing_tests, subsystems::trigger::streams::evm_stream::client::channels::Channels,
    };

    use super::*;
    use alloy_node_bindings::Anvil;

    use tokio::time::{timeout, Duration};
    use utils::test_utils::anvil::safe_spawn_anvil;

    #[tokio::test]
    async fn connection_works() {
        init_tracing_tests();

        let anvil = safe_spawn_anvil();

        let endpoints = vec![anvil.ws_endpoint()];

        let channels = Channels::new();
        let mut connection_data_rx = channels.subscription.connection_data_rx;
        let connection_send_rpc_tx = channels.subscription.connection_send_rpc_tx;

        let _connection = Connection::new(endpoints, channels.connection);

        let message_count = std::sync::Arc::new(tokio::sync::Mutex::new(0u32));
        let message_count_clone = message_count.clone();

        // Spawn task to count received messages
        tokio::spawn(async move {
            while let Some(_data) = connection_data_rx.recv().await {
                let mut counter = message_count_clone.lock().await;
                *counter += 1;
                tracing::info!("Received message, count: {}", *counter);
            }
        });

        // Wait a bit to allow connection establishment
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Send a subscription request for new block headers
        let subscription_request = RpcRequest::new_heads();

        let result = timeout(Duration::from_secs(5), async {
            // Send subscription message
            if let Err(e) = connection_send_rpc_tx.send(subscription_request) {
                tracing::error!("Failed to send subscription: {}", e);
            }

            // Wait for subscription response and potential block messages
            tokio::time::sleep(Duration::from_millis(1000)).await;
        })
        .await;

        assert!(result.is_ok(), "Test timed out after 5 seconds");

        // Verify that we actually received messages (subscription response at minimum)
        let final_count = *message_count.lock().await;
        assert!(
            final_count > 0,
            "Expected to receive at least a subscription response, but got {}",
            final_count
        );

        tracing::info!(
            "Connection test completed successfully with {} messages received",
            final_count
        );
    }

    #[tokio::test]
    async fn connection_skip_invalid() {
        init_tracing_tests();

        let anvil = safe_spawn_anvil();

        let endpoints = vec![
            "ws://localhost:99999".to_string(), // Will fail - invalid port
            anvil.ws_endpoint(),                // Will succeed
        ];

        let channels = Channels::new();
        let connection = Connection::new(endpoints, channels.connection);

        // Wait for connection to be established and current_endpoint to be set
        let result = timeout(Duration::from_secs(10), async {
            loop {
                if let Some(endpoint) = connection.current_endpoint() {
                    // Give it a moment to stabilize
                    tokio::time::sleep(Duration::from_millis(100)).await;

                    // Verify it's still the same (stable connection)
                    if connection.current_endpoint().as_ref() == Some(&endpoint) {
                        return endpoint;
                    }
                }
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        })
        .await;

        let connected_endpoint = result.expect("Timeout waiting for current_endpoint to be set");

        // Verify we're connected to one of the valid Anvil endpoints
        assert_eq!(anvil.ws_endpoint(), connected_endpoint);
    }

    #[tokio::test]
    async fn connection_cycles() {
        init_tracing_tests();

        let anvil_1 = safe_spawn_anvil();
        let anvil_2 = safe_spawn_anvil();

        let endpoints = vec![
            "ws://localhost:99999".to_string(), // Will fail - invalid port
            anvil_1.ws_endpoint(),              // Will succeed until dropped
            "ws://localhost:99999".to_string(), // Will fail - invalid port
            anvil_2.ws_endpoint(),              // Will succeed until dropped
        ];

        let anvil_1_endpoint = anvil_1.ws_endpoint();
        let anvil_2_endpoint = anvil_2.ws_endpoint();
        let anvil_1_port = anvil_1.port();
        let anvil_2_port = anvil_2.port();

        let channels = Channels::new();
        let connection = Connection::new(endpoints, channels.connection);

        // Step 1: Wait for initial connection to anvil_1
        let result = timeout(Duration::from_secs(10), async {
            loop {
                if let Some(endpoint) = connection.current_endpoint() {
                    if endpoint == anvil_1_endpoint {
                        return endpoint;
                    }
                }
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        })
        .await;
        let connected = result.expect("Should initially connect to anvil_1");
        assert_eq!(
            connected, anvil_1_endpoint,
            "Should initially connect to anvil_1"
        );
        tracing::info!("✓ Step 1: Connected to anvil_1: {}", connected);

        // Step 2: Drop anvil_1, should cycle to anvil_2
        drop(anvil_1);
        tokio::time::sleep(Duration::from_millis(100)).await; // Give time for disconnection

        let result = timeout(Duration::from_secs(10), async {
            loop {
                if let Some(endpoint) = connection.current_endpoint() {
                    if endpoint == anvil_2_endpoint {
                        return endpoint;
                    }
                }
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        })
        .await;
        let connected = result.expect("Should cycle to anvil_2 after anvil_1 drops");
        assert_eq!(connected, anvil_2_endpoint, "Should cycle to anvil_2");
        tracing::info!("✓ Step 2: Cycled to anvil_2: {}", connected);

        // Step 3: Drop anvil_2, recreate anvil_1, should cycle back to anvil_1
        drop(anvil_2);
        tokio::time::sleep(Duration::from_millis(100)).await; // Give time for disconnection

        let anvil_1_recreated = Anvil::new().port(anvil_1_port).spawn();
        let result = timeout(Duration::from_secs(10), async {
            loop {
                if let Some(endpoint) = connection.current_endpoint() {
                    if endpoint == anvil_1_endpoint {
                        return endpoint;
                    }
                }
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        })
        .await;
        let connected = result.expect("Should cycle back to anvil_1 when recreated");
        assert_eq!(
            connected, anvil_1_endpoint,
            "Should cycle back to recreated anvil_1"
        );
        tracing::info!("✓ Step 3: Cycled back to recreated anvil_1: {}", connected);

        // Step 4: Drop anvil_1 again, should cycle to anvil_2 (need to recreate it first)
        let anvil_2_recreated = Anvil::new().port(anvil_2_port).spawn();
        drop(anvil_1_recreated);
        tokio::time::sleep(Duration::from_millis(100)).await; // Give time for disconnection

        let result = timeout(Duration::from_secs(10), async {
            loop {
                if let Some(endpoint) = connection.current_endpoint() {
                    if endpoint == anvil_2_endpoint {
                        return endpoint;
                    }
                }
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        })
        .await;
        let connected = result.expect("Should cycle to anvil_2 after final anvil_1 drop");
        assert_eq!(
            connected, anvil_2_endpoint,
            "Should cycle to anvil_2 after final drop"
        );
        tracing::info!("✓ Step 4: Final cycle to anvil_2: {}", connected);

        tracing::info!("🎉 Connection cycling test completed successfully!");

        // Cleanup
        drop(anvil_2_recreated);
    }
}
