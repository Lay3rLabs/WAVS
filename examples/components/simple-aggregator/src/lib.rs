mod gas_oracle;
mod world;

use wavs_types::ChainKey;
use wavs_wasi_utils::impl_u128_conversions;
use world::{
    host,
    wavs::aggregator::input::AggregatorInput,
    wavs::aggregator::output::{
        AggregatorAction, CosmosAddress, CosmosSubmitAction, EvmSubmitAction, SubmitAction, U128,
    },
    wavs::types::chain::{AnyTxHash, EvmAddress},
    Guest,
};

impl_u128_conversions!(U128);

struct Component;

impl Guest for Component {
    fn process_input(_input: AggregatorInput) -> Result<Vec<AggregatorAction>, String> {
        let chain = host::config_var("chain").ok_or("chain config variable is required")?;
        let chain =
            AnyChainKey::from_host(&chain).ok_or(format!("no chain config for {}", chain))?;

        let service_handler_str = host::config_var("service_handler")
            .ok_or("service_handler config variable is required")?;

        let submit_action = match chain {
            AnyChainKey::Evm(chain) => {
                let address: alloy_primitives::Address = service_handler_str
                    .parse()
                    .map_err(|e| format!("Failed to parse service handler address: {e}"))?;

                // Get gas price from Etherscan if configured
                // will fail the entire operation if API key is configured but fetching fails
                let gas_price = gas_oracle::get_gas_price()?;

                SubmitAction::Evm(EvmSubmitAction {
                    chain: chain.to_string(),
                    address: EvmAddress {
                        raw_bytes: address.to_vec(),
                    },
                    gas_price: gas_price.map(|x| x.into()),
                })
            }
            AnyChainKey::Cosmos(chain) => {
                let address = layer_climb_address::CosmosAddr::new_str(&service_handler_str, None)
                    .map_err(|e| e.to_string())?;

                SubmitAction::Cosmos(CosmosSubmitAction {
                    chain: chain.to_string(),
                    address: CosmosAddress {
                        bech32_addr: address.to_string(),
                        prefix_len: address.prefix().len() as u32,
                    },
                    gas_price: None,
                })
            }
        };

        // Sanity check that we can get the event id
        if host::get_event_id().iter().all(|x| *x == 0) {
            return Err("event id is all zeros".to_string());
        }

        Ok(vec![AggregatorAction::Submit(submit_action)])
    }

    fn handle_timer_callback(_input: AggregatorInput) -> Result<Vec<AggregatorAction>, String> {
        Err("Not implemented yet".to_string())
    }

    fn handle_submit_callback(
        _input: AggregatorInput,
        tx_result: Result<AnyTxHash, String>,
    ) -> Result<(), String> {
        match tx_result {
            Ok(_) => Ok(()),
            Err(_) => Ok(()),
        }
    }
}

enum AnyChainKey {
    Evm(ChainKey),
    Cosmos(ChainKey),
}

impl AnyChainKey {
    pub fn from_host(chain: &str) -> Option<Self> {
        match host::get_evm_chain_config(chain) {
            Some(_) => Some(AnyChainKey::Evm(chain.parse().ok()?)),
            None => match host::get_cosmos_chain_config(chain) {
                Some(_) => Some(AnyChainKey::Cosmos(chain.parse().ok()?)),
                None => None,
            },
        }
    }
}

export_aggregator_world!(Component);
