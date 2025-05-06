use std::collections::HashSet;

use derive_enum_all_values::AllValues;
use serde::{Deserialize, Serialize};

use super::components::ComponentName;

#[derive(Clone, Debug, Default)]
pub struct TestMatrix {
    pub evm: HashSet<EvmService>,
    pub cosmos: HashSet<CosmosService>,
    pub cross_chain: HashSet<CrossChainService>,
}

#[derive(
    Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, AllValues,
)]
#[serde(rename_all = "snake_case")]
pub enum EvmService {
    ChainTriggerLookup,
    CosmosQuery,
    EchoData,
    EchoDataSecondaryChain,
    EchoDataAggregator,
    Permissions,
    Square,
    MultiWorkflow,
    MultiTrigger,
    BlockInterval,
    CronInterval,
}

#[derive(
    Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, AllValues,
)]
#[serde(rename_all = "snake_case")]
pub enum CosmosService {
    ChainTriggerLookup,
    CosmosQuery,
    EchoData,
    Permissions,
    Square,
    BlockInterval,
    CronInterval,
}

#[derive(
    Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, AllValues,
)]
#[serde(rename_all = "snake_case")]
pub enum CrossChainService {
    CosmosToEvmEchoData,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum AnyService {
    Evm(EvmService),
    Cosmos(CosmosService),
    CrossChain(CrossChainService),
}

impl From<EvmService> for AnyService {
    fn from(service: EvmService) -> Self {
        AnyService::Evm(service)
    }
}

impl From<CosmosService> for AnyService {
    fn from(service: CosmosService) -> Self {
        AnyService::Cosmos(service)
    }
}

impl From<CrossChainService> for AnyService {
    fn from(service: CrossChainService) -> Self {
        AnyService::CrossChain(service)
    }
}

impl TestMatrix {
    pub fn evm_regular_chain_enabled(&self) -> bool {
        // since we currently only submit to EVM, it's always enabled
        // TODO - if we have `Submit::None` then this should be false if no other test is enabled
        true
    }

    pub fn evm_secondary_chain_enabled(&self) -> bool {
        self.evm.contains(&EvmService::EchoDataSecondaryChain)
    }

    pub fn evm_aggregator_chain_enabled(&self) -> bool {
        self.evm.contains(&EvmService::EchoDataAggregator)
    }

    pub fn cosmos_regular_chain_enabled(&self) -> bool {
        self.evm.contains(&EvmService::CosmosQuery)
            || !self.cosmos.is_empty()
            || self
                .cross_chain
                .contains(&CrossChainService::CosmosToEvmEchoData)
    }
}

impl From<EvmService> for Vec<ComponentName> {
    fn from(service: EvmService) -> Self {
        match service {
            EvmService::ChainTriggerLookup => vec![ComponentName::ChainTriggerLookup],
            EvmService::CosmosQuery => vec![ComponentName::CosmosQuery],
            EvmService::EchoData => vec![ComponentName::EchoData],
            EvmService::Permissions => vec![ComponentName::Permissions],
            EvmService::Square => vec![ComponentName::Square],
            EvmService::EchoDataSecondaryChain => vec![ComponentName::EchoData],
            EvmService::EchoDataAggregator => vec![ComponentName::EchoData],
            EvmService::MultiWorkflow => vec![ComponentName::Square, ComponentName::EchoData],
            EvmService::MultiTrigger => vec![ComponentName::EchoData],
            EvmService::BlockInterval => vec![ComponentName::EchoBlockInterval],
            EvmService::CronInterval => vec![ComponentName::EchoCronInterval],
        }
    }
}

impl From<CosmosService> for Vec<ComponentName> {
    fn from(service: CosmosService) -> Self {
        vec![match service {
            CosmosService::ChainTriggerLookup => ComponentName::ChainTriggerLookup,
            CosmosService::CosmosQuery => ComponentName::CosmosQuery,
            CosmosService::EchoData => ComponentName::EchoData,
            CosmosService::Permissions => ComponentName::Permissions,
            CosmosService::Square => ComponentName::Square,
            CosmosService::BlockInterval => ComponentName::EchoBlockInterval,
            CosmosService::CronInterval => ComponentName::EchoCronInterval,
        }]
    }
}

impl From<CrossChainService> for Vec<ComponentName> {
    fn from(service: CrossChainService) -> Self {
        match service {
            CrossChainService::CosmosToEvmEchoData => vec![ComponentName::EchoData],
        }
    }
}

impl From<AnyService> for Vec<ComponentName> {
    fn from(service: AnyService) -> Self {
        match service {
            AnyService::Evm(service) => service.into(),
            AnyService::Cosmos(service) => service.into(),
            AnyService::CrossChain(service) => service.into(),
        }
    }
}
