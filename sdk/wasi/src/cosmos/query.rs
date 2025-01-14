use anyhow::Result;
use layer_climb_address::Address;
use layer_climb_config::{AddrKind, ChainConfig, ChainId};
use layer_climb_proto::Coin;
use serde::{de::DeserializeOwned, Serialize};
use wstd::runtime::Reactor;

use crate::collection::HashMapLike;

use super::rpc;

pub struct CosmosQuerier {
    pub chain_config: ChainConfig,
    pub reactor: Reactor,
}

impl From<crate::wit_bindings::CosmosChainConfig> for ChainConfig {
    fn from(config: crate::wit_bindings::CosmosChainConfig) -> ChainConfig {
        ChainConfig {
            chain_id: ChainId::new(config.chain_id),
            rpc_endpoint: config.rpc_endpoint,
            grpc_endpoint: config.grpc_endpoint,
            grpc_web_endpoint: config.grpc_web_endpoint,
            gas_denom: config.gas_denom,
            gas_price: config.gas_price,
            address_kind: AddrKind::Cosmos {
                prefix: config.bech32_prefix,
            },
        }
    }
}

impl CosmosQuerier {
    pub fn new_from_chain_name(
        chain_name: &str,
        chain_configs: &crate::bindings::lay3r::avs::layer_types::ChainConfigs,
        reactor: Reactor,
    ) -> Result<Self> {
        let chain_config = chain_configs
            .get_key(chain_name)
            .ok_or_else(|| anyhow::anyhow!("chain config not found"))?;
        match chain_config.clone() {
            crate::wit_bindings::AnyChainConfig::Cosmos(chain_config) => Ok(Self {
                chain_config: chain_config.into(),
                reactor,
            }),
            crate::wit_bindings::AnyChainConfig::Eth(..) => Err(anyhow::anyhow!(
                "expected cosmos chain config, got eth chain"
            )),
        }
    }

    pub fn new(chain_config: ChainConfig, reactor: Reactor) -> Self {
        Self {
            chain_config,
            reactor,
        }
    }

    pub async fn block_height(&self) -> Result<u64> {
        rpc::block(&self.chain_config, &self.reactor, None)
            .await
            .map(|resp| resp.block.header.height.into())
    }

    pub async fn balance(&self, address: &Address) -> Result<Option<Coin>> {
        let req = layer_climb_proto::bank::QueryBalanceRequest {
            address: address.to_string(),
            denom: self.chain_config.gas_denom.clone(),
        };

        rpc::abci_protobuf_query::<_, layer_climb_proto::bank::QueryBalanceResponse>(
            &self.chain_config,
            &self.reactor,
            "/cosmos.bank.v1beta1.Query/Balance",
            req,
            None,
        )
        .await
        .map(|resp| resp.balance)
    }

    pub async fn contract_smart<T: DeserializeOwned>(
        &self,
        address: &Address,
        query: impl Serialize,
    ) -> Result<T> {
        let req = layer_climb_proto::wasm::QuerySmartContractStateRequest {
            address: address.to_string(),
            query_data: serde_json::to_vec(&query)?,
        };

        let resp: layer_climb_proto::wasm::QuerySmartContractStateResponse =
            rpc::abci_protobuf_query(
                &self.chain_config,
                &self.reactor,
                "/cosmwasm.wasm.v1.Query/SmartContractState",
                req,
                None,
            )
            .await?;

        serde_json::from_slice(&resp.data).map_err(|e| anyhow::anyhow!(e))
    }
}
