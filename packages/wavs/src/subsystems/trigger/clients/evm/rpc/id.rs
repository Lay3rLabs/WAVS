use std::sync::{Arc, LazyLock};

use slotmap::{new_key_type, SlotMap};

new_key_type! {
    pub struct RpcId;
}

impl RpcId {
    pub fn new(kind: RpcRequestKind) -> Self {
        RPC_REQUEST_ID.write().unwrap().insert(kind)
    }

    pub fn kind(&self) -> Option<RpcRequestKind> {
        RPC_REQUEST_ID.read().unwrap().get(*self).copied()
    }

    pub fn clear_all() {
        RPC_REQUEST_ID.write().unwrap().clear();
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RpcRequestKind {
    SubscribeNewHeads,
    SubscribeLogs,
    SubscribeNewPendingTransactions,
    Unsubscribe,
}

static RPC_REQUEST_ID: LazyLock<Arc<std::sync::RwLock<SlotMap<RpcId, RpcRequestKind>>>> =
    LazyLock::new(|| Arc::new(std::sync::RwLock::new(SlotMap::with_key())));
