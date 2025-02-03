#![allow(unused_imports)]
#![allow(dead_code)]

use std::{
    future::Future,
    pin::{pin, Pin},
    sync::Arc,
    task,
};

use alloy_json_rpc::{RequestPacket, ResponsePacket};
use alloy_provider::{network::Ethereum, Network, Provider, RootProvider};
use alloy_rpc_client::RpcClient;
use alloy_transport::{
    utils::guess_local_url, BoxTransport, Pbf, TransportConnect, TransportError,
    TransportErrorKind, TransportFut,
};
use alloy_transport_http::{Http, HttpConnect};
use tower_service::Service;
use wasi::http::types::Method;
use wit_bindgen_rt::async_support::futures::pin_mut;
use wstd::{http::{Client, Request, StatusCode, IntoBody}, io::{empty, AsyncRead}, runtime::block_on};


cfg_if::cfg_if! {
    if #[cfg(target_arch = "wasm32")] {
        pub type WasiProvider<N = Ethereum> =
            RootProvider<WasiEthClient, N>;

        pub fn new_eth_provider<N: Network>(endpoint: String) -> WasiProvider<N> {
            let client = WasiEthClient::new(endpoint);
            let is_local = client.is_local();
            RootProvider::new(RpcClient::new(client, is_local))
        }

        #[derive(Clone)]
        pub struct WasiEthClient {
            pub endpoint: String,
        }

        impl WasiEthClient {
            pub fn new(endpoint: String) -> Self {
                Self { endpoint }
            }
        }

        // prior art, cloudflare does this trick too: https://github.com/cloudflare/workers-rs/blob/38af58acc4e54b29c73336c1720188f3c3e86cc4/worker/src/send.rs#L32
        unsafe impl Sync for WasiEthClient {}
        unsafe impl Send for WasiEthClient {}

        impl TransportConnect for WasiEthClient {
            type Transport = WasiEthClient;

            fn is_local(&self) -> bool {
                guess_local_url(self.endpoint.as_str())
            }

            fn get_transport<'a: 'b, 'b>(&'a self) -> Pbf<'b, Self::Transport, TransportError> {
                Box::pin(async { Ok(self.clone()) })
            }
        }

        impl Service<RequestPacket> for WasiEthClient {
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

                    let request = Request::post(endpoint).header("content-type", "application/json").body(serde_json::to_vec(&packet.serialize().map_err(transport_err)?).unwrap().into_body()).unwrap();

                    let mut res = Client::new().send(request).await.unwrap();

                    match res.status() {
                        StatusCode::OK => {
                            let body = res.body_mut();
                            let mut body_buf = Vec::new();
                            body.read_to_end(&mut body_buf).await.unwrap();
                            Ok(serde_json::from_slice(&body_buf).unwrap())
                        }
                        status => return Err(transport_err(format!("unexpected status code: {status}"))),
                    }
                };

                Box::pin(fut)
            }
        }
    } else {
        // not used, just for making the compiler happy
        pub fn new_eth_provider<N: Network>(_endpoint: String) -> RootProvider<BoxTransport, N> {
            unimplemented!()
        }
    }
}
