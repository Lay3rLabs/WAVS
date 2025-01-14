pub mod address;
mod bindings;
pub mod collection;
pub mod cosmos;
pub mod ethereum;
pub mod wasi;

pub mod wit_bindings {
    pub use crate::bindings::lay3r::avs::layer_types::*;

    // These canonicalize_* macros are used to deal with the fact that
    // different components importing the same wit file are treated as different types by the compiler
    #[macro_export]
    macro_rules! canonicalize_any_contract {
        (
            $input_enum:ty, // the type exported from the wit module 
            $value:expr // the value to canonicalize
        ) => {{
            type CanonicalAnyContract = $crate::wit_bindings::AnyContract;
            type CanonicalCosmosContract = $crate::wit_bindings::CosmosContract;

            // see https://github.com/rust-lang/rust/issues/86935#issuecomment-1699575571
            type Alias = $input_enum;

            match $value {
                Alias::Eth(contract) => CanonicalAnyContract::Eth(contract),
                Alias::Cosmos(contract) => CanonicalAnyContract::Cosmos(CanonicalCosmosContract {
                    bech32_addr: contract.bech32_addr,
                    prefix_len: contract.prefix_len,
                }),
            }
        }};
    }

    #[macro_export]
    macro_rules! canonicalize_chain_configs {
        (
            $input_enum:ty, // the type exported from the wit module 
            $value:expr // the value to canonicalize
        ) => {{
            $value
                .into_iter()
                .map(|(chain_name, chain_config)| {
                    (
                        chain_name,
                        $crate::canonicalize_any_chain_config!($input_enum, chain_config),
                    )
                })
                .collect::<Vec<_>>()
        }};
    }

    #[macro_export]
    macro_rules! canonicalize_any_chain_config {
        (
            $input_enum:ty, // the type exported from the wit module 
            $value:expr // the value to canonicalize
        ) => {{
            type CanonicalAnyChainConfig = $crate::wit_bindings::AnyChainConfig;
            type CanonicalEthChainConfig = $crate::wit_bindings::EthChainConfig;
            type CanonicalCosmosChainConfig = $crate::wit_bindings::CosmosChainConfig;

            // see https://github.com/rust-lang/rust/issues/86935#issuecomment-1699575571
            type Alias = $input_enum;

            match $value {
                Alias::Eth(chain_config) => CanonicalAnyChainConfig::Eth(CanonicalEthChainConfig {
                    ws_endpoint: chain_config.ws_endpoint,
                    http_endpoint: chain_config.http_endpoint,
                }),
                Alias::Cosmos(chain_config) => {
                    CanonicalAnyChainConfig::Cosmos(CanonicalCosmosChainConfig {
                        chain_id: chain_config.chain_id,
                        rpc_endpoint: chain_config.rpc_endpoint,
                        grpc_endpoint: chain_config.grpc_endpoint,
                        grpc_web_endpoint: chain_config.grpc_web_endpoint,
                        gas_price: chain_config.gas_price,
                        gas_denom: chain_config.gas_denom,
                        bech32_prefix: chain_config.bech32_prefix,
                    })
                }
            }
        }};
    }

    #[macro_export]
    macro_rules! canonicalize_any_event {
        (
            $input_enum:ty, // the type exported from the wit module 
            $value:expr // the value to canonicalize
        ) => {{
            type CanonicalAnyEvent = $crate::wit_bindings::AnyEvent;
            type CanonicalEthEventLogData = $crate::wit_bindings::EthEventLogData;
            type CanonicalCosmosEvent = $crate::wit_bindings::CosmosEvent;

            // see https://github.com/rust-lang/rust/issues/86935#issuecomment-1699575571
            type Alias = $input_enum;

            match $value {
                Alias::Eth(event) => CanonicalAnyEvent::Eth(CanonicalEthEventLogData {
                    topics: event.topics,
                    data: event.data,
                }),
                Alias::Cosmos(event) => CanonicalAnyEvent::Cosmos(CanonicalCosmosEvent {
                    ty: event.ty,
                    attributes: event.attributes,
                }),
            }
        }};
    }
}
