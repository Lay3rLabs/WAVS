use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

use serde::{Deserialize, Serialize};
use utils::{
    async_transaction::AsyncTransaction,
    config::EvmChainConfig,
    evm_client::EvmSigningClient,
    storage::db::{RedbStorage, Table, JSON},
};
use wavs_types::{ChainName, EventId, Packet, PacketRoute, Service, ServiceID};

use crate::{
    config::Config,
    error::{AggregatorError, AggregatorResult},
};

// key is PacketQueueId
const PACKET_QUEUES: Table<&[u8], JSON<PacketQueue>> = Table::new("packet_queues");

#[derive(
    Hash, Serialize, Deserialize, Clone, Debug, PartialEq, Eq, bincode::Decode, bincode::Encode,
)]
pub struct PacketQueueId {
    pub event_id: EventId,
    pub service_id: ServiceID,
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

// key is ServiceId
const SERVICES: Table<&str, JSON<Service>> = Table::new("services");

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
    storage: Arc<RedbStorage>,
    evm_clients: Arc<RwLock<HashMap<ChainName, EvmSigningClient>>>,
}

// Note: task queue size is bounded by quorum and cleared on execution
impl HttpState {
    pub fn new(config: Config) -> AggregatorResult<Self> {
        let storage = Arc::new(RedbStorage::new(config.data.join("db"))?);
        let evm_clients = Arc::new(RwLock::new(HashMap::new()));

        Ok(Self {
            config,
            storage,
            evm_clients,
            queue_transaction: AsyncTransaction::new(false),
        })
    }

    pub async fn get_evm_client(
        &self,
        chain_name: &ChainName,
    ) -> AggregatorResult<EvmSigningClient> {
        {
            let lock = self.evm_clients.read().unwrap();

            if let Some(client) = lock.get(chain_name) {
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

    pub fn save_packet_queue(
        &self,
        id: &PacketQueueId,
        queue: PacketQueue,
    ) -> AggregatorResult<()> {
        Ok(self.storage.set(PACKET_QUEUES, &id.to_bytes()?, &queue)?)
    }

    pub fn get_service(&self, route: &PacketRoute) -> AggregatorResult<Service> {
        match self.storage.get(SERVICES, &route.service_id)? {
            Some(destination) => Ok(destination.value()),
            None => Err(AggregatorError::MissingService(route.service_id.clone())),
        }
    }

    pub fn register_service(&self, service: &Service) -> AggregatorResult<()> {
        if self.storage.get(SERVICES, &service.id)?.is_none() {
            tracing::info!("Registering aggregator for service {}", service.id);

            self.storage.set(SERVICES, &service.id, service)?;
        } else {
            return Err(AggregatorError::RepeatService(service.id.clone()));
        }

        Ok(())
    }

    #[cfg(test)]
    pub fn unchecked_save_service(&self, service: &Service) -> AggregatorResult<()> {
        self.storage.set(SERVICES, &service.id, service)?;

        Ok(())
    }
}
