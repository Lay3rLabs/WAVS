use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

use serde::{Deserialize, Serialize};
use tracing::instrument;
use utils::{
    async_transaction::AsyncTransaction,
    config::{EvmChainConfig, EvmChainConfigExt},
    evm_client::EvmSigningClient,
    storage::{
        db::{RedbStorage, Table, JSON},
        fs::FileStorage,
    },
};
use wavs_types::{ChainName, EventId, Packet, ServiceID};

use crate::{
    config::Config,
    engine::AggregatorEngine,
    error::{AggregatorError, AggregatorResult},
};

// key is QuorumQueueId
const QUORUM_QUEUES: Table<&[u8], JSON<QuorumQueue>> = Table::new("quorum_queues");

#[derive(
    Hash, Serialize, Deserialize, Clone, Debug, PartialEq, Eq, bincode::Decode, bincode::Encode,
)]
pub struct QuorumQueueId {
    pub event_id: EventId,
    pub aggregator_action: wavs_types::AggregatorAction,
}

impl QuorumQueueId {
    pub fn to_bytes(&self) -> AggregatorResult<Vec<u8>> {
        Ok(bincode::encode_to_vec(self, bincode::config::standard())?)
    }

    pub fn from_bytes(bytes: &[u8]) -> AggregatorResult<Self> {
        Ok(bincode::borrow_decode_from_slice(bytes, bincode::config::standard())?.0)
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "snake_case")]
pub enum QuorumQueue {
    Burned,
    Active(Vec<QueuedPacket>),
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "snake_case")]
pub struct QueuedPacket {
    pub packet: Packet,
    // so we don't need to recalculate it every time
    pub signer: alloy_primitives::Address,
}

#[derive(Clone)]
pub struct HttpState {
    pub config: Config,
    pub queue_transaction: AsyncTransaction<QuorumQueueId>,
    storage: RedbStorage,
    evm_clients: Arc<RwLock<HashMap<ChainName, EvmSigningClient>>>,
    pub aggregator_engine: Arc<AggregatorEngine<FileStorage>>,
}

// key is ServiceId
const SERVICES: Table<[u8; 32], ()> = Table::new("services");

// Note: task queue size is bounded by quorum and cleared on execution
impl HttpState {
    #[instrument(level = "debug", skip(config))]
    pub fn new(config: Config) -> AggregatorResult<Self> {
        Self::new_with_engine(config)
    }

    #[instrument(level = "debug", skip(config))]
    pub fn new_with_engine(config: Config) -> AggregatorResult<Self> {
        tracing::info!("Creating file storage at: {:?}", config.data);
        let file_storage = FileStorage::new(&config.data)?;
        let ca_storage = Arc::new(file_storage);
        let storage = RedbStorage::new(config.data.join("db"))?;
        let evm_clients = Arc::new(RwLock::new(HashMap::new()));

        let engine = AggregatorEngine::new(
            config.data.join("wasm"),
            config.chains.clone(),
            config.wasm_lru_size,
            config.max_wasm_fuel,
            config.max_execution_seconds,
            storage.clone(),
            ca_storage,
        )
        .map_err(|e| AggregatorError::EngineInitialization(e.to_string()))?;

        engine
            .start()
            .map_err(|e| AggregatorError::EngineInitialization(e.to_string()))?;

        Ok(Self {
            config,
            storage,
            evm_clients,
            queue_transaction: AsyncTransaction::new(false),
            aggregator_engine: Arc::new(engine),
        })
    }

    #[instrument(level = "debug", skip(self), fields(chain_name = %chain_name))]
    pub async fn get_evm_client(
        &self,
        chain_name: &ChainName,
    ) -> AggregatorResult<EvmSigningClient> {
        {
            let lock = self.evm_clients.read().unwrap();

            if let Some(client) = lock.get(chain_name) {
                tracing::debug!("Using cached EVM client for chain: {}", chain_name);
                return Ok(client.clone());
            }
        }

        let chain_config = self
            .config
            .chains
            .get_chain(chain_name)?
            .ok_or(AggregatorError::ChainNotFound(chain_name.clone()))?;

        let chain_config = EvmChainConfig::try_from(chain_config)?;

        let client_config = chain_config.signing_client_config(
            self.config
                .credential
                .clone()
                .ok_or(AggregatorError::MissingEvmCredential)?,
        )?;

        tracing::info!("Creating new EVM client for chain: {}", chain_name);
        let evm_client = EvmSigningClient::new(client_config)
            .await
            .map_err(AggregatorError::CreateEvmClient)?;

        self.evm_clients
            .write()
            .unwrap()
            .insert(chain_name.clone(), evm_client.clone());

        Ok(evm_client)
    }

    pub async fn get_quorum_queue(&self, id: &QuorumQueueId) -> AggregatorResult<QuorumQueue> {
        let storage = self.storage.clone();
        let id_bytes = id.to_bytes()?;

        tokio::task::spawn_blocking(move || {
            Ok(storage
                .get(QUORUM_QUEUES, &id_bytes)?
                .map(|queue| queue.value())
                .unwrap_or_else(|| QuorumQueue::Active(Vec::new())))
        })
        .await
        .map_err(|e| AggregatorError::JoinError(e.to_string()))?
    }

    #[allow(clippy::result_large_err)]
    pub async fn save_quorum_queue(
        &self,
        id: &QuorumQueueId,
        queue: QuorumQueue,
    ) -> AggregatorResult<()> {
        let storage = self.storage.clone();
        let id_bytes = id.to_bytes()?;

        tokio::task::spawn_blocking(move || storage.set(QUORUM_QUEUES, &id_bytes, &queue))
            .await
            .map_err(|e| AggregatorError::JoinError(e.to_string()))?
            .map_err(Into::into)
    }

    #[instrument(level = "debug", skip(self))]
    pub async fn service_registered(&self, service_id: ServiceID) -> bool {
        let storage = self.storage.clone();

        tokio::task::spawn_blocking(move || {
            storage
                .get(SERVICES, service_id.inner())
                .ok()
                .flatten()
                .is_some()
        })
        .await
        .ok()
        .unwrap_or(false)
    }

    #[instrument(level = "debug", skip(self))]
    #[allow(clippy::result_large_err)]
    pub fn register_service(&self, service_id: &ServiceID) -> AggregatorResult<()> {
        if self.storage.get(SERVICES, service_id.inner())?.is_none() {
            tracing::info!("Registering aggregator for service {}", service_id);

            self.storage.set(SERVICES, service_id.inner(), &())?;
        } else {
            tracing::warn!("Attempted to register duplicate service: {}", service_id);
            return Err(AggregatorError::RepeatService(service_id.clone()));
        }
        Ok(())
    }
}
