use std::ops::Deref;

use super::layer_trigger::LayerTrigger;
use super::layer_trigger::NewTriggerId;
use super::LayerTriggerT;
use crate::{alloy_helpers::SolidityEventFinder, eth_client::EthSigningClient, ServiceID};
use alloy::primitives::Address;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Clone)]
pub struct LayerContractClientTrigger {
    pub eth: EthSigningClient,
    pub contract_address: Address,
    pub contract: LayerTriggerT,
}

impl LayerContractClientTrigger {
    pub fn new(eth: EthSigningClient, contract_address: Address) -> Self {
        let contract = LayerTrigger::new(contract_address, eth.provider.clone());

        Self {
            eth,
            contract_address,
            contract,
        }
    }

    // TODO - bring all newtypes into utils
    pub async fn add_trigger(&self, data: Vec<u8>) -> Result<TriggerId> {
        let event: NewTriggerId = self
            .contract
            .addTrigger(data.into())
            .send()
            .await?
            .get_receipt()
            .await?
            .solidity_event()
            .context("Not found new task creation event")?;

        Ok(TriggerId::new(event._0))
    }

    pub async fn get_trigger(&self, trigger_id: TriggerId) -> Result<TriggerResponse> {
        let resp = self
            .contract
            .getTrigger(*trigger_id)
            .call()
            .await
            .context("Failed to get trigger")?
            ._0;

        Ok(TriggerResponse {
            trigger_id: TriggerId::new(resp.triggerId),
            creator: resp.creator,
            data: resp.data.to_vec(),
        })
    }
}

// Rust-friendly API around types
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(transparent)]
pub struct TriggerId(u64);

impl TriggerId {
    pub fn new(val: u64) -> Self {
        Self(val)
    }

    /// The underlying `u64` representation.
    pub fn u64(self) -> u64 {
        self.0
    }
}

impl Deref for TriggerId {
    type Target = u64;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::fmt::Display for TriggerId {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::fmt::Debug for TriggerId {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

pub struct TriggerResponse {
    pub trigger_id: TriggerId,
    pub creator: Address,
    pub data: Vec<u8>,
}
