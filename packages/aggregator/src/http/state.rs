use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

use anyhow::bail;
use serde::{Deserialize, Serialize};
use utils::{
    eth_client::EthSigningClient,
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
    pub signer: alloy::primitives::Address,
}

#[derive(Clone)]
pub struct HttpState {
    pub config: Config,
    storage: Arc<RedbStorage>,
    eth_clients: Arc<RwLock<HashMap<ChainName, EthSigningClient>>>,
}

// Note: task queue size is bounded by quorum and cleared on execution
impl HttpState {
    pub fn new(config: Config) -> anyhow::Result<Self> {
        let storage = Arc::new(RedbStorage::new(config.data.join("db"))?);
        let eth_clients = Arc::new(RwLock::new(HashMap::new()));

        Ok(Self {
            config,
            storage,
            eth_clients,
        })
    }

    pub async fn get_eth_client(&self, chain_name: &ChainName) -> anyhow::Result<EthSigningClient> {
        {
            let lock = self.eth_clients.read().unwrap();

            if let Some(client) = lock.get(chain_name) {
                return Ok(client.clone());
            }
        }

        let eth_client = self.config.signing_client(chain_name).await?;

        self.eth_clients
            .write()
            .unwrap()
            .insert(chain_name.clone(), eth_client.clone());

        Ok(eth_client)
    }

    pub fn get_packet_queue(&self, event_id: &EventId) -> anyhow::Result<PacketQueue> {
        match self.storage.get(PACKET_QUEUES, event_id.as_ref())? {
            Some(queue) => Ok(queue.value()),
            None => Ok(PacketQueue::Alive(Vec::new())),
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
