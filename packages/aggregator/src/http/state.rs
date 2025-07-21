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
        CAStorage,
    },
};
use wavs_types::{ChainName, EventId, Packet, ServiceID};

use crate::{
    config::Config,
    engine::AggregatorEngine,
    error::{AggregatorError, AggregatorResult},
};

// key is PacketQueueId
const PACKET_QUEUES: Table<&[u8], JSON<PacketQueue>> = Table::new("packet_queues");

#[derive(
    Hash, Serialize, Deserialize, Clone, Debug, PartialEq, Eq, bincode::Decode, bincode::Encode,
)]
pub struct PacketQueueId {
    pub event_id: EventId,
    pub aggregator_index: usize,
}

impl PacketQueueId {
    pub fn to_bytes(&self) -> AggregatorResult<Vec<u8>> {
        Ok(bincode::encode_to_vec(self, bincode::config::standard())?)
    }

    pub fn from_bytes(bytes: &[u8]) -> AggregatorResult<Self> {
        Ok(bincode::borrow_decode_from_slice(bytes, bincode::config::standard())?.0)
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "snake_case")]
pub enum PacketQueue {
    Burned,
    Alive(Vec<QueuedPacket>),
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
    pub queue_transaction: AsyncTransaction<PacketQueueId>,
    storage: RedbStorage,
    evm_clients: Arc<RwLock<HashMap<ChainName, EvmSigningClient>>>,
    pub aggregator_engine: Option<Arc<AggregatorEngine<FileStorage>>>,
}

// key is ServiceId
const SERVICES: Table<[u8; 32], ()> = Table::new("services");

// Note: task queue size is bounded by quorum and cleared on execution
impl HttpState {
    #[instrument(level = "debug", skip(config))]
    pub fn new(config: Config) -> AggregatorResult<Self> {
        let storage = RedbStorage::new(config.data.join("db"))?;
        let evm_clients = Arc::new(RwLock::new(HashMap::new()));

        Ok(Self {
            config,
            storage,
            evm_clients,
            queue_transaction: AsyncTransaction::new(false),
            aggregator_engine: None,
        })
    }

    #[instrument(level = "debug", skip(config, ca_storage))]
    pub fn new_with_engine(config: Config, ca_storage: Arc<FileStorage>) -> AggregatorResult<Self> {
        let storage = RedbStorage::new(config.data.join("db"))?;
        let evm_clients = Arc::new(RwLock::new(HashMap::new()));

        let engine = AggregatorEngine::new(
            config.data.join("wasm"),
            config.chains.clone(),
            config.wasm_lru_size,
            None, // max_wasm_fuel
            None, // max_execution_seconds
            storage.clone(),
            ca_storage,
        )
        .map_err(|e| AggregatorError::Engine(e.to_string()))?;

        engine
            .start()
            .map_err(|e| AggregatorError::Engine(e.to_string()))?;

        Ok(Self {
            config,
            storage,
            evm_clients,
            queue_transaction: AsyncTransaction::new(false),
            aggregator_engine: Some(Arc::new(engine)),
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

    pub fn get_packet_queue(&self, id: &PacketQueueId) -> AggregatorResult<PacketQueue> {
        match self.storage.get(PACKET_QUEUES, &id.to_bytes()?)? {
            Some(queue) => Ok(queue.value()),
            None => Ok(PacketQueue::Alive(Vec::new())),
        }
    }

    #[allow(clippy::result_large_err)]
    pub fn save_packet_queue(
        &self,
        id: &PacketQueueId,
        queue: PacketQueue,
    ) -> AggregatorResult<()> {
        Ok(self.storage.set(PACKET_QUEUES, &id.to_bytes()?, &queue)?)
    }

    #[instrument(level = "debug", skip(self))]
    pub fn service_registered(&self, service_id: &ServiceID) -> bool {
        self.storage
            .get(SERVICES, service_id.inner())
            .ok()
            .flatten()
            .is_some()
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
