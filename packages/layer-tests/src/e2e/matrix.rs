use std::collections::HashSet;

use derive_enum_all_values::AllValues;

use super::components::ComponentName;

#[derive(Clone, Debug, Default)]
pub struct TestMatrix {
    pub eth: HashSet<EthService>,
    pub cosmos: HashSet<CosmosService>,
    pub cross_chain: HashSet<CrossChainService>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, AllValues)]
pub enum EthService {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, AllValues)]
pub enum CosmosService {
    ChainTriggerLookup,
    CosmosQuery,
    EchoData,
    Permissions,
    Square,
    BlockInterval,
    CronInterval,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, AllValues)]
pub enum CrossChainService {
    CosmosToEthEchoData,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum AnyService {
    Eth(EthService),
    Cosmos(CosmosService),
    CrossChain(CrossChainService),
}

impl AnyService {
    pub fn concurrent(&self) -> bool {
        // don't allow concurrency for cosmos
        // everything else should be fine
        match self {
            AnyService::Cosmos(_)
            | AnyService::CrossChain(_)
            | AnyService::Eth(EthService::CosmosQuery) => false,
            _ => true,
        }
    }
}

impl From<EthService> for AnyService {
    fn from(service: EthService) -> Self {
        AnyService::Eth(service)
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
    pub fn eth_regular_chain_enabled(&self) -> bool {
        // since we currently only submit to eth, it's always enabled
        // TODO - if we have `Submit::None` then this should be false if no other test is enabled
        true
    }

    pub fn eth_secondary_chain_enabled(&self) -> bool {
        self.eth.contains(&EthService::EchoDataSecondaryChain)
    }

    pub fn eth_aggregator_chain_enabled(&self) -> bool {
        self.eth.contains(&EthService::EchoDataAggregator)
    }

    pub fn cosmos_regular_chain_enabled(&self) -> bool {
        self.eth.contains(&EthService::CosmosQuery)
            || !self.cosmos.is_empty()
            || self
                .cross_chain
                .contains(&CrossChainService::CosmosToEthEchoData)
    }
}

impl From<EthService> for Vec<ComponentName> {
    fn from(service: EthService) -> Self {
        match service {
            EthService::ChainTriggerLookup => vec![ComponentName::ChainTriggerLookup],
            EthService::CosmosQuery => vec![ComponentName::CosmosQuery],
            EthService::EchoData => vec![ComponentName::EchoData],
            EthService::Permissions => vec![ComponentName::Permissions],
            EthService::Square => vec![ComponentName::Square],
            EthService::EchoDataSecondaryChain => vec![ComponentName::EchoData],
            EthService::EchoDataAggregator => vec![ComponentName::EchoData],
            EthService::MultiWorkflow => vec![ComponentName::Square, ComponentName::EchoData],
            EthService::MultiTrigger => vec![ComponentName::EchoData],
            EthService::BlockInterval => vec![ComponentName::EchoBlockInterval],
            EthService::CronInterval => vec![ComponentName::EchoCronInterval],
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
            CrossChainService::CosmosToEthEchoData => vec![ComponentName::EchoData],
        }
    }
}

impl From<AnyService> for Vec<ComponentName> {
    fn from(service: AnyService) -> Self {
        match service {
            AnyService::Eth(service) => service.into(),
            AnyService::Cosmos(service) => service.into(),
            AnyService::CrossChain(service) => service.into(),
        }
    }
}
