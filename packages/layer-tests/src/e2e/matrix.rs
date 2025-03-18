use std::collections::HashSet;

use derive_enum_all_values::AllValues;

use super::digests::DigestName;

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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, AllValues)]
pub enum CosmosService {
    ChainTriggerLookup,
    CosmosQuery,
    EchoData,
    Permissions,
    Square,
    BlockInterval,
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

impl From<EthService> for Vec<DigestName> {
    fn from(service: EthService) -> Self {
        match service {
            EthService::ChainTriggerLookup => vec![DigestName::ChainTriggerLookup],
            EthService::CosmosQuery => vec![DigestName::CosmosQuery],
            EthService::EchoData => vec![DigestName::EchoData],
            EthService::Permissions => vec![DigestName::Permissions],
            EthService::Square => vec![DigestName::Square],
            EthService::EchoDataSecondaryChain => vec![DigestName::EchoData],
            EthService::EchoDataAggregator => vec![DigestName::EchoData],
            EthService::MultiWorkflow => vec![DigestName::Square, DigestName::EchoData],
            EthService::MultiTrigger => vec![DigestName::EchoData],
            EthService::BlockInterval => vec![DigestName::EchoBlockInterval],
        }
    }
}

impl From<CosmosService> for Vec<DigestName> {
    fn from(service: CosmosService) -> Self {
        vec![match service {
            CosmosService::ChainTriggerLookup => DigestName::ChainTriggerLookup,
            CosmosService::CosmosQuery => DigestName::CosmosQuery,
            CosmosService::EchoData => DigestName::EchoData,
            CosmosService::Permissions => DigestName::Permissions,
            CosmosService::Square => DigestName::Square,
            CosmosService::BlockInterval => DigestName::EchoBlockInterval,
        }]
    }
}

impl From<CrossChainService> for Vec<DigestName> {
    fn from(service: CrossChainService) -> Self {
        match service {
            CrossChainService::CosmosToEthEchoData => vec![DigestName::EchoData],
        }
    }
}

impl From<AnyService> for Vec<DigestName> {
    fn from(service: AnyService) -> Self {
        match service {
            AnyService::Eth(service) => service.into(),
            AnyService::Cosmos(service) => service.into(),
            AnyService::CrossChain(service) => service.into(),
        }
    }
}
