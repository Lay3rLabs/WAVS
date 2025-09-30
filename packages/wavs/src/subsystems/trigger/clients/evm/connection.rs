use std::{sync::Arc, time::Duration};

use futures::{
    stream::{SplitSink, SplitStream},
    SinkExt, StreamExt,
};
use tokio::{net::TcpStream, sync::oneshot};
use tokio_tungstenite::{connect_async, tungstenite::Message, MaybeTlsStream, WebSocketStream};

/// A handle for managing WebSocket connections with intelligent retry logic
///
/// This loops forever, attempting to connect to one of the provided WebSocket endpoints.
///
/// ## Retry Strategy
///
/// - **Endpoint Cycling**: When a connection fails, it cycles through all provided endpoints
///   at the current backoff level before increasing the backoff time
/// - **Backoff Timing**: Starts with a 1-second backoff, doubles after each complete cycle
///   of endpoint failures, with a maximum cap of 30 seconds
/// - **Fair Treatment**: All endpoints within the same retry cycle get the same backoff time
/// - **Reset on Success**: Both backoff time and cycle tracking reset when any connection succeeds
///
/// ## Example Behavior
///
/// With endpoints [A, B, C] and failures on all:
/// 1. Try A (fail) → sleep 1s
/// 2. Try B (fail) → sleep 1s
/// 3. Try C (fail) → sleep 1s → increase backoff to 2s for next cycle
/// 4. Try A (fail) → sleep 2s
/// 5. Try B (success) → reset backoff to 1s, reset cycle
/// -> disconnection happens
/// 6. Try C (fail) → sleep 1s
/// 7. Try A (success)
/// ... and so on
#[allow(dead_code)]
pub struct Connection {
    handle: tokio::task::JoinHandle<()>,
    // wrapped in a tokio Mutex to allow async access
    current_sink: Arc<
        tokio::sync::Mutex<Option<SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>>>,
    >,
    current_endpoint: Arc<std::sync::RwLock<Option<String>>>,
    shutdown_tx: Option<oneshot::Sender<()>>,
}

pub enum ConnectionData {
    Text(String),
    Binary(Vec<u8>),
}

impl Connection {
    pub const BACKOFF_BASE: Duration = Duration::from_secs(1);
    pub const BACKOFF_CAP: Duration = Duration::from_secs(30);

    pub fn new<F>(endpoints: Vec<String>, on_message: F) -> Self
    where
        F: Fn(ConnectionData) + Send + Sync + 'static,
    {
        let current_sink = Arc::new(tokio::sync::Mutex::new(None));
        let current_endpoint = Arc::new(std::sync::RwLock::new(None));

        let (shutdown_tx, shutdown_rx) = oneshot::channel();

        let handle = tokio::spawn(connection_loop(
            endpoints,
            current_sink.clone(),
            current_endpoint.clone(),
            shutdown_rx,
            on_message,
        ));

        Self {
            handle,
            current_sink,
            current_endpoint,
            shutdown_tx: Some(shutdown_tx),
        }
    }

    pub async fn send_message(&self, msg: Message) -> Result<(), String> {
        let mut guard = self.current_sink.lock().await;
        if let Some(sink) = guard.as_mut() {
            sink.send(msg).await.map_err(|e| e.to_string())?;
            Ok(())
        } else {
            Err("No active connection".to_string())
        }
    }

    pub fn current_endpoint(&self) -> Option<String> {
        self.current_endpoint.read().unwrap().clone()
    }
}

impl Drop for Connection {
    fn drop(&mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }

        self.handle.abort();
    }
}

