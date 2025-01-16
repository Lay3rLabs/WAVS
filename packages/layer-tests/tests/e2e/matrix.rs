pub struct TestMatrix {
    pub eth: TestMatrixEth,
    pub cosmos: TestMatrixCosmos,
    pub cross_chain: TestMatrixCrossChain,
}

impl TestMatrix {
    pub fn new() -> Self {
        Self {
            eth: TestMatrixEth::new(),
            cosmos: TestMatrixCosmos::new(),
            cross_chain: TestMatrixCrossChain::new(),
        }
    }
}

#[derive(Default)]
pub struct TestMatrixEth {
    pub chain_trigger_lookup: bool,
    pub cosmos_query: bool,
    pub echo_data: bool,
    pub echo_data_aggregator: bool,
    pub permissions: bool,
    pub square: bool,
}

impl TestMatrixEth {
    cfg_if::cfg_if! {
        if #[cfg(feature = "ethereum")] {
            pub fn new() -> Self {
                cfg_if::cfg_if! {
                    if #[cfg(feature = "aggregator")] {
                        let echo_data_aggregator = true;
                    } else {
                        let echo_data_aggregator = false;
                    }
                }
                // hint, for quick dev, comment out all but square (or whatever you want to debug)
                Self {
                    square: true,
                    permissions: true,
                    echo_data: true,
                    // chain_trigger_lookup: true,
                    // cosmos_query: true,
                    echo_data_aggregator,
                    ..Default::default()
                }
            }
        } else {
            pub fn new() -> Self {
                Self::default()
            }
        }
    }

    pub fn any(&self) -> bool {
        self.chain_trigger_lookup
            || self.cosmos_query
            || self.echo_data
            || self.echo_data_aggregator
            || self.permissions
            || self.square
    }
}

#[derive(Default)]
pub struct TestMatrixCosmos {
    pub chain_trigger_lookup: bool,
    pub cosmos_query: bool,
    pub echo_data: bool,
    pub permissions: bool,
    pub square: bool,
}

impl TestMatrixCosmos {
    cfg_if::cfg_if! {
        if #[cfg(feature = "cosmos")] {
            pub fn new() -> Self {
                Self {
                    chain_trigger_lookup: true,
                    cosmos_query: true,
                    echo_data: true,
                    permissions: true,
                    square: true,
                    ..Default::default()
                }
            }
        } else {
            pub fn new() -> Self {
                Self::default()
            }
        }
    }
}

#[derive(Default)]
pub struct TestMatrixCrossChain {}

impl TestMatrixCrossChain {
    cfg_if::cfg_if! {
        if #[cfg(feature = "cross-chain")] {
            pub fn new() -> Self {
                Self { }
            }
        } else {
            pub fn new() -> Self {
                Self::default()
            }
        }
    }
}
