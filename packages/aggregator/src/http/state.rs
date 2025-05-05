use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
    time::Duration,
};

use anyhow::{bail, Context};
use serde::{Deserialize, Serialize};
use utils::{
    async_transaction::AsyncTransaction,
    config::EvmChainConfig,
    evm_client::EvmSigningClient,
    storage::db::{DBError, RedbStorage, Table, JSON},
};
use wavs_types::{ChainName, EventId, Packet, PacketRoute, Service};

use crate::config::Config;

// key is EventId
const PACKET_QUEUES: Table<&[u8], JSON<PacketQueue>> = Table::new("packet_queues");

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
    pub event_transaction: AsyncTransaction<EventId>,
    storage: Arc<RedbStorage>,
    evm_clients: Arc<RwLock<HashMap<ChainName, EvmSigningClient>>>,
}

// Note: task queue size is bounded by quorum and cleared on execution
impl HttpState {
    pub fn new(config: Config) -> anyhow::Result<Self> {
        let storage = Arc::new(RedbStorage::new(config.data.join("db"))?);
        let evm_clients = Arc::new(RwLock::new(HashMap::new()));

        Ok(Self {
            config,
            storage,
            evm_clients,
            event_transaction: AsyncTransaction::new(false),
        })
    }

    pub async fn get_evm_client(&self, chain_name: &ChainName) -> anyhow::Result<EvmSigningClient> {
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
            .context(format!("chain not found for {}", chain_name))?;

        let chain_config = EvmChainConfig::try_from(chain_config)?;

        let mut client_config = chain_config.signing_client_config(
            self.config
                .credential
                .clone()
                .context("missing evm_credential")?,
        )?;

        if self.config.evm_poll_interval_ms > 0 {
            client_config = client_config
                .with_poll_interval(Duration::from_millis(self.config.evm_poll_interval_ms));
        }

        let evm_client = EvmSigningClient::new(client_config).await?;

        self.evm_clients
            .write()
            .unwrap()
            .insert(chain_name.clone(), evm_client.clone());

        Ok(evm_client)
    }

    pub fn get_packet_queue(&self, event_id: &EventId) -> anyhow::Result<PacketQueue> {
        match self.storage.get(PACKET_QUEUES, event_id.as_ref())? {
            Some(queue) => Ok(queue.value()),
            None => Ok(PacketQueue::Alive(Vec::new())),
        }
    }

    pub fn get_live_packet_queue(&self, event_id: &EventId) -> anyhow::Result<Vec<QueuedPacket>> {
        match self.storage.get(PACKET_QUEUES, event_id.as_ref())? {
            Some(queue) => match queue.value() {
                PacketQueue::Alive(queue) => Ok(queue),
                PacketQueue::Burned => Err(anyhow::anyhow!("Packet queue {event_id} is burned")),
            },
            None => Ok(Vec::new()),
        }
    }

    pub fn save_packet_queue(&self, event_id: &EventId, queue: PacketQueue) -> Result<(), DBError> {
        self.storage.set(PACKET_QUEUES, event_id.as_ref(), &queue)
    }

    pub fn get_service(&self, route: &PacketRoute) -> anyhow::Result<Service> {
        match self.storage.get(SERVICES, &route.service_id)? {
            Some(destination) => Ok(destination.value()),
            None => Err(anyhow::anyhow!(
                "Service {} is not registered",
                route.service_id
            )),
        }
    }

    pub fn register_service(&self, service: &Service) -> anyhow::Result<()> {
        if self.storage.get(SERVICES, &service.id)?.is_none() {
            tracing::info!("Registering aggregator for service {}", service.id);

            self.storage.set(SERVICES, &service.id, service)?;
        } else {
            bail!("{} already registered", service.id);
        }

        Ok(())
    }
}
