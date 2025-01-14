use utils::config::ChainConfigs;

pub mod any_contract_event;
pub mod cosmos_contract_event;
pub mod eth_contract_event;
pub mod raw;

pub fn convert_wit_chain_configs(
    chain_configs: ChainConfigs,
) -> layer_wasi::wit_bindings::ChainConfigs {
    let mut list = Vec::new();

    for (chain_name, chain_config) in chain_configs.eth.into_iter() {
        list.push((
            chain_name,
            layer_wasi::wit_bindings::AnyChainConfig::Eth(
                layer_wasi::wit_bindings::EthChainConfig {
                    ws_endpoint: Some(chain_config.ws_endpoint),
                    http_endpoint: chain_config.http_endpoint,
                },
            ),
        ));
    }

    for (chain_name, chain_config) in chain_configs.cosmos.into_iter() {
        list.push((
            chain_name,
            layer_wasi::wit_bindings::AnyChainConfig::Cosmos(
                layer_wasi::wit_bindings::CosmosChainConfig {
                    chain_id: chain_config.chain_id,
                    rpc_endpoint: chain_config.rpc_endpoint,
                    grpc_endpoint: chain_config.grpc_endpoint,
                    grpc_web_endpoint: None,
                    gas_price: chain_config.gas_price,
                    gas_denom: chain_config.gas_denom,
                    bech32_prefix: chain_config.bech32_prefix,
                },
            ),
        ));
    }

    list
}
