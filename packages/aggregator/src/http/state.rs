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
use wavs_types::{
    Aggregator, ChainName, EthereumContractSubmission, EventId, Packet, PacketRoute, Service,
    Submit,
};

use crate::config::Config;

fn packet_route_key(route: &PacketRoute) -> String {
    format!("{}|{}", route.service_id, route.workflow_id)
}

// key is EventId
const PACKET_QUEUES: Table<&[u8], JSON<PacketQueue>> = Table::new("packet_queues");

// key is PacketRoute
const DESTINATIONS: Table<&str, JSON<Destination>> = Table::new("destinations");

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Destination {
    Eth(EthereumContractSubmission),
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "snake_case")]
pub enum PacketQueue {
    Burned,
    Alive(Vec<Packet>),
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

    pub fn get_destination(&self, route: &PacketRoute) -> anyhow::Result<Destination> {
        let key = packet_route_key(route);

        match self.storage.get(DESTINATIONS, &key)? {
            Some(destination) => Ok(destination.value()),
            None => Err(anyhow::anyhow!(
                "Destination for route {key} is not registered",
            )),
        }
    }

    pub fn register_service(&self, service: &Service) -> anyhow::Result<()> {
        for (workflow_id, workflow) in service.workflows.iter() {
            if matches!(workflow.submit, Submit::Aggregator { .. }) {
                match &workflow.aggregator {
                    None => {
                        bail!(
                            "No aggregator set for service {} workflow {}",
                            service.id,
                            workflow_id
                        );
                    }
                    Some(Aggregator::Ethereum(submission)) => {
                        let key = packet_route_key(&PacketRoute {
                            service_id: service.id.clone(),
                            workflow_id: workflow_id.clone(),
                        });

                        if self.storage.get(DESTINATIONS, &key)?.is_none() {
                            tracing::info!("Registering aggregator for {key}");

                            self.storage.set(
                                DESTINATIONS,
                                &key,
                                &Destination::Eth(submission.clone()),
                            )?;
                        } else {
                            // this should error for the first workflow if there's any duplicate
                            // and others are unreachable
                            bail!("{key} already registered");
                        }
                    }
                }
            }
        }

        Ok(())
    }
}
