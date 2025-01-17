use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct TestMatrix {
    pub eth: TestMatrixEth,
    pub cosmos: TestMatrixCosmos,
    pub crosschain: TestMatrixCrossChain,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "kebab-case")]
pub struct TestMatrixEth {
    pub chain_trigger_lookup: bool,
    pub cosmos_query: bool,
    pub echo_data: bool,
    pub echo_data_multichain: bool,
    pub echo_data_aggregator: bool,
    pub permissions: bool,
    pub square: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "kebab-case")]
pub struct TestMatrixCosmos {
    pub chain_trigger_lookup: bool,
    pub cosmos_query: bool,
    pub echo_data: bool,
    pub permissions: bool,
    pub square: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "kebab-case")]
pub struct TestMatrixCrossChain {
    pub eth_to_cosmos_echo_data: bool,
}

impl TestMatrix {
    pub fn has_aggregator(&self) -> bool {
        self.eth.has_aggregator() || self.cosmos.has_aggregator()
    }

    pub fn eth_chain_count(&self) -> usize {
        if self.eth.multichain_enabled() {
            2
        } else if self.eth.any_enabled() || self.crosschain.any_eth_enabled() {
            1
        } else {
            0
        }
    }

    pub fn cosmos_chain_count(&self) -> usize {
        if self.cosmos.multichain_enabled() {
            2
        } else if self.cosmos.any_enabled() || self.crosschain.any_cosmos_enabled() {
            1
        } else {
            0
        }
    }
    pub fn overwrite_isolated(&mut self, isolated: &str) {
        *self = Self::default();

        match isolated {
            "eth-chain-trigger-lookup" => {
                self.eth.chain_trigger_lookup = true;
            }
            "eth-cosmos-query" => {
                self.eth.cosmos_query = true;
            }
            "eth-echo-data" => {
                self.eth.echo_data = true;
            }
            "eth-echo-data-multichain" => {
                self.eth.echo_data_multichain = true;
            }
            "eth-echo-data-aggregator" => {
                self.eth.echo_data_aggregator = true;
            }
            "eth-permissions" => {
                self.eth.permissions = true;
            }
            "eth-square" => {
                self.eth.square = true;
            }
            "cosmos-chain-trigger-lookup" => {
                self.cosmos.chain_trigger_lookup = true;
            }
            "cosmos-cosmos-query" => {
                self.cosmos.cosmos_query = true;
            }
            "cosmos-echo-data" => {
                self.cosmos.echo_data = true;
            }
            "cosmos-permissions" => {
                self.cosmos.permissions = true;
            }
            "cosmos-square" => {
                self.cosmos.square = true;
            }
            "crosschain-eth-to-cosmos-echo-data" => {
                self.crosschain.eth_to_cosmos_echo_data = true;
            }
            _ => {
                panic!("Unknown isolated test: {}", isolated);
            }
        }
    }
}

impl TestMatrixEth {
    pub fn any_enabled(&self) -> bool {
        self.chain_trigger_lookup
            || self.cosmos_query
            || self.echo_data
            || self.echo_data_aggregator
            || self.permissions
            || self.square
            || self.echo_data_multichain
    }

    pub fn multichain_enabled(&self) -> bool {
        self.echo_data_multichain
    }

    pub fn has_aggregator(&self) -> bool {
        self.echo_data_aggregator
    }
}

impl TestMatrixCosmos {
    pub fn any_enabled(&self) -> bool {
        self.chain_trigger_lookup
            || self.cosmos_query
            || self.echo_data
            || self.permissions
            || self.square
    }

    pub fn multichain_enabled(&self) -> bool {
        false
    }

    pub fn has_aggregator(&self) -> bool {
        false
    }
}

impl TestMatrixCrossChain {
    pub fn any_eth_enabled(&self) -> bool {
        self.eth_to_cosmos_echo_data
    }
    pub fn any_cosmos_enabled(&self) -> bool {
        self.eth_to_cosmos_echo_data
    }
}
