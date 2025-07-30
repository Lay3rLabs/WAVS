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
    ChangeWorkflow,
    EchoDataSecondaryChain,
    KvStore,
    Permissions,
    Square,
    MultiWorkflow,
    MultiTrigger,
    BlockInterval,
    BlockIntervalStartStop,
    CronInterval,
    EmptyToEchoData,
    SimpleAggregator,
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
    BlockIntervalStartStop,
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
    // Returns a list of all enabled services across all chain types
    pub fn enabled_services(self) -> Vec<AnyService> {
        let mut services = Vec::new();

        // Add enabled EVM services
        for service in self.evm {
            services.push(service.into());
        }

        // Add enabled Cosmos services
        for service in self.cosmos {
            services.push(service.into());
        }

        // Add enabled cross-chain services
        for service in self.cross_chain {
            services.push(service.into());
        }

        services
    }

    pub fn evm_regular_chain_enabled(&self) -> bool {
        // since we currently only submit to EVM, it's always enabled
        // TODO - if we have `Submit::None` then this should be false if no other test is enabled
        true
    }

    pub fn evm_secondary_chain_enabled(&self) -> bool {
        self.evm.contains(&EvmService::EchoDataSecondaryChain)
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
            EvmService::ChainTriggerLookup => vec![
                ComponentName::ChainTriggerLookup,
                ComponentName::SimpleAggregator,
            ],
            EvmService::CosmosQuery => {
                vec![ComponentName::CosmosQuery, ComponentName::SimpleAggregator]
            }
            EvmService::EchoData => vec![ComponentName::EchoData, ComponentName::SimpleAggregator],
            EvmService::ChangeWorkflow => vec![
                ComponentName::Square,
                ComponentName::EchoData,
                ComponentName::SimpleAggregator,
            ],
            EvmService::EchoDataSecondaryChain => {
                vec![ComponentName::EchoData, ComponentName::SimpleAggregator]
            }
            EvmService::KvStore => vec![ComponentName::KvStore, ComponentName::SimpleAggregator],
            EvmService::Permissions => {
                vec![ComponentName::Permissions, ComponentName::SimpleAggregator]
            }
            EvmService::Square => vec![ComponentName::Square, ComponentName::SimpleAggregator],
            EvmService::MultiWorkflow => vec![
                ComponentName::Square,
                ComponentName::EchoData,
                ComponentName::SimpleAggregator,
            ],
            EvmService::MultiTrigger => {
                vec![ComponentName::EchoData, ComponentName::SimpleAggregator]
            }
            EvmService::BlockInterval => vec![
                ComponentName::EchoBlockInterval,
                ComponentName::SimpleAggregator,
            ],
            EvmService::BlockIntervalStartStop => vec![
                ComponentName::EchoBlockInterval,
                ComponentName::SimpleAggregator,
            ],
            EvmService::CronInterval => vec![
                ComponentName::EchoCronInterval,
                ComponentName::SimpleAggregator,
            ],
            EvmService::EmptyToEchoData => {
                vec![ComponentName::EchoData, ComponentName::SimpleAggregator]
            }
            EvmService::SimpleAggregator => {
                vec![ComponentName::EchoData, ComponentName::SimpleAggregator]
            }
        }
    }
}

impl From<CosmosService> for Vec<ComponentName> {
    fn from(service: CosmosService) -> Self {
        match service {
            CosmosService::ChainTriggerLookup => vec![
                ComponentName::ChainTriggerLookup,
                ComponentName::SimpleAggregator,
            ],
            CosmosService::CosmosQuery => {
                vec![ComponentName::CosmosQuery, ComponentName::SimpleAggregator]
            }
            CosmosService::EchoData => {
                vec![ComponentName::EchoData, ComponentName::SimpleAggregator]
            }
            CosmosService::Permissions => {
                vec![ComponentName::Permissions, ComponentName::SimpleAggregator]
            }
            CosmosService::Square => vec![ComponentName::Square, ComponentName::SimpleAggregator],
            CosmosService::BlockInterval => vec![
                ComponentName::EchoBlockInterval,
                ComponentName::SimpleAggregator,
            ],
            CosmosService::BlockIntervalStartStop => vec![
                ComponentName::EchoBlockInterval,
                ComponentName::SimpleAggregator,
            ],
            CosmosService::CronInterval => vec![
                ComponentName::EchoCronInterval,
                ComponentName::SimpleAggregator,
            ],
        }
    }
}

impl From<CrossChainService> for Vec<ComponentName> {
    fn from(service: CrossChainService) -> Self {
        match service {
            CrossChainService::CosmosToEvmEchoData => {
                vec![ComponentName::EchoData, ComponentName::SimpleAggregator]
            }
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
