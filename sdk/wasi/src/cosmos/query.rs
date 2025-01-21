#![allow(unused_imports)]
#![allow(dead_code)]
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use layer_climb::prelude::*;
use serde::{de::DeserializeOwned, Serialize};
use std::sync::Arc;
use wasi::http::types::Method;
use wstd::runtime::Reactor;

use crate::{
    bindings::compat::CosmosChainConfig,
    wasi::{Request, WasiPollable},
};

struct WasiCosmosRpcTransport {
    reactor: Reactor,
}

// prior art, cloudflare does this trick too: https://github.com/cloudflare/workers-rs/blob/38af58acc4e54b29c73336c1720188f3c3e86cc4/worker/src/send.rs#L32
unsafe impl Sync for WasiCosmosRpcTransport {}
unsafe impl Send for WasiCosmosRpcTransport {}

cfg_if::cfg_if! {
    if #[cfg(target_arch = "wasm32")] {
        #[async_trait(?Send)]
        impl layer_climb::network::rpc::RpcTransport for WasiCosmosRpcTransport {
            async fn post_json_bytes(&self, url: &str, body: Vec<u8>) -> anyhow::Result<String> {
                let mut req = Request::new(Method::Post, url)
                    .map_err(|e| anyhow!("{:?}", e))?;

                req.body = body;
                req.headers
                    .push(("content-type".to_string(), "application/json".to_string()));

                let res = self.reactor.send(req).await.map_err(|e| anyhow!("{:?}", e))?;

                match res.status {
                    200 => String::from_utf8(res.body).map_err(|err| anyhow::anyhow!(err)),
                    status => Err(anyhow!("unexpected status code: {status}")),
                }
            }
        }

        pub async fn new_cosmos_query_client(chain_config: CosmosChainConfig, reactor: Reactor) -> Result<QueryClient> {
            let chain_config:layer_climb_config::ChainConfig = chain_config.into();
            QueryClient::new(chain_config.clone(), Some(Connection {
                rpc: Arc::new(WasiCosmosRpcTransport {
                    reactor
                }),
                preferred_mode: Some(ConnectionMode::Rpc),
            })).await
        }
    } else {
        // not used, just for making the IDE happy
        pub async fn new_cosmos_query_client(chain_config: CosmosChainConfig, _reactor: Reactor) -> Result<QueryClient> {
            let chain_config:layer_climb_config::ChainConfig = chain_config.into();
            QueryClient::new(chain_config.clone(), Some(Connection {
                preferred_mode: Some(ConnectionMode::Rpc),
                ..Default::default()
            })).await
        }
    }
}
