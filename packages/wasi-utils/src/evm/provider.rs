#![allow(unused_imports)]
#![allow(dead_code)]

use std::{
    future::Future,
    pin::{pin, Pin},
    sync::Arc,
    task,
};

use alloy_json_rpc::{RequestPacket, ResponsePacket};
use alloy_primitives::BlockHash;
use alloy_provider::{
    network::{BlockResponse, Ethereum},
    Network, Provider, RootProvider,
};
use alloy_rpc_client::RpcClient;
use alloy_transport::{
    utils::guess_local_url, BoxTransport, Pbf, TransportConnect, TransportError,
    TransportErrorKind, TransportFut,
};
use alloy_transport_http::{Http, HttpConnect};
use futures_utils_wasm::impl_future;
use tower_service::Service;
use wasi::http::types::Method;
use wstd::{
    http::{Client, IntoBody, Request, StatusCode},
    io::{empty, AsyncRead},
    runtime::block_on,
};

cfg_if::cfg_if! {
    if #[cfg(target_arch = "wasm32")] {
        pub fn new_evm_provider<N: Network>(endpoint: String) -> RootProvider<N> {
            let client = WasiEvmClient::new(endpoint);
            let is_local = client.is_local();
            RootProvider::new(RpcClient::new(client, is_local))
        }

        #[derive(Clone)]
        pub struct WasiEvmClient {
            pub endpoint: String,
        }

        impl WasiEvmClient {
            pub fn new(endpoint: String) -> Self {
                Self { endpoint }
            }
        }

        // prior art, cloudflare does this trick too: https://github.com/cloudflare/workers-rs/blob/38af58acc4e54b29c73336c1720188f3c3e86cc4/worker/src/send.rs#L32
        unsafe impl Sync for WasiEvmClient {}
        unsafe impl Send for WasiEvmClient {}

        impl TransportConnect for WasiEvmClient {
            fn is_local(&self) -> bool {
                guess_local_url(self.endpoint.as_str())
            }

            fn get_transport(&self) -> impl_future!(<Output = Result<BoxTransport, TransportError>>) {
                async { Ok(BoxTransport::new(self.clone())) }
            }
        }

        impl Service<RequestPacket> for WasiEvmClient {
            type Response = ResponsePacket;
            type Error = TransportError;
            type Future = TransportFut<'static>;

            #[inline]
            fn poll_ready(&mut self, _cx: &mut task::Context<'_>) -> task::Poll<Result<(), Self::Error>> {
                // `reqwest` always returns `Ok(())`.
                task::Poll::Ready(Ok(()))
            }

            #[inline]
            fn call(&mut self, packet: RequestPacket) -> Self::Future {
                let endpoint = self.endpoint.clone();
                let fut = async move {
                    fn transport_err(e: impl ToString) -> TransportError {
                        TransportError::Transport(TransportErrorKind::Custom(e.to_string().into()))
                    }

                    let request = Request::post(endpoint).header("content-type", "application/json").body(serde_json::to_vec(&packet.serialize().map_err(transport_err)?).map_err(transport_err)?.into_body()).map_err(transport_err)?;

                    let mut res = Client::new().send(request).await.map_err(transport_err)?;

                    match res.status() {
                        StatusCode::OK => {
                            let body = res.body_mut();
                            let mut body_buf = Vec::new();
                            body.read_to_end(&mut body_buf).await.map_err(transport_err)?;
                            Ok(serde_json::from_slice::<ResponsePacket>(&body_buf).map_err(transport_err)?)
                        }
                        status => return Err(transport_err(format!("unexpected status code: {status}"))),
                    }
                };

                Box::pin(fut)
            }
        }
    } else {
        // not used, just for making the compiler happy
        pub fn new_evm_provider<N: Network>(_endpoint: String) -> RootProvider {
            unimplemented!()
        }

    }
}
