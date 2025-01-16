use anyhow::Result;
use layer_climb_address::Address;
use layer_climb_proto::Coin;
use serde::{de::DeserializeOwned, Serialize};
use wstd::runtime::Reactor;

use crate::bindings::compat::CosmosChainConfig;

use super::rpc;

pub struct CosmosQuerier {
    pub chain_config: layer_climb_config::ChainConfig,
    pub reactor: Reactor,
}

impl CosmosQuerier {
    pub fn new(chain_config: CosmosChainConfig, reactor: Reactor) -> Self {
        Self {
            chain_config: chain_config.into(),
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
