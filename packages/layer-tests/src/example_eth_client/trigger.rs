use std::ops::Deref;

use alloy_primitives::Address;
use alloy_provider::DynProvider;
use alloy_sol_types::SolValue;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use utils::{alloy_helpers::SolidityEventFinder, eth_client::EthSigningClient};

use super::{
    example_trigger::ISimpleTrigger::TriggerInfo,
    solidity_types::{
        example_trigger::SimpleTrigger::{self, NewTrigger},
        SimpleTriggerT,
    },
};

pub struct SimpleEthTriggerClient {
    pub eth: EthSigningClient,
    pub contract_address: Address,
    pub contract: SimpleTriggerT,
}

impl SimpleEthTriggerClient {
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

    pub async fn deploy(provider: DynProvider) -> Result<Address> {
        Ok(*SimpleTrigger::deploy(provider).await?.address())
    }

    // just a static helper to simulate the data that would be emitted from the contract
    pub fn trigger_info_bytes(
        creator: Address,
        trigger_id: u64,
        data: impl AsRef<[u8]>,
    ) -> Vec<u8> {
        TriggerInfo {
            triggerId: trigger_id,
            creator,
            data: data.as_ref().to_vec().into(),
        }
        .abi_encode()
    }

    pub async fn add_trigger(&self, data: Vec<u8>) -> Result<TriggerId> {
        let event: NewTrigger = self
            .contract
            .addTrigger(data.into())
            .send()
            .await?
            .get_receipt()
            .await?
            .solidity_event()
            .context("Not found new task creation event")?;

        let trigger_info = TriggerInfo::abi_decode(&event._0, false)?;

        Ok(TriggerId::new(trigger_info.triggerId))
    }

    // Returns the inner trigger data (i.e. the data that was sent via add_trigger)
    pub async fn get_trigger_data(&self, trigger_id: TriggerId) -> Result<Vec<u8>> {
        Ok(self
            .contract
            .getTrigger(*trigger_id)
            .call()
            .await
            .context("Failed to get trigger")?
            ._0
            .data
            .to_vec())
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
    pub data: Vec<u8>,
}
