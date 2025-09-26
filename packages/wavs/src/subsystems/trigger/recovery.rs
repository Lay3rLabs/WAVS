use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use wavs_types::ChainKey;

#[derive(Debug, Clone, Default)]
pub struct ChainRecoveryState {
    pub last_processed_block: Option<u64>,
    pub last_error_time: Option<std::time::Instant>,
    pub is_in_recovery: bool,
    pub recovery_block: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct RecoveryManager {
    states: Arc<RwLock<HashMap<ChainKey, ChainRecoveryState>>>,
    max_recovery_delay: std::time::Duration,
}

impl RecoveryManager {
    pub fn new(max_recovery_delay: std::time::Duration) -> Self {
        Self {
            states: Arc::new(RwLock::new(HashMap::new())),
            max_recovery_delay,
        }
    }

    /// Record that a block was successfully processed
    pub async fn record_successful_block(&self, chain: &ChainKey, block_number: u64) {
        let mut states = self.states.write().await;
        let state = states.entry(chain.clone()).or_default();
        state.last_processed_block = Some(block_number);
        state.last_error_time = None;
        state.is_in_recovery = false;
        state.recovery_block = None;
    }

    /// Record that an error occurred while processing a stream
    pub async fn record_stream_error(&self, chain: &ChainKey) {
        let mut states = self.states.write().await;
        let state = states.entry(chain.clone()).or_default();
        state.last_error_time = Some(std::time::Instant::now());

        // Check if we need to start recovery mode
        if let Some(last_processed) = state.last_processed_block {
            let should_start_recovery = state.last_error_time.map_or(false, |error_time| {
                error_time.elapsed() > self.max_recovery_delay
            });

            if should_start_recovery && !state.is_in_recovery {
                state.is_in_recovery = true;
                state.recovery_block = Some(last_processed + 1);
            }
        }
    }

    /// Check if a chain needs recovery
    pub async fn needs_recovery(&self, chain: &ChainKey) -> Option<u64> {
        let states = self.states.read().await;
        if let Some(state) = states.get(chain) {
            if state.is_in_recovery {
                return state.recovery_block;
            }
        }
        None
    }

    /// Start recovery mode from a specific block
    pub async fn start_recovery(&self, chain: &ChainKey, from_block: u64) -> bool {
        let mut states = self.states.write().await;
        if let Some(state) = states.get_mut(chain) {
            state.is_in_recovery = true;
            state.recovery_block = Some(from_block);
            return true;
        }
        false
    }

    /// End recovery mode
    pub async fn end_recovery(&self, chain: &ChainKey) {
        let mut states = self.states.write().await;
        if let Some(state) = states.get_mut(chain) {
            state.is_in_recovery = false;
            state.recovery_block = None;
        }
    }

    /// Get the current recovery state for a chain
    pub async fn get_state(&self, chain: &ChainKey) -> Option<ChainRecoveryState> {
        let states = self.states.read().await;
        states.get(chain).cloned()
    }

    /// Get all chains that are in recovery mode
    pub async fn get_all_recovery_chains(&self) -> Vec<ChainKey> {
        let states = self.states.read().await;
        states
            .iter()
            .filter(|(_, state)| state.is_in_recovery)
            .map(|(chain, _)| chain.clone())
            .collect()
    }

    /// Clean up old recovery states
    pub async fn cleanup_old_states(&self, older_than: std::time::Duration) {
        let mut states = self.states.write().await;
        states.retain(|_, state| {
            if let Some(error_time) = state.last_error_time {
                error_time.elapsed() < older_than
            } else {
                true
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn test_recovery_manager() {
        let manager = RecoveryManager::new(Duration::from_secs(10));
        let chain: ChainKey = "evm:1".parse().unwrap();

        // Initially no recovery needed
        assert!(manager.needs_recovery(&chain).await.is_none());

        // Record a successful block
        manager.record_successful_block(&chain, 100).await;
        assert!(manager.needs_recovery(&chain).await.is_none());

        // Record an error
        manager.record_stream_error(&chain).await;
        assert!(manager.needs_recovery(&chain).await.is_none());

        // Wait for recovery delay
        tokio::time::sleep(Duration::from_secs(11)).await;

        // Now recovery should be needed
        assert_eq!(manager.needs_recovery(&chain).await, Some(101));
    }

    #[tokio::test]
    async fn test_start_recovery() {
        let manager = RecoveryManager::new(Duration::from_secs(10));
        let chain: ChainKey = "evm:1".parse().unwrap();

        // Start recovery directly
        assert!(manager.start_recovery(&chain, 50).await);
        assert_eq!(manager.needs_recovery(&chain).await, Some(50));

        // End recovery
        manager.end_recovery(&chain).await;
        assert!(manager.needs_recovery(&chain).await.is_none());
    }
}
