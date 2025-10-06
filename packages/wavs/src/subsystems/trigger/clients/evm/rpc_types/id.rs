use std::{
    collections::HashSet,
    sync::{Arc, LazyLock},
};

use alloy_primitives::{Address, B256};
use slotmap::{new_key_type, SlotMap};

new_key_type! {
    pub struct RpcId;
}

impl RpcId {
    pub fn new(kind: RpcRequestKind) -> Self {
        RPC_REQUEST_ID.write().unwrap().insert(kind)
    }

    pub fn kind(&self) -> Option<RpcRequestKind> {
        RPC_REQUEST_ID.read().unwrap().get(*self).cloned()
    }

    pub fn clear_all() {
        RPC_REQUEST_ID.write().unwrap().clear();
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum RpcRequestKind {
    SubscribeNewHeads,
    SubscribeLogs {
        address: HashSet<Address>,
        topics: HashSet<B256>,
    },
    SubscribeNewPendingTransactions,
    Unsubscribe {
        subscription_id: String,
    },
}

static RPC_REQUEST_ID: LazyLock<Arc<std::sync::RwLock<SlotMap<RpcId, RpcRequestKind>>>> =
    LazyLock::new(|| Arc::new(std::sync::RwLock::new(SlotMap::with_key())));
