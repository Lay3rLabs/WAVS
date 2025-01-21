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
use wstd::runtime::Reactor;

use crate::wasi::{Request, Response, WasiPollable};

cfg_if::cfg_if! {
    if #[cfg(target_arch = "wasm32")] {
        pub type WasiProvider<N = Ethereum> =
            RootProvider<WasiEthClient, N>;

        pub fn new_eth_provider<N: Network>(reactor: Reactor, endpoint: String) -> WasiProvider<N> {
            let client = WasiEthClient::new(reactor, endpoint);
            let is_local = client.is_local();
            RootProvider::new(RpcClient::new(client, is_local))
        }

        #[derive(Clone)]
        pub struct WasiEthClient {
            pub reactor: Reactor,
            pub endpoint: String,
        }

        impl WasiEthClient {
            pub fn new(reactor: Reactor, endpoint: String) -> Self {
                Self { reactor, endpoint }
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
                let reactor = self.reactor.clone();
                let endpoint = self.endpoint.clone();
                let fut = async move {
                    fn transport_err(e: impl ToString) -> TransportError {
                        TransportError::Transport(TransportErrorKind::Custom(e.to_string().into()))
                    }

                    let mut req = Request::new(Method::Post, &endpoint).map_err(transport_err)?;

                    let body = packet.serialize().map_err(transport_err)?;

                    req.body = serde_json::to_vec(&body).map_err(transport_err)?;
                    req.headers
                        .push(("content-type".to_string(), "application/json".to_string()));

                    let res = reactor.send(req).await.map_err(transport_err)?;

                    match res.status {
                        200 => {
                            let res = res.json::<ResponsePacket>().map_err(transport_err)?;
                            Ok(res)
                        }
                        status => return Err(transport_err(format!("unexpected status code: {status}"))),
                    }
                };

                Box::pin(fut)
            }
        }
    } else {
        // not used, just for making the compiler happy
        pub fn new_eth_provider<N: Network>(_reactor: Reactor, _endpoint: String) -> RootProvider<BoxTransport, N> {
            unimplemented!()
        }
    }
}
