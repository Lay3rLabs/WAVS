use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};

use futures::{stream::SplitSink, SinkExt, StreamExt};
use thiserror::Error;
use tokio::{
    net::TcpStream,
    sync::{mpsc::UnboundedSender, oneshot, Notify},
};
use tokio_tungstenite::{connect_async, tungstenite::Message, MaybeTlsStream, WebSocketStream};

use super::{
    channels::ConnectionChannels,
    rpc_types::{id::RpcIds, outbound::RpcRequest},
};
use utils::evm_client::EvmEndpoint;
use utils::health::check_evm_chain_endpoint_health_query;
use wavs_types::ChainKey;

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
pub struct Connection {
    handles: Option<[tokio::task::JoinHandle<()>; 2]>,
    current_endpoint: Arc<std::sync::RwLock<Option<String>>>,
    shutdown_txs: Option<[oneshot::Sender<()>; 2]>,
    health_check_handle: Option<tokio::task::JoinHandle<()>>,
    health_shutdown_tx: Option<oneshot::Sender<()>>,
}

pub struct PriorityEndpoint {
    pub index: usize,
    pub chain_key: ChainKey,
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
    pub const PRIORITY_HEALTH_CHECK_INTERVAL: Duration = Duration::from_secs(5);

    pub fn new(
        rpc_ids: RpcIds,
        endpoints: Vec<String>,
        channels: ConnectionChannels,
        priority_endpoint: Option<PriorityEndpoint>,
    ) -> Self {
        let ConnectionChannels {
            connection_send_rpc_rx,
            connection_data_tx,
            connection_state_tx,
        } = channels;

        // Use tokio::sync::Mutex for sink because we need to hold the lock across .await points
        // when performing async WebSocket operations (sink.send, sink.close)
        let current_sink = Arc::new(tokio::sync::Mutex::new(None));
        let current_endpoint = Arc::new(std::sync::RwLock::new(None));

        let (main_shutdown_tx, main_shutdown_rx) = oneshot::channel();
        let (message_shutdown_tx, message_shutdown_rx) = oneshot::channel();
        let (health_shutdown_tx, mut health_shutdown_rx) = oneshot::channel();

        // Create shared Arc instances for health check and connection loop communication
        let force_switch_flag = Arc::new(AtomicBool::new(priority_endpoint.is_some()));
        let force_switch_notify = Arc::new(Notify::new());
        let is_using_priority = Arc::new(std::sync::RwLock::new(false));

        // Spawn health check task separately
        let health_check_handle = if let Some(priority_endpoint) = priority_endpoint.as_ref() {
            if priority_endpoint.index < endpoints.len() {
                let chain_key = priority_endpoint.chain_key.clone();
                let priority_endpoint = endpoints[priority_endpoint.index].clone();

                // Validate endpoint once at spawn time
                let evm_endpoint = match EvmEndpoint::new_ws(&priority_endpoint) {
                    Ok(endpoint) => Some(endpoint),
                    Err(err) => {
                        tracing::error!(
                            "EVM: failed to construct priority endpoint {}, health check disabled: {:?}",
                            priority_endpoint,
                            err
                        );
                        None
                    }
                };

                if let Some(evm_endpoint) = evm_endpoint {
                    let force_switch_flag_clone = force_switch_flag.clone();
                    let force_switch_notify_clone = force_switch_notify.clone();
                    let is_using_priority_clone = is_using_priority.clone();

                    Some(tokio::spawn(async move {
                        let mut interval =
                            tokio::time::interval(Connection::PRIORITY_HEALTH_CHECK_INTERVAL);

                        loop {
                            tokio::select! {
                                _ = &mut health_shutdown_rx => {
                                    tracing::info!("EVM: health check task shutdown requested");
                                    break;
                                }
                                _ = interval.tick() => {
                                    if *is_using_priority_clone.read().unwrap() {
                                        continue;
                                    }

                                    match check_evm_chain_endpoint_health_query(chain_key.clone(), evm_endpoint.clone())
                                        .await
                                    {
                                        Ok(_) => {
                                            tracing::info!(
                                                "EVM: priority endpoint {} is healthy, preparing to switch",
                                                priority_endpoint
                                            );
                                            let already_requested =
                                                force_switch_flag_clone.swap(true, Ordering::SeqCst);
                                            if !already_requested {
                                                force_switch_notify_clone.notify_waiters();
                                            }
                                        }
                                        Err(e) => {
                                            tracing::debug!(
                                                "EVM: priority endpoint {} health check failed: {:?}",
                                                priority_endpoint,
                                                e
                                            );
                                        }
                                    }
                                }
                            }
                        }
                    }))
                } else {
                    None
                }
            } else {
                tracing::warn!(
                    "EVM: priority endpoint index {} is out of bounds",
                    priority_endpoint.index
                );
                None
            }
        } else {
            None
        };

        let main_handle = tokio::spawn(connection_loop(
            endpoints,
            current_sink.clone(),
            current_endpoint.clone(),
            main_shutdown_rx,
            connection_data_tx,
            connection_state_tx,
            rpc_ids,
            priority_endpoint,
            force_switch_flag,
            force_switch_notify,
            is_using_priority,
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
            health_check_handle,
            health_shutdown_tx: Some(health_shutdown_tx),
        }
    }

    pub fn current_endpoint(&self) -> Option<String> {
        self.current_endpoint.try_read().ok()?.clone()
    }

    /// Explicitly shutdown the connection, waiting for all tasks to complete.
    /// This is the preferred way to shutdown a Connection over relying on Drop.
    pub async fn shutdown(mut self) {
        tracing::info!("EVM: explicit shutdown requested");

        // Send shutdown signal to health check task first
        if let Some(health_tx) = self.health_shutdown_tx.take() {
            let _ = health_tx.send(());
        }

        // Send shutdown signals to main tasks
        if let Some(txs) = self.shutdown_txs.take() {
            for tx in txs {
                let _ = tx.send(());
            }
        }

        // Wait for health check task to finish with timeout
        if let Some(health_handle) = self.health_check_handle.take() {
            if tokio::time::timeout(Duration::from_millis(500), health_handle)
                .await
                .is_err()
            {
                tracing::warn!("EVM: health check task did not shut down in time");
            }
        }

        // Wait for main tasks to finish with timeout
        if let Some(handles) = self.handles.take() {
            for handle in handles {
                if tokio::time::timeout(Duration::from_millis(500), handle)
                    .await
                    .is_err()
                {
                    tracing::warn!("EVM: connection task did not shut down in time");
                }
            }
        }

        tracing::info!("EVM: explicit shutdown completed");
    }
}

impl Drop for Connection {
    fn drop(&mut self) {
        tracing::info!("EVM: connection dropped");

        // Just send shutdown signals
        if let Some(health_tx) = self.health_shutdown_tx.take() {
            let _ = health_tx.send(());
        }

        if let Some(txs) = self.shutdown_txs.take() {
            for tx in txs {
                let _ = tx.send(());
            }
        }

        // Don't wait, just take the handles to drop them
        let _ = self.health_check_handle.take();
        let _ = self.handles.take();
    }
}

#[allow(clippy::too_many_arguments)]
async fn connection_loop(
    endpoints: Vec<String>,
    current_sink: Arc<
        tokio::sync::Mutex<Option<SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>>>,
    >,
    current_endpoint: Arc<std::sync::RwLock<Option<String>>>,
    mut shutdown_rx: oneshot::Receiver<()>,
    connection_data_tx: UnboundedSender<ConnectionData>,
    connection_state_tx: UnboundedSender<ConnectionState>,
    rpc_ids: RpcIds,
    priority_endpoint: Option<PriorityEndpoint>,
    force_switch_flag: Arc<AtomicBool>,
    force_switch_notify: Arc<Notify>,
    is_using_priority: Arc<std::sync::RwLock<bool>>,
) {
    let mut endpoint_idx = 0;
    let mut current_backoff = Connection::BACKOFF_BASE;
    let mut failures_in_cycle = 0;

    'connection_loop: loop {
        tokio::select! {
            _ = &mut shutdown_rx => {
                tracing::info!("EVM: shutdown requested, exiting connection loop");
                if let Err(e) = connection_state_tx.send(ConnectionState::Disconnected) {
                    tracing::error!("Failed to send disconnected state: {}", e);
                }
                break 'connection_loop;
            }

            (result, endpoint) = async {
                if let Some(priority_endpoint) = priority_endpoint.as_ref() {
                    if force_switch_flag.load(Ordering::SeqCst) {
                        let desired_index = priority_endpoint.index;
                        if endpoint_idx != desired_index {
                            tracing::info!(
                                "EVM: honoring force switch request, prioritizing endpoint index {}",
                                desired_index
                            );
                        }
                        endpoint_idx = desired_index;
                    }
                }

                let endpoint = endpoints[endpoint_idx].clone();
                tracing::info!("EVM: connecting to {endpoint}");
                let result = connect_async(&endpoint).await;
                (result, endpoint)
            } => {
                match result {
                    Ok((ws, _)) => {
                        tracing::info!("EVM: connected {endpoint}");
                        current_backoff = Connection::BACKOFF_BASE; // reset backoff on success
                        failures_in_cycle = 0; // reset failure count

                        // Check if we're connecting to priority endpoint
                        let using_priority = if let Some(priority_endpoint) = priority_endpoint.as_ref() {
                            endpoint_idx == priority_endpoint.index
                        } else {
                            false
                        };
                        *is_using_priority.write().unwrap() = using_priority;

                        if using_priority {
                            force_switch_flag.store(false, Ordering::SeqCst);
                            tracing::info!("EVM: connected to priority endpoint {}", endpoint);
                        } else {
                            tracing::info!("EVM: connected to non-priority endpoint {}", endpoint);
                        }

                        let (sink, mut stream) = ws.split();

                        {
                            let mut guard = current_sink.lock().await;
                            *guard = Some(sink);
                        }
                        *current_endpoint.write().unwrap()= Some(endpoint.clone());

                        if let Err(e) = connection_state_tx.send(ConnectionState::Connected(endpoint.clone())) {
                            tracing::error!("Failed to send connected state: {}", e);
                        }

                        let mut forced_switch = false;

                        loop {
                            tokio::select! {
                                _ = &mut shutdown_rx => {
                                    tracing::info!("EVM: shutdown requested, exiting connection loop");
                                    break 'connection_loop; // Exit the outer loop directly
                                }
                                _ = force_switch_notify.notified(), if priority_endpoint.is_some() => {
                                    if let Some(priority_endpoint) = priority_endpoint.as_ref() {
                                        if force_switch_flag.load(Ordering::SeqCst)
                                            && !*is_using_priority.read().unwrap()
                                        {
                                            tracing::info!(
                                                "EVM: force switching to priority endpoint at index {}",
                                                priority_endpoint.index
                                            );
                                            forced_switch = true;
                                            endpoint_idx = priority_endpoint.index;
                                            current_backoff = Connection::BACKOFF_BASE;
                                            failures_in_cycle = 0;
                                            break;
                                        }
                                    }
                                }
                                message = stream.next() => {
                                    match message {
                                        Some(Ok(Message::Text(msg))) => {
                                            if let Err(e) = connection_data_tx
                                                .send(ConnectionData::Text(msg.to_string()))
                                            {
                                                tracing::error!("Failed to send text message from {}: {}", endpoint, e);
                                            }
                                        }
                                        Some(Ok(Message::Binary(msg))) => {
                                            if let Err(e) = connection_data_tx
                                                .send(ConnectionData::Binary(msg.to_vec()))
                                            {
                                                tracing::error!("Failed to send binary message from {}: {}", endpoint, e);
                                            }
                                        }
                                        // tungstenite automatically responds to pings
                                        Some(Ok(Message::Ping(_))) => {
                                            tracing::debug!("EVM: Received ping from {}", endpoint);
                                        }
                                        Some(Ok(Message::Close(_))) => {
                                            tracing::info!("EVM: WebSocket closed gracefully from {}", endpoint);
                                            break;
                                        }
                                        Some(Ok(_)) => {
                                            tracing::debug!("EVM: Received unhandled message type from {}", endpoint);
                                        }
                                        Some(Err(err)) => {
                                            tracing::error!("EVM connection lost from {endpoint}: {err:?}");
                                            break;
                                        }
                                        None => {
                                            tracing::info!("EVM: disconnected {endpoint}");
                                            break;
                                        }
                                    }
                                }
                            }
                        }

                        let sink_opt = {
                            let mut guard = current_sink.lock().await;
                            guard.take()
                        };
                        if let Some(mut sink) = sink_opt {
                            if let Err(e) = sink.close().await {
                                tracing::debug!("EVM: error closing sink for {endpoint}: {}", e);
                            }
                        }

                        rpc_ids.clear_all();

                        if let Err(e) = connection_state_tx.send(ConnectionState::Disconnected) {
                            tracing::error!("Failed to send disconnected state: {}", e);
                        }

                        *current_endpoint.write().unwrap() = None;
                        *is_using_priority.write().unwrap() = false; // reset priority usage

                        if forced_switch {
                            continue;
                        } else {
                            endpoint_idx = (endpoint_idx + 1) % endpoints.len(); // cycle to next endpoint on disconnection
                        }
                    }
                    Err(err) => {
                        if let Err(e) = connection_state_tx.send(ConnectionState::Disconnected) {
                            tracing::error!("Failed to send disconnected state: {}", e);
                        }
                        tracing::error!("EVM: connect error to {endpoint}: {err:?}");

                        // Clear force_switch_flag if we failed to connect to priority endpoint
                        if let Some(priority_endpoint) = priority_endpoint.as_ref() {
                            if endpoint_idx == priority_endpoint.index {
                                force_switch_flag.store(false, Ordering::SeqCst);
                                tracing::info!("EVM: clearing force switch flag due to priority endpoint connection failure");
                                *is_using_priority.write().unwrap() = false;
                            }
                        }

                        failures_in_cycle += 1;
                        endpoint_idx = (endpoint_idx + 1) % endpoints.len(); // cycle the endpoints

                        rpc_ids.clear_all(); // clear pending requests on failure

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
                let sink_opt = {
                    let mut guard = current_sink.lock().await;
                    guard.take()
                };
                if let Some(mut sink) = sink_opt {
                    let _ = sink.close().await;
                }
                break;
            }
            Some(msg) = connection_send_rpc_rx.recv() => {
                match serde_json::to_string(&msg) {
                    Ok(msg) => {

                        tracing::debug!("EVM: sending message: {}", msg);
                        let send_result = {
                            let mut guard = current_sink.lock().await;
                            if let Some(sink) = guard.as_mut() {
                                Some(sink.send(Message::Text(msg.into())).await)
                            } else {
                                tracing::error!("{:#?}", ConnectionError::NoActiveConnection);
                                None
                            }
                        };

                        if let Some(result) = send_result {
                            match result {
                                Ok(_) => {},
                                Err(e) => {
                                    tracing::error!("Failed to send message: {}", e);
                                }
                            }
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

        let rpc_ids = RpcIds::new();
        let _connection = Connection::new(rpc_ids.clone(), endpoints, channels.connection, None);

        let message_count = std::sync::Arc::new(std::sync::Mutex::new(0u32));
        let message_count_clone = message_count.clone();

        // Spawn task to count received messages
        tokio::spawn(async move {
            while let Some(_data) = connection_data_rx.recv().await {
                let mut counter = message_count_clone.lock().unwrap();
                *counter += 1;
                tracing::info!("Received message, count: {}", *counter);
            }
        });

        // Wait a bit to allow connection establishment
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Send a subscription request for new block headers
        let subscription_request = RpcRequest::new_heads(&rpc_ids);

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
        let final_count = *message_count.lock().unwrap();
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
        let rpc_ids = RpcIds::new();
        let connection = Connection::new(rpc_ids, endpoints, channels.connection, None);

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
        let rpc_ids = RpcIds::new();
        let connection = Connection::new(rpc_ids, endpoints, channels.connection, None);

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

    #[tokio::test]
    async fn priority_endpoint_basic_logic() {
        init_tracing_tests();

        let anvil_1 = safe_spawn_anvil();
        let anvil_2 = safe_spawn_anvil();

        let endpoints = vec![anvil_1.ws_endpoint(), anvil_2.ws_endpoint()];

        let channels = Channels::new();
        let rpc_ids = RpcIds::new();

        // Set anvil_2 (index 1) as priority endpoint
        let priority_endpoint = PriorityEndpoint {
            index: 1,
            chain_key: wavs_types::ChainKey::new("evm:31337").expect("Invalid chain key format"),
        };

        let connection = Connection::new(
            rpc_ids,
            endpoints,
            channels.connection,
            Some(priority_endpoint),
        );

        // Step 1: Should connect to priority endpoint (anvil_2) first
        let result = timeout(Duration::from_secs(10), async {
            loop {
                if let Some(endpoint) = connection.current_endpoint() {
                    if endpoint == anvil_2.ws_endpoint() {
                        tokio::time::sleep(Duration::from_millis(100)).await;
                        if connection.current_endpoint().as_ref() == Some(&endpoint) {
                            return endpoint;
                        }
                    }
                }
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        })
        .await;

        let initial_connection = result.expect("Should connect to priority endpoint first");
        assert_eq!(anvil_2.ws_endpoint(), initial_connection);
        tracing::info!("✓ Connected to priority endpoint (anvil_2) first");

        // Step 2: Drop priority endpoint to simulate connection loss
        let anvil_2_port = anvil_2.port();
        drop(anvil_2);
        tokio::time::sleep(Duration::from_millis(200)).await;

        // Should fallback to anvil_1
        let result = timeout(Duration::from_secs(10), async {
            loop {
                if let Some(endpoint) = connection.current_endpoint() {
                    if endpoint == anvil_1.ws_endpoint() {
                        tokio::time::sleep(Duration::from_millis(100)).await;
                        if connection.current_endpoint().as_ref() == Some(&endpoint) {
                            return endpoint;
                        }
                    }
                }
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        })
        .await;

        let fallback_connection = result.expect("Should fallback to non-priority endpoint");
        assert_eq!(anvil_1.ws_endpoint(), fallback_connection);
        tracing::info!(
            "✓ Fell back to non-priority endpoint: {}",
            fallback_connection
        );

        // Step 3: Recreate priority endpoint and verify reconnection
        let anvil_2_recreated = Anvil::new().port(anvil_2_port).spawn();
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Wait for health check to detect the priority endpoint and force reconnection
        let result = timeout(Duration::from_secs(15), async {
            loop {
                if let Some(endpoint) = connection.current_endpoint() {
                    if endpoint == anvil_2_recreated.ws_endpoint() {
                        tokio::time::sleep(Duration::from_millis(200)).await;
                        if connection.current_endpoint().as_ref() == Some(&endpoint) {
                            return endpoint;
                        }
                    }
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        })
        .await;

        let reconnected_endpoint = result.expect("Should reconnect to priority endpoint");
        assert_eq!(anvil_2_recreated.ws_endpoint(), reconnected_endpoint);
        tracing::info!(
            "✓ Successfully reconnected to priority endpoint: {}",
            reconnected_endpoint
        );

        drop(anvil_2_recreated);
    }
}
