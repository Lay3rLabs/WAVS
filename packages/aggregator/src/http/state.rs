use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

use anyhow::{bail, Context};
use serde::{Deserialize, Serialize};
use utils::{
    config::EthereumChainConfig,
    eth_client::{
        pool::{BalanceMaintainer, EthSigningClientPool, EthSigningClientPoolBuilder},
        EthClientBuilder, EthSigningClient,
    },
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
    eth_clients: Arc<RwLock<HashMap<ChainName, EthClient>>>,
}

#[derive(Clone)]
pub enum EthClient {
    TokioMutex(Arc<tokio::sync::Mutex<EthSigningClient>>),
    Pool(EthSigningClientPool),
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

    pub async fn get_eth_client(&self, chain_name: &ChainName) -> anyhow::Result<EthClient> {
        {
            let lock = self.eth_clients.read().unwrap();

            if let Some(client) = lock.get(chain_name) {
                return Ok(client.clone());
            }
        }

        let chain_config = self
            .config
            .chains
            .get_chain(chain_name)?
            .context(format!("chain not found for {}", chain_name))?;

        let chain_config = EthereumChainConfig::try_from(chain_config)?;
        let client_config = chain_config.to_client_config(None, self.config.mnemonic.clone(), None);

        let eth_client = EthClientBuilder::new(client_config)
            .build_signing()
            .await
            .unwrap();

        let eth_client = match &self.config.submission_pool_config {
            Some(pool_config) => {
                let pool = EthSigningClientPoolBuilder::new(
                    eth_client,
                    self.config.mnemonic.clone().context("Missing mnemonic")?,
                    chain_config,
                )
                .with_max_size(pool_config.size as usize)
                .with_initial_client_wei(pool_config.initial_wei)
                .with_balance_maintainer(BalanceMaintainer::new(
                    pool_config.threshhold_wei,
                    pool_config.topup_wei,
                )?)
                .build()?;

                EthClient::Pool(pool)
            }
            None => {
                let eth_client = Arc::new(tokio::sync::Mutex::new(eth_client));
                EthClient::TokioMutex(eth_client)
            }
        };

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
