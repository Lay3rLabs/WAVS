#![allow(unused_imports)]
#![allow(dead_code)]
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use layer_climb::prelude::*;
use serde::{de::DeserializeOwned, Serialize};
use std::sync::Arc;
use wasi::http::types::Method;
use wstd::{
    http::{Client, IntoBody, Request, StatusCode},
    io::{empty, AsyncRead},
    runtime::block_on,
};

struct WasiCosmosRpcTransport {}

// prior art, cloudflare does this trick too: https://github.com/cloudflare/workers-rs/blob/38af58acc4e54b29c73336c1720188f3c3e86cc4/worker/src/send.rs#L32
unsafe impl Sync for WasiCosmosRpcTransport {}
unsafe impl Send for WasiCosmosRpcTransport {}

cfg_if::cfg_if! {
    if #[cfg(target_arch = "wasm32")] {
        #[async_trait(?Send)]
        impl layer_climb::network::rpc::RpcTransport for WasiCosmosRpcTransport {
            async fn post_json_bytes(&self, url: &str, body: Vec<u8>) -> anyhow::Result<String> {
                let request = Request::post(url).header("content-type", "application/json").body(body.into_body())?;
                let mut res = Client::new().send(request).await?;

                match res.status() {
                    StatusCode::OK => {
                        let body = res.body_mut();
                        let mut body_buf = Vec::new();
                        body.read_to_end(&mut body_buf).await?;
                        String::from_utf8(body_buf).map_err(|err| anyhow::anyhow!(err))
                    },
                    status => Err(anyhow!("unexpected status code: {status}")),
                }
            }
        }

        pub async fn new_cosmos_query_client(chain_config: layer_climb::prelude::ChainConfig) -> Result<QueryClient> {
            QueryClient::new(chain_config.clone(), Some(Connection {
                rpc: Arc::new(WasiCosmosRpcTransport {}),
                preferred_mode: Some(ConnectionMode::Rpc),
            })).await
        }
    } else {
        // not used, just for making the IDE happy
        pub async fn new_cosmos_query_client(chain_config: layer_climb::prelude::ChainConfig) -> Result<QueryClient> {
            QueryClient::new(chain_config.clone(), Some(Connection {
                preferred_mode: Some(ConnectionMode::Rpc),
                ..Default::default()
            })).await
        }
    }
}
