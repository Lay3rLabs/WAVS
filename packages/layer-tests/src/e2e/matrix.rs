use std::collections::HashSet;

use super::digests::DigestName;

#[derive(Clone, Debug, Default)]
pub struct TestMatrix {
    pub eth: HashSet<EthService>,
    pub cosmos: HashSet<CosmosService>,
    pub cross_chain: HashSet<CrossChainService>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum EthService {
    ChainTriggerLookup,
    CosmosQuery,
    EchoData,
    EchoDataSecondaryChain,
    EchoDataAggregator,
    Permissions,
    Square,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum CosmosService {
    ChainTriggerLookup,
    CosmosQuery,
    EchoData,
    Permissions,
    Square,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
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
        self.eth.contains(&EthService::ChainTriggerLookup)
            || self.eth.contains(&EthService::CosmosQuery)
            || self.eth.contains(&EthService::EchoData)
            || self.eth.contains(&EthService::Permissions)
            || self.eth.contains(&EthService::Square)
            || self
                .cross_chain
                .contains(&CrossChainService::CosmosToEthEchoData)
    }

    pub fn eth_secondary_chain_enabled(&self) -> bool {
        self.eth.contains(&EthService::EchoDataSecondaryChain)
    }

    pub fn eth_aggregator_chain_enabled(&self) -> bool {
        self.eth.contains(&EthService::EchoDataAggregator)
    }

    pub fn cosmos_regular_chain_enabled(&self) -> bool {
        self.cosmos.contains(&CosmosService::ChainTriggerLookup)
            || self.cosmos.contains(&CosmosService::CosmosQuery)
            || self.cosmos.contains(&CosmosService::EchoData)
            || self.cosmos.contains(&CosmosService::Permissions)
            || self.cosmos.contains(&CosmosService::Square)
            || self
                .cross_chain
                .contains(&CrossChainService::CosmosToEthEchoData)
    }
}

impl From<EthService> for DigestName {
    fn from(service: EthService) -> Self {
        match service {
            EthService::ChainTriggerLookup => DigestName::ChainTriggerLookup,
            EthService::CosmosQuery => DigestName::CosmosQuery,
            EthService::EchoData => DigestName::EchoData,
            EthService::Permissions => DigestName::Permissions,
            EthService::Square => DigestName::Square,
            EthService::EchoDataSecondaryChain => DigestName::EchoData,
            EthService::EchoDataAggregator => DigestName::EchoData,
        }
    }
}

impl From<CosmosService> for DigestName {
    fn from(service: CosmosService) -> Self {
        match service {
            CosmosService::ChainTriggerLookup => DigestName::ChainTriggerLookup,
            CosmosService::CosmosQuery => DigestName::CosmosQuery,
            CosmosService::EchoData => DigestName::EchoData,
            CosmosService::Permissions => DigestName::Permissions,
            CosmosService::Square => DigestName::Square,
        }
    }
}

impl From<CrossChainService> for DigestName {
    fn from(service: CrossChainService) -> Self {
        match service {
            CrossChainService::CosmosToEthEchoData => DigestName::EchoData,
        }
    }
}

impl From<AnyService> for DigestName {
    fn from(service: AnyService) -> Self {
        match service {
            AnyService::Eth(service) => service.into(),
            AnyService::Cosmos(service) => service.into(),
            AnyService::CrossChain(service) => service.into(),
        }
    }
}
