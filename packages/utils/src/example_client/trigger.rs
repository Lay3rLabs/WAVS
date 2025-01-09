use std::ops::Deref;

use crate::{
    alloy_helpers::SolidityEventFinder, eigen_client::solidity_types::BoxSigningProvider,
    eth_client::EthSigningClient,
};
use alloy::{primitives::Address, sol_types::SolValue};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use super::{
    example_trigger::TriggerInfo,
    solidity_types::{
        example_trigger::SimpleTrigger::{self, NewTriggerId},
        SimpleTriggerT,
    },
};

#[derive(Clone)]
pub struct SimpleTriggerClient {
    pub eth: EthSigningClient,
    pub contract_address: Address,
    pub contract: SimpleTriggerT,
}

impl SimpleTriggerClient {
    pub fn new(eth: EthSigningClient, contract_address: Address) -> Self {
        let contract = SimpleTrigger::new(contract_address, eth.provider.clone());

        Self {
            eth,
            contract_address,
            contract,
        }
    }

    pub async fn new_deploy(eth: EthSigningClient) -> Result<Self> {
        let contract_address = Self::deploy(eth.provider.clone()).await?;
        Ok(Self::new(eth, contract_address))
    }

    pub async fn deploy(provider: BoxSigningProvider) -> Result<Address> {
        let contract = SimpleTrigger::deploy(provider).await?;
        Ok(*contract.address())
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

    pub async fn get_trigger_info(&self, trigger_id: TriggerId) -> Result<TriggerInfo> {
        Ok(self
            .contract
            .getTrigger(*trigger_id)
            .call()
            .await
            .context("Failed to get trigger")?
            ._0)
    }

    pub async fn get_trigger_payload(&self, trigger_id: TriggerId) -> Result<Vec<u8>> {
        Ok(self.get_trigger_info(trigger_id).await?.abi_encode())
    }

    pub async fn get_trigger_friendly(&self, trigger_id: TriggerId) -> Result<TriggerResponse> {
        let info = self.get_trigger_info(trigger_id).await?;

        Ok(TriggerResponse {
            trigger_id: TriggerId::new(info.triggerId),
            creator: info.creator,
            data: info.data.to_vec(),
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

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TriggerResponse {
    pub trigger_id: TriggerId,
    pub creator: Address,
    pub data: Vec<u8>,
}
