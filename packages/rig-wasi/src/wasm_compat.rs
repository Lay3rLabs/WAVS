use bytes::Bytes;
use std::future::Future;
use std::pin::Pin;

use futures::Stream;

// P3: Unified cfg detection — all WASM checks use target_family = "wasm"
// This fires on wasm32-wasip2 automatically without requiring the "wasm" feature flag.
// Previously upstream used #[cfg(all(feature = "wasm", target_arch = "wasm32"))] which
// does NOT fire on wasip2 without the "wasm" cargo feature enabled.

#[cfg(not(target_family = "wasm"))]
pub trait WasmCompatSend: Send {}
#[cfg(target_family = "wasm")]
pub trait WasmCompatSend {}

#[cfg(not(target_family = "wasm"))]
impl<T> WasmCompatSend for T where T: Send {}
#[cfg(target_family = "wasm")]
impl<T> WasmCompatSend for T {}

#[cfg(not(target_family = "wasm"))]
pub trait WasmCompatSendStream:
    Stream<Item = Result<Bytes, crate::http_client::Error>> + Send
{
    type InnerItem: Send;
}

#[cfg(target_family = "wasm")]
pub trait WasmCompatSendStream: Stream<Item = Result<Bytes, crate::http_client::Error>> {
    type InnerItem;
}

#[cfg(not(target_family = "wasm"))]
impl<T> WasmCompatSendStream for T
where
    T: Stream<Item = Result<Bytes, crate::http_client::Error>> + Send,
{
    type InnerItem = Result<Bytes, crate::http_client::Error>;
}

#[cfg(target_family = "wasm")]
impl<T> WasmCompatSendStream for T
where
    T: Stream<Item = Result<Bytes, crate::http_client::Error>>,
{
    type InnerItem = Result<Bytes, crate::http_client::Error>;
}

#[cfg(not(target_family = "wasm"))]
pub trait WasmCompatSync: Sync {}
#[cfg(target_family = "wasm")]
pub trait WasmCompatSync {}

#[cfg(not(target_family = "wasm"))]
impl<T> WasmCompatSync for T where T: Sync {}
#[cfg(target_family = "wasm")]
impl<T> WasmCompatSync for T {}

#[cfg(not(target_family = "wasm"))]
pub type WasmBoxedFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

#[cfg(target_family = "wasm")]
pub type WasmBoxedFuture<'a, T> = Pin<Box<dyn Future<Output = T> + 'a>>;

#[macro_export]
macro_rules! if_wasm {
    ($($tokens:tt)*) => {
        #[cfg(target_family = "wasm")]
        $($tokens)*

    };
}

#[macro_export]
macro_rules! if_not_wasm {
    ($($tokens:tt)*) => {
        #[cfg(not(target_family = "wasm"))]
        $($tokens)*

    };
}
