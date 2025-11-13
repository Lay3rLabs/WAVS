use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

use layer_climb::{prelude::KeySigner, signing::SigningClient};
use serde::{Deserialize, Serialize};
use tracing::instrument;
use utils::{
    async_transaction::AsyncTransaction,
    config::EvmChainConfigExt,
    evm_client::EvmSigningClient,
    storage::{
        db::{handles, Table, TableHandle, WavsDb},
        fs::FileStorage,
    },
};
use wavs_types::{ChainKey, CosmosChainConfig, EventId, EvmChainConfig, Packet, ServiceId};

use crate::{
    config::Config,
    engine::AggregatorEngine,
    error::{AggregatorError, AggregatorResult},
};

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
    storage: WavsDb,
    evm_clients: Arc<RwLock<HashMap<ChainKey, EvmSigningClient>>>,
    cosmos_clients: Arc<RwLock<HashMap<ChainKey, SigningClient>>>,
    pub aggregator_engine: Arc<AggregatorEngine<FileStorage>>,
    pub metrics: utils::telemetry::AggregatorMetrics,
}

const QUORUM_QUEUE_TABLE: TableHandle<Vec<u8>, QuorumQueue> = TableHandle::new(Table::QuorumQueues);

// Note: task queue size is bounded by quorum and cleared on execution
impl HttpState {
    #[instrument(skip(config, metrics))]
    pub fn new(
        config: Config,
        metrics: utils::telemetry::AggregatorMetrics,
    ) -> AggregatorResult<Self> {
        Self::new_with_engine(config, metrics)
    }

    #[instrument(skip(config, metrics))]
    pub fn new_with_engine(
        config: Config,
        metrics: utils::telemetry::AggregatorMetrics,
    ) -> AggregatorResult<Self> {
        tracing::info!("Creating file storage at: {:?}", config.data);
        let file_storage = FileStorage::new(&config.data)?;
        let ca_storage = Arc::new(file_storage);
        let storage = WavsDb::new()?;
        let evm_clients = Arc::new(RwLock::new(HashMap::new()));
        let cosmos_clients = Arc::new(RwLock::new(HashMap::new()));

        let engine = AggregatorEngine::new(
            config.data.join("wasm"),
            config.chains.clone(),
            config.wasm_lru_size,
            config.max_wasm_fuel,
            config.max_execution_seconds,
            storage.clone(),
            ca_storage,
            metrics.clone(),
            config.ipfs_gateway.clone(),
        )
        .map_err(|e| AggregatorError::EngineInitialization(e.to_string()))?;

        Ok(Self {
            config,
            storage,
            evm_clients,
            cosmos_clients,
            queue_transaction: AsyncTransaction::new(false),
            aggregator_engine: Arc::new(engine),
            metrics,
        })
    }

    #[instrument(skip(self), fields(chain = %chain))]
    pub async fn get_evm_client(&self, chain: &ChainKey) -> AggregatorResult<EvmSigningClient> {
        {
            let lock = self.evm_clients.read().unwrap();

            if let Some(client) = lock.get(chain) {
                tracing::debug!("Using cached EVM client for chain: {chain}");
                return Ok(client.clone());
            }
        }

        let chain_config = self
            .config
            .chains
            .read()
            .map_err(|_| anyhow::anyhow!("Chain configs lock is poisoned"))?
            .get_chain(chain)
            .ok_or(AggregatorError::ChainNotFound(chain.clone()))?;

        let chain_config = EvmChainConfig::try_from(chain_config)?;

        let client_config = chain_config.signing_client_config(
            self.config
                .credential
                .clone()
                .ok_or(AggregatorError::MissingEvmCredential)?,
        )?;

        tracing::info!("Creating new EVM client for chain: {}", chain);
        let evm_client = EvmSigningClient::new(client_config)
            .await
            .map_err(AggregatorError::CreateEvmClient)?;

        self.evm_clients
            .write()
            .unwrap()
            .insert(chain.clone(), evm_client.clone());

        Ok(evm_client)
    }

    #[instrument(skip(self), fields(chain = %chain))]
    pub async fn get_cosmos_client(&self, chain: &ChainKey) -> AggregatorResult<SigningClient> {
        {
            let lock = self.cosmos_clients.read().unwrap();

            if let Some(client) = lock.get(chain) {
                tracing::debug!("Using cached Cosmos client for chain: {chain}");
                return Ok(client.clone());
            }
        }

        let chain_config = self
            .config
            .chains
            .read()
            .map_err(|_| anyhow::anyhow!("Chain configs lock is poisoned"))?
            .get_chain(chain)
            .ok_or(AggregatorError::ChainNotFound(chain.clone()))?;

        let chain_config = CosmosChainConfig::try_from(chain_config)?;

        let key_signer = KeySigner::new_mnemonic_str(
            &self
                .config
                .cosmos_credential
                .clone()
                .ok_or(AggregatorError::MissingCosmosCredential)?,
            None,
        )?;

        tracing::info!("Creating new Cosmos client for chain: {}", chain);
        let cosmos_client = SigningClient::new(chain_config.into(), key_signer, None)
            .await
            .map_err(AggregatorError::CreateEvmClient)?;

        self.cosmos_clients
            .write()
            .unwrap()
            .insert(chain.clone(), cosmos_client.clone());

        Ok(cosmos_client)
    }

    pub async fn get_quorum_queue(&self, id: &QuorumQueueId) -> AggregatorResult<QuorumQueue> {
        let storage = self.storage.clone();
        let id_bytes = id.to_bytes()?;

        tokio::task::spawn_blocking(move || {
            Ok(storage
                .get(&QUORUM_QUEUE_TABLE, id_bytes)?
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

        tokio::task::spawn_blocking(move || storage.set(&QUORUM_QUEUE_TABLE, id_bytes, queue))
            .await
            .map_err(|e| AggregatorError::JoinError(e.to_string()))?
            .map_err(Into::into)
    }

    #[instrument(skip(self))]
    pub async fn service_registered(&self, service_id: ServiceId) -> bool {
        let storage = self.storage.clone();

        tokio::task::spawn_blocking(move || {
            storage
                .get(&handles::AGGREGATOR_SERVICES, service_id.inner())
                .ok()
                .flatten()
                .is_some()
        })
        .await
        .ok()
        .unwrap_or(false)
    }

    #[instrument(skip(self))]
    #[allow(clippy::result_large_err)]
    pub fn register_service(&self, service_id: &ServiceId) -> AggregatorResult<()> {
        if self
            .storage
            .get(&handles::AGGREGATOR_SERVICES, service_id.inner())?
            .is_none()
        {
            tracing::info!("Registering aggregator for service {}", service_id);

            self.storage
                .set(&handles::AGGREGATOR_SERVICES, service_id.inner(), ())?;
        } else {
            tracing::warn!("Attempted to register duplicate service: {}", service_id);
            return Err(AggregatorError::RepeatService(service_id.clone()));
        }
        Ok(())
    }
}