async fn connection_loop<F>(
    endpoints: Vec<String>,
    current_sink: Arc<
        tokio::sync::Mutex<Option<SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>>>,
    >,
    current_endpoint: Arc<std::sync::RwLock<Option<String>>>,
    mut shutdown_rx: oneshot::Receiver<()>,
    on_message: F,
) where
    F: Fn(ConnectionData) + Send + Sync + 'static,
{
    let mut endpoint_idx = 0;
    let mut current_backoff = Connection::BACKOFF_BASE;
    let mut failures_in_cycle = 0;

    loop {
        tokio::select! {
            _ = &mut shutdown_rx => {
                tracing::info!("EVM: shutdown requested, exiting connection loop");
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

                        // Handle the connection until it disconnects
                        if let Err(err) = handle_connection(stream, &on_message).await {
                            tracing::error!("EVM connection lost from {endpoint}: {err:?}");
                        } else {
                            tracing::info!("EVM: disconnected {endpoint}");
                        }

                        *current_sink.lock().await = None;
                        *current_endpoint.write().unwrap() = None;

                        endpoint_idx += 1; // cycle to next endpoint on disconnection
                    }
                    Err(err) => {
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

async fn handle_connection<F>(
    mut stream: SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>,
    on_message: &F,
) -> Result<(), Box<dyn std::error::Error>>
where
    F: Fn(ConnectionData) + Send + Sync + 'static,
{
    // Keep the connection alive and handle messages
    while let Some(msg) = stream.next().await {
        match msg? {
            Message::Text(msg) => {
                on_message(ConnectionData::Text(msg.to_string()));
            }
            Message::Binary(msg) => {
                on_message(ConnectionData::Binary(msg.into()));
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

#[cfg(test)]
mod test {
    use crate::init_tracing_tests;

    use super::*;
    use alloy_node_bindings::Anvil;
    use futures::StreamExt;
    use tokio::time::{timeout, Duration};

    #[tokio::test]
    async fn evm_connection_works() {
        init_tracing_tests();

        let anvil = Anvil::new().spawn();

        let endpoints = vec![anvil.ws_endpoint()];

        let message_count = std::sync::Arc::new(tokio::sync::Mutex::new(0u32));
        let message_count_clone = message_count.clone();

        let connection = Connection::new(endpoints, move |_data| {
            let count = message_count_clone.clone();
            tokio::spawn(async move {
                let mut counter = count.lock().await;
                *counter += 1;
                tracing::info!("Received message, count: {}", *counter);
            });
        });

        // Wait a bit to allow connection establishment
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Send a subscription request for new block headers
        let subscription_msg = tokio_tungstenite::tungstenite::Message::Text(
            r#"{"id":1,"method":"eth_subscribe","params":["newHeads"]}"#
                .to_string()
                .into(),
        );

        let result = timeout(Duration::from_secs(5), async {
            // Send subscription message
            if let Err(e) = connection.send_message(subscription_msg).await {
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
    async fn evm_connection_skip_invalid() {
        init_tracing_tests();

        let anvil = Anvil::new().spawn();

        let endpoints = vec![
            "ws://localhost:99999".to_string(), // Will fail - invalid port
            anvil.ws_endpoint(),                // Will succeed
        ];

        let connection = Connection::new(endpoints, |_data| {
            // Message callback - not needed for this test
        });

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
    async fn evm_connection_cycles() {
        init_tracing_tests();

        let anvil_1 = Anvil::new().spawn();
        let anvil_2 = Anvil::new().port(anvil_1.port() + 1).spawn();

        let endpoints = vec![
            "ws://localhost:99999".to_string(), // Will fail - invalid port
            anvil_1.ws_endpoint(),              // Will succeed until dropped
            "ws://localhost:99999".to_string(), // Will fail - invalid port
            anvil_2.ws_endpoint(),              // Will succeed until dropped
        ];

        let anvil_1_endpoint = anvil_1.ws_endpoint();
        let anvil_2_endpoint = anvil_2.ws_endpoint();
        let anvil_1_port = anvil_1.port();

        let connection = Connection::new(endpoints, |_data| {
            // Message callback - not needed for this test
        });

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
        let anvil_2_recreated = Anvil::new().port(anvil_1_port + 1).spawn();
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
